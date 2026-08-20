use std::{
    collections::BTreeSet,
    fs::{self, File},
    net::{IpAddr, SocketAddr},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Instant,
};

use rewrite_types::{CancellationToken, Digest};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    process::{Pid, PidfdFlags, geteuid, pidfd_open},
};

use super::file::hash_opened_file;
use crate::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint, ensure_active,
};

const LISTEN_STATE: &str = "0A";

pub(crate) struct Lease {
    endpoint: ListenerEndpoint,
    owner: ListenerOwner,
    pidfd: OwnedFd,
    entrypoint: File,
    initial: AttachedProcessEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ListenerOwner {
    pid: u32,
    socket_inode: u64,
}

impl Lease {
    pub(crate) fn attach(
        endpoint: ListenerEndpoint,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<Self, AttachedProcessWitnessError> {
        let owner = listener_owner(endpoint.socket(), limits, cancellation, started)?;
        let pid = owner.pid;
        let raw_pid = i32::try_from(pid)
            .ok()
            .and_then(Pid::from_raw)
            .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
        let pidfd = pidfd_open(raw_pid, PidfdFlags::empty()).map_err(|error| {
            if error == rustix::io::Errno::PERM || error == rustix::io::Errno::ACCESS {
                AttachedProcessWitnessError::ProcessAccessDenied
            } else {
                AttachedProcessWitnessError::ProcessInstanceUnavailable
            }
        })?;
        ensure_pidfd_alive(&pidfd)?;
        let mut entrypoint = open_entrypoint(pid)?;
        let initial = observe_process(
            endpoint,
            owner,
            &mut entrypoint,
            limits,
            cancellation,
            started,
        )?;
        let confirmed = listener_owner(endpoint.socket(), limits, cancellation, started)?;
        if confirmed != owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        ensure_pidfd_alive(&pidfd)?;
        Ok(Self {
            endpoint,
            owner,
            pidfd,
            entrypoint,
            initial,
        })
    }

    pub(crate) fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    pub(crate) fn reobserve(
        &mut self,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
        started: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        ensure_pidfd_alive(&self.pidfd)?;
        let owner = listener_owner(self.endpoint.socket(), limits, cancellation, started)?;
        if owner != self.owner {
            return Err(AttachedProcessWitnessError::ListenerRebound);
        }
        let retained_metadata = entrypoint_metadata(&self.entrypoint)?;
        let retained_digest = hash_opened_file(
            &mut self.entrypoint,
            retained_metadata.len(),
            limits,
            cancellation,
            started,
        )?;
        if retained_digest != *self.initial.entrypoint_digest() {
            return Err(AttachedProcessWitnessError::EntrypointChanged);
        }
        let mut current = open_entrypoint(self.owner.pid)?;
        let observed = observe_process(
            self.endpoint,
            self.owner,
            &mut current,
            limits,
            cancellation,
            started,
        )?;
        ensure_pidfd_alive(&self.pidfd)?;
        Ok(observed)
    }
}

fn listener_owner(
    endpoint: SocketAddr,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ListenerOwner, AttachedProcessWitnessError> {
    let path = match endpoint.ip() {
        IpAddr::V4(_) => Path::new("/proc/net/tcp"),
        IpAddr::V6(_) => Path::new("/proc/net/tcp6"),
    };
    let bytes = read_bounded(path, limits.maximum_socket_table_bytes)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
    let expected_local = encoded_local_address(endpoint);
    let expected_uid = geteuid().as_raw();
    let mut inodes = BTreeSet::new();
    let mut rows = 0_usize;
    for line in text.lines().skip(1) {
        ensure_active(cancellation, started, limits)?;
        rows = rows
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if rows > limits.maximum_socket_table_entries {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 || fields[1] != expected_local || fields[3] != LISTEN_STATE {
            continue;
        }
        let uid = fields[7]
            .parse::<u32>()
            .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
        if uid != expected_uid {
            return Err(AttachedProcessWitnessError::ProcessAccessDenied);
        }
        let inode = fields[9]
            .parse::<u64>()
            .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
        if inode == 0 {
            return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
        }
        inodes.insert(inode);
    }
    if inodes.is_empty() {
        return Err(AttachedProcessWitnessError::ListenerNotFound);
    }
    if inodes.len() != 1 {
        return Err(AttachedProcessWitnessError::ListenerOwnershipAmbiguous);
    }
    let inode = *inodes
        .first()
        .ok_or(AttachedProcessWitnessError::ListenerNotFound)?;
    Ok(ListenerOwner {
        pid: owners_for_inode(inode, expected_uid, limits, cancellation, started)?,
        socket_inode: inode,
    })
}

fn owners_for_inode(
    inode: u64,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u32, AttachedProcessWitnessError> {
    let mut owners = BTreeSet::new();
    let mut process_count = 0_usize;
    let entries = fs::read_dir("/proc")
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
    for entry in entries {
        ensure_active(cancellation, started, limits)?;
        let entry =
            entry.map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        process_count = process_count
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if process_count > limits.maximum_processes {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(AttachedProcessWitnessError::ProcessAccessDenied);
            }
            Err(_) => return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        };
        if metadata.uid() != expected_uid {
            continue;
        }
        if process_has_inode(
            pid,
            inode,
            limits.maximum_descriptors_per_process,
            cancellation,
            started,
            limits,
        )? {
            owners.insert(pid);
        }
    }
    match owners.len() {
        0 => Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        1 => owners
            .first()
            .copied()
            .ok_or(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        _ => Err(AttachedProcessWitnessError::ListenerOwnershipAmbiguous),
    }
}

fn process_has_inode(
    pid: u32,
    inode: u64,
    maximum_descriptors: usize,
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
) -> Result<bool, AttachedProcessWitnessError> {
    let directory = PathBuf::from(format!("/proc/{pid}/fd"));
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(AttachedProcessWitnessError::ProcessAccessDenied);
        }
        Err(_) => return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
    };
    let expected = format!("socket:[{inode}]");
    let mut descriptor_count = 0_usize;
    for entry in entries {
        ensure_active(cancellation, started, limits)?;
        descriptor_count = descriptor_count
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if descriptor_count > maximum_descriptors {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let entry =
            entry.map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
        match fs::read_link(entry.path()) {
            Ok(target) if target.as_os_str() == expected.as_str() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        }
    }
    Ok(false)
}

fn observe_process(
    endpoint: ListenerEndpoint,
    owner: ListenerOwner,
    entrypoint: &mut File,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    let pid = owner.pid;
    let start_token = process_start_token(pid)?;
    let self_namespace = fs::read_link("/proc/self/ns/net")
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
    let process_namespace = fs::read_link(format!("/proc/{pid}/ns/net")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::ProcessInstanceUnavailable
        }
    })?;
    if self_namespace != process_namespace {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    let metadata = entrypoint_metadata(entrypoint)?;
    let entrypoint_digest =
        hash_opened_file(entrypoint, metadata.len(), limits, cancellation, started)?;
    let process_instance_digest =
        Digest::sha256(format!("linux-pid-start-v1\0{pid}\0{start_token}").as_bytes());
    let object_material = format!(
        "linux-file-object-v1\0{}\0{}\0{}",
        metadata.dev(),
        metadata.ino(),
        metadata.len()
    );
    let endpoint_material = format!(
        "linux-listener-owner-v1\0{}\0{pid}\0{start_token}\0{}",
        endpoint.socket(),
        owner.socket_inode
    );
    let platform_material = format!(
        "linux-platform-evidence-v1\0{}",
        self_namespace.to_string_lossy()
    );
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::LinuxProcPidfd,
        owner_pid: pid,
        process_instance_digest,
        ownership_snapshot_digest: Digest::sha256(endpoint_material.as_bytes()),
        entrypoint_object_digest: Digest::sha256(object_material.as_bytes()),
        entrypoint_digest,
        entrypoint_bytes: metadata.len(),
        platform_evidence_digest: Digest::sha256(platform_material.as_bytes()),
    })
}

fn open_entrypoint(pid: u32) -> Result<File, AttachedProcessWitnessError> {
    File::open(format!("/proc/{pid}/exe")).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::EntrypointUnavailable
        }
    })
}

fn entrypoint_metadata(file: &File) -> Result<fs::Metadata, AttachedProcessWitnessError> {
    let metadata = file
        .metadata()
        .map_err(|_error| AttachedProcessWitnessError::EntrypointUnavailable)?;
    if !metadata.is_file() {
        return Err(AttachedProcessWitnessError::EntrypointNotRegular);
    }
    if metadata.nlink() != 1 {
        return Err(AttachedProcessWitnessError::EntrypointAliased);
    }
    Ok(metadata)
}

fn process_start_token(pid: u32) -> Result<u64, AttachedProcessWitnessError> {
    let bytes = read_bounded(Path::new(&format!("/proc/{pid}/stat")), 16 * 1024)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    let close = text
        .rfind(')')
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    let fields = text
        .get(close.saturating_add(1)..)
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let start = fields
        .get(19)
        .ok_or(AttachedProcessWitnessError::ProcessInstanceUnavailable)?
        .parse::<u64>()
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    if start == 0 {
        return Err(AttachedProcessWitnessError::ProcessInstanceUnavailable);
    }
    Ok(start)
}

fn ensure_pidfd_alive(pidfd: &OwnedFd) -> Result<(), AttachedProcessWitnessError> {
    let mut descriptors = [PollFd::new(pidfd, PollFlags::IN)];
    let ready = poll(&mut descriptors, Some(&Timespec::default()))
        .map_err(|_error| AttachedProcessWitnessError::ProcessInstanceUnavailable)?;
    if ready == 0 {
        Ok(())
    } else {
        Err(AttachedProcessWitnessError::ProcessExited)
    }
}

fn read_bounded(path: &Path, maximum: usize) -> Result<Vec<u8>, AttachedProcessWitnessError> {
    use std::io::Read as _;

    let file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            AttachedProcessWitnessError::ProcessAccessDenied
        } else {
            AttachedProcessWitnessError::ListenerSnapshotIncomplete
        }
    })?;
    let limit = u64::try_from(maximum)
        .map_err(|_error| AttachedProcessWitnessError::ResourceLimit)?
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(maximum.min(64 * 1024));
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(|_error| AttachedProcessWitnessError::ListenerSnapshotIncomplete)?;
    if bytes.len() > maximum {
        return Err(AttachedProcessWitnessError::ResourceLimit);
    }
    Ok(bytes)
}

fn encoded_local_address(endpoint: SocketAddr) -> String {
    use std::fmt::Write as _;

    let port = endpoint.port();
    match endpoint.ip() {
        IpAddr::V4(address) => {
            let encoded = u32::from_ne_bytes(address.octets());
            format!("{encoded:08X}:{port:04X}")
        }
        IpAddr::V6(address) => {
            let octets = address.octets();
            let mut encoded = String::with_capacity(32);
            for chunk in octets.chunks_exact(4) {
                let word = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let _ = write!(encoded, "{word:08X}");
            }
            format!("{encoded}:{port:04X}")
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{Ipv4Addr, Ipv6Addr, TcpListener},
        os::fd::AsRawFd,
        time::Instant,
    };

    use rewrite_types::CancellationToken;

    use super::{
        ListenerOwner, encoded_local_address, observe_process, open_entrypoint, process_has_inode,
    };
    use crate::{AttachedProcessWitnessLimits, ListenerEndpoint};

    #[test]
    fn proc_addresses_use_native_word_byte_order() {
        assert_eq!(
            encoded_local_address((Ipv4Addr::LOCALHOST, 11_434).into()),
            "0100007F:2CAA"
        );
        assert_eq!(
            encoded_local_address((Ipv6Addr::LOCALHOST, 11_434).into()),
            "00000000000000000000000001000000:2CAA"
        );
    }

    #[test]
    fn current_process_descriptor_and_executable_produce_native_evidence() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
        let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
            .expect("loopback endpoint");
        let target = fs::read_link(format!("/proc/self/fd/{}", listener.as_raw_fd()))
            .expect("listener descriptor link");
        let target = target.to_str().expect("socket descriptor text");
        let inode = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .expect("socket inode wrapper")
            .parse::<u64>()
            .expect("socket inode");
        let limits = AttachedProcessWitnessLimits::default();
        let cancellation = CancellationToken::new();
        assert!(
            process_has_inode(
                std::process::id(),
                inode,
                limits.maximum_descriptors_per_process,
                &cancellation,
                Instant::now(),
                limits,
            )
            .expect("inspect current descriptors")
        );
        let mut entrypoint = open_entrypoint(std::process::id()).expect("open current executable");
        let evidence = observe_process(
            endpoint,
            ListenerOwner {
                pid: std::process::id(),
                socket_inode: inode,
            },
            &mut entrypoint,
            limits,
            &cancellation,
            Instant::now(),
        )
        .expect("observe current process");
        assert_eq!(evidence.owner_pid(), std::process::id());
        assert!(evidence.entrypoint_bytes() > 0);
    }
}
