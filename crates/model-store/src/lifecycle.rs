use std::fs::{File, TryLockError};

/// Live exclusive lock capability for artifact lifecycle mutations.
///
/// The capability is non-cloneable and retains the locked file handle for its
/// complete lifetime. Application code is responsible for opening the exact
/// pinned repository lifecycle-lock entry before acquiring this capability.
/// Durable removal transitions require a live reference to this type.
pub struct ExclusiveArtifactLifecycleLock {
    _file: File,
}

impl ExclusiveArtifactLifecycleLock {
    /// Attempts to acquire an exclusive lifecycle lock on an already opened file.
    ///
    /// # Errors
    ///
    /// Returns [`TryLockError::WouldBlock`] when another shared or exclusive holder
    /// owns the lock, or [`TryLockError::Error`] when the operating system cannot
    /// complete the lock operation.
    pub fn try_acquire(file: File) -> Result<Self, TryLockError> {
        file.try_lock()?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::TryLockError, io::Write as _};

    use super::ExclusiveArtifactLifecycleLock;

    #[test]
    fn capability_holds_the_exclusive_lock_until_drop() {
        let mut temporary = tempfile::NamedTempFile::new().expect("temporary lock file");
        temporary.write_all(b"lock").expect("initialize lock file");
        let contender = temporary.reopen().expect("open competing handle");
        let capability = ExclusiveArtifactLifecycleLock::try_acquire(
            temporary.reopen().expect("open capability handle"),
        )
        .expect("acquire exclusive capability");

        assert!(matches!(
            contender.try_lock_shared(),
            Err(TryLockError::WouldBlock)
        ));
        drop(capability);
        contender
            .try_lock_shared()
            .expect("lock becomes available after capability drop");
    }
}
