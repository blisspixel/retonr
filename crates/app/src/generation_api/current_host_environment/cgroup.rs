use std::collections::BTreeSet;

use super::CurrentHostEnvironmentError;
use super::parser::{
    cgroup_type_is_domain, cpu_max_is_unlimited, memory_max_is_unlimited, parse_online_cpu_list,
    parse_unified_cgroup_path,
};

pub(super) const INITIAL_CGROUP_NAMESPACE_INODE: u64 = 0xEFFF_FFFB;
const CGROUP2_MOUNT_POINT: &str = "/sys/fs/cgroup";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CgroupControlFile {
    CpuMax,
    MemoryMax,
    MemoryHigh,
    CgroupType,
    EffectiveCpuSet,
}

pub(super) fn require_initial_namespace_inode(
    inode: Option<u64>,
) -> Result<(), CurrentHostEnvironmentError> {
    match inode {
        Some(INITIAL_CGROUP_NAMESPACE_INODE) => Ok(()),
        Some(_) => Err(CurrentHostEnvironmentError::ConstrainedEnvironment),
        None => Err(CurrentHostEnvironmentError::ObservationUnavailable),
    }
}

pub(super) fn require_returned_mount_id(
    mount_id_was_returned: bool,
    observed_mount_id: u64,
    expected_mount_id: Option<u64>,
) -> Result<u64, CurrentHostEnvironmentError> {
    if !mount_id_was_returned || observed_mount_id == 0 {
        return Err(CurrentHostEnvironmentError::ObservationUnavailable);
    }
    if expected_mount_id.is_some_and(|expected| observed_mount_id != expected) {
        return Err(CurrentHostEnvironmentError::ConstrainedEnvironment);
    }
    Ok(observed_mount_id)
}

pub(super) fn validate_cgroup2_mount_root(
    mountinfo: &[u8],
    expected_mount_id: u64,
) -> Result<(), CurrentHostEnvironmentError> {
    let text = std::str::from_utf8(mountinfo)
        .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
    let mut matching_mount = None;
    for line in text.lines() {
        let (mount, filesystem) = line
            .split_once(" - ")
            .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
        let fields = mount.split_ascii_whitespace().collect::<Vec<_>>();
        let Some(mount_point) = fields.get(4) else {
            return Err(CurrentHostEnvironmentError::InvalidObservation);
        };
        if mount_point.starts_with("/sys/fs/cgroup/") {
            return Err(CurrentHostEnvironmentError::ConstrainedEnvironment);
        }
        if mount_point != &CGROUP2_MOUNT_POINT {
            continue;
        }
        if matching_mount.is_some() {
            return Err(CurrentHostEnvironmentError::ObservationUnavailable);
        }
        let root = fields
            .get(3)
            .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
        let mount_id = fields
            .first()
            .ok_or(CurrentHostEnvironmentError::InvalidObservation)?
            .parse::<u64>()
            .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
        let filesystem_type = filesystem
            .split_ascii_whitespace()
            .next()
            .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
        matching_mount = Some((*root, filesystem_type, mount_id));
    }
    match matching_mount {
        Some(("/", "cgroup2", mount_id)) if mount_id == expected_mount_id => Ok(()),
        Some(_) => Err(CurrentHostEnvironmentError::ConstrainedEnvironment),
        None => Err(CurrentHostEnvironmentError::ObservationUnavailable),
    }
}

pub(super) fn validate_controls(
    membership: &[u8],
    online_cpus: &BTreeSet<u32>,
    mut read: impl FnMut(&[String], CgroupControlFile) -> Result<Vec<u8>, CurrentHostEnvironmentError>,
) -> Result<(), CurrentHostEnvironmentError> {
    let components = parse_unified_cgroup_path(membership)?;
    for depth in 0..=components.len() {
        let ancestor = &components[..depth];
        ensure_unlimited(
            &read(ancestor, CgroupControlFile::CpuMax)?,
            cpu_max_is_unlimited,
        )?;
        ensure_unlimited(
            &read(ancestor, CgroupControlFile::MemoryMax)?,
            memory_max_is_unlimited,
        )?;
        ensure_unlimited(
            &read(ancestor, CgroupControlFile::MemoryHigh)?,
            memory_max_is_unlimited,
        )?;
    }
    if !cgroup_type_is_domain(&read(&components, CgroupControlFile::CgroupType)?)? {
        return Err(CurrentHostEnvironmentError::ConstrainedEnvironment);
    }
    let cpuset = read(&components, CgroupControlFile::EffectiveCpuSet)?;
    if parse_online_cpu_list(&cpuset)? == *online_cpus {
        Ok(())
    } else {
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    }
}

fn ensure_unlimited(
    bytes: &[u8],
    parser: fn(&[u8]) -> Result<bool, CurrentHostEnvironmentError>,
) -> Result<(), CurrentHostEnvironmentError> {
    if parser(bytes)? {
        Ok(())
    } else {
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    }
}
