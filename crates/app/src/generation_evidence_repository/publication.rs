use std::ffi::OsStr;

use rewrite_model::{
    CandidateGenerationEvidenceBundleManifestV1, CandidateGenerationEvidenceBundleReadbackV1,
};
use rewrite_model_store::{
    CandidateGenerationEvidenceBundleStorageV1, CandidateGenerationEvidenceStorageContractError,
};
use rewrite_types::CancellationToken;

use crate::{
    CandidateGenerationEvidenceBundleLimits, CandidateGenerationEvidenceBundlePublication,
    CandidateGenerationEvidenceBundlePublicationPlan,
    CandidateGenerationEvidenceBundleReadbackLease,
    generation_evidence_bundle::{CandidateGenerationEvidenceBundlePublisher, acquire_from_parent},
};

use super::{
    CandidateGenerationEvidenceRepository, CandidateGenerationEvidenceRepositoryError,
    layout::{
        FixedChildState, FixedEntryKind, fixed_child_state, open_existing_canonical_directory,
        open_or_create_canonical_directory,
    },
};

const MAXIMUM_PARENT_ENTRIES: usize =
    crate::MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES;

/// Repository-bound successful publication and fresh complete readback.
pub struct CandidateGenerationEvidenceRepositoryPublication {
    storage: CandidateGenerationEvidenceBundleStorageV1,
    publication: CandidateGenerationEvidenceBundlePublication,
}

impl CandidateGenerationEvidenceRepositoryPublication {
    /// Returns the canonical durable local storage reference.
    #[must_use]
    pub const fn storage(&self) -> &CandidateGenerationEvidenceBundleStorageV1 {
        &self.storage
    }

    /// Returns the fresh inert readback record.
    #[must_use]
    pub const fn readback(&self) -> &CandidateGenerationEvidenceBundleReadbackV1 {
        self.publication.readback()
    }

    /// Returns the retained, independently verified bundle lease.
    #[must_use]
    pub const fn lease(&self) -> &CandidateGenerationEvidenceBundleReadbackLease {
        self.publication.lease()
    }

    /// Consumes the result into its durable reference and retained lease.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CandidateGenerationEvidenceBundleStorageV1,
        CandidateGenerationEvidenceBundleReadbackLease,
    ) {
        (self.storage, self.publication.into_lease())
    }
}

/// Repository-bound reacquisition of one exact persisted bundle readback.
pub struct CandidateGenerationEvidenceRepositoryReadbackLease {
    storage: CandidateGenerationEvidenceBundleStorageV1,
    lease: CandidateGenerationEvidenceBundleReadbackLease,
}

impl CandidateGenerationEvidenceRepositoryReadbackLease {
    /// Returns the canonical durable local storage reference.
    #[must_use]
    pub const fn storage(&self) -> &CandidateGenerationEvidenceBundleStorageV1 {
        &self.storage
    }

    /// Returns the retained exact manifest.
    #[must_use]
    pub const fn manifest(&self) -> &CandidateGenerationEvidenceBundleManifestV1 {
        self.lease.manifest()
    }

    /// Returns the independently reconstructed readback record.
    #[must_use]
    pub const fn readback(&self) -> &CandidateGenerationEvidenceBundleReadbackV1 {
        self.lease.readback()
    }

    /// Returns the complete retained low-level readback lease.
    #[must_use]
    pub const fn lease(&self) -> &CandidateGenerationEvidenceBundleReadbackLease {
        &self.lease
    }

    /// Consumes the result into its durable reference and retained lease.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CandidateGenerationEvidenceBundleStorageV1,
        CandidateGenerationEvidenceBundleReadbackLease,
    ) {
        (self.storage, self.lease)
    }
}

impl CandidateGenerationEvidenceRepository {
    /// Publishes one exact bundle through retained parent handles and freshly reads it back.
    ///
    /// The typed storage reference must bind the plan, planned attempt, bundle,
    /// local root, and all three read-side limits exactly. No absolute host path
    /// leaves this repository boundary.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] for a foreign
    /// reference, fixed-layout drift, unsafe parent, occupied destination, failed
    /// no-replace publication, or incomplete fresh readback.
    pub fn publish_bundle(
        &self,
        storage: CandidateGenerationEvidenceBundleStorageV1,
        plan: CandidateGenerationEvidenceBundlePublicationPlan,
        cancellation: &CancellationToken,
    ) -> Result<
        CandidateGenerationEvidenceRepositoryPublication,
        CandidateGenerationEvidenceRepositoryError,
    > {
        self.validate_bundle_storage(&storage)?;
        validate_plan_binding(&storage, &plan)?;
        let parent = self.bundle_parent(&storage, true)?;
        reject_case_alias(&parent, storage.evidence_bundle_id().digest().as_str())?;
        let publication =
            CandidateGenerationEvidenceBundlePublisher::publish_from_staging_into_parent(
                plan,
                &self.staging,
                &parent,
                OsStr::new(storage.evidence_bundle_id().digest().as_str()),
                cancellation,
            )
            .map_err(CandidateGenerationEvidenceRepositoryError::EvidenceBundle)?;
        self.revalidate()?;
        Ok(CandidateGenerationEvidenceRepositoryPublication {
            storage,
            publication,
        })
    }

    /// Reacquires one persisted bundle through retained parent handles.
    ///
    /// The persisted three read-side limits are restored exactly. Fixed
    /// publication-parent and staging ceilings remain process-local and do not
    /// enter the durable reference.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceRepositoryError`] for a foreign or
    /// missing reference, fixed-layout drift, substituted manifest or readback,
    /// cancellation, or any tree and byte mismatch.
    pub fn reacquire_bundle(
        &self,
        storage: CandidateGenerationEvidenceBundleStorageV1,
        expected_manifest: &CandidateGenerationEvidenceBundleManifestV1,
        persisted_readback: &CandidateGenerationEvidenceBundleReadbackV1,
        cancellation: &CancellationToken,
    ) -> Result<
        CandidateGenerationEvidenceRepositoryReadbackLease,
        CandidateGenerationEvidenceRepositoryError,
    > {
        self.validate_bundle_storage(&storage)?;
        if storage.evidence_bundle_id() != expected_manifest.evidence_bundle_id() {
            return Err(invalid_reference());
        }
        let parent = self.bundle_parent(&storage, false)?;
        require_exact_bundle(&parent, storage.evidence_bundle_id().digest().as_str())?;
        let lease = acquire_from_parent(
            &parent,
            OsStr::new(storage.evidence_bundle_id().digest().as_str()),
            expected_manifest,
            Some(persisted_readback),
            reacquisition_limits(&storage)?,
            cancellation,
        )
        .map_err(CandidateGenerationEvidenceRepositoryError::EvidenceBundle)?;
        self.revalidate()?;
        Ok(CandidateGenerationEvidenceRepositoryReadbackLease { storage, lease })
    }

    fn bundle_parent(
        &self,
        storage: &CandidateGenerationEvidenceBundleStorageV1,
        create: bool,
    ) -> Result<crate::artifact_storage::PinnedDirectory, CandidateGenerationEvidenceRepositoryError>
    {
        let open = |parent: &crate::artifact_storage::PinnedDirectory, name: &str| {
            if create {
                open_or_create_canonical_directory(parent, name, MAXIMUM_PARENT_ENTRIES)
            } else {
                open_existing_canonical_directory(parent, name, MAXIMUM_PARENT_ENTRIES)
            }
        };
        let plan = open(
            &self.layout_version,
            storage.qualification_plan_id().digest().as_str(),
        )?;
        open(&plan, storage.planned_attempt_id().digest().as_str())
    }
}

fn validate_plan_binding(
    storage: &CandidateGenerationEvidenceBundleStorageV1,
    plan: &CandidateGenerationEvidenceBundlePublicationPlan,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    let stored_limits = storage.limits();
    let plan_limits = plan.limits();
    if storage.qualification_plan_id() != plan.qualification_plan_id()
        || storage.planned_attempt_id() != plan.planned_attempt_id()
        || storage.evidence_bundle_id() != plan.manifest().evidence_bundle_id()
        || usize::try_from(stored_limits.maximum_tree_entries()).ok()
            != Some(plan_limits.maximum_tree_entries)
        || usize::from(stored_limits.maximum_tree_depth()) != plan_limits.maximum_tree_depth
        || stored_limits.maximum_aggregate_bytes() != plan_limits.maximum_total_bytes
    {
        Err(invalid_reference())
    } else {
        Ok(())
    }
}

fn reacquisition_limits(
    storage: &CandidateGenerationEvidenceBundleStorageV1,
) -> Result<CandidateGenerationEvidenceBundleLimits, CandidateGenerationEvidenceRepositoryError> {
    let limits = storage.limits();
    Ok(CandidateGenerationEvidenceBundleLimits {
        maximum_tree_entries: usize::try_from(limits.maximum_tree_entries())
            .map_err(|_| invalid_reference())?,
        maximum_tree_depth: usize::from(limits.maximum_tree_depth()),
        maximum_total_bytes: limits.maximum_aggregate_bytes(),
        maximum_destination_entries:
            crate::MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES,
        maximum_staging_roots: crate::MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS,
    })
}

fn reject_case_alias(
    parent: &crate::artifact_storage::PinnedDirectory,
    canonical: &str,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    let _ = fixed_child_state(
        parent,
        canonical,
        FixedEntryKind::Directory,
        MAXIMUM_PARENT_ENTRIES,
    )?;
    Ok(())
}

fn require_exact_bundle(
    parent: &crate::artifact_storage::PinnedDirectory,
    canonical: &str,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    if fixed_child_state(
        parent,
        canonical,
        FixedEntryKind::Directory,
        MAXIMUM_PARENT_ENTRIES,
    )? == FixedChildState::Absent
    {
        Err(CandidateGenerationEvidenceRepositoryError::StorageChanged)
    } else {
        Ok(())
    }
}

fn invalid_reference() -> CandidateGenerationEvidenceRepositoryError {
    CandidateGenerationEvidenceRepositoryError::StorageContract(
        CandidateGenerationEvidenceStorageContractError::InvalidReference,
    )
}
