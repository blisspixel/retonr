use std::{
    fs,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
    time::Duration,
};

use rustix::{
    process::{Resource, Rlimit, getrlimit, setrlimit},
    thread::{
        CapabilitySet, CapabilitySets, capabilities, capability_is_in_bounding_set, no_new_privs,
        remove_capability_from_bounding_set, set_capabilities, set_no_new_privs,
    },
};

use super::linux_helper_setup::HelperFailure;

const CANARY_TIMEOUT: Duration = Duration::from_millis(100);

pub(super) fn apply_resource_limits(
    (open_files, processes): (u64, u64),
) -> Result<(), HelperFailure> {
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

pub(super) fn run_network_canaries() -> Result<(), HelperFailure> {
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

pub(super) fn drop_privileges() -> Result<(), HelperFailure> {
    let last_capability = read_capability_limit()?;
    drop_capabilities(last_capability)?;
    if privileges_are_fully_reduced()? {
        Ok(())
    } else {
        Err(HelperFailure::PrivilegeDrop)
    }
}

pub(super) fn read_capability_limit() -> Result<u32, HelperFailure> {
    fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value < u64::BITS)
        .ok_or(HelperFailure::PrivilegeDrop)
}

pub(super) fn drop_capabilities(last_capability: u32) -> Result<(), HelperFailure> {
    set_no_new_privs(true).map_err(|_| HelperFailure::PrivilegeDrop)?;
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
    set_capabilities(None, empty).map_err(|_| HelperFailure::PrivilegeDrop)
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
