use std::{collections::BTreeSet, fmt};

use rewrite_inference::{
    CandidateOutputPolicy, StructuredCompletionRequest, parse_candidate_output,
};
use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, CandidateArtifactEntryV1,
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations, CandidateGenerationEvidenceBundleRoleV1,
    GenerationCaseManifestV1, GenerationQualificationPlanId, GenerationQualificationPlanV1,
    ManagedOllamaCandidateGenerationEvidenceV2, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1, StructuredResponseArtifactV1Input,
};
use rewrite_types::CancellationToken;

use super::{
    CandidateGenerationEvidenceBundleError, CandidateGenerationEvidenceBundleLimits,
    RetainedStructuredResponseArtifactV1,
    contract::{digest_bytes, ensure_active},
};

/// Exact bytes for one manifest-declared auxiliary observation.
pub struct CandidateGenerationEvidenceBundleAuxiliaryArtifact<'a> {
    /// Exact manifest path for this observation.
    pub relative_path: &'a ArtifactSetRelativePath,
    /// Exact closed role declared for this observation.
    pub role: CandidateGenerationEvidenceBundleRoleV1,
    /// Exact retained observation bytes.
    pub bytes: &'a [u8],
}

/// Exact typed inputs for compiling one single-use publication plan.
pub struct CandidateGenerationEvidenceBundlePublicationPlanInput<'a> {
    /// Exact immutable qualification plan.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Exact selected planned candidate attempt.
    pub planned_attempt: &'a PlannedCandidateAttemptV1,
    /// Exact case manifest named by the planned attempt.
    pub case_manifest: &'a GenerationCaseManifestV1,
    /// Exact prelaunch precursor.
    pub precursor: &'a CandidateGenerationAttemptPrecursorV1,
    /// Exact completed managed generation evidence.
    pub managed_evidence: &'a ManagedOllamaCandidateGenerationEvidenceV2,
    /// Exact cleanup and final-revalidation record.
    pub cleanup: &'a CandidateGenerationCleanupRecordV1,
    /// Exact canonical retained structured response artifact.
    pub structured_response: &'a RetainedStructuredResponseArtifactV1,
    /// Exact structured request bound by the precursor and retained response.
    pub structured_request: &'a StructuredCompletionRequest,
    /// Ordered exact auxiliary paths and bytes.
    pub auxiliary_artifacts: &'a [CandidateGenerationEvidenceBundleAuxiliaryArtifact<'a>],
    /// Exact model-owned bundle manifest.
    pub manifest: &'a CandidateGenerationEvidenceBundleManifestV1,
}

pub(super) struct PlannedBundleFile {
    pub(super) path: ArtifactSetRelativePath,
    pub(super) artifact_id: ArtifactId,
    pub(super) bytes: Vec<u8>,
}

/// Noncloneable, single-use authority to publish one exact evidence bundle.
pub struct CandidateGenerationEvidenceBundlePublicationPlan {
    qualification_plan_id: GenerationQualificationPlanId,
    planned_attempt_id: PlannedCandidateAttemptId,
    pub(super) manifest: CandidateGenerationEvidenceBundleManifestV1,
    pub(super) manifest_bytes: Vec<u8>,
    pub(super) files: Vec<PlannedBundleFile>,
    pub(super) limits: CandidateGenerationEvidenceBundleLimits,
    pub(super) exact_tree_entries: usize,
}

impl CandidateGenerationEvidenceBundlePublicationPlan {
    /// Compiles a single-use plan and rederives every byte-level binding.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] for any substituted
    /// relationship, record, candidate, observation, manifest, ceiling, or
    /// cancellation request.
    pub fn compile(
        input: &CandidateGenerationEvidenceBundlePublicationPlanInput<'_>,
        limits: CandidateGenerationEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, CandidateGenerationEvidenceBundleError> {
        ensure_active(cancellation)?;
        let limits = limits.validate()?;
        validate_auxiliary_closure(input, limits, cancellation)?;
        validate_response_joins(input)?;
        let candidates = derive_candidates(input, cancellation)?;
        ensure_active(cancellation)?;
        let manifest_bytes = input
            .manifest
            .to_canonical_json_bytes()
            .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
        if input
            .manifest
            .rederive_evidence_bundle_id()
            .map_err(CandidateGenerationEvidenceBundleError::Contract)?
            != *input.manifest.evidence_bundle_id()
        {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
        let response_input = StructuredResponseArtifactV1Input::new(
            input.structured_response.content_artifact_id().clone(),
            input.structured_response.byte_size(),
        )
        .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
        let relations = CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: input.qualification_plan,
            planned_attempt: input.planned_attempt,
            precursor: input.precursor,
            managed_evidence: input.managed_evidence,
            cleanup: input.cleanup,
            structured_response_artifact: &response_input,
        };
        let candidate_entries = candidates
            .iter()
            .map(|candidate| candidate.entry.clone())
            .collect::<Vec<_>>();
        let decoded = CandidateGenerationEvidenceBundleManifestV1::from_json_bytes(
            &manifest_bytes,
            relations,
            &candidate_entries,
        )
        .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
        if decoded != *input.manifest {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }

        let fixed = fixed_record_bytes(input)?;
        let mut files = Vec::with_capacity(input.manifest.entries().len());
        for entry in input.manifest.entries() {
            ensure_active(cancellation)?;
            let bytes = bytes_for_entry(entry, input, &fixed, &candidates)?;
            require_entry_bytes(entry, &bytes, cancellation)?;
            files.push(PlannedBundleFile {
                path: entry.relative_path().clone(),
                artifact_id: entry.artifact_id().clone(),
                bytes,
            });
        }
        let exact_tree_entries = validate_tree_plan(
            &files,
            manifest_bytes.len(),
            input.qualification_plan,
            limits,
            cancellation,
        )?;
        Ok(Self {
            qualification_plan_id: input.qualification_plan.qualification_plan_id().clone(),
            planned_attempt_id: input.planned_attempt.planned_attempt_id().clone(),
            manifest: input.manifest.clone(),
            manifest_bytes,
            files,
            limits,
            exact_tree_entries,
        })
    }

    /// Returns the exact model-owned manifest.
    #[must_use]
    pub const fn manifest(&self) -> &CandidateGenerationEvidenceBundleManifestV1 {
        &self.manifest
    }

    pub(crate) const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }

    pub(crate) const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }

    pub(crate) const fn limits(&self) -> CandidateGenerationEvidenceBundleLimits {
        self.limits
    }
}

impl fmt::Debug for CandidateGenerationEvidenceBundlePublicationPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationEvidenceBundlePublicationPlan")
            .field("bundle_id", self.manifest.evidence_bundle_id())
            .field("content_entry_count", &self.files.len())
            .field("tree_entry_count", &self.exact_tree_entries)
            .finish_non_exhaustive()
    }
}

struct FixedRecordBytes {
    planned: Vec<u8>,
    precursor: Vec<u8>,
    managed: Vec<u8>,
    response: Vec<u8>,
    cleanup: Vec<u8>,
}

struct DerivedCandidate {
    entry: CandidateArtifactEntryV1,
    bytes: Vec<u8>,
}

fn fixed_record_bytes(
    input: &CandidateGenerationEvidenceBundlePublicationPlanInput<'_>,
) -> Result<FixedRecordBytes, CandidateGenerationEvidenceBundleError> {
    Ok(FixedRecordBytes {
        planned: encode_record(input.planned_attempt)?,
        precursor: encode_record(input.precursor)?,
        managed: encode_record(input.managed_evidence)?,
        response: input.structured_response.canonical_json_bytes().to_vec(),
        cleanup: encode_record(input.cleanup)?,
    })
}

fn encode_record(
    value: &impl serde::Serialize,
) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
    serde_json::to_vec(value).map_err(|_| CandidateGenerationEvidenceBundleError::PlanMismatch)
}

fn bytes_for_entry(
    entry: &CandidateGenerationEvidenceBundleEntryV1,
    input: &CandidateGenerationEvidenceBundlePublicationPlanInput<'_>,
    fixed: &FixedRecordBytes,
    candidates: &[DerivedCandidate],
) -> Result<Vec<u8>, CandidateGenerationEvidenceBundleError> {
    let bytes = match entry.role() {
        CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt => fixed.planned.clone(),
        CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor => fixed.precursor.clone(),
        CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence => fixed.managed.clone(),
        CandidateGenerationEvidenceBundleRoleV1::StructuredResponse => fixed.response.clone(),
        CandidateGenerationEvidenceBundleRoleV1::CleanupRecord => fixed.cleanup.clone(),
        CandidateGenerationEvidenceBundleRoleV1::Candidate => candidates
            .iter()
            .find(|candidate| candidate.entry.relative_path() == entry.relative_path())
            .map(|candidate| candidate.bytes.clone())
            .ok_or(CandidateGenerationEvidenceBundleError::PlanMismatch)?,
        CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation => input
            .auxiliary_artifacts
            .iter()
            .find(|artifact| artifact.relative_path == entry.relative_path())
            .map(|artifact| artifact.bytes.to_vec())
            .ok_or(CandidateGenerationEvidenceBundleError::PlanMismatch)?,
    };
    Ok(bytes)
}

fn require_entry_bytes(
    entry: &CandidateGenerationEvidenceBundleEntryV1,
    bytes: &[u8],
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    let size = u64::try_from(bytes.len())
        .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    if size != entry.byte_size()
        || &ArtifactId::from_digest(digest_bytes(bytes, cancellation)?) != entry.artifact_id()
    {
        Err(CandidateGenerationEvidenceBundleError::PlanMismatch)
    } else {
        Ok(())
    }
}

fn validate_response_joins(
    input: &CandidateGenerationEvidenceBundlePublicationPlanInput<'_>,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    if input.structured_response.response_id() != input.managed_evidence.response_id()
        || input.structured_response.response_id() != input.manifest.response_id()
        || input.structured_response.structured_request_binding_id()
            != input.precursor.structured_request_binding_id()
        || input.structured_response.structured_request_binding_id()
            != input.managed_evidence.structured_request_binding_id()
        || input.structured_request.structured_request_binding_id()
            != *input.structured_response.structured_request_binding_id()
        || input.planned_attempt.case_id() != input.case_manifest.case_id()
    {
        Err(CandidateGenerationEvidenceBundleError::PlanMismatch)
    } else {
        Ok(())
    }
}

fn derive_candidates(
    input: &CandidateGenerationEvidenceBundlePublicationPlanInput<'_>,
    cancellation: &CancellationToken,
) -> Result<Vec<DerivedCandidate>, CandidateGenerationEvidenceBundleError> {
    ensure_active(cancellation)?;
    let response = input
        .structured_response
        .reconstruct_response(input.structured_request)
        .map_err(CandidateGenerationEvidenceBundleError::ResponseArtifact)?;
    let ceilings = input.planned_attempt.output_ceilings();
    let policy = CandidateOutputPolicy::new(
        ceilings.candidate_count(),
        ceilings.maximum_candidate_bytes(),
        ceilings.maximum_aggregate_candidate_bytes(),
    )
    .map_err(CandidateGenerationEvidenceBundleError::CandidateOutput)?;
    let parsed = parse_candidate_output(response.output_json().as_bytes(), policy)
        .map_err(CandidateGenerationEvidenceBundleError::CandidateOutput)?;
    ensure_active(cancellation)?;
    if parsed.len() != input.manifest.candidate_artifacts().len() {
        return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
    }
    let mut derived = Vec::with_capacity(parsed.len());
    for (candidate, expected) in parsed.iter().zip(input.manifest.candidate_artifacts()) {
        ensure_active(cancellation)?;
        if candidate.ordinal != expected.ordinal() {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
        let entry = CandidateArtifactEntryV1::new(
            input.precursor,
            input.planned_attempt,
            input.case_manifest,
            candidate.ordinal,
            expected.relative_path().clone(),
            candidate.text.as_bytes(),
        )
        .map_err(CandidateGenerationEvidenceBundleError::Contract)?;
        if &entry != expected {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
        derived.push(DerivedCandidate {
            entry,
            bytes: candidate.text.as_bytes().to_vec(),
        });
    }
    Ok(derived)
}

fn validate_auxiliary_closure(
    input: &CandidateGenerationEvidenceBundlePublicationPlanInput<'_>,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    ensure_active(cancellation)?;
    let expected = input.manifest.entries().iter().filter(|entry| {
        entry.role() == CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation
    });
    if expected.count() != input.auxiliary_artifacts.len() {
        return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
    }
    let mut aggregate_bytes = 0_u64;
    for (expected, provided) in input
        .manifest
        .entries()
        .iter()
        .filter(|entry| {
            entry.role() == CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation
        })
        .zip(input.auxiliary_artifacts)
    {
        ensure_active(cancellation)?;
        if expected.relative_path() != provided.relative_path
            || provided.role != CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation
            || provided.role != expected.role()
        {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
        let byte_size = u64::try_from(provided.bytes.len())
            .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?;
        if byte_size != expected.byte_size() {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
        aggregate_bytes = aggregate_bytes
            .checked_add(byte_size)
            .ok_or(CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    }
    if aggregate_bytes > limits.maximum_total_bytes
        || aggregate_bytes
            > input
                .qualification_plan
                .limits()
                .maximum_evidence_bundle_bytes()
    {
        return Err(CandidateGenerationEvidenceBundleError::LimitExceeded);
    }
    for (expected, provided) in input
        .manifest
        .entries()
        .iter()
        .filter(|entry| {
            entry.role() == CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation
        })
        .zip(input.auxiliary_artifacts)
    {
        ensure_active(cancellation)?;
        if &ArtifactId::from_digest(digest_bytes(provided.bytes, cancellation)?)
            != expected.artifact_id()
        {
            return Err(CandidateGenerationEvidenceBundleError::PlanMismatch);
        }
    }
    Ok(())
}

fn validate_tree_plan(
    files: &[PlannedBundleFile],
    manifest_bytes: usize,
    qualification_plan: &GenerationQualificationPlanV1,
    limits: CandidateGenerationEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<usize, CandidateGenerationEvidenceBundleError> {
    ensure_active(cancellation)?;
    let mut directories = BTreeSet::new();
    let mut total = u64::try_from(manifest_bytes)
        .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    for file in files {
        ensure_active(cancellation)?;
        let depth = file.path.as_str().split('/').count();
        if depth > limits.maximum_tree_depth {
            return Err(CandidateGenerationEvidenceBundleError::LimitExceeded);
        }
        total = total
            .checked_add(
                u64::try_from(file.bytes.len())
                    .map_err(|_| CandidateGenerationEvidenceBundleError::LimitExceeded)?,
            )
            .ok_or(CandidateGenerationEvidenceBundleError::LimitExceeded)?;
        collect_directories(&file.path, &mut directories)?;
    }
    let tree_entries = files
        .len()
        .checked_add(directories.len())
        .and_then(|count| count.checked_add(1))
        .ok_or(CandidateGenerationEvidenceBundleError::LimitExceeded)?;
    if total > limits.maximum_total_bytes
        || qualification_plan.limits().maximum_evidence_bundle_bytes()
            < files
                .iter()
                .try_fold(0_u64, |sum, file| {
                    sum.checked_add(u64::try_from(file.bytes.len()).ok()?)
                })
                .ok_or(CandidateGenerationEvidenceBundleError::LimitExceeded)?
        || tree_entries > limits.maximum_tree_entries
    {
        Err(CandidateGenerationEvidenceBundleError::LimitExceeded)
    } else {
        Ok(tree_entries)
    }
}

fn collect_directories(
    path: &ArtifactSetRelativePath,
    directories: &mut BTreeSet<ArtifactSetRelativePath>,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    let mut prefix = String::new();
    let components = path.as_str().split('/').collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        directories.insert(
            ArtifactSetRelativePath::new(prefix.clone())
                .map_err(|_| CandidateGenerationEvidenceBundleError::PlanMismatch)?,
        );
    }
    Ok(())
}
