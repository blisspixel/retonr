use std::{io::Write as _, path::Path};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;

use super::{
    CompiledRuntimeAdmissionEvidenceAssembly, RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH,
    RuntimeAdmissionEvidenceAssemblyError, RuntimeAdmissionEvidenceBundleDestination,
    RuntimeAdmissionEvidenceBundleError, RuntimeAdmissionEvidenceBundleLease,
    RuntimeAdmissionEvidenceBundleLimits,
    assembly::AssemblySourceEvidence,
    verify::{acquire_retained, active, map_storage, verify_staged},
};
use crate::{
    RuntimeSourceBuildEvidenceBundleLease,
    artifact_storage::{
        ManagedTreeLimits, NoReplacePublicationFailure, OwnedStagingTree, PinnedDirectory,
        is_indirect,
    },
};

/// No-replace publisher for a first inert admission-evidence byte closure.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionEvidenceAssemblyPublisher;

impl RuntimeAdmissionEvidenceAssemblyPublisher {
    /// Revalidates the source-bound foundation, copies bounded immutable assembly
    /// bytes into an owned staging tree, verifies and synchronizes that tree, and
    /// publishes through an atomic no-replace rename with fresh retained readback.
    ///
    /// The complete published root remains inert. Opaque controls and policy
    /// material are never interpreted as passed, admitted, or generation authority.
    /// Final source verification uses a fresh uncancelled context. A failure after
    /// commit is explicitly reported and does not promote or remove the inert root.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionEvidenceAssemblyError`] for limits, overlapping
    /// source and destination, existing destination, cancellation, source drift,
    /// unsafe storage, publication, cleanup, or independent finalization failure.
    pub fn publish(
        assembly: &CompiledRuntimeAdmissionEvidenceAssembly,
        source: &RuntimeSourceBuildEvidenceBundleLease,
        destination: &RuntimeAdmissionEvidenceBundleDestination,
        limits: RuntimeAdmissionEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceAssemblyError> {
        publish_view(assembly, source, destination, limits, cancellation)
    }
}

pub(super) fn publish_view(
    assembly: &CompiledRuntimeAdmissionEvidenceAssembly,
    source: &impl AssemblySourceEvidence,
    destination: &RuntimeAdmissionEvidenceBundleDestination,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceAssemblyError> {
    limits
        .validate()
        .map_err(RuntimeAdmissionEvidenceBundleError::from)?;
    active(cancellation)?;
    super::RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
        assembly.plan.canonical_bytes(),
        limits,
    )
    .map_err(RuntimeAdmissionEvidenceBundleError::from)?;
    let operation = publish_inner(assembly, source, destination, limits, cancellation);
    let finalization = source.verify_foundation(&assembly.foundation, &CancellationToken::new());
    match (operation, finalization) {
        (Ok(lease), Ok(())) => {
            lease
                .revalidate(&CancellationToken::new())
                .map_err(|failure| {
                    RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization {
                        failure: Box::new(failure.into()),
                    }
                })?;
            active(cancellation).map_err(|failure| {
                RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization {
                    failure: Box::new(failure.into()),
                }
            })?;
            Ok(lease)
        }
        (Ok(_), Err(failure)) => Err(
            RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization {
                failure: Box::new(failure),
            },
        ),
        (Err(operation), Ok(())) => Err(operation),
        (Err(operation), Err(finalization)) => {
            let committed = matches!(
                operation,
                RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization { .. }
            );
            let failure = RuntimeAdmissionEvidenceAssemblyError::FinalizationAfterFailure {
                operation: Box::new(operation),
                finalization: Box::new(finalization),
            };
            if committed {
                Err(
                    RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization {
                        failure: Box::new(failure),
                    },
                )
            } else {
                Err(failure)
            }
        }
    }
}

fn publish_inner(
    assembly: &CompiledRuntimeAdmissionEvidenceAssembly,
    source: &impl AssemblySourceEvidence,
    destination: &RuntimeAdmissionEvidenceBundleDestination,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceAssemblyError> {
    if source.overlaps_path(&destination.path) {
        return Err(RuntimeAdmissionEvidenceBundleError::UnsafeBoundary.into());
    }
    require_absent(&destination.path)?;
    let metadata = std::fs::symlink_metadata(&destination.parent)
        .map_err(RuntimeAdmissionEvidenceBundleError::StorageIo)?;
    if is_indirect(&metadata) || !metadata.is_dir() {
        return Err(RuntimeAdmissionEvidenceBundleError::UnsafeBoundary.into());
    }
    let parent = PinnedDirectory::open_existing(&destination.parent).map_err(map_storage)?;
    let tree_limits = ManagedTreeLimits::new(limits.maximum_tree_entries).map_err(map_storage)?;
    source.verify_foundation(&assembly.foundation, cancellation)?;
    check_parent_binding(&parent, &destination.parent)?;
    OwnedStagingTree::preflight_no_replace_publication_preserving_cleanup(
        &parent,
        &parent,
        tree_limits,
        limits.maximum_staging_roots,
        limits.maximum_destination_entries,
        cancellation,
    )
    .map_err(map_publication)?;
    source.verify_foundation(&assembly.foundation, cancellation)?;
    let mut staging = OwnedStagingTree::create(
        &parent,
        tree_limits,
        limits.maximum_staging_roots,
        cancellation,
    )
    .map_err(map_storage)?;
    let preparation = prepare_staging(&mut staging, assembly, source, limits, cancellation);
    if let Err(error) = preparation {
        return fail_with_cleanup(staging, error);
    }
    let synced = staging.into_synced().map_err(map_storage)?;
    if let Err(operation) = check_parent_binding(&parent, &destination.parent) {
        return match synced.cleanup() {
            Ok(()) => Err(operation),
            Err(cleanup) => Err(RuntimeAdmissionEvidenceAssemblyError::CleanupAfterFailure {
                operation: Box::new(operation),
                cleanup: Box::new(map_storage(cleanup)),
            }),
        };
    }
    let published = synced
        .publish_no_replace_preserving_cleanup(
            &parent,
            &destination.name,
            limits.maximum_destination_entries,
            cancellation,
        )
        .map_err(map_publication)?;
    let fresh = CancellationToken::new();
    let finalization = (|| {
        source.verify_foundation(&assembly.foundation, &fresh)?;
        let lease = acquire_retained(destination.path.clone(), published, limits, &fresh)?;
        if lease.manifest() != &assembly.manifest || lease.foundation() != &assembly.foundation {
            return Err(RuntimeAdmissionEvidenceBundleError::Changed.into());
        }
        source.verify_foundation(&assembly.foundation, &fresh)?;
        lease.revalidate(&fresh)?;
        Ok(lease)
    })();
    finalization.map_err(
        |failure| RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization {
            failure: Box::new(failure),
        },
    )
}

fn prepare_staging(
    staging: &mut OwnedStagingTree,
    assembly: &CompiledRuntimeAdmissionEvidenceAssembly,
    source: &impl AssemblySourceEvidence,
    limits: RuntimeAdmissionEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionEvidenceAssemblyError> {
    for (path, bytes) in &assembly.bytes {
        write_member(staging, path, bytes, cancellation)?;
    }
    write_member(
        staging,
        RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH,
        assembly.manifest.canonical_json().as_bytes(),
        cancellation,
    )?;
    source.verify_foundation(&assembly.foundation, cancellation)?;
    staging.sync_bottom_up(cancellation).map_err(map_storage)?;
    verify_staged(staging.root(), &assembly.manifest, limits, cancellation)?;
    source.verify_foundation(&assembly.foundation, cancellation)?;
    active(cancellation)?;
    Ok(())
}

fn write_member(
    staging: &mut OwnedStagingTree,
    path: &str,
    bytes: &[u8],
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionEvidenceAssemblyError> {
    active(cancellation)?;
    let path = ArtifactSetRelativePath::new(path)
        .map_err(|_| RuntimeAdmissionEvidenceBundleError::InvalidTree)?;
    if let Some((parent, _)) = path.as_str().rsplit_once('/') {
        staging
            .ensure_directory(
                &ArtifactSetRelativePath::new(parent)
                    .map_err(|_| RuntimeAdmissionEvidenceBundleError::InvalidTree)?,
            )
            .map_err(map_storage)?;
    }
    let mut destination = staging.create_file(&path).map_err(map_storage)?;
    for chunk in bytes.chunks(64 * 1024) {
        active(cancellation)?;
        destination
            .file
            .write_all(chunk)
            .map_err(RuntimeAdmissionEvidenceBundleError::StorageIo)?;
    }
    active(cancellation)?;
    Ok(())
}

fn require_absent(path: &Path) -> Result<(), RuntimeAdmissionEvidenceAssemblyError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(RuntimeAdmissionEvidenceBundleError::Changed.into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RuntimeAdmissionEvidenceBundleError::StorageIo(error).into()),
    }
}

fn check_parent_binding(
    parent: &PinnedDirectory,
    path: &Path,
) -> Result<(), RuntimeAdmissionEvidenceAssemblyError> {
    let held = parent.fingerprint().map_err(map_storage)?;
    let named = PinnedDirectory::fingerprint_path(path).map_err(map_storage)?;
    if !held.same_identity(&named) {
        return Err(RuntimeAdmissionEvidenceBundleError::Changed.into());
    }
    Ok(())
}

fn map_publication(error: NoReplacePublicationFailure) -> RuntimeAdmissionEvidenceAssemblyError {
    let (operation, cleanup, committed) = error.into_parts();
    let operation = RuntimeAdmissionEvidenceAssemblyError::from(map_storage(operation));
    let operation = match cleanup {
        Some(cleanup) => RuntimeAdmissionEvidenceAssemblyError::CleanupAfterFailure {
            operation: Box::new(operation),
            cleanup: Box::new(map_storage(cleanup)),
        },
        None => operation,
    };
    if committed {
        RuntimeAdmissionEvidenceAssemblyError::CommittedFinalization {
            failure: Box::new(operation),
        }
    } else {
        operation
    }
}

fn fail_with_cleanup<T>(
    staging: OwnedStagingTree,
    operation: RuntimeAdmissionEvidenceAssemblyError,
) -> Result<T, RuntimeAdmissionEvidenceAssemblyError> {
    match staging.cleanup() {
        Ok(()) => Err(operation),
        Err(cleanup) => Err(RuntimeAdmissionEvidenceAssemblyError::CleanupAfterFailure {
            operation: Box::new(operation),
            cleanup: Box::new(map_storage(cleanup)),
        }),
    }
}

#[cfg(test)]
mod tests;
