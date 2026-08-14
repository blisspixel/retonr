use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
};

use rewrite_types::CancellationToken;

use super::{
    ArtifactInventoryError, FileShare, MetadataFingerprint, PinnedDirectory, map_active_error,
    map_initial_error, validate_directory_handle, validate_regular_file,
};
mod platform;
mod sync;
use platform::{
    create_directory, hard_link, inspect_entry, open_or_create_file, remove_file,
    remove_verified_file,
};
pub(super) use platform::{create_directory_exclusive, create_new_file, random_staging_name};
use sync::sync_directory;

#[cfg(unix)]
pub(crate) fn set_private_directory_permissions(
    directory: &PinnedDirectory,
) -> Result<(), ArtifactInventoryError> {
    platform::set_private_permissions(&directory.handle)
}

const TEMP_NAME_ATTEMPTS: usize = 1_024;

pub(crate) struct ManagedFile {
    pub(crate) file: File,
    pub(crate) fingerprint: MetadataFingerprint,
    pub(crate) byte_size: u64,
}

struct EntryStatus {
    direct_regular_file: bool,
    indirect: bool,
    byte_size: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExactEntryCapacity {
    Present,
    Available,
    Full,
}

impl PinnedDirectory {
    pub(crate) fn create_child_directory_exclusive(
        &self,
        name: &OsStr,
    ) -> Result<Self, ArtifactInventoryError> {
        let directory = create_directory_exclusive(&self.handle, name)?;
        validate_directory_handle(&directory, true)?;
        Ok(Self { handle: directory })
    }

    pub(crate) fn is_empty(&self) -> Result<bool, ArtifactInventoryError> {
        match self.raw_entries(1, &CancellationToken::new()) {
            Ok(entries) => Ok(entries.is_empty()),
            Err(ArtifactInventoryError::StorageEntryLimitExceeded) => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn has_direct_regular_child(
        &self,
        name: &OsStr,
    ) -> Result<bool, ArtifactInventoryError> {
        Ok(inspect_entry(&self.handle, name)?
            .is_some_and(|entry| entry.direct_regular_file && !entry.indirect))
    }

    pub(crate) fn ensure_child_directory(
        &self,
        name: &OsStr,
    ) -> Result<Self, ArtifactInventoryError> {
        create_directory(&self.handle, name)?;
        let directory = self.open_directory(name).map_err(map_initial_error)?;
        validate_directory_handle(&directory, true)?;
        #[cfg(unix)]
        platform::set_private_permissions(&directory)?;
        Ok(Self { handle: directory })
    }

    pub(crate) fn open_or_create_lock_file(
        &self,
        name: &OsStr,
    ) -> Result<(File, MetadataFingerprint), ArtifactInventoryError> {
        if let Some(entry) = inspect_entry(&self.handle, name)?
            && (entry.indirect || !entry.direct_regular_file)
        {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        let file = open_or_create_file(&self.handle, name).map_err(map_initial_error)?;
        validate_regular_file(&file, true)?;
        let fingerprint =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        Ok((file, fingerprint))
    }

    pub(crate) fn create_staging_file(
        &self,
        prefix: &str,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<(OsString, File), ArtifactInventoryError> {
        let entries = self.raw_entries(maximum_entries, cancellation)?;
        if entries.len() == maximum_entries {
            return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
        }
        self.create_staging_file_with(|| random_staging_name(prefix))
    }

    pub(crate) fn create_staging_file_with(
        &self,
        mut next_name: impl FnMut() -> Result<OsString, ArtifactInventoryError>,
    ) -> Result<(OsString, File), ArtifactInventoryError> {
        for _ in 0..TEMP_NAME_ATTEMPTS {
            let name = next_name()?;
            match create_new_file(&self.handle, &name) {
                Ok(file) => return Ok((name, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(ArtifactInventoryError::StorageIo(error)),
            }
        }
        Err(ArtifactInventoryError::StorageIo(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not reserve a unique staging file",
        )))
    }

    pub(crate) fn open_managed_file(
        &self,
        name: &OsStr,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        self.open_managed_file_with_share(
            name,
            FileShare::Verification,
            maximum_entries,
            cancellation,
        )
    }

    pub(crate) fn open_managed_file_for_sync(
        &self,
        name: &OsStr,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        self.open_managed_file_with_share(name, FileShare::Sync, maximum_entries, cancellation)
    }

    pub(crate) fn open_managed_file_for_removal(
        &self,
        name: &OsStr,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        self.open_managed_file_with_share(name, FileShare::Removal, maximum_entries, cancellation)
    }

    fn open_managed_file_for_cleanup(
        &self,
        name: &OsStr,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        self.open_managed_file_by_name(name, FileShare::Lifecycle)
    }

    fn open_managed_file_with_share(
        &self,
        name: &OsStr,
        share: FileShare,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        let (has_exact_name, at_capacity) =
            self.has_exact_entry_name(name, maximum_entries, cancellation)?;
        if !has_exact_name {
            if at_capacity {
                return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
            }
            if inspect_entry(&self.handle, name)?.is_some() {
                return Err(ArtifactInventoryError::ConcurrentModification);
            }
            return Ok(None);
        }
        let Some(entry) = inspect_entry(&self.handle, name)? else {
            return Err(ArtifactInventoryError::ConcurrentModification);
        };
        self.open_managed_file_by_name_with_entry(name, share, &entry)
    }

    fn open_managed_file_by_name(
        &self,
        name: &OsStr,
        share: FileShare,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        let Some(entry) = inspect_entry(&self.handle, name)? else {
            return Ok(None);
        };
        self.open_managed_file_by_name_with_entry(name, share, &entry)
    }

    fn open_managed_file_by_name_with_entry(
        &self,
        name: &OsStr,
        share: FileShare,
        entry: &EntryStatus,
    ) -> Result<Option<ManagedFile>, ArtifactInventoryError> {
        if entry.indirect || !entry.direct_regular_file {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        let file = match self.open_file(name, share) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) if is_unsafe_entry_error(&error) => {
                return Err(ArtifactInventoryError::UnsafeStorageLayout);
            }
            Err(error) => return Err(ArtifactInventoryError::StorageIo(error)),
        };
        validate_regular_file(&file, true)?;
        let fingerprint =
            MetadataFingerprint::from_file(&file).map_err(ArtifactInventoryError::StorageIo)?;
        let byte_size = fingerprint.length;
        if byte_size != entry.byte_size {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(Some(ManagedFile {
            file,
            fingerprint,
            byte_size,
        }))
    }

    pub(crate) fn recheck_managed_file_for_lifecycle(
        &self,
        name: &OsStr,
        expected: &MetadataFingerprint,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<(), ArtifactInventoryError> {
        self.recheck_managed_file_with_share(
            name,
            expected,
            FileShare::Lifecycle,
            maximum_entries,
            cancellation,
        )
    }

    pub(crate) fn recheck_managed_file_for_removal(
        &self,
        name: &OsStr,
        expected: &MetadataFingerprint,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<(), ArtifactInventoryError> {
        self.recheck_managed_file_with_share(
            name,
            expected,
            FileShare::Removal,
            maximum_entries,
            cancellation,
        )
    }

    pub(crate) fn remove_held_managed_file(
        &self,
        name: &OsStr,
        held: File,
        expected: MetadataFingerprint,
        maximum_entries: usize,
    ) -> Result<(), ArtifactInventoryError> {
        let current =
            MetadataFingerprint::from_file(&held).map_err(ArtifactInventoryError::StorageIo)?;
        if current != expected || !current.has_single_link() {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let reopened = self
            .open_managed_file_with_share(
                name,
                FileShare::Removal,
                maximum_entries,
                &CancellationToken::new(),
            )?
            .ok_or(ArtifactInventoryError::ConcurrentModification)?;
        if reopened.fingerprint != expected {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        #[cfg(windows)]
        drop(current);
        drop(reopened);
        #[cfg(windows)]
        drop(expected);
        remove_verified_file(self, name, held)?;
        if self
            .open_managed_file_by_name(name, FileShare::Removal)?
            .is_some()
        {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(())
    }

    pub(crate) fn confirm_managed_file_absent(
        &self,
        name: &OsStr,
        maximum_entries: usize,
    ) -> Result<(), ArtifactInventoryError> {
        let (has_exact_name, _) =
            self.has_exact_entry_name(name, maximum_entries, &CancellationToken::new())?;
        if has_exact_name || inspect_entry(&self.handle, name)?.is_some() {
            Err(ArtifactInventoryError::ConcurrentModification)
        } else {
            Ok(())
        }
    }

    fn recheck_managed_file_with_share(
        &self,
        name: &OsStr,
        expected: &MetadataFingerprint,
        share: FileShare,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<(), ArtifactInventoryError> {
        let current = self
            .open_managed_file_with_share(name, share, maximum_entries, cancellation)?
            .ok_or(ArtifactInventoryError::ConcurrentModification)?;
        if &current.fingerprint == expected {
            Ok(())
        } else {
            Err(ArtifactInventoryError::ConcurrentModification)
        }
    }

    fn has_exact_entry_name(
        &self,
        expected: &OsStr,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<(bool, bool), ArtifactInventoryError> {
        let entries = self.raw_entries(maximum_entries, cancellation)?;
        let at_capacity = entries.len() == maximum_entries;
        Ok((
            entries.into_iter().any(|entry| entry.name == expected),
            at_capacity,
        ))
    }

    pub(crate) fn exact_entry_capacity(
        &self,
        expected: &OsStr,
        maximum_entries: usize,
        cancellation: &CancellationToken,
    ) -> Result<ExactEntryCapacity, ArtifactInventoryError> {
        let (present, full) = self.has_exact_entry_name(expected, maximum_entries, cancellation)?;
        Ok(if present {
            ExactEntryCapacity::Present
        } else if full {
            ExactEntryCapacity::Full
        } else {
            ExactEntryCapacity::Available
        })
    }

    pub(crate) fn hard_link_to(
        &self,
        source_name: &OsStr,
        destination: &Self,
        destination_name: &OsStr,
    ) -> io::Result<()> {
        hard_link(
            &self.handle,
            source_name,
            &destination.handle,
            destination_name,
        )
    }

    pub(crate) fn remove_file(&self, name: &OsStr) -> Result<(), ArtifactInventoryError> {
        remove_file(&self.handle, name).map_err(map_active_error)
    }

    pub(crate) fn remove_file_if_same_identity(
        &self,
        name: &OsStr,
        held: &File,
    ) -> Result<(), ArtifactInventoryError> {
        let expected =
            MetadataFingerprint::from_file(held).map_err(ArtifactInventoryError::StorageIo)?;
        let current = self
            .open_managed_file_for_cleanup(name)?
            .ok_or(ArtifactInventoryError::ConcurrentModification)?;
        let same_identity = current.fingerprint.same_identity(&expected);
        drop(current);
        #[cfg(windows)]
        drop(expected);
        if !same_identity {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        self.remove_file(name)
    }

    pub(crate) fn sync(&self) -> Result<(), ArtifactInventoryError> {
        sync_directory(&self.handle).map_err(ArtifactInventoryError::StorageIo)
    }

    pub(crate) fn recover_owned_staging(
        &self,
        prefix: &str,
        maximum_entries: usize,
    ) -> Result<(), ArtifactInventoryError> {
        let query_limit = maximum_entries
            .checked_add(1)
            .ok_or(ArtifactInventoryError::StorageEntryLimitExceeded)?;
        let entries = self.raw_entries(query_limit, &CancellationToken::new())?;
        if entries.len() > maximum_entries {
            return Err(ArtifactInventoryError::StorageEntryLimitExceeded);
        }
        let reserved = entries
            .into_iter()
            .filter(|entry| entry.name.to_string_lossy().starts_with(prefix))
            .collect::<Vec<_>>();
        for entry in &reserved {
            if entry.indirect || !entry.direct_regular_file {
                return Err(ArtifactInventoryError::UnsafeStorageLayout);
            }
        }
        for entry in &reserved {
            self.remove_file(&entry.name)?;
        }
        if !reserved.is_empty() {
            self.sync()?;
        }
        Ok(())
    }
}

fn is_unsafe_entry_error(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::NotADirectory {
        return true;
    }
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
    }
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(4390)
    }
}
