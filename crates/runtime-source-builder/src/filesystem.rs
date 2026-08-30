use std::{fs, io::Read as _, path::Path};

use rewrite_ollama_package::SelfContainedLinuxExecutable;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::BuildError;

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn capability_path(fd: i32) -> Result<std::path::PathBuf, BuildError> {
    if fd <= 2 {
        return Err(BuildError::InvalidCapability);
    }
    Ok(std::path::PathBuf::from(format!("/proc/self/fd/{fd}")))
}

pub(super) fn exact_capability_root(
    name: &str,
    expected: &str,
    descriptor_path: &Path,
) -> Result<std::path::PathBuf, BuildError> {
    let selected = std::env::var_os(name).ok_or(BuildError::InvalidCapability)?;
    if selected != expected {
        return Err(BuildError::InvalidCapability);
    }
    let path = std::path::PathBuf::from(selected);
    let descriptor = fs::metadata(descriptor_path).map_err(|_| BuildError::InvalidCapability)?;
    let mounted = fs::metadata(&path).map_err(|_| BuildError::InvalidCapability)?;
    if same_object(&descriptor, &mounted) {
        Ok(path)
    } else {
        Err(BuildError::InvalidCapability)
    }
}

pub(super) fn exact_capability_fd(name: &str) -> Result<i32, BuildError> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|fd| *fd > 2)
        .ok_or(BuildError::InvalidCapability)
}

pub(super) fn require_empty_output(root: &Path) -> Result<(), BuildError> {
    if fs::read_dir(root)
        .map_err(|_| BuildError::InvalidCapability)?
        .next()
        .is_some()
    {
        Err(BuildError::OutputInvalid)
    } else {
        Ok(())
    }
}

pub(super) fn verify_self_contained(path: &Path) -> Result<(), BuildError> {
    let file = fs::File::open(path).map_err(|_| BuildError::InvalidInput)?;
    let metadata = file.metadata().map_err(|_| BuildError::InvalidInput)?;
    if !metadata.is_file()
        || metadata.len()
            > u64::try_from(SelfContainedLinuxExecutable::MAXIMUM_BYTES).unwrap_or(u64::MAX)
    {
        return Err(BuildError::ToolNotSelfContained);
    }
    let capacity = usize::try_from(metadata.len()).map_err(|_| BuildError::ToolNotSelfContained)?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(
        u64::try_from(SelfContainedLinuxExecutable::MAXIMUM_BYTES)
            .unwrap_or(u64::MAX)
            .saturating_add(1),
    )
    .read_to_end(&mut bytes)
    .map_err(|_| BuildError::InvalidInput)?;
    SelfContainedLinuxExecutable::verify(&bytes)
        .map(|_verified| ())
        .map_err(|_| BuildError::ToolNotSelfContained)
}

pub(super) fn digest_file(path: &Path) -> Result<(u64, Digest), BuildError> {
    let mut file = fs::File::open(path).map_err(|_| BuildError::OutputInvalid)?;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut bytes = 0_u64;
    let mut hasher = Sha256::new();
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| BuildError::OutputInvalid)?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).map_err(|_| BuildError::OutputInvalid)?)
            .ok_or(BuildError::OutputInvalid)?;
        hasher.update(&buffer[..read]);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| BuildError::OutputInvalid)?;
    Ok((bytes, digest))
}

pub(super) fn copy_file(source: &Path, destination: &Path) -> Result<(), BuildError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|_| BuildError::OutputInvalid)?;
    }
    let metadata = fs::symlink_metadata(source).map_err(|_| BuildError::InvalidInput)?;
    if !metadata.is_file() {
        return Err(BuildError::InvalidInput);
    }
    fs::copy(source, destination).map_err(|_| BuildError::OutputInvalid)?;
    Ok(())
}

fn same_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino()
}
