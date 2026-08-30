use std::{fs::File, io::Read as _, path::Path, time::Instant};

use rewrite_types::CancellationToken;

use super::super::process::ObjectKey;
use crate::managed_worker::ensure_worker_active;
use crate::{ManagedGenerationWorkerError, ManagedGenerationWorkerLimits};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MappingRow {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) executable: bool,
    pub(super) key: ObjectKey,
    pub(super) path: String,
}

pub(super) fn read_mappings(
    pid: u32,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<Vec<MappingRow>, ManagedGenerationWorkerError> {
    let mut file = File::open(format!("/proc/{pid}/maps")).map_err(map_mapping_error)?;
    let mut bytes = Vec::with_capacity(
        limits
            .native_load
            .maximum_mapping_metadata_bytes
            .min(64 * 1024),
    );
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        ensure_worker_active(cancellation, started, limits)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) > limits.native_load.maximum_mapping_metadata_bytes {
            return Err(ManagedGenerationWorkerError::ResourceLimit);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let mut mappings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index >= limits.native_load.maximum_mapping_regions {
            return Err(ManagedGenerationWorkerError::ResourceLimit);
        }
        if let Some(mapping) = parse_mapping(line)? {
            mappings.push(mapping);
        }
    }
    mappings.sort_by_key(|mapping| (mapping.key, mapping.start, mapping.end));
    Ok(mappings)
}

fn parse_mapping(line: &str) -> Result<Option<MappingRow>, ManagedGenerationWorkerError> {
    let fields = line.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.len() < 5 {
        return Err(ManagedGenerationWorkerError::PlatformObservationFailed);
    }
    let permissions = fields[1].as_bytes();
    if permissions.len() != 4
        || !matches!(permissions[0], b'r' | b'-')
        || !matches!(permissions[1], b'w' | b'-')
        || !matches!(permissions[2], b'x' | b'-')
        || !matches!(permissions[3], b'p' | b's')
    {
        return Err(ManagedGenerationWorkerError::PlatformObservationFailed);
    }
    u64::from_str_radix(fields[2], 16)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let (start, end) = fields[0]
        .split_once('-')
        .ok_or(ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let start = u64::from_str_radix(start, 16)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let end = u64::from_str_radix(end, 16)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let (major, minor) = fields[3]
        .split_once(':')
        .ok_or(ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let major = u64::from_str_radix(major, 16)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let minor = u64::from_str_radix(minor, 16)
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    let inode = fields[4]
        .parse::<u64>()
        .map_err(|_error| ManagedGenerationWorkerError::PlatformObservationFailed)?;
    if start >= end || major > u64::from(u32::MAX) || minor > u64::from(u32::MAX) {
        return Err(ManagedGenerationWorkerError::PlatformObservationFailed);
    }
    let path = fields.get(5).copied().unwrap_or_default();
    if permissions[2] == b'x' && fields.len() == 6 && matches!(path, "[vdso]" | "[vsyscall]") {
        return Ok(None);
    }
    if permissions[2] == b'x'
        && (path.is_empty()
            || path.starts_with('[')
            || fields.last() == Some(&"(deleted)")
            || inode == 0)
    {
        return Err(ManagedGenerationWorkerError::UnverifiableExecutableMapping);
    }
    if path.is_empty() || path.starts_with('[') || inode == 0 {
        return Ok(None);
    }
    if fields.len() != 6 || fields.last() == Some(&"(deleted)") {
        return Err(ManagedGenerationWorkerError::ModelMappingMismatch);
    }
    Ok(Some(MappingRow {
        start,
        end,
        executable: permissions[2] == b'x',
        key: ObjectKey {
            device: linux_device(major, minor),
            inode,
        },
        path: path.to_owned(),
    }))
}

pub(super) fn open_map_file(
    pid: u32,
    mapping: &MappingRow,
) -> Result<File, ManagedGenerationWorkerError> {
    File::open(format!(
        "/proc/{pid}/map_files/{:x}-{:x}",
        mapping.start, mapping.end
    ))
    .map_err(map_mapping_error)
}

pub(super) fn accelerator_path(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_ascii_lowercase();
    [
        "cuda",
        "nvidia",
        "rocm",
        "amdhip",
        "hiprtc",
        "vulkan",
        "opencl",
        "ze_loader",
        "level_zero",
    ]
    .into_iter()
    .any(|needle| name.contains(needle))
}

fn linux_device(major: u64, minor: u64) -> u64 {
    ((major & 0xffff_f000) << 32)
        | ((major & 0x0000_0fff) << 8)
        | ((minor & 0xffff_ff00) << 12)
        | (minor & 0x0000_00ff)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "Result::map_err supplies the owned I/O error"
)]
fn map_mapping_error(error: std::io::Error) -> ManagedGenerationWorkerError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        ManagedGenerationWorkerError::ProcessVisibilityInsufficient
    } else {
        ManagedGenerationWorkerError::PlatformObservationFailed
    }
}

#[cfg(test)]
mod tests;
