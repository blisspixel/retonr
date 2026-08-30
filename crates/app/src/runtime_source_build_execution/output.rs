use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
#[cfg(target_os = "linux")]
use std::{fs, io::Read as _};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::RuntimeSourceBuildOutputTree;
#[cfg(target_os = "linux")]
use rewrite_ollama_package::RuntimeSourceBuildOutputTreeEntry;
use rewrite_runtime_isolation::{ControlledBuildOutputTree, IsolationError};
#[cfg(target_os = "linux")]
use rewrite_runtime_isolation::{
    ControlledBuildOutputTreeEntry, MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES,
};
use rewrite_types::{CancellationToken, Digest};
#[cfg(target_os = "linux")]
use sha2::{Digest as _, Sha256};

#[cfg(target_os = "linux")]
use crate::artifact_storage::{ManagedTreeEntryKind, is_indirect};
use crate::artifact_storage::{
    ManagedTreeLimits, ManagedTreeSnapshot, MetadataFingerprint, PinnedDirectory,
    StableMetadataFingerprint,
};

#[cfg(target_os = "linux")]
use super::RuntimeSourceBuildOutputSource;
use super::{
    MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES, RuntimeSourceBuildExecutionError, ensure_active,
    map_completed_output, map_output,
};

#[cfg(target_os = "linux")]
const OUTPUT_HASH_BUFFER_BYTES: usize = 1024 * 1024;

pub(crate) struct PinnedRuntimeSourceBuildOutput {
    path: PathBuf,
    pub(super) root: PinnedDirectory,
    baseline: StableMetadataFingerprint,
    sealed: Option<SealedRuntimeSourceBuildOutput>,
}

struct SealedRuntimeSourceBuildOutput {
    snapshot: ManagedTreeSnapshot,
    files: BTreeMap<ArtifactSetRelativePath, SealedRuntimeSourceBuildFile>,
    tree: ControlledBuildOutputTree,
}

struct SealedRuntimeSourceBuildFile {
    file: std::fs::File,
    fingerprint: MetadataFingerprint,
    digest: Digest,
}

impl PinnedRuntimeSourceBuildOutput {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn validate_sealed(
        &self,
        limits: ManagedTreeLimits,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildExecutionError> {
        self.revalidate(cancellation)?;
        let sealed = self
            .sealed
            .as_ref()
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
        if sealed.snapshot.entries().len() > limits.maximum_entries() {
            return Err(RuntimeSourceBuildExecutionError::OutputTreeLimitExceeded);
        }
        let total_file_bytes = sealed
            .files
            .values()
            .try_fold(0_u64, |total, file| {
                total.checked_add(file.fingerprint.byte_size())
            })
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
        if usize::try_from(sealed.tree.entry_count()).ok() != Some(sealed.snapshot.entries().len())
            || usize::try_from(sealed.tree.regular_file_count()).ok() != Some(sealed.files.len())
            || sealed.tree.total_file_bytes() != total_file_bytes
            || sealed.tree.digest().as_str().len() != 64
        {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        let current = self
            .root
            .enumerate_tree(limits, cancellation)
            .map_err(map_completed_output)?;
        if current != sealed.snapshot {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        for retained in sealed.files.values() {
            let current = MetadataFingerprint::from_file(&retained.file)
                .map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
            if current != retained.fingerprint
                || !current.has_single_link()
                || retained.digest.as_str().len() != 64
            {
                return Err(RuntimeSourceBuildExecutionError::OutputChanged);
            }
        }
        self.revalidate(cancellation)?;
        Ok(())
    }

    pub(crate) fn sealed_snapshot(
        &self,
    ) -> Result<&ManagedTreeSnapshot, RuntimeSourceBuildExecutionError> {
        self.sealed
            .as_ref()
            .map(|sealed| &sealed.snapshot)
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn compile_portable_output_tree(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildOutputTree, RuntimeSourceBuildExecutionError> {
        let limits = ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES)
            .map_err(map_output)?;
        self.validate_sealed(limits, cancellation)?;
        let sealed = self
            .sealed
            .as_ref()
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
        let entries = sealed
            .snapshot
            .entries()
            .iter()
            .map(|entry| match entry.kind() {
                ManagedTreeEntryKind::Directory => RuntimeSourceBuildOutputTreeEntry::directory(
                    entry.relative_path().clone(),
                    entry.fingerprint().unix_mode(),
                )
                .map_err(|_| RuntimeSourceBuildExecutionError::OutputChanged),
                ManagedTreeEntryKind::RegularFile => {
                    let retained = sealed
                        .files
                        .get(entry.relative_path())
                        .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
                    RuntimeSourceBuildOutputTreeEntry::regular_file(
                        entry.relative_path().clone(),
                        entry.fingerprint().unix_mode(),
                        entry.byte_size(),
                        retained.digest.clone(),
                    )
                    .map_err(|_| RuntimeSourceBuildExecutionError::OutputChanged)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if compile_output_tree(&sealed.snapshot, &sealed.files)? != sealed.tree {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        let portable = RuntimeSourceBuildOutputTree::compile(entries)
            .map_err(|_| RuntimeSourceBuildExecutionError::OutputChanged)?;
        self.validate_sealed(limits, cancellation)?;
        Ok(portable)
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn compile_portable_output_tree(
        &self,
        _cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildOutputTree, RuntimeSourceBuildExecutionError> {
        let _ = self;
        Err(IsolationError::UnsupportedPlatform.into())
    }

    pub(crate) fn open_regular_file(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<std::fs::File, RuntimeSourceBuildExecutionError> {
        self.open_committed_regular_file(path, cancellation)
            .map(|(file, _byte_size, _digest)| file)
    }

    pub(crate) fn open_committed_regular_file(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<(std::fs::File, u64, Digest), RuntimeSourceBuildExecutionError> {
        let limits = ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES)
            .map_err(map_output)?;
        self.validate_sealed(limits, cancellation)?;
        let retained = self
            .sealed
            .as_ref()
            .and_then(|sealed| sealed.files.get(path))
            .ok_or(RuntimeSourceBuildExecutionError::UnsafeOutput)?;
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd as _;

            let opened =
                std::fs::File::open(format!("/proc/self/fd/{}", retained.file.as_raw_fd()))
                    .map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
            let fingerprint = MetadataFingerprint::from_file(&opened)
                .map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
            if fingerprint != retained.fingerprint || !fingerprint.has_single_link() {
                return Err(RuntimeSourceBuildExecutionError::OutputChanged);
            }
            if digest_retained_output_file(&opened, fingerprint.byte_size(), cancellation)?
                != retained.digest
            {
                return Err(RuntimeSourceBuildExecutionError::OutputChanged);
            }
            self.validate_sealed(limits, cancellation)?;
            Ok((opened, fingerprint.byte_size(), retained.digest.clone()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = retained;
            Err(IsolationError::UnsupportedPlatform.into())
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn open(
        source: &RuntimeSourceBuildOutputSource,
        cancellation: &CancellationToken,
    ) -> Result<Self, RuntimeSourceBuildExecutionError> {
        ensure_active(cancellation)?;
        let metadata = fs::symlink_metadata(source.path())
            .map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
        if is_indirect(&metadata) {
            return Err(RuntimeSourceBuildExecutionError::UnsafeOutput);
        }
        let root = PinnedDirectory::open_existing(source.path()).map_err(map_output)?;
        let baseline = root.fingerprint().map_err(map_output)?.stable();
        let snapshot = root
            .enumerate_tree(ManagedTreeLimits::new(1).map_err(map_output)?, cancellation)
            .map_err(map_output)?;
        if !snapshot.entries().is_empty() {
            return Err(RuntimeSourceBuildExecutionError::OutputNotEmpty);
        }
        let pinned = Self {
            path: source.path().to_path_buf(),
            root,
            baseline,
            sealed: None,
        };
        pinned.revalidate(cancellation)?;
        Ok(pinned)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn clone_empty_root(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<std::fs::File, RuntimeSourceBuildExecutionError> {
        ensure_active(cancellation)?;
        if self.sealed.is_some() {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        self.revalidate(cancellation)?;
        self.root.clone_handle().map_err(map_output)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn seal(
        &mut self,
        expected: &ControlledBuildOutputTree,
        cancellation: &CancellationToken,
    ) -> Result<ControlledBuildOutputTree, RuntimeSourceBuildExecutionError> {
        ensure_active(cancellation)?;
        if self.sealed.is_some() {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        let limits = ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES)
            .map_err(map_output)?;
        self.revalidate(cancellation)?;
        let snapshot = self
            .root
            .enumerate_tree(limits, cancellation)
            .map_err(map_completed_output)?;
        validate_shape_before_hashing(&snapshot, expected)?;
        let mut files = BTreeMap::new();
        for entry in snapshot
            .entries()
            .iter()
            .filter(|entry| entry.kind() == ManagedTreeEntryKind::RegularFile)
        {
            if !entry.has_single_link() {
                return Err(RuntimeSourceBuildExecutionError::UnsafeOutput);
            }
            let opened = self
                .root
                .open_relative_regular_file(entry.relative_path())
                .map_err(map_output)?;
            let digest =
                digest_retained_output_file(&opened.file, entry.byte_size(), cancellation)?;
            if opened.fingerprint != *entry.fingerprint()
                || !opened.fingerprint.has_single_link()
                || files
                    .insert(
                        entry.relative_path().clone(),
                        SealedRuntimeSourceBuildFile {
                            file: opened.file,
                            fingerprint: opened.fingerprint,
                            digest,
                        },
                    )
                    .is_some()
            {
                return Err(RuntimeSourceBuildExecutionError::OutputChanged);
            }
        }
        let after = self
            .root
            .enumerate_tree(limits, cancellation)
            .map_err(map_completed_output)?;
        if snapshot != after || files.len() != regular_file_count(&snapshot) {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        let tree = compile_output_tree(&snapshot, &files)?;
        if &tree != expected {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        self.revalidate(cancellation)?;
        self.sealed = Some(SealedRuntimeSourceBuildOutput {
            snapshot,
            files,
            tree,
        });
        self.validate_sealed(limits, cancellation)?;
        self.sealed
            .as_ref()
            .map(|sealed| sealed.tree.clone())
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)
    }

    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildExecutionError> {
        ensure_active(cancellation)?;
        let held = self.root.fingerprint().map_err(map_output)?;
        let selected = PinnedDirectory::fingerprint_path(&self.path).map_err(map_output)?;
        if self.baseline.same_identity(&held) && self.baseline.same_identity(&selected) {
            Ok(())
        } else {
            Err(RuntimeSourceBuildExecutionError::OutputChanged)
        }
    }
}

#[cfg(target_os = "linux")]
fn validate_shape_before_hashing(
    snapshot: &ManagedTreeSnapshot,
    expected: &ControlledBuildOutputTree,
) -> Result<(), RuntimeSourceBuildExecutionError> {
    if snapshot
        .entries()
        .iter()
        .any(|entry| entry.fingerprint().has_special_unix_mode_bits())
    {
        return Err(RuntimeSourceBuildExecutionError::UnsafeOutput);
    }
    let entry_count = u32::try_from(snapshot.entries().len())
        .map_err(|_| RuntimeSourceBuildExecutionError::OutputTreeLimitExceeded)?;
    let (regular_file_count, total_file_bytes) = snapshot
        .entries()
        .iter()
        .filter(|entry| entry.kind() == ManagedTreeEntryKind::RegularFile)
        .try_fold((0_u32, 0_u64), |(count, bytes), entry| {
            let count = count
                .checked_add(1)
                .ok_or(RuntimeSourceBuildExecutionError::OutputTreeLimitExceeded)?;
            let bytes = bytes
                .checked_add(entry.byte_size())
                .filter(|bytes| *bytes <= MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES)
                .ok_or(RuntimeSourceBuildExecutionError::OutputByteLimitExceeded)?;
            Ok::<(u32, u64), RuntimeSourceBuildExecutionError>((count, bytes))
        })?;
    if entry_count == expected.entry_count()
        && regular_file_count == expected.regular_file_count()
        && total_file_bytes == expected.total_file_bytes()
    {
        Ok(())
    } else {
        Err(RuntimeSourceBuildExecutionError::OutputChanged)
    }
}

#[cfg(target_os = "linux")]
fn compile_output_tree(
    snapshot: &ManagedTreeSnapshot,
    files: &BTreeMap<ArtifactSetRelativePath, SealedRuntimeSourceBuildFile>,
) -> Result<ControlledBuildOutputTree, RuntimeSourceBuildExecutionError> {
    let entries = snapshot
        .entries()
        .iter()
        .map(|entry| match entry.kind() {
            ManagedTreeEntryKind::Directory => ControlledBuildOutputTreeEntry::directory(
                entry.relative_path().as_str(),
                entry.fingerprint().unix_mode(),
            ),
            ManagedTreeEntryKind::RegularFile => {
                let retained = files
                    .get(entry.relative_path())
                    .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
                ControlledBuildOutputTreeEntry::regular_file(
                    entry.relative_path().as_str(),
                    entry.byte_size(),
                    retained.digest.clone(),
                    entry.fingerprint().unix_mode(),
                )
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    ControlledBuildOutputTree::compile(entries).map_err(RuntimeSourceBuildExecutionError::Isolation)
}

#[cfg(target_os = "linux")]
fn digest_retained_output_file(
    retained: &std::fs::File,
    byte_size: u64,
    cancellation: &CancellationToken,
) -> Result<Digest, RuntimeSourceBuildExecutionError> {
    use std::os::fd::AsRawFd as _;

    let mut file = std::fs::File::open(format!("/proc/self/fd/{}", retained.as_raw_fd()))
        .map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
    let mut observed = 0_u64;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; OUTPUT_HASH_BUFFER_BYTES];
    while observed < byte_size {
        ensure_active(cancellation)?;
        let maximum = usize::try_from(byte_size - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = file
            .read(&mut buffer[..maximum])
            .map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
        if read == 0 {
            return Err(RuntimeSourceBuildExecutionError::OutputChanged);
        }
        hasher.update(&buffer[..read]);
        observed = observed
            .checked_add(
                u64::try_from(read).map_err(|_| RuntimeSourceBuildExecutionError::OutputChanged)?,
            )
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(RuntimeSourceBuildExecutionError::OutputIo)?
        != 0
    {
        return Err(RuntimeSourceBuildExecutionError::OutputChanged);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeSourceBuildExecutionError::OutputChanged)
}

#[cfg(target_os = "linux")]
fn regular_file_count(snapshot: &ManagedTreeSnapshot) -> usize {
    snapshot
        .entries()
        .iter()
        .filter(|entry| entry.kind() == ManagedTreeEntryKind::RegularFile)
        .count()
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
