use std::{
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs::File,
    io,
};

#[cfg(windows)]
use std::path::Path;

use rewrite_types::CancellationToken;

#[cfg(windows)]
use super::windows_indirect;
use super::{
    ArtifactInventoryError, FileShare, MetadataFingerprint, PinnedDirectory, map_active_error,
    map_initial_error, validate_directory_handle, validate_regular_file,
};
mod sync;
use sync::sync_directory;

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
    pub(crate) fn ensure_child_directory(
        &self,
        name: &OsStr,
    ) -> Result<Self, ArtifactInventoryError> {
        create_directory(&self.handle, name)?;
        let directory = self.open_directory(name).map_err(map_initial_error)?;
        validate_directory_handle(&directory, true)?;
        #[cfg(unix)]
        set_private_permissions(&directory)?;
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

#[cfg(unix)]
fn inspect_entry(
    parent: &File,
    name: &OsStr,
) -> Result<Option<EntryStatus>, ArtifactInventoryError> {
    use rustix::fs::{AtFlags, FileType};

    let stat = match rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => {
            return Err(map_active_error(io::Error::from(error)));
        }
    };
    let file_type = FileType::from_raw_mode(stat.st_mode);
    Ok(Some(EntryStatus {
        direct_regular_file: file_type == FileType::RegularFile,
        indirect: file_type == FileType::Symlink,
        byte_size: u64::try_from(stat.st_size)
            .map_err(|_| ArtifactInventoryError::ConcurrentModification)?,
    }))
}

#[cfg(windows)]
fn inspect_entry(
    parent: &File,
    name: &OsStr,
) -> Result<Option<EntryStatus>, ArtifactInventoryError> {
    use cap_primitives::fs::{FollowSymlinks, MetadataExt as _};

    let metadata = match cap_primitives::fs::stat(parent, Path::new(name), FollowSymlinks::No) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(map_active_error(error)),
    };
    let indirect = windows_indirect(metadata.file_attributes());
    Ok(Some(EntryStatus {
        direct_regular_file: metadata.is_file() && !indirect,
        indirect,
        byte_size: metadata.len(),
    }))
}

fn random_staging_name(prefix: &str) -> Result<OsString, ArtifactInventoryError> {
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random).map_err(|_| {
        ArtifactInventoryError::StorageIo(io::Error::other(
            "could not generate a staging file name",
        ))
    })?;
    let mut name = String::with_capacity(prefix.len() + random.len() * 2);
    name.push_str(prefix);
    for byte in random {
        write!(&mut name, "{byte:02x}").map_err(|_| {
            ArtifactInventoryError::StorageIo(io::Error::other(
                "could not format a staging file name",
            ))
        })?;
    }
    Ok(OsString::from(name))
}

#[cfg(unix)]
fn create_directory(parent: &File, name: &OsStr) -> Result<(), ArtifactInventoryError> {
    use rustix::fs::Mode;

    match rustix::fs::mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
        Ok(()) => Ok(()),
        Err(rustix::io::Errno::EXIST) => Ok(()),
        Err(error) => Err(map_initial_error(io::Error::from(error))),
    }
}

#[cfg(windows)]
fn create_directory(parent: &File, name: &OsStr) -> Result<(), ArtifactInventoryError> {
    let options = cap_primitives::fs::DirOptions::new();
    match cap_primitives::fs::create_dir(parent, Path::new(name), &options) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(map_initial_error(error)),
    }
}

#[cfg(unix)]
fn set_private_permissions(directory: &File) -> Result<(), ArtifactInventoryError> {
    use rustix::fs::Mode;

    rustix::fs::fchmod(directory, Mode::RUSR | Mode::WUSR | Mode::XUSR)
        .map_err(io::Error::from)
        .map_err(ArtifactInventoryError::StorageIo)
}

#[cfg(unix)]
fn open_or_create_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
fn open_or_create_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use cap_fs_ext::OpenOptionsFollowExt as _;
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};

    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .share_mode(0x0000_0001 | 0x0000_0002)
        .follow(FollowSymlinks::No);
    cap_primitives::fs::open(parent, Path::new(name), &options)
}

#[cfg(unix)]
fn create_new_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(io::Error::from)
}

#[cfg(windows)]
fn create_new_file(parent: &File, name: &OsStr) -> io::Result<File> {
    use cap_fs_ext::OpenOptionsFollowExt as _;
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, OpenOptionsExt as _};

    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(0x0000_0001 | 0x0000_0002 | 0x0000_0004)
        .follow(FollowSymlinks::No);
    cap_primitives::fs::open(parent, Path::new(name), &options)
}

#[cfg(unix)]
fn hard_link(
    source_parent: &File,
    source_name: &OsStr,
    destination_parent: &File,
    destination_name: &OsStr,
) -> io::Result<()> {
    rustix::fs::linkat(
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        rustix::fs::AtFlags::empty(),
    )
    .map_err(io::Error::from)
}

#[cfg(windows)]
fn hard_link(
    source_parent: &File,
    source_name: &OsStr,
    destination_parent: &File,
    destination_name: &OsStr,
) -> io::Result<()> {
    cap_primitives::fs::hard_link(
        source_parent,
        Path::new(source_name),
        destination_parent,
        Path::new(destination_name),
    )
}

#[cfg(unix)]
fn remove_file(parent: &File, name: &OsStr) -> io::Result<()> {
    rustix::fs::unlinkat(parent, name, rustix::fs::AtFlags::empty()).map_err(io::Error::from)
}

#[cfg(windows)]
fn remove_file(parent: &File, name: &OsStr) -> io::Result<()> {
    cap_primitives::fs::remove_file(parent, Path::new(name))
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
