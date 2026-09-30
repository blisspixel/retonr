use rewrite_model::{
    CandidateGenerationAttemptRecordV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationLicenseEvidenceV1Relations,
    GenerationQualificationPlatformEvidenceV1Relations, GenerationQualificationRecordV1,
    GenerationQualificationRecordV1Relations, GenerationQualificationRequestProjectionV1Relations,
    GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultRecordV1Relations, GenerationResourceEvidenceManifestV1Relations,
};

use super::{Closure, Cohort, receipt_relations, scope};
use crate::store::generation_qualification_preregistration::tests::support::Fixture;

impl Cohort {
    pub(crate) fn qualification_record(
        &self,
        fixture: &Fixture,
        attempts: &[CandidateGenerationAttemptRecordV1],
    ) -> GenerationQualificationRecordV1 {
        let ledger_relations = GenerationAttemptLedgerManifestV1Relations {
            scope: scope(fixture),
            phase_policy_digest: fixture.policy.attempt_ledger_policy_digest(),
            planned_attempts: &fixture.attempts,
            attempt_records: attempts,
            status: self.ledger.status(),
        };
        let result_relations = self
            .results
            .iter()
            .map(|result| GenerationRepeatabilityResultRecordV1Relations {
                scope: scope(fixture),
                repetition: &fixture.repetitions[0],
                attempt_ledger: &self.ledger,
                attempt_ledger_relations: ledger_relations,
                terminal_stage: result.terminal_stage(),
                candidate_receipt_set: None,
                deterministic_evaluation: None,
                candidate_judge_join: None,
                terminal_evidence_digest: result.terminal_evidence_digest(),
            })
            .collect::<Vec<_>>();
        let closure = Closure {
            fixture,
            platform: &self.platform,
            license: &self.license,
            ledger: &self.ledger,
            repeatability: &self.repeatability,
            resource: &self.resource,
            human: &self.human,
        };
        GenerationQualificationRecordV1::new(&GenerationQualificationRecordV1Relations {
            operation_receipt: &self.receipt,
            operation_receipt_relations: receipt_relations(&closure),
            operation_receipt_input: self.receipt_input,
            operation_policy_relations: fixture.relations(),
            operation_policy_input: &fixture.policy_input,
            request_projection_relations: GenerationQualificationRequestProjectionV1Relations {
                operation_policy: &fixture.policy,
                qualification_plan: &fixture.plan,
                suite: &fixture.suite,
                planned_attempts: &fixture.attempts,
            },
            request_projection_entry_inputs: &fixture.entry_inputs,
            platform_evidence_relations: GenerationQualificationPlatformEvidenceV1Relations {
                operation_policy: &fixture.policy,
                request_projection: &fixture.projection,
                target_generation_system: &fixture.systems[0],
                target_generation_system_relations: fixture.system.relations(),
            },
            platform_evidence_input: self.platform_input,
            license_evidence_relations: GenerationQualificationLicenseEvidenceV1Relations {
                operation_policy: &fixture.policy,
                request_projection: &fixture.projection,
                target_generation_system: &fixture.systems[0],
                target_generation_system_relations: fixture.system.relations(),
                model_package_foundation_id: &self.foundation_id,
                model_license_control_id: &self.control_id,
            },
            license_evidence_input: self.license_input,
            attempt_ledger_relations: ledger_relations,
            repeatability_manifest_relations: GenerationRepeatabilityEvidenceManifestV1Relations {
                scope: scope(fixture),
                phase_policy_digest: fixture.policy.repeatability_policy_digest(),
                planned_attempts: &fixture.attempts,
                preregistered_repetitions: &fixture.repetitions,
                results: &self.results,
                status: self.repeatability.status(),
            },
            repeatability_result_relations: &result_relations,
            resource_manifest_relations: GenerationResourceEvidenceManifestV1Relations {
                scope: scope(fixture),
                phase_policy_digest: fixture.policy.resource_policy_digest(),
                evidence_record_digests: &self.resource_digests,
                status: self.resource.status(),
            },
            human_adjudication_manifest_relations:
                GenerationHumanAdjudicationEvidenceManifestV1Relations {
                    scope: scope(fixture),
                    phase_policy_digest: fixture.policy.human_adjudication_policy_digest(),
                    evidence_record_digests: &self.human_digests,
                    status: self.human.status(),
                },
        })
        .expect("qualification record")
    }
}
