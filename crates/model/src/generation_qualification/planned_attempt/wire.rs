use serde::Deserialize;

use rewrite_types::Digest;

use super::{
    CandidateOutputCeilingsV1, PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
};
use crate::ArtifactId;
use crate::generation_qualification::{
    GenerationCaseId, GenerationClusterId, GenerationQualificationContractError,
    GenerationRepetitionId, GenerationRequestBindingId, GenerationSuiteManifestId,
    GenerationSystemId,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttemptWire {
    pub(super) schema_version: u32,
    suite_manifest_id: GenerationSuiteManifestId,
    case_id: GenerationCaseId,
    cluster_id: GenerationClusterId,
    repetition_id: GenerationRepetitionId,
    attempt_ordinal: u32,
    declared_seed: u64,
    generation_system_id: GenerationSystemId,
    source_artifact_id: ArtifactId,
    source_digest: Digest,
    source_byte_count: u64,
    case_contract_digest: Digest,
    grounded_request_digest: Digest,
    generation_request_binding_id: GenerationRequestBindingId,
    candidate_output_contract_digest: Digest,
    candidate_count: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_envelope_bytes: u64,
}

impl AttemptWire {
    pub(super) fn input(
        &self,
    ) -> Result<PlannedCandidateAttemptV1Input, GenerationQualificationContractError> {
        Ok(PlannedCandidateAttemptV1Input {
            attempt_ordinal: self.attempt_ordinal,
            declared_seed: self.declared_seed,
            grounded_request_digest: self.grounded_request_digest.clone(),
            generation_request_binding_id: self.generation_request_binding_id.clone(),
            candidate_output_contract_digest: self.candidate_output_contract_digest.clone(),
            output_ceilings: CandidateOutputCeilingsV1::from_wire(
                self.candidate_count,
                self.maximum_candidate_bytes,
                self.maximum_aggregate_candidate_bytes,
                self.maximum_envelope_bytes,
            )?,
        })
    }

    pub(super) fn matches(
        &self,
        relations: PlannedCandidateAttemptV1Relations<'_>,
        input: &PlannedCandidateAttemptV1Input,
    ) -> bool {
        self.suite_manifest_id == *relations.suite.suite_manifest_id()
            && self.case_id == *relations.case.case_id()
            && self.cluster_id == *relations.cluster.cluster_id()
            && self.repetition_id == *relations.repetition.repetition_id()
            && self.generation_system_id == *relations.generation_system.generation_system_id()
            && self.source_artifact_id == *relations.case.source_artifact_id()
            && self.source_digest == *relations.case.source_digest()
            && self.source_byte_count == relations.case.source_byte_count()
            && self.case_contract_digest == *relations.case.case_contract_digest()
            && self.attempt_ordinal == input.attempt_ordinal
    }
}
