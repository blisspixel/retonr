use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{
    append_count, append_digest, append_text, append_u32, append_u64, validate_canonical_json,
};
use super::{
    CANDIDATE_GENERATION_RECEIPT_ID_DOMAIN, CandidateArtifactEntryV1,
    CandidateGenerationAttemptPrecursorId, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationCleanupId, CandidateGenerationCleanupRecordV1,
    CandidateGenerationEvidenceBundleId, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleReadbackId, CandidateGenerationEvidenceBundleReadbackV1,
    CandidateGenerationReceiptId, FrozenExternalComponentSetId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId, GenerationCaseManifestV1,
    GenerationClusterId, GenerationClusterRecordV1, GenerationQualificationContractError,
    GenerationQualificationPlanId, GenerationQualificationPlanV1, GenerationRepetitionId,
    GenerationRepetitionRecordV1, GenerationRequestBindingId, GenerationSuiteManifestId,
    GenerationSuiteManifestV1, GenerationSystemId, GenerationSystemRecordV1,
    ManagedGenerationPathId, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Id, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, OllamaRetainedSessionResponseId,
    PlannedCandidateAttemptId, PlannedCandidateAttemptV1, RuntimeAdmissionJoinId,
    StructuredCompletionRequestBindingId,
};
use crate::{
    ArtifactId, ArtifactSetId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId,
    ModelPackageManifestId, RuntimeBuildId, RuntimePackageManifestId,
};

mod accessors;
mod validation;
mod wire;

use validation::validate_receipt_relations;
use wire::ReceiptWire;

/// Maximum JSON bytes accepted for one candidate-generation receipt.
pub const MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES: usize = 16_384;

/// Opaque result of cheap bounded receipt syntax, schema, and canonicality checks.
///
/// This token contains no trusted relationship and grants no candidate or
/// qualification authority. It can only be obtained by parsing the complete exact
/// receipt bytes, and it must be consumed by the relationship-aware decoder.
///
/// ```compile_fail
/// use rewrite_model::CandidateGenerationReceiptV1Preflight;
///
/// fn require_clone<T: Clone>() {}
///
/// fn clone_preflight() {
///     require_clone::<CandidateGenerationReceiptV1Preflight>();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_model::CandidateGenerationReceiptV1Preflight;
///
/// fn serialize_preflight(value: &CandidateGenerationReceiptV1Preflight) {
///     let _bytes = serde_json::to_vec(value).expect("preflight must not serialize");
/// }
/// ```
pub struct CandidateGenerationReceiptV1Preflight {
    schema_version: u32,
    usage_observation: CandidateGenerationUsageObservationV1,
    canonical_json: Box<[u8]>,
}

impl CandidateGenerationReceiptV1Preflight {
    /// Returns the already-validated receipt schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the inert usage observation carried by the receipt bytes.
    ///
    /// This value is untrusted until the receipt is relationship-decoded and an
    /// upper layer independently compares it with the retained response.
    #[must_use]
    pub const fn usage_observation(&self) -> CandidateGenerationUsageObservationV1 {
        self.usage_observation
    }
}

impl fmt::Debug for CandidateGenerationReceiptV1Preflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationReceiptV1Preflight")
            .field("schema_version", &self.schema_version)
            .finish_non_exhaustive()
    }
}

/// Fixed bounded token and duration observation retained with one response.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationUsageObservationV1 {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    generation_micros: Option<u64>,
}

impl CandidateGenerationUsageObservationV1 {
    /// Creates an inert typed response-usage observation.
    #[must_use]
    pub const fn new(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        generation_micros: Option<u64>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            generation_micros,
        }
    }
    /// Returns the observed input token count.
    #[must_use]
    pub const fn input_tokens(self) -> Option<u64> {
        self.input_tokens
    }
    /// Returns the observed output token count.
    #[must_use]
    pub const fn output_tokens(self) -> Option<u64> {
        self.output_tokens
    }
    /// Returns the observed generation duration in microseconds.
    #[must_use]
    pub const fn generation_micros(self) -> Option<u64> {
        self.generation_micros
    }
}

/// Closed evidence class for one cleanup-and-readback-gated receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationReceiptEvidenceClassV1 {
    /// A managed bracket followed by successful cleanup and verified readback.
    CleanupAndReadbackVerifiedManagedBracket,
}

/// Exact upstream records required to derive and reload one receipt.
#[derive(Clone, Copy)]
pub struct CandidateGenerationReceiptV1Relations<'a> {
    /// Exact qualification plan.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Exact suite selected by the plan.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact selected case.
    pub case: &'a GenerationCaseManifestV1,
    /// Exact cluster selected by the case.
    pub cluster: &'a GenerationClusterRecordV1,
    /// Exact selected repetition.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Exact planned attempt.
    pub planned_attempt: &'a PlannedCandidateAttemptV1,
    /// Exact execution precursor.
    pub precursor: &'a CandidateGenerationAttemptPrecursorV1,
    /// Exact stable generation system.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Exact managed generation evidence.
    pub managed_evidence: &'a ManagedOllamaCandidateGenerationEvidenceV2,
    /// Exact successful cleanup record.
    pub cleanup: &'a CandidateGenerationCleanupRecordV1,
    /// Exact published evidence-bundle manifest.
    pub bundle: &'a CandidateGenerationEvidenceBundleManifestV1,
    /// Exact verified evidence-bundle readback.
    pub readback: &'a CandidateGenerationEvidenceBundleReadbackV1,
}

/// Inert portable receipt for a cleanup-and-readback-complete candidate batch.
///
/// The record does not prove model use, application-handler execution, formal
/// placement, semantic quality, qualification, release approval, or live authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the versioned receipt keeps every negative claim explicit"
)]
pub struct CandidateGenerationReceiptV1 {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    case_id: GenerationCaseId,
    cluster_id: GenerationClusterId,
    repetition_id: GenerationRepetitionId,
    planned_attempt_id: PlannedCandidateAttemptId,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    generation_system_id: GenerationSystemId,
    source_digest: Digest,
    case_contract_digest: Digest,
    grounded_request_digest: Digest,
    generation_request_binding_id: GenerationRequestBindingId,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    bracket_observation_v1_id: ManagedOllamaGenerationBracketObservationV1Id,
    response_id: OllamaRetainedSessionResponseId,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
    static_model_binding_digest: Digest,
    candidate_output_contract_digest: Digest,
    cleanup_id: CandidateGenerationCleanupId,
    bundle_id: CandidateGenerationEvidenceBundleId,
    readback_id: CandidateGenerationEvidenceBundleReadbackId,
    candidate_entries: Vec<CandidateArtifactEntryV1>,
    usage_observation: CandidateGenerationUsageObservationV1,
    evidence_class: CandidateGenerationReceiptEvidenceClassV1,
    model_loaded_proven: bool,
    model_used_proven: bool,
    application_handler_proven: bool,
    formal_placement_proven: bool,
    qualified: bool,
    #[serde(skip)]
    id: CandidateGenerationReceiptId,
}

impl CandidateGenerationReceiptV1 {
    /// Derives an inert receipt after exact portable relationship validation.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless all upstream records,
    /// successful cleanup, verified readback, and candidate closure agree exactly.
    pub fn new(
        relations: CandidateGenerationReceiptV1Relations<'_>,
        usage_observation: CandidateGenerationUsageObservationV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            relations,
            usage_observation,
        )
    }

    fn build(
        schema_version: u32,
        relations: CandidateGenerationReceiptV1Relations<'_>,
        usage_observation: CandidateGenerationUsageObservationV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        validate_receipt_relations(relations)?;
        let mut value = Self::from_relations(schema_version, relations, usage_observation);
        let canonical = value.canonical_bytes()?;
        if canonical.len() > MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        value.id = CandidateGenerationReceiptId(Digest::sha256(&canonical));
        Ok(value)
    }

    fn from_relations(
        schema_version: u32,
        relations: CandidateGenerationReceiptV1Relations<'_>,
        usage_observation: CandidateGenerationUsageObservationV1,
    ) -> Self {
        Self {
            schema_version,
            qualification_plan_id: relations.qualification_plan.qualification_plan_id().clone(),
            suite_manifest_id: relations.suite.suite_manifest_id().clone(),
            case_id: relations.case.case_id().clone(),
            cluster_id: relations.cluster.cluster_id().clone(),
            repetition_id: relations.repetition.repetition_id().clone(),
            planned_attempt_id: relations.planned_attempt.planned_attempt_id().clone(),
            precursor_id: relations.precursor.precursor_id().clone(),
            generation_system_id: relations.generation_system.generation_system_id().clone(),
            source_digest: relations.planned_attempt.source_digest().clone(),
            case_contract_digest: relations.planned_attempt.case_contract_digest().clone(),
            grounded_request_digest: relations.planned_attempt.grounded_request_digest().clone(),
            generation_request_binding_id: relations
                .planned_attempt
                .generation_request_binding_id()
                .clone(),
            structured_request_binding_id: relations
                .precursor
                .structured_request_binding_id()
                .clone(),
            managed_evidence_id: relations.managed_evidence.managed_evidence_v2_id().clone(),
            bracket_observation_v1_id: relations
                .managed_evidence
                .bracket_observation_v1_id()
                .clone(),
            response_id: relations.managed_evidence.response_id().clone(),
            runtime_admission_join_id: relations.precursor.runtime_admission_join_id().clone(),
            managed_generation_path_id: relations.precursor.managed_generation_path_id().clone(),
            frozen_external_component_set_id: relations
                .precursor
                .frozen_external_component_set_id()
                .clone(),
            runtime_package_manifest_id: relations.precursor.runtime_package_manifest_id().clone(),
            runtime_build_id: relations.precursor.runtime_build_id().clone(),
            effective_runtime_state_id: relations
                .precursor
                .expected_effective_runtime_state_id()
                .clone(),
            effective_runtime_state_join_id: relations
                .managed_evidence
                .effective_runtime_state_join_id()
                .clone(),
            model_artifact_set_id: relations.precursor.model_artifact_set_id().clone(),
            model_package_manifest_id: relations.precursor.model_package_manifest_id().clone(),
            model_artifact_id: relations.precursor.model_artifact_id().clone(),
            effective_package_evidence_v2_id: relations
                .precursor
                .effective_package_evidence_v2_id()
                .clone(),
            runtime_installation_generation: relations.precursor.runtime_installation_generation(),
            model_installation_generation: relations.precursor.model_installation_generation(),
            static_model_binding_digest: relations.precursor.static_model_binding_digest().clone(),
            candidate_output_contract_digest: relations
                .planned_attempt
                .candidate_output_contract_digest()
                .clone(),
            cleanup_id: relations.cleanup.cleanup_id().clone(),
            bundle_id: relations.bundle.evidence_bundle_id().clone(),
            readback_id: relations.readback.readback_id().clone(),
            candidate_entries: relations.bundle.candidate_artifacts().to_vec(),
            usage_observation,
            evidence_class:
                CandidateGenerationReceiptEvidenceClassV1::CleanupAndReadbackVerifiedManagedBracket,
            model_loaded_proven: true,
            model_used_proven: false,
            application_handler_proven: false,
            formal_placement_proven: false,
            qualified: false,
            id: CandidateGenerationReceiptId(Digest::sha256(b"uninitialized candidate receipt")),
        }
    }

    /// Parses canonical bounded JSON and reloads every exact upstream record.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// future-schema, noncanonical, stale, or substituted receipt bytes.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: CandidateGenerationReceiptV1Relations<'_>,
        expected_usage: CandidateGenerationUsageObservationV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        let preflight = Self::preflight_json_bytes(bytes)?;
        Self::from_preflight(preflight, relations, expected_usage)
    }

    /// Checks the complete receipt encoding before any external evidence access.
    ///
    /// The hard byte ceiling is enforced before JSON decoding. Success proves only
    /// that the bytes use the supported schema and its one canonical JSON encoding.
    /// It does not validate any receipt relationship or create authority.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unknown-field-bearing, duplicated, trailing, unsupported, or noncanonical
    /// JSON.
    pub fn preflight_json_bytes(
        bytes: &[u8],
    ) -> Result<CandidateGenerationReceiptV1Preflight, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: ReceiptWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version() != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version(),
            ));
        }
        validate_canonical_json(bytes, &wire)?;
        Ok(CandidateGenerationReceiptV1Preflight {
            schema_version: wire.schema_version(),
            usage_observation: wire.usage_observation(),
            canonical_json: bytes.into(),
        })
    }

    /// Consumes preflighted bytes and reloads every exact upstream relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if the supplied records or
    /// response usage do not derive the exact preflighted receipt bytes.
    pub fn from_preflight(
        preflight: CandidateGenerationReceiptV1Preflight,
        relations: CandidateGenerationReceiptV1Relations<'_>,
        expected_usage: CandidateGenerationUsageObservationV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        let CandidateGenerationReceiptV1Preflight {
            schema_version,
            usage_observation: _,
            canonical_json,
        } = preflight;
        let value = Self::build(schema_version, relations, expected_usage)?;
        if serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            != canonical_json.as_ref()
        {
            return Err(GenerationQualificationContractError::ReceiptRelationshipMismatch);
        }
        Ok(value)
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let mut output = CANDIDATE_GENERATION_RECEIPT_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.qualification_plan_id.digest(),
            self.suite_manifest_id.digest(),
            self.case_id.digest(),
            self.cluster_id.digest(),
            self.repetition_id.digest(),
            self.planned_attempt_id.digest(),
            self.precursor_id.digest(),
            self.generation_system_id.digest(),
            &self.source_digest,
            &self.case_contract_digest,
            &self.grounded_request_digest,
            self.generation_request_binding_id.digest(),
            self.structured_request_binding_id.digest(),
            self.managed_evidence_id.digest(),
            self.bracket_observation_v1_id.digest(),
            self.response_id.digest(),
            self.runtime_admission_join_id.digest(),
            self.managed_generation_path_id.digest(),
            self.frozen_external_component_set_id.digest(),
            self.runtime_package_manifest_id.digest(),
            self.runtime_build_id.digest(),
            self.effective_runtime_state_id.digest(),
            self.effective_runtime_state_join_id.digest(),
            self.model_artifact_set_id.digest(),
            self.model_package_manifest_id.digest(),
            self.model_artifact_id.digest(),
            self.effective_package_evidence_v2_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.runtime_installation_generation);
        append_u64(&mut output, self.model_installation_generation);
        for digest in [
            &self.static_model_binding_digest,
            &self.candidate_output_contract_digest,
            self.cleanup_id.digest(),
            self.bundle_id.digest(),
            self.readback_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        append_count(&mut output, self.candidate_entries.len())?;
        for candidate in &self.candidate_entries {
            append_candidate(&mut output, candidate);
        }
        for value in [
            self.usage_observation.input_tokens,
            self.usage_observation.output_tokens,
            self.usage_observation.generation_micros,
        ] {
            append_optional_u64(&mut output, value);
        }
        output.push(0);
        output.extend([1, 0, 0, 0, 0]);
        Ok(output)
    }
}

fn append_candidate(output: &mut Vec<u8>, candidate: &CandidateArtifactEntryV1) {
    append_digest(output, candidate.candidate_evidence_id().digest());
    output.push(candidate.ordinal());
    append_u64(output, candidate.byte_count());
    append_digest(output, candidate.artifact_id().digest());
    append_text(output, candidate.relative_path().as_str());
    append_digest(output, candidate.candidate_digest());
}

fn append_optional_u64(output: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            output.push(1);
            append_u64(output, value);
        }
        None => output.push(0),
    }
}

impl fmt::Debug for CandidateGenerationReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationReceiptV1")
            .field("receipt_id", &self.id)
            .field("candidate_count", &self.candidate_entries.len())
            .field("evidence_class", &self.evidence_class)
            .finish_non_exhaustive()
    }
}
