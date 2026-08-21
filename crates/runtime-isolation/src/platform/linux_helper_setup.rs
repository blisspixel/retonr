use std::{
    fs::{self, File},
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    os::unix::fs::MetadataExt as _,
    path::Path,
    time::Duration,
};

use rustix::{
    process::{Resource, Rlimit, getgid, getrlimit, getuid, setrlimit},
    thread::{
        CapabilitySet, CapabilitySets, UnshareFlags, capabilities, capability_is_in_bounding_set,
        no_new_privs, remove_capability_from_bounding_set, set_capabilities, set_no_new_privs,
    },
};

const CANARY_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HelperFailure {
    HostPolicyDenied,
    NamespaceSetup,
    LoopbackSetup,
    NetworkCanary,
    DescriptorLeak,
    PrivilegeDrop,
    SocketPolicy,
    InvalidLaunch,
}

impl HelperFailure {
    pub(super) const fn code(self) -> &'static str {
        match self {
            Self::HostPolicyDenied => "host-policy-denied",
            Self::NamespaceSetup => "namespace-setup",
            Self::LoopbackSetup => "loopback-setup",
            Self::NetworkCanary => "network-canary",
            Self::DescriptorLeak => "descriptor-leak",
            Self::PrivilegeDrop => "privilege-drop",
            Self::SocketPolicy => "socket-policy",
            Self::InvalidLaunch => "invalid-launch",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EstablishedIsolation {
    pub(super) loopback_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RawNamespaceIdentity {
    pub(super) device: u64,
    pub(super) inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NamespaceEvidence {
    pub(super) network: RawNamespaceIdentity,
    pub(super) user: RawNamespaceIdentity,
    pub(super) process: RawNamespaceIdentity,
}

impl NamespaceEvidence {
    pub(super) fn current() -> Result<Self, HelperFailure> {
        Ok(Self {
            network: namespace_identity("/proc/self/ns/net")?,
            user: namespace_identity("/proc/self/ns/user")?,
            process: namespace_identity("/proc/self/ns/pid")?,
        })
    }
}

pub(super) fn establish_isolation(
    limits: (u64, u64),
) -> Result<EstablishedIsolation, HelperFailure> {
    if visible_thread_count()? != 1 {
        return Err(HelperFailure::NamespaceSetup);
    }
    let host_user_id = getuid().as_raw();
    let host_group_id = getgid().as_raw();
    #[expect(
        deprecated,
        reason = "the dedicated helper is verified single-threaded"
    )]
    let result = rustix::thread::unshare(
        UnshareFlags::NEWUSER | UnshareFlags::NEWNET | UnshareFlags::NEWPID,
    );
    result.map_err(classify_unshare_error)?;
    write_identity_maps(host_user_id, host_group_id)?;
    apply_resource_limits(limits)?;
    let loopback_index = super::linux_link::enable_and_validate_loopback()?;
    run_network_canaries()?;
    drop_privileges()?;
    Ok(EstablishedIsolation { loopback_index })
}

pub(super) fn validate_descriptor_set() -> Result<(), HelperFailure> {
    let entries = fs::read_dir("/proc/self/fd").map_err(|_| HelperFailure::DescriptorLeak)?;
    let descriptors = entries
        .map(|entry| {
            entry
                .map_err(|_| HelperFailure::DescriptorLeak)?
                .file_name()
                .to_string_lossy()
                .parse::<u32>()
                .map_err(|_| HelperFailure::DescriptorLeak)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for descriptor in descriptors.into_iter().filter(|descriptor| *descriptor > 2) {
        if Path::new(&format!("/proc/self/fd/{descriptor}")).exists() {
            return Err(HelperFailure::DescriptorLeak);
        }
    }
    Ok(())
}

fn visible_thread_count() -> Result<usize, HelperFailure> {
    fs::read_dir("/proc/self/task")
        .map_err(|_| HelperFailure::NamespaceSetup)?
        .try_fold(0_usize, |count, entry| {
            entry
                .map(|_entry| count.saturating_add(1))
                .map_err(|_| HelperFailure::NamespaceSetup)
        })
}

fn write_identity_maps(uid: u32, gid: u32) -> Result<(), HelperFailure> {
    fs::write("/proc/self/setgroups", "deny\n").map_err(|error| classify_mapping_error(&error))?;
    fs::write("/proc/self/uid_map", format!("0 {uid} 1\n"))
        .map_err(|error| classify_mapping_error(&error))?;
    fs::write("/proc/self/gid_map", format!("0 {gid} 1\n"))
        .map_err(|error| classify_mapping_error(&error))?;
    Ok(())
}

fn classify_unshare_error(error: rustix::io::Errno) -> HelperFailure {
    if matches!(error, rustix::io::Errno::PERM | rustix::io::Errno::ACCESS) {
        HelperFailure::HostPolicyDenied
    } else {
        HelperFailure::NamespaceSetup
    }
}

fn classify_mapping_error(error: &io::Error) -> HelperFailure {
    if error.kind() == io::ErrorKind::PermissionDenied {
        HelperFailure::HostPolicyDenied
    } else {
        HelperFailure::NamespaceSetup
    }
}

fn apply_resource_limits((open_files, processes): (u64, u64)) -> Result<(), HelperFailure> {
    let open_files = bounded_resource_limit(Resource::Nofile, open_files);
    let processes = bounded_resource_limit(Resource::Nproc, processes);
    setrlimit(
        Resource::Nofile,
        Rlimit {
            current: Some(open_files),
            maximum: Some(open_files),
        },
    )
    .map_err(|_| HelperFailure::NamespaceSetup)?;
    setrlimit(
        Resource::Nproc,
        Rlimit {
            current: Some(processes),
            maximum: Some(processes),
        },
    )
    .map_err(|_| HelperFailure::NamespaceSetup)
}

fn bounded_resource_limit(resource: Resource, requested: u64) -> u64 {
    getrlimit(resource)
        .maximum
        .map_or(requested, |maximum| requested.min(maximum))
}

fn run_network_canaries() -> Result<(), HelperFailure> {
    allow_loopback(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    allow_loopback(SocketAddr::from((Ipv6Addr::LOCALHOST, 0)))?;
    deny_non_loopback(SocketAddr::from((Ipv4Addr::new(192, 0, 2, 1), 9)))?;
    deny_non_loopback(SocketAddr::from((
        Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
        9,
    )))?;
    Ok(())
}

fn allow_loopback(address: SocketAddr) -> Result<(), HelperFailure> {
    let listener = TcpListener::bind(address).map_err(|_| HelperFailure::NetworkCanary)?;
    let address = listener
        .local_addr()
        .map_err(|_| HelperFailure::NetworkCanary)?;
    TcpStream::connect_timeout(&address, CANARY_TIMEOUT)
        .map(|_stream| ())
        .map_err(|_| HelperFailure::NetworkCanary)
}

fn deny_non_loopback(address: SocketAddr) -> Result<(), HelperFailure> {
    if TcpStream::connect_timeout(&address, CANARY_TIMEOUT).is_err() {
        Ok(())
    } else {
        Err(HelperFailure::NetworkCanary)
    }
}

fn drop_privileges() -> Result<(), HelperFailure> {
    set_no_new_privs(true).map_err(|_| HelperFailure::PrivilegeDrop)?;
    let last_capability = fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value < u64::BITS)
        .ok_or(HelperFailure::PrivilegeDrop)?;
    for bit in 0..=last_capability {
        let capability = CapabilitySet::from_bits_retain(1_u64 << bit);
        if capability_is_in_bounding_set(capability).map_err(|_| HelperFailure::PrivilegeDrop)? {
            remove_capability_from_bounding_set(capability)
                .map_err(|_| HelperFailure::PrivilegeDrop)?;
        }
    }
    let empty = CapabilitySets {
        effective: CapabilitySet::empty(),
        permitted: CapabilitySet::empty(),
        inheritable: CapabilitySet::empty(),
    };
    set_capabilities(None, empty).map_err(|_| HelperFailure::PrivilegeDrop)?;
    if privileges_are_fully_reduced()? {
        Ok(())
    } else {
        Err(HelperFailure::PrivilegeDrop)
    }
}

pub(super) fn privileges_are_fully_reduced() -> Result<bool, HelperFailure> {
    let current = capabilities(None).map_err(|_| HelperFailure::PrivilegeDrop)?;
    let status =
        fs::read_to_string("/proc/self/status").map_err(|_| HelperFailure::PrivilegeDrop)?;
    Ok(no_new_privs().map_err(|_| HelperFailure::PrivilegeDrop)?
        && current.effective.is_empty()
        && current.permitted.is_empty()
        && current.inheritable.is_empty()
        && ["CapBnd:\t0000000000000000", "CapAmb:\t0000000000000000"]
            .iter()
            .all(|field| status.lines().any(|line| line == *field)))
}

fn namespace_identity(path: &str) -> Result<RawNamespaceIdentity, HelperFailure> {
    let metadata = File::open(path)
        .and_then(|file| file.metadata())
        .map_err(|_| HelperFailure::NamespaceSetup)?;
    Ok(RawNamespaceIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
