use std::{
    ffi::{OsStr, OsString},
    fs::{File, TryLockError},
    io::{self, Read as _},
    path::Path,
};

use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::artifact_inventory::ArtifactInventoryError;

mod entry;
mod errors;
mod existing;
mod fingerprint;
mod mutation;
#[cfg(test)]
pub(crate) mod test_support;
mod tree;
mod verification;
pub(crate) use entry::is_indirect;
use errors::{map_active_error, map_initial_error};
pub(crate) use existing::{ExistingArtifactStorage, LIFECYCLE_LOCK_FILE, LifecycleLockMode};
pub(crate) use fingerprint::{MetadataFingerprint, StableMetadataFingerprint};
#[cfg(unix)]
pub(crate) use mutation::set_private_directory_permissions;
pub(crate) use mutation::{ExactEntryCapacity, ManagedFile};
pub(crate) use tree::{
    ManagedTreeEntryKind, ManagedTreeLimits, ManagedTreeSnapshot, OwnedStagingTree,
    remove_verified_managed_tree,
};
pub(crate) use verification::{
    ExactArtifactExpectation, ExactArtifactSync, ExactArtifactVerificationError,
    VerifiedManagedArtifact, verify_exact_artifact, verify_exact_artifact_for_removal,
    verify_exact_artifact_for_runtime,
};

pub(crate) fn managed_storage_key(digest: &Digest) -> String {
    format!("artifacts/{}", digest.as_str())
}

#[cfg(test)]
mod mutation_tests;

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct DirectoryEntrySnapshot {
    pub(super) name: OsString,
    metadata: Option<MetadataFingerprint>,
    pub(super) direct_regular_file: bool,
    pub(super) indirect: bool,
    pub(super) byte_size: u64,
}

impl DirectoryEntrySnapshot {
    pub(crate) fn has_single_link(&self) -> bool {
        self.metadata
            .as_ref()
            .is_some_and(MetadataFingerprint::has_single_link)
    }
}

pub(crate) struct PinnedDirectory {
    handle: File,
}

impl PinnedDirectory {
    pub(crate) fn open_existing(path: &Path) -> Result<Self, ArtifactInventoryError> {
        let absolute = std::path::absolute(path).map_err(ArtifactInventoryError::StorageIo)?;
        let directory = open_directory_path(&absolute).map_err(map_initial_error)?;
        validate_directory_handle(&directory, true)?;
        Ok(Self { handle: directory })
    }

    pub(crate) fn fingerprint_path(
        path: &Path,
    ) -> Result<MetadataFingerprint, ArtifactInventoryError> {
        let directory = open_directory_path(path).map_err(map_active_error)?;
        validate_directory_handle(&directory, false)?;
        MetadataFingerprint::from_file(&directory).map_err(ArtifactInventoryError::StorageIo)
    }

    pub(crate) fn open_child_directory(
        &self,
        name: &OsStr,
    ) -> Result<Self, ArtifactInventoryError> {
        self.require_direct_directory_name(name)?;
        let directory = self.open_directory(name).map_err(map_initial_error)?;
        validate_directory_handle(&directory, true)?;
        Ok(Self { handle: directory })
    }

    pub(crate) fn open_lock_file(
        &self,
        name: &OsStr,
    ) -> Result<(File, MetadataFingerprint), ArtifactInventoryError> {
        let file = self
            .open_file(name, FileShare::Lifecycle)
            .map_err(map_initial_error)?;
        validate_regular_file(&file, true)?;
        let fingerprint =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        Ok((file, fingerprint))
    }

    pub(super) fn snapshot(
        &self,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<DirectoryEntrySnapshot>, ArtifactInventoryError> {
        let raw_entries = self.raw_entries(maximum_entries, cancellation)?;
        let mut entries = Vec::with_capacity(raw_entries.len());
        for raw in raw_entries {
            ensure_not_cancelled(cancellation)?;
            let metadata = if raw.direct_regular_file {
                let file = self
                    .open_file(&raw.name, FileShare::Verification)
                    .map_err(map_active_error)?;
                validate_regular_file(&file, false)?;
                let fingerprint = MetadataFingerprint::from_file(&file)
                    .map_err(ArtifactInventoryError::StorageIo)?;
                if fingerprint.length != raw.byte_size {
                    return Err(ArtifactInventoryError::ConcurrentModification);
                }
                Some(fingerprint)
            } else {
                None
            };
            entries.push(DirectoryEntrySnapshot {
                name: raw.name,
                metadata,
                direct_regular_file: raw.direct_regular_file,
                indirect: raw.indirect,
                byte_size: raw.byte_size,
            });
        }
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    pub(super) fn hash_entry(
        &self,
        entry: &DirectoryEntrySnapshot,
        cancellation: &CancellationToken,
    ) -> Result<Digest, ArtifactInventoryError> {
        let mut file = self
            .open_file(&entry.name, FileShare::Verification)
            .map_err(map_active_error)?;
        validate_regular_file(&file, false)?;
        let opened =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        if Some(&opened) != entry.metadata.as_ref() {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let observed = hash_exact_bytes(&mut file, entry.byte_size, cancellation)?;
        let final_handle =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        if final_handle != opened {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let final_entry = self
            .open_file(&entry.name, FileShare::Verification)
            .map_err(map_active_error)?;
        let final_entry = MetadataFingerprint::from_file(&final_entry)
            .map_err(ArtifactInventoryError::StorageIo)?;
        if final_entry != opened {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(observed)
    }

    pub(crate) fn fingerprint(&self) -> Result<MetadataFingerprint, ArtifactInventoryError> {
        validate_directory_handle(&self.handle, false)?;
        MetadataFingerprint::from_file(&self.handle).map_err(ArtifactInventoryError::StorageIo)
    }

    pub(crate) fn child_directory_fingerprint(
        &self,
        name: &OsStr,
    ) -> Result<MetadataFingerprint, ArtifactInventoryError> {
        let directory = self.open_directory(name).map_err(map_active_error)?;
        validate_directory_handle(&directory, false)?;
        MetadataFingerprint::from_file(&directory).map_err(ArtifactInventoryError::StorageIo)
    }

    pub(crate) fn child_file_fingerprint(
        &self,
        name: &OsStr,
    ) -> Result<MetadataFingerprint, ArtifactInventoryError> {
        let file = self
            .open_file(name, FileShare::Lifecycle)
            .map_err(map_active_error)?;
        validate_regular_file(&file, false)?;
        MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)
    }

    #[cfg(unix)]
    pub(super) fn open_directory(&self, name: &OsStr) -> io::Result<File> {
        use rustix::fs::{Mode, OFlags};

        rustix::fs::openat(
            &self.handle,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from)
    }

    #[cfg(windows)]
    pub(super) fn open_directory(&self, name: &OsStr) -> io::Result<File> {
        let mut options = fs_at::OpenOptions::default();
        options
            .read(true)
            .follow(false)
            .open_dir_at(&self.handle, Path::new(name))
    }

    #[cfg(unix)]
    pub(super) fn open_file(&self, name: &OsStr, share: FileShare) -> io::Result<File> {
        use rustix::fs::{Mode, OFlags};

        let access = if matches!(share, FileShare::Sync) {
            OFlags::RDWR
        } else {
            OFlags::RDONLY
        };
        rustix::fs::openat(
            &self.handle,
            name,
            access | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(io::Error::from)
    }

    #[cfg(windows)]
    pub(super) fn open_file(&self, name: &OsStr, share: FileShare) -> io::Result<File> {
        use cap_fs_ext::OpenOptionsFollowExt as _;
        use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};

        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_SHARE_WRITE: u32 = 0x0000_0002;
        const FILE_SHARE_DELETE: u32 = 0x0000_0004;
        let share_mode = match share {
            FileShare::Lifecycle => FILE_SHARE_READ | FILE_SHARE_WRITE,
            FileShare::Verification | FileShare::Sync => FILE_SHARE_READ,
            FileShare::Sealed | FileShare::Removal => {
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
            }
        };
        let mut options = OpenOptions::new();
        options.read(true).share_mode(share_mode);
        if matches!(share, FileShare::Removal) {
            use winx::file::AccessMode;

            options.access_mode((AccessMode::GENERIC_READ | AccessMode::DELETE).bits());
        }
        if matches!(share, FileShare::Sync) {
            options.write(true);
        }
        options.follow(FollowSymlinks::No);
        cap_primitives::fs::open(&self.handle, Path::new(name), &options)
    }

    #[cfg(unix)]
    pub(super) fn raw_entries(
        &self,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<RawDirectoryEntry>, ArtifactInventoryError> {
        use std::os::unix::ffi::OsStringExt as _;

        use rustix::fs::{AtFlags, Dir, FileType};

        let mut result = Vec::new();
        let mut directory = Dir::read_from(&self.handle)
            .map_err(io::Error::from)
            .map_err(map_active_error)?;
        for entry in &mut directory {
            ensure_not_cancelled(cancellation)?;
            let entry = entry.map_err(io::Error::from).map_err(map_active_error)?;
            let name_bytes = entry.file_name().to_bytes();
            if matches!(name_bytes, b"." | b"..") {
                continue;
            }
            if result.len() == maximum_entries {
                return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
            }
            let name = OsString::from_vec(name_bytes.to_vec());
            let stat = rustix::fs::statat(&self.handle, &name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(io::Error::from)
                .map_err(map_active_error)?;
            let file_type = FileType::from_raw_mode(stat.st_mode);
            result.push(RawDirectoryEntry {
                name,
                direct_regular_file: file_type == FileType::RegularFile,
                indirect: file_type == FileType::Symlink,
                byte_size: u64::try_from(stat.st_size)
                    .map_err(|_| ArtifactInventoryError::ConcurrentModification)?,
            });
        }
        Ok(result)
    }

    #[cfg(windows)]
    pub(super) fn raw_entries(
        &self,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Vec<RawDirectoryEntry>, ArtifactInventoryError> {
        use cap_primitives::fs::{FollowSymlinks, MetadataExt as _};

        let mut result = Vec::new();
        let entries = cap_primitives::fs::read_base_dir(&self.handle).map_err(map_active_error)?;
        for entry in entries {
            ensure_not_cancelled(cancellation)?;
            if result.len() == maximum_entries {
                return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
            }
            let entry = entry.map_err(map_active_error)?;
            let name = entry.file_name();
            let metadata =
                cap_primitives::fs::stat(&self.handle, Path::new(&name), FollowSymlinks::No)
                    .map_err(map_active_error)?;
            let indirect = windows_indirect(metadata.file_attributes());
            result.push(RawDirectoryEntry {
                name,
                direct_regular_file: metadata.is_file() && !indirect,
                indirect,
                byte_size: metadata.len(),
            });
        }
        Ok(result)
    }
}

#[cfg(windows)]
fn windows_indirect(file_attributes: u32) -> bool {
    file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

pub(super) struct RawDirectoryEntry {
    pub(super) name: OsString,
    pub(super) direct_regular_file: bool,
    pub(super) indirect: bool,
    pub(super) byte_size: u64,
}

#[derive(Clone, Copy)]
pub(super) enum FileShare {
    Lifecycle,
    Verification,
    Sealed,
    Sync,
    Removal,
}

pub(crate) fn fingerprint_std_file(
    file: &File,
) -> Result<MetadataFingerprint, ArtifactInventoryError> {
    MetadataFingerprint::from_file(file).map_err(ArtifactInventoryError::StorageIo)
}

pub(crate) fn lock_shared(file: &File) -> Result<(), ArtifactInventoryError> {
    match file.try_lock_shared() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(ArtifactInventoryError::StorageInUse),
        Err(TryLockError::Error(error)) => Err(ArtifactInventoryError::StorageIo(error)),
    }
}

#[cfg(unix)]
fn open_directory_path(path: &Path) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
fn open_directory_path(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

pub(super) fn validate_directory_handle(
    file: &File,
    opening: bool,
) -> Result<(), ArtifactInventoryError> {
    let metadata = file.metadata().map_err(ArtifactInventoryError::StorageIo)?;
    if metadata.is_dir() && !is_indirect(&metadata) {
        Ok(())
    } else if opening {
        Err(ArtifactInventoryError::UnsafeStorageLayout)
    } else {
        Err(ArtifactInventoryError::ConcurrentModification)
    }
}

pub(super) fn validate_regular_file(
    file: &File,
    opening: bool,
) -> Result<(), ArtifactInventoryError> {
    let metadata = file.metadata().map_err(ArtifactInventoryError::StorageIo)?;
    if metadata.is_file() && !is_indirect(&metadata) {
        Ok(())
    } else if opening {
        Err(ArtifactInventoryError::UnsafeStorageLayout)
    } else {
        Err(ArtifactInventoryError::ConcurrentModification)
    }
}

pub(crate) fn hash_exact_bytes(
    file: &mut File,
    expected_size: u64,
    cancellation: &CancellationToken,
) -> Result<Digest, ArtifactInventoryError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let mut observed = 0_u64;
    while observed < expected_size {
        ensure_not_cancelled(cancellation)?;
        let remaining = expected_size - observed;
        let maximum = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let count = file
            .read(&mut buffer[..maximum])
            .map_err(ArtifactInventoryError::StorageIo)?;
        if count == 0 {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        observed = observed
            .checked_add(
                u64::try_from(count).map_err(|_| ArtifactInventoryError::ConcurrentModification)?,
            )
            .ok_or(ArtifactInventoryError::ConcurrentModification)?;
        hasher.update(&buffer[..count]);
    }
    let mut trailing = [0u8; 1];
    if file
        .read(&mut trailing)
        .map_err(ArtifactInventoryError::StorageIo)?
        != 0
    {
        return Err(ArtifactInventoryError::ConcurrentModification);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| ArtifactInventoryError::ConcurrentModification)
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactInventoryError> {
    if cancellation.is_cancelled() {
        Err(ArtifactInventoryError::Cancelled)
    } else {
        Ok(())
    }
}
