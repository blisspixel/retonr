use rewrite_app::CandidateGenerationReceiptCompilation;
use rewrite_inference::GenerationCandidate;
use rewrite_model::{
    CandidateArtifactEntryV1, CandidateGenerationAttemptRecordV1,
    CandidateGenerationReceiptEvidenceClassV1, CandidateGenerationUsageObservationV1,
};
use rewrite_ollama::derive_ollama_retained_session_response_id;
use rewrite_types::CancellationToken;

use crate::VerifiedCompletedManagedCandidateAttempt;

use super::{VerifiedCandidateBatchError, VerifiedCandidateBatchRelationship as Relationship};

#[expect(
    clippy::too_many_lines,
    reason = "the complete authority comparison remains visible in one fixed relationship join"
)]
pub(super) fn validate_fixed_relationships(
    attempt: &VerifiedCompletedManagedCandidateAttempt,
    compilation: &CandidateGenerationReceiptCompilation,
) -> Result<(), VerifiedCandidateBatchError> {
    let receipt = compilation.receipt();
    let planned = attempt.planned_attempt();
    let system = attempt.generation_system();
    let generation_request = attempt.generation_request();
    let structured_request = attempt.structured_request();
    let precursor = attempt.precursor();
    let managed = attempt.managed_evidence();
    let cleanup = attempt.cleanup();
    let effective_package = attempt.effective_package().evidence();
    let response = attempt.response();
    let residency = attempt.residency_receipt().execution();

    let completed_record =
        CandidateGenerationAttemptRecordV1::completed(planned, precursor, receipt);
    let usage = response.usage();
    let checks = FixedRelationshipChecks {
        planned_attempt: receipt.planned_attempt_id() == planned.planned_attempt_id()
            && receipt.source_digest() == planned.source_digest()
            && receipt.case_contract_digest() == planned.case_contract_digest()
            && receipt.grounded_request_digest() == planned.grounded_request_digest()
            && receipt.generation_request_binding_id()
                == planned.generation_request_binding_id()
            && receipt.candidate_output_contract_digest()
                == planned.candidate_output_contract_digest(),
        generation_system: receipt.generation_system_id() == system.generation_system_id()
            && receipt.runtime_admission_join_id() == system.runtime_admission_join_id()
            && receipt.managed_generation_path_id() == system.managed_generation_path_id()
            && receipt.frozen_external_component_set_id()
                == system.frozen_external_component_set_id()
            && receipt.runtime_package_manifest_id() == system.runtime_package_manifest_id()
            && receipt.runtime_build_id() == system.runtime_build_id()
            && receipt.effective_runtime_state_id() == system.effective_runtime_state_id()
            && receipt.model_artifact_set_id() == system.model_artifact_set_id()
            && receipt.model_package_manifest_id() == system.model_package_manifest_id()
            && receipt.model_artifact_id() == system.model_artifact_id()
            && receipt.effective_package_evidence_v2_id()
                == system.effective_package_evidence_v2_id()
            && receipt.static_model_binding_digest() == system.static_model_binding_digest(),
        generation_request: generation_request.validate().is_ok()
            && generation_request.generation_request_binding_id()
                == *receipt.generation_request_binding_id()
            && &generation_request.artifact_id == receipt.model_artifact_id()
            && &generation_request.artifact_digest == receipt.model_artifact_id().digest(),
        structured_request: structured_request.validate().is_ok()
            && structured_request.structured_request_binding_id()
                == *receipt.structured_request_binding_id()
            && &structured_request.artifact_id == receipt.model_artifact_id()
            && &structured_request.artifact_digest == receipt.model_artifact_id().digest(),
        precursor: receipt.precursor_id() == precursor.precursor_id()
            && receipt.planned_attempt_id() == precursor.planned_attempt_id()
            && receipt.runtime_installation_generation()
                == precursor.runtime_installation_generation()
            && receipt.model_installation_generation() == precursor.model_installation_generation()
            && receipt.runtime_admission_join_id() == precursor.runtime_admission_join_id()
            && receipt.managed_generation_path_id() == precursor.managed_generation_path_id()
            && receipt.frozen_external_component_set_id()
                == precursor.frozen_external_component_set_id()
            && receipt.runtime_package_manifest_id() == precursor.runtime_package_manifest_id()
            && receipt.runtime_build_id() == precursor.runtime_build_id()
            && receipt.effective_runtime_state_id()
                == precursor.expected_effective_runtime_state_id()
            && receipt.model_artifact_set_id() == precursor.model_artifact_set_id()
            && receipt.model_package_manifest_id() == precursor.model_package_manifest_id()
            && receipt.model_artifact_id() == precursor.model_artifact_id()
            && receipt.effective_package_evidence_v2_id()
                == precursor.effective_package_evidence_v2_id()
            && receipt.static_model_binding_digest() == precursor.static_model_binding_digest()
            && receipt.structured_request_binding_id()
                == precursor.structured_request_binding_id(),
        managed_evidence: receipt.managed_evidence_id() == managed.managed_evidence_v2_id()
            && receipt.precursor_id() == managed.precursor_id()
            && receipt.bracket_observation_v1_id() == managed.bracket_observation_v1_id()
            && receipt.effective_package_evidence_v2_id()
                == managed.effective_package_evidence_v2_id()
            && receipt.effective_runtime_state_id() == managed.effective_runtime_state_id()
            && receipt.effective_runtime_state_join_id()
                == managed.effective_runtime_state_join_id()
            && receipt.generation_request_binding_id()
                == managed.generation_request_binding_id()
            && receipt.structured_request_binding_id()
                == managed.structured_request_binding_id()
            && receipt.response_id() == managed.response_id()
            && receipt.model_loaded_proven() == managed.model_loaded_proven()
            && receipt.model_used_proven() == managed.model_used_proven()
            && receipt.application_handler_proven() == managed.application_handler_proven()
            && receipt.formal_placement_proven() == managed.formal_placement_proven()
            && receipt.evidence_class()
                == CandidateGenerationReceiptEvidenceClassV1::CleanupAndReadbackVerifiedManagedBracket,
        cleanup: receipt.cleanup_id() == cleanup.cleanup_id()
            && receipt.precursor_id() == cleanup.precursor_id()
            && receipt.managed_evidence_id() == cleanup.managed_evidence_id(),
        effective_package: receipt.effective_package_evidence_v2_id()
            == &effective_package.effective_package_evidence_v2_id()
            && receipt.model_artifact_set_id() == effective_package.artifact_set_id()
            && receipt.runtime_build_id() == effective_package.runtime_build_id()
            && receipt.effective_runtime_state_id()
                == effective_package.effective_runtime_state_id(),
        response: receipt.response_id() == &derive_ollama_retained_session_response_id(response)
            && receipt.response_id() == &residency.retained_response_id()
            && response.request_binding_digest() == &structured_request.binding_digest()
            && residency.request_digest() == response.request_binding_digest()
            && residency.response_digest() == receipt.response_id().digest()
            && response.artifact_id() == receipt.model_artifact_id()
            && response.artifact_digest() == receipt.model_artifact_id().digest(),
        response_usage: receipt.usage_observation()
            == CandidateGenerationUsageObservationV1::new(
                usage.input_tokens,
                usage.output_tokens,
                usage.generation_micros,
            ),
        attempt_record: completed_record
            .as_ref()
            .is_ok_and(|record| record == compilation.attempt_record()),
        receipt_compilation: receipt.bundle_id() == compilation.lease().bundle_id()
            && receipt.readback_id() == compilation.lease().readback().readback_id(),
    };
    checks.validate()
}

pub(super) fn validate_candidates(
    attempt: &VerifiedCompletedManagedCandidateAttempt,
    compilation: &CandidateGenerationReceiptCompilation,
    cancellation: &CancellationToken,
) -> Result<(), VerifiedCandidateBatchError> {
    let candidates = attempt.candidates();
    let receipt_entries = compilation.receipt().candidate_entries();
    require_relationship(
        // The first managed structured-request protocol admits exactly one candidate. This also
        // bounds candidate access to one whole-tree-revalidated read in the current protocol.
        candidates.len() == 1
            && candidates.len() == compilation.candidate_count()
            && candidates.len() == receipt_entries.len()
            && candidates.len()
                == usize::from(
                    attempt
                        .planned_attempt()
                        .output_ceilings()
                        .candidate_count(),
                ),
        Relationship::CandidateCount,
    )?;
    for (ordinal, (candidate, receipt_entry)) in candidates.iter().zip(receipt_entries).enumerate()
    {
        let ordinal = u8::try_from(ordinal).map_err(|_| {
            VerifiedCandidateBatchError::Relationship(Relationship::CandidateOrdinal)
        })?;
        let compilation_entry = compilation.candidate_entry(ordinal).ok_or(
            VerifiedCandidateBatchError::Relationship(Relationship::CandidateOrdinal),
        )?;
        let retained = compilation.candidate_bytes(ordinal, cancellation)?;
        CandidateRelationshipChecks::new(
            candidate,
            receipt_entry,
            compilation_entry,
            &retained,
            ordinal,
        )
        .validate()?;
    }
    Ok(())
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "each independently rejected authority relationship remains explicit"
)]
struct FixedRelationshipChecks {
    planned_attempt: bool,
    generation_system: bool,
    generation_request: bool,
    structured_request: bool,
    precursor: bool,
    managed_evidence: bool,
    cleanup: bool,
    effective_package: bool,
    response: bool,
    response_usage: bool,
    attempt_record: bool,
    receipt_compilation: bool,
}

impl FixedRelationshipChecks {
    fn validate(&self) -> Result<(), VerifiedCandidateBatchError> {
        for (matches, relationship) in [
            (self.planned_attempt, Relationship::PlannedAttempt),
            (self.generation_system, Relationship::GenerationSystem),
            (self.generation_request, Relationship::GenerationRequest),
            (self.structured_request, Relationship::StructuredRequest),
            (self.precursor, Relationship::Precursor),
            (self.managed_evidence, Relationship::ManagedEvidence),
            (self.cleanup, Relationship::Cleanup),
            (self.effective_package, Relationship::EffectivePackage),
            (self.response, Relationship::Response),
            (self.response_usage, Relationship::ResponseUsage),
            (self.attempt_record, Relationship::AttemptRecord),
            (self.receipt_compilation, Relationship::ReceiptCompilation),
        ] {
            require_relationship(matches, relationship)?;
        }
        Ok(())
    }
}

struct CandidateRelationshipChecks {
    ordinal: bool,
    bytes: bool,
    artifact: bool,
}

impl CandidateRelationshipChecks {
    fn new(
        candidate: &GenerationCandidate,
        receipt_entry: &CandidateArtifactEntryV1,
        compilation_entry: &CandidateArtifactEntryV1,
        retained: &[u8],
        expected_ordinal: u8,
    ) -> Self {
        Self {
            ordinal: candidate.ordinal == expected_ordinal
                && receipt_entry.ordinal() == expected_ordinal
                && compilation_entry.ordinal() == expected_ordinal,
            bytes: candidate.text.as_bytes() == retained,
            // The app compiler authenticated the complete entry against retained bytes. The join
            // compares entries and exact bytes without adding an uncancellable second full hash.
            artifact: receipt_entry == compilation_entry
                && receipt_entry.byte_count() == retained.len() as u64,
        }
    }

    fn validate(&self) -> Result<(), VerifiedCandidateBatchError> {
        for (matches, relationship) in [
            (self.ordinal, Relationship::CandidateOrdinal),
            (self.bytes, Relationship::CandidateBytes),
            (self.artifact, Relationship::CandidateArtifact),
        ] {
            require_relationship(matches, relationship)?;
        }
        Ok(())
    }
}

fn require_relationship(
    matches: bool,
    relationship: Relationship,
) -> Result<(), VerifiedCandidateBatchError> {
    if matches {
        Ok(())
    } else {
        Err(VerifiedCandidateBatchError::Relationship(relationship))
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
