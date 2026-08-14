use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read as _, Write as _},
};

use rewrite_model::ArtifactSetManifest;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::artifact_storage::{
    ManagedTreeEntryKind, ManagedTreeLimits, ManagedTreeSnapshot, OwnedStagingTree,
    PinnedDirectory, fingerprint_std_file,
};

use super::{
    ArtifactSetImportError, ArtifactSetImportProgress, ArtifactSetImportStage, ValidatedSetPlan,
    boundary::{map_managed_tree, map_source_tree},
    report_progress,
};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

pub(super) fn copy_and_verify_source<F>(
    source: &PinnedDirectory,
    manifest: &ArtifactSetManifest,
    plan: &ValidatedSetPlan,
    limits: ManagedTreeLimits,
    staging: Option<&OwnedStagingTree>,
    cancellation: &CancellationToken,
    progress: &mut F,
) -> Result<(), ArtifactSetImportError>
where
    F: FnMut(ArtifactSetImportProgress),
{
    let before = source
        .enumerate_tree(limits, cancellation)
        .map_err(map_source_tree)?;
    validate_tree_shape(&before, manifest, plan, false, true)?;
    let stage = if staging.is_some() {
        ArtifactSetImportStage::StagingAndVerifying
    } else {
        ArtifactSetImportStage::VerifyingSource
    };
    report_progress(progress, stage, 0, 0, manifest);

    let mut completed_bytes = 0u64;
    for (index, member) in manifest.members().iter().enumerate() {
        ensure_not_cancelled(cancellation)?;
        let mut opened = source
            .open_relative_regular_file(member.relative_path())
            .map_err(map_source_tree)?;
        if opened.byte_size != member.byte_size() {
            return Err(ArtifactSetImportError::SizeMismatch);
        }
        let mut destination = match staging {
            Some(staging) => Some(
                staging
                    .create_file(member.relative_path())
                    .map_err(map_managed_tree)?,
            ),
            None => None,
        };
        if destination
            .as_ref()
            .is_some_and(|file| !file.fingerprint.has_single_link())
        {
            return Err(ArtifactSetImportError::StorageChanged);
        }
        let observed = hash_exact_member(
            &mut opened.file,
            destination.as_mut().map(|value| &mut value.file),
            member.byte_size(),
            cancellation,
            ReadContext::Source,
        )?;
        if &observed != member.artifact_id().digest() {
            return Err(ArtifactSetImportError::DigestMismatch);
        }
        if fingerprint_std_file(&opened.file).map_err(map_source_tree)? != opened.fingerprint {
            return Err(ArtifactSetImportError::StorageChanged);
        }
        source
            .recheck_relative_regular_file(member.relative_path(), &opened.fingerprint)
            .map_err(map_source_tree)?;
        completed_bytes = completed_bytes
            .checked_add(member.byte_size())
            .ok_or(ArtifactSetImportError::StorageChanged)?;
        report_progress(progress, stage, index + 1, completed_bytes, manifest);
    }
    let after = source
        .enumerate_tree(limits, cancellation)
        .map_err(map_source_tree)?;
    if before != after {
        return Err(ArtifactSetImportError::StorageChanged);
    }
    Ok(())
}

pub(super) fn verify_final_tree(
    root: &PinnedDirectory,
    manifest: &ArtifactSetManifest,
    plan: &ValidatedSetPlan,
    limits: ManagedTreeLimits,
    cancellation: &CancellationToken,
) -> Result<(), ArtifactSetImportError> {
    let before = root
        .enumerate_tree(limits, cancellation)
        .map_err(map_managed_tree)?;
    validate_tree_shape(&before, manifest, plan, true, false)?;
    for member in manifest.members() {
        ensure_not_cancelled(cancellation)?;
        let mut opened = root
            .open_relative_regular_file(member.relative_path())
            .map_err(map_managed_tree)?;
        if opened.byte_size != member.byte_size() || !opened.fingerprint.has_single_link() {
            return Err(ArtifactSetImportError::StorageConflict);
        }
        let observed = hash_exact_member(
            &mut opened.file,
            None,
            member.byte_size(),
            cancellation,
            ReadContext::Managed,
        )?;
        if &observed != member.artifact_id().digest() {
            return Err(ArtifactSetImportError::StorageConflict);
        }
        if fingerprint_std_file(&opened.file).map_err(map_managed_tree)? != opened.fingerprint {
            return Err(ArtifactSetImportError::StorageChanged);
        }
        root.recheck_relative_regular_file(member.relative_path(), &opened.fingerprint)
            .map_err(map_managed_tree)?;
    }
    let after = root
        .enumerate_tree(limits, cancellation)
        .map_err(map_managed_tree)?;
    if before == after {
        Ok(())
    } else {
        Err(ArtifactSetImportError::StorageChanged)
    }
}

pub(super) fn validate_staged_snapshot(
    snapshot: &ManagedTreeSnapshot,
    manifest: &ArtifactSetManifest,
    plan: &ValidatedSetPlan,
) -> Result<(), ArtifactSetImportError> {
    validate_tree_shape(snapshot, manifest, plan, true, false)
}

fn validate_tree_shape(
    snapshot: &ManagedTreeSnapshot,
    manifest: &ArtifactSetManifest,
    plan: &ValidatedSetPlan,
    require_single_link: bool,
    source: bool,
) -> Result<(), ArtifactSetImportError> {
    let mut expected = BTreeMap::new();
    for directory in &plan.directories {
        expected.insert(directory.as_str(), (ManagedTreeEntryKind::Directory, 0));
    }
    for member in manifest.members() {
        expected.insert(
            member.relative_path().as_str(),
            (ManagedTreeEntryKind::RegularFile, member.byte_size()),
        );
    }
    let matches = snapshot.entries().len() == plan.tree_entries
        && snapshot.entries().len() == expected.len()
        && snapshot.entries().iter().all(|entry| {
            expected
                .get(entry.relative_path().as_str())
                .is_some_and(|(kind, size)| {
                    entry.kind() == *kind
                        && entry.byte_size() == *size
                        && (!require_single_link
                            || entry.kind() != ManagedTreeEntryKind::RegularFile
                            || entry.has_single_link())
                })
        });
    if matches {
        Ok(())
    } else if source {
        Err(ArtifactSetImportError::SourceTreeMismatch)
    } else {
        Err(ArtifactSetImportError::StorageConflict)
    }
}

fn hash_exact_member(
    source: &mut File,
    mut destination: Option<&mut File>,
    expected_size: u64,
    cancellation: &CancellationToken,
    context: ReadContext,
) -> Result<Digest, ArtifactSetImportError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut observed = 0u64;
    while observed < expected_size {
        ensure_not_cancelled(cancellation)?;
        let remaining = expected_size - observed;
        let maximum = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let count = source
            .read(&mut buffer[..maximum])
            .map_err(|error| context.io_error(error))?;
        if count == 0 {
            return Err(context.size_error());
        }
        if let Some(destination) = destination.as_deref_mut() {
            destination
                .write_all(&buffer[..count])
                .map_err(ArtifactSetImportError::StorageIo)?;
        }
        observed = observed
            .checked_add(u64::try_from(count).map_err(|_| context.size_error())?)
            .ok_or_else(|| context.size_error())?;
        hasher.update(&buffer[..count]);
    }
    if let Some(destination) = destination {
        let written = destination
            .metadata()
            .map_err(ArtifactSetImportError::StorageIo)?
            .len();
        if written != expected_size {
            return Err(ArtifactSetImportError::StorageChanged);
        }
    }
    let mut trailing = [0u8; 1];
    if source
        .read(&mut trailing)
        .map_err(|error| context.io_error(error))?
        != 0
    {
        return Err(context.size_error());
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| ArtifactSetImportError::DigestMismatch)
}

#[derive(Clone, Copy)]
enum ReadContext {
    Source,
    Managed,
}

impl ReadContext {
    fn io_error(self, error: std::io::Error) -> ArtifactSetImportError {
        match self {
            Self::Source => ArtifactSetImportError::SourceIo(error),
            Self::Managed => ArtifactSetImportError::StorageIo(error),
        }
    }

    const fn size_error(self) -> ArtifactSetImportError {
        match self {
            Self::Source => ArtifactSetImportError::SizeMismatch,
            Self::Managed => ArtifactSetImportError::StorageConflict,
        }
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ArtifactSetImportError> {
    if cancellation.is_cancelled() {
        Err(ArtifactSetImportError::Cancelled)
    } else {
        Ok(())
    }
}
