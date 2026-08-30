use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata},
    io::{Read as _, Seek as _},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest as _, Sha256};
use tar::EntryType;

use crate::BuildError;

const SOURCE_DATE_EPOCH: u64 = 1_725_000_000;
const MAXIMUM_ARCHIVE_ENTRIES: usize = 262_144;
const MAXIMUM_RAW_ARCHIVE_ENTRIES: usize = MAXIMUM_ARCHIVE_ENTRIES * 2;
const MAXIMUM_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAXIMUM_ARCHIVE_FILE_BYTES: u64 = 66 * 1024 * 1024 * 1024;
const MAXIMUM_PATH_BYTES: usize = 4_096;
const MAXIMUM_AGGREGATE_PATH_BYTES: usize = 256 * 1024 * 1024;
const MAXIMUM_EXTENSION_BYTES: usize = MAXIMUM_PATH_BYTES + 1;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn extract_exact_tar(
    archive_path: &Path,
    destination: &Path,
    expected_root: &str,
) -> Result<(), BuildError> {
    validate_root(expected_root)?;
    fs::create_dir(destination).map_err(|_| BuildError::OutputInvalid)?;
    let Ok(destination_identity) = same_file::Handle::from_path(destination) else {
        let _ = fs::remove_dir(destination);
        return Err(BuildError::OutputInvalid);
    };
    let result = extract_acquired(archive_path, destination, expected_root);
    if result.is_err() {
        cleanup_created_directory(destination, destination_identity);
    }
    result
}

fn extract_acquired(
    archive_path: &Path,
    destination: &Path,
    expected_root: &str,
) -> Result<(), BuildError> {
    let mut acquired = AcquiredArchive::open(archive_path)?;
    validate_raw_layout(&mut acquired.file, acquired.metadata.length)?;
    acquired
        .file
        .rewind()
        .map_err(|_| BuildError::InvalidArchive)?;
    extract_semantic_entries(&mut acquired.file, destination, expected_root)?;
    acquired.verify(archive_path)
}

fn validate_raw_layout(file: &mut File, archive_length: u64) -> Result<(), BuildError> {
    let mut archive = tar::Archive::new(file);
    let entries = archive
        .entries()
        .map_err(|_| BuildError::InvalidArchive)?
        .raw(true);
    let mut count = 0_usize;
    let mut extension_bytes = 0_usize;
    let mut pending_long_name = false;
    for entry in entries {
        let mut entry = entry.map_err(|_| BuildError::InvalidArchive)?;
        count = count
            .checked_add(1)
            .filter(|count| *count <= MAXIMUM_RAW_ARCHIVE_ENTRIES)
            .ok_or(BuildError::InvalidArchive)?;
        let raw_path = entry.path_bytes();
        validate_path_bytes(&raw_path)?;
        let entry_type = entry.header().entry_type();
        let size = entry
            .header()
            .size()
            .map_err(|_| BuildError::InvalidArchive)?;
        validate_raw_extent(&entry, size, archive_length)?;
        if entry_type.is_gnu_longname() {
            if pending_long_name {
                return Err(BuildError::UnsafeArchive);
            }
            let size = usize::try_from(size)
                .ok()
                .filter(|size| (2..=MAXIMUM_EXTENSION_BYTES).contains(size))
                .ok_or(BuildError::UnsafeArchive)?;
            extension_bytes = extension_bytes
                .checked_add(size)
                .filter(|bytes| *bytes <= MAXIMUM_AGGREGATE_PATH_BYTES)
                .ok_or(BuildError::InvalidArchive)?;
            let mut value = Vec::with_capacity(size);
            entry
                .read_to_end(&mut value)
                .map_err(|_| BuildError::InvalidArchive)?;
            if value.len() != size
                || value.last() != Some(&0)
                || value[..value.len() - 1].contains(&0)
                || std::str::from_utf8(&value[..value.len() - 1]).is_err()
            {
                return Err(BuildError::UnsafeArchive);
            }
            pending_long_name = true;
        } else if matches!(entry_type, EntryType::Regular | EntryType::Directory) {
            pending_long_name = false;
        } else {
            return Err(BuildError::UnsafeArchive);
        }
    }
    let file = archive.into_inner();
    let end_marker_position = file
        .stream_position()
        .map_err(|_| BuildError::InvalidArchive)?;
    if end_marker_position.checked_add(512) != Some(archive_length) {
        return Err(BuildError::InvalidArchive);
    }
    let mut terminal_block = [0_u8; 512];
    file.read_exact(&mut terminal_block)
        .map_err(|_| BuildError::InvalidArchive)?;
    if terminal_block.iter().any(|byte| *byte != 0) {
        return Err(BuildError::InvalidArchive);
    }
    if count == 0 || pending_long_name {
        Err(BuildError::InvalidArchive)
    } else {
        Ok(())
    }
}

fn validate_raw_extent<R: std::io::Read>(
    entry: &tar::Entry<'_, R>,
    size: u64,
    archive_length: u64,
) -> Result<(), BuildError> {
    let padded = size.checked_add(511).ok_or(BuildError::InvalidArchive)? / 512 * 512;
    entry
        .raw_file_position()
        .checked_add(padded)
        .filter(|end| *end <= archive_length)
        .ok_or(BuildError::InvalidArchive)?;
    Ok(())
}

fn extract_semantic_entries(
    file: &mut File,
    destination: &Path,
    expected_root: &str,
) -> Result<(), BuildError> {
    let mut archive = tar::Archive::new(file);
    let entries = archive.entries().map_err(|_| BuildError::InvalidArchive)?;
    let mut seen = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut previous = None::<Vec<u8>>;
    let mut total_bytes = 0_u64;
    let mut total_path_bytes = 0_usize;
    let mut count = 0_usize;
    for entry in entries {
        let mut entry = entry.map_err(|_| BuildError::InvalidArchive)?;
        count = count
            .checked_add(1)
            .filter(|count| *count <= MAXIMUM_ARCHIVE_ENTRIES)
            .ok_or(BuildError::InvalidArchive)?;
        let path_bytes = entry.path_bytes().into_owned();
        total_path_bytes = total_path_bytes
            .checked_add(path_bytes.len())
            .filter(|bytes| *bytes <= MAXIMUM_AGGREGATE_PATH_BYTES)
            .ok_or(BuildError::InvalidArchive)?;
        let path = validate_effective_path(&path_bytes, expected_root)?;
        if previous
            .as_ref()
            .is_some_and(|previous| previous >= &path_bytes)
            || !seen.insert(path.clone())
        {
            return Err(BuildError::UnsafeArchive);
        }
        let entry_type = entry.header().entry_type();
        validate_canonical_header(entry.header(), entry_type)?;
        if count == 1 && (path_bytes != expected_root.as_bytes() || !entry_type.is_dir()) {
            return Err(BuildError::InvalidArchive);
        }
        previous = Some(path_bytes);
        if count != 1 {
            let parent = path.parent().ok_or(BuildError::UnsafeArchive)?;
            if !directories.contains(parent) {
                return Err(BuildError::UnsafeArchive);
            }
        }
        if entry_type.is_file() {
            total_bytes = total_bytes
                .checked_add(
                    entry
                        .header()
                        .size()
                        .map_err(|_| BuildError::InvalidArchive)?,
                )
                .filter(|bytes| *bytes <= MAXIMUM_ARCHIVE_BYTES)
                .ok_or(BuildError::InvalidArchive)?;
        } else if entry_type.is_dir() {
            directories.insert(path.clone());
        } else {
            return Err(BuildError::UnsafeArchive);
        }
        entry
            .unpack_in(destination)
            .map_err(|_| BuildError::UnsafeArchive)?;
    }
    if count == 0 || !directories.contains(Path::new(expected_root)) {
        Err(BuildError::InvalidArchive)
    } else {
        Ok(())
    }
}

fn validate_canonical_header(
    header: &tar::Header,
    entry_type: EntryType,
) -> Result<(), BuildError> {
    let size = header.size().map_err(|_| BuildError::InvalidArchive)?;
    let mode = header.mode().map_err(|_| BuildError::InvalidArchive)?;
    if header.as_gnu().is_none()
        || header.username_bytes() != Some(b"".as_slice())
        || header.groupname_bytes() != Some(b"".as_slice())
        || header.uid().map_err(|_| BuildError::InvalidArchive)? != 0
        || header.gid().map_err(|_| BuildError::InvalidArchive)? != 0
        || header.mtime().map_err(|_| BuildError::InvalidArchive)? != SOURCE_DATE_EPOCH
        || (entry_type.is_dir() && (size != 0 || mode != 0o755))
        || (entry_type.is_file() && !matches!(mode, 0o644 | 0o755))
    {
        Err(BuildError::UnsafeArchive)
    } else {
        Ok(())
    }
}

fn validate_root(root: &str) -> Result<(), BuildError> {
    if root.is_empty()
        || root.len() > 64
        || !root
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || root.starts_with('-')
        || root.ends_with('-')
    {
        Err(BuildError::InvalidArguments)
    } else {
        Ok(())
    }
}

fn validate_path_bytes(path: &[u8]) -> Result<(), BuildError> {
    if path.is_empty()
        || path.len() > MAXIMUM_PATH_BYTES
        || path.contains(&0)
        || std::str::from_utf8(path).is_err()
    {
        Err(BuildError::UnsafeArchive)
    } else {
        Ok(())
    }
}

fn validate_effective_path(path: &[u8], expected_root: &str) -> Result<PathBuf, BuildError> {
    validate_path_bytes(path)?;
    let path = std::str::from_utf8(path).map_err(|_| BuildError::UnsafeArchive)?;
    if path.ends_with('/') || path.contains('\\') || path.chars().any(char::is_control) {
        return Err(BuildError::UnsafeArchive);
    }
    let path = PathBuf::from(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(BuildError::UnsafeArchive);
    }
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(root)) if root == expected_root => {}
        _ => return Err(BuildError::UnsafeArchive),
    }
    if components.any(|component| !matches!(component, Component::Normal(_))) {
        return Err(BuildError::UnsafeArchive);
    }
    Ok(path)
}

struct AcquiredArchive {
    file: File,
    identity: same_file::Handle,
    metadata: InputMetadata,
    digest: [u8; 32],
}

impl AcquiredArchive {
    fn open(path: &Path) -> Result<Self, BuildError> {
        let path_metadata = fs::symlink_metadata(path).map_err(|_| BuildError::InvalidInput)?;
        let metadata = InputMetadata::new(&path_metadata)?;
        if metadata.length == 0
            || metadata.length > MAXIMUM_ARCHIVE_FILE_BYTES
            || metadata.length % 512 != 0
        {
            return Err(BuildError::InvalidArchive);
        }
        let mut file = File::open(path).map_err(|_| BuildError::InvalidInput)?;
        let identity =
            same_file::Handle::from_file(file.try_clone().map_err(|_| BuildError::InvalidInput)?)
                .map_err(|_| BuildError::InvalidInput)?;
        validate_input_object(path, &file, &identity, &metadata)?;
        let digest = hash_file(&mut file, metadata.length)?;
        validate_input_object(path, &file, &identity, &metadata)?;
        file.rewind().map_err(|_| BuildError::InvalidArchive)?;
        Ok(Self {
            file,
            identity,
            metadata,
            digest,
        })
    }

    fn verify(&mut self, path: &Path) -> Result<(), BuildError> {
        validate_input_object(path, &self.file, &self.identity, &self.metadata)?;
        let digest = hash_file(&mut self.file, self.metadata.length)?;
        validate_input_object(path, &self.file, &self.identity, &self.metadata)?;
        if digest == self.digest {
            Ok(())
        } else {
            Err(BuildError::InvalidArchive)
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct InputMetadata {
    length: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    links: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
    #[cfg(windows)]
    attributes: u32,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
}

impl InputMetadata {
    fn new(metadata: &Metadata) -> Result<Self, BuildError> {
        if !metadata.is_file() {
            return Err(BuildError::InvalidInput);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if metadata.nlink() != 1 {
                return Err(BuildError::InvalidInput);
            }
            Ok(Self {
                length: metadata.len(),
                device: metadata.dev(),
                inode: metadata.ino(),
                links: metadata.nlink(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(BuildError::InvalidInput);
            }
            Ok(Self {
                length: metadata.file_size(),
                attributes: metadata.file_attributes(),
                creation_time: metadata.creation_time(),
                last_write_time: metadata.last_write_time(),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                length: metadata.len(),
            })
        }
    }
}

fn validate_input_object(
    path: &Path,
    file: &File,
    expected_identity: &same_file::Handle,
    expected_metadata: &InputMetadata,
) -> Result<(), BuildError> {
    let path_metadata = fs::symlink_metadata(path).map_err(|_| BuildError::InvalidInput)?;
    let handle_metadata = file.metadata().map_err(|_| BuildError::InvalidInput)?;
    let path_identity = same_file::Handle::from_path(path).map_err(|_| BuildError::InvalidInput)?;
    let handle_identity =
        same_file::Handle::from_file(file.try_clone().map_err(|_| BuildError::InvalidInput)?)
            .map_err(|_| BuildError::InvalidInput)?;
    if path_identity != *expected_identity
        || handle_identity != *expected_identity
        || InputMetadata::new(&path_metadata)? != *expected_metadata
        || InputMetadata::new(&handle_metadata)? != *expected_metadata
    {
        Err(BuildError::InvalidInput)
    } else {
        Ok(())
    }
}

fn hash_file(file: &mut File, expected_length: u64) -> Result<[u8; 32], BuildError> {
    file.rewind().map_err(|_| BuildError::InvalidArchive)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES].into_boxed_slice();
    let mut total = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| BuildError::InvalidArchive)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(count).expect("read count fits u64"))
            .filter(|total| *total <= expected_length)
            .ok_or(BuildError::InvalidArchive)?;
        hasher.update(&buffer[..count]);
    }
    if total != expected_length {
        return Err(BuildError::InvalidArchive);
    }
    Ok(hasher.finalize().into())
}

fn cleanup_created_directory(destination: &Path, expected: same_file::Handle) {
    let same = same_file::Handle::from_path(destination).is_ok_and(|actual| actual == expected);
    drop(expected);
    if same {
        let _ = fs::remove_dir_all(destination);
    }
}

#[cfg(test)]
#[path = "archive/tests.rs"]
mod tests;
