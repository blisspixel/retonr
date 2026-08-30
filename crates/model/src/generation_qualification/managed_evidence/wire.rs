use serde::Deserialize;

use super::{
    ManagedOllamaCandidateGenerationEvidenceClassV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations,
};
use crate::generation_qualification::{
    CandidateGenerationAttemptPrecursorId, GenerationRequestBindingId,
    ManagedOllamaEffectiveRuntimeStateJoinId, ManagedOllamaGenerationBracketObservationV1Id,
    OllamaRetainedSessionResponseId, StructuredCompletionRequestBindingId,
};
use crate::{EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the strict decoder mirrors every explicit claim in the fixed V2 wire schema"
)]
pub(super) struct ManagedEvidenceWire {
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
}

impl ManagedEvidenceWire {
    pub(super) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(super) const fn has_exact_claims(&self) -> bool {
        matches!(
            self.evidence_class,
            ManagedOllamaCandidateGenerationEvidenceClassV2::RetainedManagedBracketWithEffectiveState
        ) && self.model_loaded_proven
            && !self.model_used_proven
            && !self.application_handler_proven
            && !self.formal_placement_proven
            && !self.qualified
    }

    pub(super) fn matches(
        &self,
        relations: ManagedOllamaCandidateGenerationEvidenceV2Relations<'_>,
        input: &ManagedOllamaCandidateGenerationEvidenceV2Input,
    ) -> bool {
        self.precursor_id == *relations.precursor.precursor_id()
            && self.bracket_observation_v1_id == input.bracket_observation_v1_id
            && self.effective_package_evidence_v2_id
                == relations
                    .effective_package_evidence_v2
                    .effective_package_evidence_v2_id()
            && self.effective_runtime_state_id
                == *relations.precursor.expected_effective_runtime_state_id()
            && self.effective_runtime_state_join_id == input.effective_runtime_state_join_id
            && self.generation_request_binding_id
                == *relations.planned_attempt.generation_request_binding_id()
            && self.structured_request_binding_id
                == *relations.precursor.structured_request_binding_id()
            && self.response_id == input.response_id
    }
}
