use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read as _,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    time::Instant,
};

use rewrite_model::{
    NativeLoadEvidenceClass, NativeLoadObservation, NativeLoadOrigin, NativeLoadedComponent,
    NativeMappingClass, RuntimeOperatingSystem, RuntimePackageManifest, RuntimePackageMember,
};
use rewrite_types::{CancellationToken, Digest};
use rustix::fd::OwnedFd;

use super::{
    linux::ensure_pidfd_alive,
    native_load_common::{HashBudget, finish_observation, hash_file},
};
use crate::{
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeLoadObserverError,
    ensure_native_active, native_load::is_retained_package_member,
};

const CONTRACT_ID: &str = "linux-proc-map-files";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ObjectKey {
    device: u64,
    inode: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableMapping {
    start: usize,
    end: usize,
    key: ObjectKey,
}

impl super::linux::Lease {
    pub(crate) fn observe_native_load(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        limits: NativeLoadObservationLimits,
        cancellation: &CancellationToken,
        started: Instant,
        process_evidence_digest: &Digest,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError> {
        observe(
            self.owner.pid,
            &self.pidfd,
            &self.entrypoint,
            request,
            limits,
            cancellation,
            started,
            process_evidence_digest,
        )
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the observer makes every retained capability and resource boundary explicit"
)]
pub(super) fn observe(
    pid: u32,
    pidfd: &OwnedFd,
    entrypoint: &File,
    request: &NativeLoadObservationRequest<'_>,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
    process_evidence_digest: &Digest,
) -> Result<NativeLoadObservation, NativeLoadObserverError> {
    if request.package.target().operating_system() != RuntimeOperatingSystem::Linux {
        return Err(NativeLoadObserverError::InvalidRequest);
    }
    ensure_native_active(cancellation, started, limits)?;
    ensure_pidfd_alive(pidfd).map_err(|_error| NativeLoadObserverError::ProcessChanged)?;
    let mut budget = HashBudget::new(limits.maximum_aggregate_hash_bytes);
    let package = package_index(
        request.retained_package_members,
        request.package,
        limits,
        cancellation,
        started,
        &mut budget,
    )?;
    let entrypoint_key = metadata_key(
        &entrypoint
            .metadata()
            .map_err(|_error| NativeLoadObserverError::MappedObjectUnavailable)?,
    )?;
    let first = snapshot(
        pid,
        request,
        limits,
        cancellation,
        started,
        process_evidence_digest,
        &package,
        entrypoint_key,
        &mut budget,
    )?;
    ensure_pidfd_alive(pidfd).map_err(|_error| NativeLoadObserverError::ProcessChanged)?;
    let second = snapshot(
        pid,
        request,
        limits,
        cancellation,
        started,
        process_evidence_digest,
        &package,
        entrypoint_key,
        &mut budget,
    )?;
    ensure_pidfd_alive(pidfd).map_err(|_error| NativeLoadObserverError::ProcessChanged)?;
    if first != second {
        return Err(NativeLoadObserverError::ObservationChanged);
    }
    Ok(first)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the snapshot makes every retained capability and resource boundary explicit"
)]
fn snapshot(
    pid: u32,
    request: &NativeLoadObservationRequest<'_>,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
    process_evidence_digest: &Digest,
    package: &BTreeMap<ObjectKey, &RuntimePackageMember>,
    entrypoint_key: ObjectKey,
    budget: &mut HashBudget,
) -> Result<NativeLoadObservation, NativeLoadObserverError> {
    let mappings = read_executable_mappings(pid, limits, cancellation, started)?;
    let mut components = Vec::new();
    let mut prior = None;
    for mapping in mappings {
        ensure_native_active(cancellation, started, limits)?;
        if prior == Some(mapping.key) {
            continue;
        }
        prior = Some(mapping.key);
        if components.len() >= limits.maximum_components {
            return Err(NativeLoadObserverError::ResourceLimit);
        }
        components.push(observe_mapping(
            pid,
            &mapping,
            package,
            entrypoint_key,
            limits,
            cancellation,
            started,
            budget,
        )?);
    }
    finish_observation(
        request.package,
        request.expected_external_components,
        NativeLoadEvidenceClass::LinuxProcMapFiles,
        CONTRACT_ID,
        process_evidence_digest,
        components,
    )
}

fn package_index<'a>(
    retained: &[crate::RetainedNativePackageMember],
    package: &'a RuntimePackageManifest,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
    budget: &mut HashBudget,
) -> Result<BTreeMap<ObjectKey, &'a RuntimePackageMember>, NativeLoadObserverError> {
    let mut index = BTreeMap::new();
    let packaged_code = package
        .members()
        .iter()
        .filter(|member| is_retained_package_member(member));
    for (member, retained) in packaged_code.zip(retained) {
        ensure_native_active(cancellation, started, limits)?;
        if member.relative_path() != retained.relative_path()
            || member.artifact_id() != retained.artifact_id()
            || member.byte_size() != retained.byte_size()
        {
            return Err(NativeLoadObserverError::InvalidRequest);
        }
        let mut file = retained
            .file()
            .try_clone()
            .map_err(|_error| NativeLoadObserverError::MappedObjectUnavailable)?;
        let before = file_metadata(&file)?;
        let artifact_id = hash_file(
            &mut file,
            before.length,
            budget,
            limits,
            cancellation,
            started,
        )?;
        let after = file_metadata(&file)?;
        if before != after
            || artifact_id != *member.artifact_id()
            || before.length != member.byte_size()
        {
            return Err(NativeLoadObserverError::ObservationChanged);
        }
        if index.insert(before.key, member).is_some() {
            return Err(NativeLoadObserverError::InvalidRequest);
        }
    }
    Ok(index)
}

fn read_executable_mappings(
    pid: u32,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<ExecutableMapping>, NativeLoadObserverError> {
    let path = PathBuf::from(format!("/proc/{pid}/maps"));
    let bytes = read_bounded(&path, limits, cancellation, started)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    let mut mappings = Vec::new();
    let mut rows = 0_usize;
    for line in text.lines() {
        ensure_native_active(cancellation, started, limits)?;
        rows = rows
            .checked_add(1)
            .ok_or(NativeLoadObserverError::ResourceLimit)?;
        if rows > limits.maximum_mapping_regions {
            return Err(NativeLoadObserverError::ResourceLimit);
        }
        if let Some(mapping) = parse_mapping(line)? {
            mappings.push(mapping);
        }
    }
    mappings.sort_by_key(|mapping| (mapping.key, mapping.start, mapping.end));
    Ok(mappings)
}

fn parse_mapping(line: &str) -> Result<Option<ExecutableMapping>, NativeLoadObserverError> {
    let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.len() < 5 {
        return Err(NativeLoadObserverError::PlatformObservationFailed);
    }
    let permissions = fields[1].as_bytes();
    if permissions.len() != 4
        || !matches!(permissions[0], b'r' | b'-')
        || !matches!(permissions[1], b'w' | b'-')
        || !matches!(permissions[2], b'x' | b'-')
        || !matches!(permissions[3], b'p' | b's')
    {
        return Err(NativeLoadObserverError::PlatformObservationFailed);
    }
    let _offset = u64::from_str_radix(fields[2], 16)
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    let (start, end) = fields[0]
        .split_once('-')
        .ok_or(NativeLoadObserverError::PlatformObservationFailed)?;
    let start = usize::from_str_radix(start, 16)
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    let end = usize::from_str_radix(end, 16)
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    let (major, minor) = fields[3]
        .split_once(':')
        .ok_or(NativeLoadObserverError::PlatformObservationFailed)?;
    let major = u64::from_str_radix(major, 16)
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    let minor = u64::from_str_radix(minor, 16)
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    let inode = fields[4]
        .parse::<u64>()
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    if start >= end || major > u64::from(u32::MAX) || minor > u64::from(u32::MAX) {
        return Err(NativeLoadObserverError::PlatformObservationFailed);
    }
    if permissions[2] != b'x' {
        return Ok(None);
    }
    let name = fields.get(5).copied().unwrap_or_default();
    // These two exact names are kernel-synthesized executable mappings with no
    // file object. The file-backed scope excludes them. Every other anonymous
    // or bracket-named executable mapping fails closed.
    if fields.len() == 6 && matches!(name, "[vdso]" | "[vsyscall]") {
        return Ok(None);
    }
    if name.is_empty()
        || name.starts_with('[')
        || fields.last().is_some_and(|value| *value == "(deleted)")
        || inode == 0
    {
        return Err(NativeLoadObserverError::UnverifiableExecutableMapping);
    }
    Ok(Some(ExecutableMapping {
        start,
        end,
        key: ObjectKey {
            device: linux_device(major, minor),
            inode,
        },
    }))
}

fn linux_device(major: u64, minor: u64) -> u64 {
    ((major & 0xffff_f000) << 32)
        | ((major & 0x0000_0fff) << 8)
        | ((minor & 0xffff_ff00) << 12)
        | (minor & 0x0000_00ff)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the observer makes every bounded input explicit"
)]
fn observe_mapping(
    pid: u32,
    mapping: &ExecutableMapping,
    package: &BTreeMap<ObjectKey, &RuntimePackageMember>,
    entrypoint_key: ObjectKey,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
    budget: &mut HashBudget,
) -> Result<NativeLoadedComponent, NativeLoadObserverError> {
    let path = format!(
        "/proc/{pid}/map_files/{:x}-{:x}",
        mapping.start, mapping.end
    );
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            NativeLoadObserverError::ProcessVisibilityInsufficient
        } else {
            NativeLoadObserverError::MappedObjectUnavailable
        }
    })?;
    let before = file_metadata(&file)?;
    if before.key != mapping.key {
        return Err(NativeLoadObserverError::ObservationChanged);
    }
    let artifact_id = hash_file(
        &mut file,
        before.length,
        budget,
        limits,
        cancellation,
        started,
    )?;
    let after = file_metadata(&file)?;
    if before != after {
        return Err(NativeLoadObserverError::ObservationChanged);
    }
    let mapping_class = if before.key == entrypoint_key {
        NativeMappingClass::ExecutableImage
    } else {
        NativeMappingClass::ExecutableMapped
    };
    let origin = match package.get(&before.key) {
        Some(member) => NativeLoadOrigin::PackagedMember {
            relative_path: member.relative_path().clone(),
        },
        None => NativeLoadOrigin::ExternalPlatformComponent,
    };
    Ok(NativeLoadedComponent::new(
        artifact_id,
        before.length,
        origin,
        mapping_class,
        object_digest(&before),
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileMetadata {
    key: ObjectKey,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

fn file_metadata(file: &File) -> Result<FileMetadata, NativeLoadObserverError> {
    let metadata = file
        .metadata()
        .map_err(|_error| NativeLoadObserverError::MappedObjectUnavailable)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(NativeLoadObserverError::InvalidMappedObject);
    }
    Ok(FileMetadata {
        key: metadata_key(&metadata)?,
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}

fn metadata_key(metadata: &fs::Metadata) -> Result<ObjectKey, NativeLoadObserverError> {
    if metadata.dev() == 0 || metadata.ino() == 0 {
        return Err(NativeLoadObserverError::InvalidMappedObject);
    }
    Ok(ObjectKey {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn object_digest(metadata: &FileMetadata) -> Digest {
    let material = format!(
        "linux-native-file-object-v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        metadata.key.device,
        metadata.key.inode,
        metadata.length,
        metadata.modified_seconds,
        metadata.modified_nanoseconds,
        metadata.changed_seconds,
        metadata.changed_nanoseconds
    );
    Digest::sha256(material.as_bytes())
}

fn read_bounded(
    path: &std::path::Path,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<u8>, NativeLoadObserverError> {
    let mut file = File::open(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            NativeLoadObserverError::ProcessVisibilityInsufficient
        } else {
            NativeLoadObserverError::PlatformObservationFailed
        }
    })?;
    let mut bytes = Vec::with_capacity(limits.maximum_mapping_metadata_bytes.min(64 * 1024));
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        ensure_native_active(cancellation, started, limits)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > limits.maximum_mapping_metadata_bytes {
            return Err(NativeLoadObserverError::ResourceLimit);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{NativeLoadObserverError, parse_mapping};

    #[test]
    fn parser_rejects_anonymous_deleted_and_malformed_executable_rows() {
        assert_eq!(
            parse_mapping("1000-2000 r-xp 00000000 00:00 0"),
            Err(NativeLoadObserverError::UnverifiableExecutableMapping)
        );
        assert_eq!(
            parse_mapping("1000-2000 r-xp 00000000 08:01 2 /tmp/a (deleted)"),
            Err(NativeLoadObserverError::UnverifiableExecutableMapping)
        );
        assert_eq!(
            parse_mapping("invalid"),
            Err(NativeLoadObserverError::PlatformObservationFailed)
        );
        assert_eq!(
            parse_mapping("1000-2000 zzzp 00000000 08:01 2 /tmp/a"),
            Err(NativeLoadObserverError::PlatformObservationFailed)
        );
        assert_eq!(
            parse_mapping("1000-2000 r-xp zzzzzzzz 08:01 2 /tmp/a"),
            Err(NativeLoadObserverError::PlatformObservationFailed)
        );
    }

    #[test]
    fn parser_admits_exact_kernel_synthetic_exceptions_only() {
        assert_eq!(
            parse_mapping("1000-2000 r-xp 00000000 00:00 0 [vdso]").expect("vDSO exception"),
            None
        );
        assert_eq!(
            parse_mapping("1000-2000 r-xp 00000000 00:00 0 [jit]"),
            Err(NativeLoadObserverError::UnverifiableExecutableMapping)
        );
    }
}
