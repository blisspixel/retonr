use std::io;

use super::ArtifactInventoryError;

pub(super) fn map_initial_error(error: io::Error) -> ArtifactInventoryError {
    if error.kind() == io::ErrorKind::NotFound {
        ArtifactInventoryError::StorageNotInitialized
    } else if error.kind() == io::ErrorKind::NotADirectory || is_link_error(&error) {
        ArtifactInventoryError::UnsafeStorageLayout
    } else {
        ArtifactInventoryError::StorageIo(error)
    }
}

pub(super) fn map_active_error(error: io::Error) -> ArtifactInventoryError {
    if error.kind() == io::ErrorKind::NotFound
        || error.kind() == io::ErrorKind::NotADirectory
        || is_link_error(&error)
        || is_sharing_violation(&error)
    {
        ArtifactInventoryError::ConcurrentModification
    } else {
        ArtifactInventoryError::StorageIo(error)
    }
}

fn is_link_error(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error())
    }
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(4390)
    }
}

fn is_sharing_violation(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        let _ = error;
        false
    }
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(32)
    }
}
