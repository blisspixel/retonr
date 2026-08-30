use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::codec::{append_digest, append_u32, validate_canonical_json};
use super::{
    CandidateGenerationAttemptPrecursorId, CandidateGenerationAttemptPrecursorV1,
    GenerationQualificationContractError, GenerationRequestBindingId, GenerationSystemRecordV1,
    ManagedOllamaEffectiveRuntimeStateJoinId, ManagedOllamaGenerationBracketObservationV1Id,
    OllamaRetainedSessionResponseId, PlannedCandidateAttemptV1,
    StructuredCompletionRequestBindingId,
};
use super::{
    MANAGED_OLLAMA_CANDIDATE_EVIDENCE_ID_DOMAIN, ManagedOllamaCandidateGenerationEvidenceV2Id,
};
use crate::{EffectivePackageEvidenceV2, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId};

mod validation;
mod wire;

use validation::validate_relations;
use wire::ManagedEvidenceWire;

/// Schema version for managed Ollama candidate-generation evidence V2.
pub const MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_SCHEMA_VERSION: u32 = 2;
/// Maximum JSON bytes accepted for managed Ollama candidate evidence V2.
pub const MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES: usize = 64 * 1_024;
const MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_CANONICAL_BYTES: usize = 4_096;

/// Closed evidence class for the managed candidate-generation bracket.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedOllamaCandidateGenerationEvidenceClassV2 {
    /// A retained managed bracket joined to one exact observed effective state.
    RetainedManagedBracketWithEffectiveState,
}

/// Exact model-owned records needed to validate managed evidence relationships.
#[derive(Clone, Copy)]
pub struct ManagedOllamaCandidateGenerationEvidenceV2Relations<'a> {
    /// Exact execution-specific precursor.
    pub precursor: &'a CandidateGenerationAttemptPrecursorV1,
    /// Exact planned attempt selected by the precursor.
    pub planned_attempt: &'a PlannedCandidateAttemptV1,
    /// Exact stable generation system selected by the planned attempt.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Exact effective-package V2 record selected before launch.
    pub effective_package_evidence_v2: &'a EffectivePackageEvidenceV2,
}

/// Upper-layer typed identities that have no model-owned record dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedOllamaCandidateGenerationEvidenceV2Input {
    /// Unchanged binding identity of the exact bracket V1 observation.
    pub bracket_observation_v1_id: ManagedOllamaGenerationBracketObservationV1Id,
    /// App-owned live effective-state join identity used by the bracket.
    pub effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    /// Exact retained-session response identity.
    pub response_id: OllamaRetainedSessionResponseId,
}

/// Inert portable relationship record for one managed Ollama generation bracket.
///
/// The record closes typed identities over an exact precursor, planned attempt,
/// generation system, and effective-package record. It does not prove execution,
/// model use, handler placement, semantic quality, qualification, or live authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the versioned wire contract keeps every evidence limitation independently explicit"
)]
pub struct ManagedOllamaCandidateGenerationEvidenceV2 {
    schema_version: u32,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    bracket_observation_v1_id: ManagedOllamaGenerationBracketObservationV1Id,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    effective_runtime_state_join_id: ManagedOllamaEffectiveRuntimeStateJoinId,
    generation_request_binding_id: GenerationRequestBindingId,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    response_id: OllamaRetainedSessionResponseId,
    evidence_class: ManagedOllamaCandidateGenerationEvidenceClassV2,
    model_loaded_proven: bool,
    model_used_proven: bool,
    application_handler_proven: bool,
    formal_placement_proven: bool,
    qualified: bool,
    #[serde(skip)]
    id: ManagedOllamaCandidateGenerationEvidenceV2Id,
}

impl ManagedOllamaCandidateGenerationEvidenceV2 {
    /// Creates one inert V2 record from exact model records and typed upper-layer IDs.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if any precursor, request,
    /// system, package, or effective-state relationship differs.
    pub fn new(
        relations: ManagedOllamaCandidateGenerationEvidenceV2Relations<'_>,
        input: ManagedOllamaCandidateGenerationEvidenceV2Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_SCHEMA_VERSION,
            relations,
            input,
            None,
        )
    }

    fn build(
        schema_version: u32,
        relations: ManagedOllamaCandidateGenerationEvidenceV2Relations<'_>,
        input: ManagedOllamaCandidateGenerationEvidenceV2Input,
        wire: Option<&ManagedEvidenceWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        validate_relations(relations)?;
        if wire.is_some_and(|value| !value.has_exact_claims()) {
            return Err(GenerationQualificationContractError::InvalidManagedEvidenceClaims);
        }
        if wire.is_some_and(|value| !value.matches(relations, &input)) {
            return Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch);
        }
        let mut record = Self {
            schema_version,
            precursor_id: relations.precursor.precursor_id().clone(),
            bracket_observation_v1_id: input.bracket_observation_v1_id,
            effective_package_evidence_v2_id: relations
                .effective_package_evidence_v2
                .effective_package_evidence_v2_id(),
            effective_runtime_state_id: relations
                .precursor
                .expected_effective_runtime_state_id()
                .clone(),
            effective_runtime_state_join_id: input.effective_runtime_state_join_id,
            generation_request_binding_id: relations
                .planned_attempt
                .generation_request_binding_id()
                .clone(),
            structured_request_binding_id: relations
                .precursor
                .structured_request_binding_id()
                .clone(),
            response_id: input.response_id,
            evidence_class:
                ManagedOllamaCandidateGenerationEvidenceClassV2::RetainedManagedBracketWithEffectiveState,
            model_loaded_proven: true,
            model_used_proven: false,
            application_handler_proven: false,
            formal_placement_proven: false,
            qualified: false,
            id: ManagedOllamaCandidateGenerationEvidenceV2Id(rewrite_types::Digest::sha256(
                b"uninitialized managed candidate evidence",
            )),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        record.id =
            ManagedOllamaCandidateGenerationEvidenceV2Id(rewrite_types::Digest::sha256(&canonical));
        Ok(record)
    }

    /// Parses canonical bounded JSON and reloads all exact model-owned relationships.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unsupported, noncanonical, claim-widened, stale, or substituted input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: ManagedOllamaCandidateGenerationEvidenceV2Relations<'_>,
        expected_input: &ManagedOllamaCandidateGenerationEvidenceV2Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: ManagedEvidenceWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version() != MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version(),
            ));
        }
        let record = Self::build(
            wire.schema_version(),
            relations,
            expected_input.clone(),
            Some(&wire),
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this inert record against the exact supplied model records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if any relationship or
    /// typed upper-layer identity differs from a fresh checked derivation.
    pub fn validate_against(
        &self,
        relations: ManagedOllamaCandidateGenerationEvidenceV2Relations<'_>,
        expected_input: &ManagedOllamaCandidateGenerationEvidenceV2Input,
    ) -> Result<(), GenerationQualificationContractError> {
        let expected = Self::new(relations, expected_input.clone())?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
        }
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact precursor identity.
    #[must_use]
    pub const fn precursor_id(&self) -> &CandidateGenerationAttemptPrecursorId {
        &self.precursor_id
    }
    /// Returns the unchanged bracket V1 observation identity.
    #[must_use]
    pub const fn bracket_observation_v1_id(
        &self,
    ) -> &ManagedOllamaGenerationBracketObservationV1Id {
        &self.bracket_observation_v1_id
    }
    /// Returns the exact effective-package V2 identity.
    #[must_use]
    pub const fn effective_package_evidence_v2_id(&self) -> &EffectivePackageEvidenceV2Id {
        &self.effective_package_evidence_v2_id
    }
    /// Returns the observed effective-runtime-state identity.
    #[must_use]
    pub const fn effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.effective_runtime_state_id
    }
    /// Returns the app-owned effective-state live-join identity.
    #[must_use]
    pub const fn effective_runtime_state_join_id(
        &self,
    ) -> &ManagedOllamaEffectiveRuntimeStateJoinId {
        &self.effective_runtime_state_join_id
    }
    /// Returns the provider-neutral request identity.
    #[must_use]
    pub const fn generation_request_binding_id(&self) -> &GenerationRequestBindingId {
        &self.generation_request_binding_id
    }
    /// Returns the structured wire-request identity.
    #[must_use]
    pub const fn structured_request_binding_id(&self) -> &StructuredCompletionRequestBindingId {
        &self.structured_request_binding_id
    }
    /// Returns the retained-session response identity.
    #[must_use]
    pub const fn response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.response_id
    }
    /// Returns the closed evidence class.
    #[must_use]
    pub const fn evidence_class(&self) -> ManagedOllamaCandidateGenerationEvidenceClassV2 {
        self.evidence_class
    }
    /// Returns whether model loading was proven by the retained bracket.
    #[must_use]
    pub const fn model_loaded_proven(&self) -> bool {
        self.model_loaded_proven
    }
    /// Returns whether model use was proven.
    #[must_use]
    pub const fn model_used_proven(&self) -> bool {
        self.model_used_proven
    }
    /// Returns whether application-handler execution was proven.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        self.application_handler_proven
    }
    /// Returns whether formal execution placement was proven.
    #[must_use]
    pub const fn formal_placement_proven(&self) -> bool {
        self.formal_placement_proven
    }
    /// Returns the content-derived managed-evidence identity.
    #[must_use]
    pub const fn managed_evidence_v2_id(&self) -> &ManagedOllamaCandidateGenerationEvidenceV2Id {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = MANAGED_OLLAMA_CANDIDATE_EVIDENCE_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.precursor_id.digest(),
            self.bracket_observation_v1_id.digest(),
            self.effective_package_evidence_v2_id.digest(),
            self.effective_runtime_state_id.digest(),
            self.effective_runtime_state_join_id.digest(),
            self.generation_request_binding_id.digest(),
            self.structured_request_binding_id.digest(),
            self.response_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        output.push(evidence_class_tag(self.evidence_class));
        output.extend([
            u8::from(self.model_loaded_proven),
            u8::from(self.model_used_proven),
            u8::from(self.application_handler_proven),
            u8::from(self.formal_placement_proven),
            u8::from(self.qualified),
        ]);
        output
    }
}

impl fmt::Debug for ManagedOllamaCandidateGenerationEvidenceV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedOllamaCandidateGenerationEvidenceV2")
            .field("schema_version", &self.schema_version)
            .field("managed_evidence_v2_id", &self.id)
            .field("evidence_class", &self.evidence_class)
            .finish_non_exhaustive()
    }
}

const fn evidence_class_tag(value: ManagedOllamaCandidateGenerationEvidenceClassV2) -> u8 {
    match value {
        ManagedOllamaCandidateGenerationEvidenceClassV2::RetainedManagedBracketWithEffectiveState => 0,
    }
}
