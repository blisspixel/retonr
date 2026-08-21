use std::{collections::BTreeSet, fs::File, io::Read as _, mem::MaybeUninit, time::Instant};

use rewrite_types::CancellationToken;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    fs::{CWD, Mode, OFlags, RawDir, openat, readlinkat},
    io::Errno,
    process::{Pid, PidfdFlags, pidfd_open},
};

use crate::{AttachedProcessWitnessError, AttachedProcessWitnessLimits, ensure_active};

const DIRECTORY_BUFFER_BYTES: usize = 16 * 1024;
const MAXIMUM_STATUS_BYTES: usize = 64 * 1024;
const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW);
const FILE_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW);

struct AnchoredProcess {
    pidfd: OwnedFd,
    directory: OwnedFd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcfsErrorClass {
    Missing,
    AccessDenied,
    ResourceLimit,
    Incomplete,
}

pub(super) fn visible_same_uid_holders(
    inode: u64,
    expected_uid: u32,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<u32>, AttachedProcessWitnessError> {
    let proc_root = open_proc_root()?;
    let mut buffer = [MaybeUninit::uninit(); DIRECTORY_BUFFER_BYTES];
    let mut entries = RawDir::new(&proc_root, &mut buffer);
    let mut owners = BTreeSet::new();
    let mut process_count = 0_usize;
    while let Some(entry) = entries.next() {
        ensure_active(cancellation, started, limits)?;
        let entry = entry.map_err(observation_error)?;
        let Some(pid) = parse_proc_pid(entry.file_name().to_bytes())? else {
            continue;
        };
        process_count = process_count
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if process_count > limits.maximum_processes {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        let Some(process) = open_process(&proc_root, pid)? else {
            continue;
        };
        let Some(effective_uid) = read_effective_uid(&process.directory, &process.pidfd)? else {
            continue;
        };
        if effective_uid != expected_uid {
            continue;
        }
        let Some(found) = process_has_inode_at(
            &process.directory,
            &process.pidfd,
            inode,
            limits.maximum_descriptors_per_process,
            cancellation,
            started,
            limits,
        )?
        else {
            continue;
        };
        ensure_active(cancellation, started, limits)?;
        let Some(confirmed_uid) = read_effective_uid(&process.directory, &process.pidfd)? else {
            continue;
        };
        require_unchanged_effective_uid(effective_uid, confirmed_uid)?;
        if found {
            owners.insert(pid);
        }
    }
    Ok(owners.into_iter().collect())
}

pub(super) fn effective_uid_for_pid(
    pid: u32,
    pidfd: &OwnedFd,
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<u32, AttachedProcessWitnessError> {
    ensure_active(cancellation, started, limits)?;
    let proc_root = open_proc_root()?;
    let directory = open_process_directory(&proc_root, pid, pidfd)?
        .ok_or(AttachedProcessWitnessError::ProcessExited)?;
    read_effective_uid(&directory, pidfd)?.ok_or(AttachedProcessWitnessError::ProcessExited)
}

#[cfg(test)]
pub(super) fn process_has_inode(
    pid: u32,
    inode: u64,
    maximum_descriptors: usize,
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
) -> Result<bool, AttachedProcessWitnessError> {
    let proc_root = open_proc_root()?;
    let Some(process) = open_process(&proc_root, pid)? else {
        return Ok(false);
    };
    Ok(process_has_inode_at(
        &process.directory,
        &process.pidfd,
        inode,
        maximum_descriptors,
        cancellation,
        started,
        limits,
    )?
    .unwrap_or(false))
}

fn open_proc_root() -> Result<OwnedFd, AttachedProcessWitnessError> {
    openat(CWD, "/proc", DIRECTORY_FLAGS, Mode::empty()).map_err(observation_error)
}

fn open_process(
    proc_root: &OwnedFd,
    pid: u32,
) -> Result<Option<AnchoredProcess>, AttachedProcessWitnessError> {
    let raw_pid = raw_pid(pid)?;
    let pidfd = match pidfd_open(raw_pid, PidfdFlags::empty()) {
        Ok(pidfd) => pidfd,
        Err(error) if skip_uninspectable(error) => return Ok(None),
        Err(error) => return Err(observation_error(error)),
    };
    let Some(directory) = open_process_directory(proc_root, pid, &pidfd)? else {
        return Ok(None);
    };
    Ok(Some(AnchoredProcess { pidfd, directory }))
}

fn open_process_directory(
    proc_root: &OwnedFd,
    pid: u32,
    pidfd: &OwnedFd,
) -> Result<Option<OwnedFd>, AttachedProcessWitnessError> {
    let directory = match openat(proc_root, pid.to_string(), DIRECTORY_FLAGS, Mode::empty()) {
        Ok(directory) => directory,
        Err(error) if classify_errno(error) == ProcfsErrorClass::Missing => {
            return missing_process(pidfd);
        }
        Err(error) if classify_errno(error) == ProcfsErrorClass::AccessDenied => return Ok(None),
        Err(error) => return Err(observation_error(error)),
    };
    if pidfd_alive(pidfd)? {
        Ok(Some(directory))
    } else {
        Ok(None)
    }
}

fn read_effective_uid(
    process_directory: &OwnedFd,
    pidfd: &OwnedFd,
) -> Result<Option<u32>, AttachedProcessWitnessError> {
    let status = match openat(process_directory, "status", FILE_FLAGS, Mode::empty()) {
        Ok(status) => status,
        Err(error) if classify_errno(error) == ProcfsErrorClass::Missing => {
            return missing_process(pidfd);
        }
        Err(error) => return Err(observation_error(error)),
    };
    let mut bytes = Vec::with_capacity(MAXIMUM_STATUS_BYTES.min(16 * 1024));
    let mut reader = File::from(status).take((MAXIMUM_STATUS_BYTES as u64).saturating_add(1));
    if let Err(error) = reader.read_to_end(&mut bytes) {
        return match error.raw_os_error().map(Errno::from_raw_os_error) {
            Some(errno) if classify_errno(errno) == ProcfsErrorClass::Missing => {
                missing_process(pidfd)
            }
            Some(errno) => Err(observation_error(errno)),
            None => Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete),
        };
    }
    let effective_uid = parse_effective_uid(&bytes)?;
    if pidfd_alive(pidfd)? {
        Ok(Some(effective_uid))
    } else {
        Ok(None)
    }
}

fn process_has_inode_at(
    process_directory: &OwnedFd,
    pidfd: &OwnedFd,
    inode: u64,
    maximum_descriptors: usize,
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
) -> Result<Option<bool>, AttachedProcessWitnessError> {
    let descriptor_directory = match openat(process_directory, "fd", DIRECTORY_FLAGS, Mode::empty())
    {
        Ok(directory) => directory,
        Err(error) if classify_errno(error) == ProcfsErrorClass::Missing => {
            return missing_process(pidfd);
        }
        Err(error) => return Err(observation_error(error)),
    };
    let expected = format!("socket:[{inode}]");
    let mut buffer = [MaybeUninit::uninit(); DIRECTORY_BUFFER_BYTES];
    let mut entries = RawDir::new(&descriptor_directory, &mut buffer);
    let mut descriptor_count = 0_usize;
    let mut found = false;
    while let Some(entry) = entries.next() {
        ensure_active(cancellation, started, limits)?;
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) if classify_errno(error) == ProcfsErrorClass::Missing => {
                return missing_process(pidfd);
            }
            Err(error) => return Err(observation_error(error)),
        };
        let name = entry.file_name();
        let name_bytes = name.to_bytes();
        if matches!(name_bytes, b"." | b"..") {
            continue;
        }
        parse_descriptor(name_bytes)?;
        descriptor_count = descriptor_count
            .checked_add(1)
            .ok_or(AttachedProcessWitnessError::ResourceLimit)?;
        if descriptor_count > maximum_descriptors {
            return Err(AttachedProcessWitnessError::ResourceLimit);
        }
        match readlinkat(&descriptor_directory, name, Vec::new()) {
            Ok(target) if target.to_bytes() == expected.as_bytes() => found = true,
            Ok(_) | Err(Errno::NOENT) => {}
            Err(error) if classify_errno(error) == ProcfsErrorClass::Missing => {
                if !pidfd_alive(pidfd)? {
                    return Ok(None);
                }
                return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
            }
            Err(error) => return Err(observation_error(error)),
        }
    }
    if pidfd_alive(pidfd)? {
        Ok(Some(found))
    } else {
        Ok(None)
    }
}

fn missing_process<T>(pidfd: &OwnedFd) -> Result<Option<T>, AttachedProcessWitnessError> {
    if pidfd_alive(pidfd)? {
        Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
    } else {
        Ok(None)
    }
}

fn pidfd_alive(pidfd: &OwnedFd) -> Result<bool, AttachedProcessWitnessError> {
    let mut descriptors = [PollFd::new(pidfd, PollFlags::IN)];
    let ready = poll(&mut descriptors, Some(&Timespec::default())).map_err(observation_error)?;
    Ok(ready == 0)
}

fn raw_pid(pid: u32) -> Result<Pid, AttachedProcessWitnessError> {
    i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
}

fn parse_proc_pid(name: &[u8]) -> Result<Option<u32>, AttachedProcessWitnessError> {
    if !name.iter().all(u8::is_ascii_digit) {
        return Ok(None);
    }
    let pid = parse_decimal_u32(name)?;
    raw_pid(pid)?;
    Ok(Some(pid))
}

fn parse_descriptor(name: &[u8]) -> Result<u32, AttachedProcessWitnessError> {
    if name.is_empty() || !name.iter().all(u8::is_ascii_digit) {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    parse_decimal_u32(name)
}

fn parse_effective_uid(status: &[u8]) -> Result<u32, AttachedProcessWitnessError> {
    if status.len() > MAXIMUM_STATUS_BYTES {
        return Err(AttachedProcessWitnessError::ResourceLimit);
    }
    let mut effective_uid = None;
    for line in status.split(|byte| *byte == b'\n') {
        let Some(fields) = line.strip_prefix(b"Uid:") else {
            continue;
        };
        if effective_uid.is_some() {
            return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
        }
        if !fields.first().is_some_and(u8::is_ascii_whitespace) {
            return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
        }
        let fields = fields
            .split(u8::is_ascii_whitespace)
            .filter(|field| !field.is_empty())
            .map(parse_decimal_u32)
            .collect::<Result<Vec<_>, _>>()?;
        let [_, effective, _, _] = fields.as_slice() else {
            return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
        };
        effective_uid = Some(*effective);
    }
    effective_uid.ok_or(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
}

fn require_unchanged_effective_uid(
    initial: u32,
    confirmed: u32,
) -> Result<(), AttachedProcessWitnessError> {
    if confirmed == initial {
        Ok(())
    } else {
        Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
    }
}

fn parse_decimal_u32(bytes: &[u8]) -> Result<u32, AttachedProcessWitnessError> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete);
    }
    bytes.iter().try_fold(0_u32, |value, byte| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(*byte - b'0')))
            .ok_or(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
    })
}

fn classify_errno(error: Errno) -> ProcfsErrorClass {
    match error {
        Errno::NOENT | Errno::SRCH => ProcfsErrorClass::Missing,
        Errno::ACCESS | Errno::PERM => ProcfsErrorClass::AccessDenied,
        Errno::MFILE | Errno::NFILE | Errno::NOMEM => ProcfsErrorClass::ResourceLimit,
        _ => ProcfsErrorClass::Incomplete,
    }
}

fn skip_uninspectable(error: Errno) -> bool {
    matches!(
        classify_errno(error),
        ProcfsErrorClass::Missing | ProcfsErrorClass::AccessDenied
    )
}

fn observation_error(error: Errno) -> AttachedProcessWitnessError {
    match classify_errno(error) {
        ProcfsErrorClass::AccessDenied => AttachedProcessWitnessError::ProcessAccessDenied,
        ProcfsErrorClass::ResourceLimit => AttachedProcessWitnessError::ResourceLimit,
        ProcfsErrorClass::Missing | ProcfsErrorClass::Incomplete => {
            AttachedProcessWitnessError::ListenerSnapshotIncomplete
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        net::{Ipv4Addr, TcpListener},
        os::{fd::AsRawFd as _, unix::fs::MetadataExt as _},
        process::Command,
        time::Instant,
    };

    use rewrite_types::CancellationToken;
    use rustix::process::{
        DumpableBehavior, Pid, PidfdFlags, geteuid, pidfd_open, set_dumpable_behavior,
    };

    use super::{
        MAXIMUM_STATUS_BYTES, ProcfsErrorClass, classify_errno, effective_uid_for_pid,
        parse_effective_uid, process_has_inode, require_unchanged_effective_uid,
        skip_uninspectable,
    };
    use crate::{AttachedProcessWitnessError, AttachedProcessWitnessLimits};

    const CHILD_ENVIRONMENT: &str = "REWRITE_ATTESTOR_NONDUMPABLE_CHILD";
    const NONDUMPABLE_TEST: &str = concat!(
        "platform::linux_proc_holders::tests::",
        "nondumpable_self_uses_status_euid_and_anchored_descriptors"
    );

    #[test]
    fn strict_status_parser_selects_only_one_exact_effective_uid() {
        let status = b"Name:\ttarget\nUid:\t91\t92\t93\t94\nGid:\t1\t2\t3\t4\n";
        assert_eq!(parse_effective_uid(status), Ok(92));
        assert_eq!(
            parse_effective_uid(b"Uid:\t0\t4294967295\t0\t0\n"),
            Ok(u32::MAX)
        );

        for malformed in [
            &b"Name:\ttarget\n"[..],
            &b"Uid:\t1\t2\t3\n"[..],
            &b"Uid:\t1\t2\t3\t4\t5\n"[..],
            &b"Uid:\t1\t-2\t3\t4\n"[..],
            &b"Uid:\t1\t+2\t3\t4\n"[..],
            &b"Uid:\t1\t0x2\t3\t4\n"[..],
            &b"Uid:\t1\t4294967296\t3\t4\n"[..],
            &b"Uid:1\t2\t3\t4\n"[..],
            &b"Uid:\t1\t2\t3\t4\nUid:\t1\t2\t3\t4\n"[..],
        ] {
            assert_eq!(
                parse_effective_uid(malformed),
                Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
            );
        }
        assert_eq!(
            parse_effective_uid(&vec![b'x'; MAXIMUM_STATUS_BYTES + 1]),
            Err(AttachedProcessWitnessError::ResourceLimit)
        );
    }

    #[test]
    fn holder_effective_uid_must_remain_stable_across_descriptor_observation() {
        assert_eq!(require_unchanged_effective_uid(92, 92), Ok(()));
        assert_eq!(
            require_unchanged_effective_uid(92, 93),
            Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
        );
    }

    #[test]
    fn procfs_errno_classes_keep_denial_and_resource_failures_terminal() {
        use ProcfsErrorClass::{AccessDenied, Incomplete, Missing, ResourceLimit};
        use rustix::io::Errno;
        for (error, class, skip) in [
            (Errno::NOENT, Missing, true),
            (Errno::SRCH, Missing, true),
            (Errno::ACCESS, AccessDenied, true),
            (Errno::PERM, AccessDenied, true),
            (Errno::MFILE, ResourceLimit, false),
            (Errno::NFILE, ResourceLimit, false),
            (Errno::NOMEM, ResourceLimit, false),
            (Errno::IO, Incomplete, false),
        ] {
            assert_eq!(classify_errno(error), class);
            assert_eq!(skip_uninspectable(error), skip);
        }
    }

    #[test]
    fn nondumpable_self_uses_status_euid_and_anchored_descriptors() {
        if env::var_os(CHILD_ENVIRONMENT).is_none() {
            let status = Command::new(env::current_exe().expect("current test executable"))
                .args(["--exact", NONDUMPABLE_TEST, "--test-threads=1"])
                .env(CHILD_ENVIRONMENT, "1")
                .status()
                .expect("launch nondumpable test child");
            assert!(status.success(), "nondumpable test child failed: {status}");
            return;
        }

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
        let descriptor = listener.as_raw_fd();
        let target =
            fs::read_link(format!("/proc/self/fd/{descriptor}")).expect("read listener descriptor");
        let inode = target
            .to_str()
            .and_then(|target| target.strip_prefix("socket:["))
            .and_then(|target| target.strip_suffix(']'))
            .and_then(|target| target.parse::<u64>().ok())
            .expect("parse listener inode");
        set_dumpable_behavior(DumpableBehavior::NotDumpable).expect("disable dumpability");
        let effective_uid = geteuid().as_raw();
        if effective_uid != 0 {
            assert_ne!(
                fs::metadata("/proc/self/fd")
                    .expect("descriptor directory metadata")
                    .uid(),
                effective_uid
            );
        }
        let pid = Pid::from_raw(i32::try_from(std::process::id()).expect("PID fits i32"))
            .expect("nonzero PID");
        let pidfd = pidfd_open(pid, PidfdFlags::empty()).expect("open current pidfd");
        let limits = AttachedProcessWitnessLimits::default();
        let cancellation = CancellationToken::new();
        assert_eq!(
            effective_uid_for_pid(
                std::process::id(),
                &pidfd,
                limits,
                &cancellation,
                Instant::now(),
            ),
            Ok(effective_uid)
        );
        assert_eq!(
            process_has_inode(
                std::process::id(),
                inode,
                limits.maximum_descriptors_per_process,
                &cancellation,
                Instant::now(),
                limits,
            ),
            Ok(true)
        );
    }
}
