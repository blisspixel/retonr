use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read as _, Seek as _, SeekFrom},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
};

use rewrite_types::Digest as DomainDigest;
use sha2::{Digest as _, Sha256};

use crate::MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES;

use super::HelperFailure;

const COPY_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn hash_file(path: impl AsRef<Path>) -> Result<DomainDigest, HelperFailure> {
    let mut file = File::open(path).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::BootstrapRootVerification)
}

pub(super) fn hash_tree(root: &Path) -> Result<DomainDigest, HelperFailure> {
    let mut entries = Vec::new();
    collect_tree(root, root, &mut entries)?;
    entries.sort();
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry);
        hasher.update([0]);
    }
    DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::BootstrapRootVerification)
}

fn collect_tree(root: &Path, path: &Path, output: &mut Vec<Vec<u8>>) -> Result<(), HelperFailure> {
    for raw in fs::read_dir(path).map_err(|_| HelperFailure::BootstrapRootVerification)? {
        let raw = raw.map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let child = raw.path();
        let relative = child
            .strip_prefix(root)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let metadata =
            fs::symlink_metadata(&child).map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let mut record = relative.as_os_str().as_encoded_bytes().to_vec();
        record.extend_from_slice(&metadata.mode().to_be_bytes());
        if metadata.is_dir() {
            record.push(b'd');
            output.push(record);
            collect_tree(root, &child, output)?;
        } else if metadata.is_file() {
            record.push(b'f');
            record.extend_from_slice(hash_file(&child)?.as_str().as_bytes());
            output.push(record);
        } else {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        if output.len()
            > usize::try_from(MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES).unwrap_or(usize::MAX)
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    Ok(())
}

pub(super) fn merge_tree(source: &Path, destination: &Path) -> Result<(), HelperFailure> {
    require_matching_directory_mode(source, destination)?;
    let mut seen = BTreeSet::new();
    merge_directory(source, destination, &mut seen)
}

fn merge_directory(
    source: &Path,
    destination: &Path,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), HelperFailure> {
    if !source.is_dir() {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    for raw in fs::read_dir(source).map_err(|_| HelperFailure::BootstrapRootVerification)? {
        let raw = raw.map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let source_path = raw.path();
        let destination_path = destination.join(raw.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .map_err(|_| HelperFailure::BootstrapRootVerification)?;
        if !seen.insert(destination_path.clone()) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        if metadata.is_dir() {
            if destination_path.exists() {
                require_matching_directory_mode(&source_path, &destination_path)?;
            } else {
                fs::create_dir(&destination_path)
                    .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
                fs::set_permissions(
                    &destination_path,
                    fs::Permissions::from_mode(metadata.mode() & 0o777),
                )
                .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
            }
            merge_directory(&source_path, &destination_path, seen)?;
        } else if metadata.is_file() {
            let source_file =
                File::open(&source_path).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
            copy_file(&source_file, &destination_path, metadata.mode() & 0o777)?;
        } else {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    Ok(())
}

fn require_matching_directory_mode(source: &Path, destination: &Path) -> Result<(), HelperFailure> {
    let source =
        fs::symlink_metadata(source).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let destination =
        fs::symlink_metadata(destination).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if source.is_dir()
        && destination.is_dir()
        && source.mode() & 0o777 == destination.mode() & 0o777
    {
        Ok(())
    } else {
        Err(HelperFailure::BootstrapRootVerification)
    }
}

pub(super) fn copy_file(source: &File, destination: &Path, mode: u32) -> Result<(), HelperFailure> {
    let mut source = source
        .try_clone()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(destination)
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    std::io::copy(&mut source, &mut destination)
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    destination
        .sync_data()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_rejects_non_regular_entries_and_is_deterministic() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let source = temporary.path().join("source");
        let target = temporary.path().join("target");
        fs::create_dir(&source).expect("source");
        fs::create_dir(&target).expect("target");
        fs::write(source.join("file"), b"value").expect("file");
        merge_tree(&source, &target).expect("merge");
        let first = hash_tree(&target).expect("first digest");
        let second = hash_tree(&target).expect("second digest");
        assert_eq!(first, second);
        let linked_source = temporary.path().join("linked-source");
        let linked_target = temporary.path().join("linked-target");
        fs::create_dir(&linked_source).expect("linked source");
        fs::create_dir(&linked_target).expect("linked target");
        std::os::unix::fs::symlink("missing", linked_source.join("link")).expect("link");
        assert_eq!(
            merge_tree(&linked_source, &linked_target),
            Err(HelperFailure::BootstrapRootVerification)
        );
    }
}
