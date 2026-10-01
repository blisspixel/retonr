//! Exact synthetic live subjects, real retained material, and real durable cohorts.

use rewrite_app::{
    GenerationQualificationLicenseAssessmentCompiler, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};
use rewrite_model::{
    CandidateDeterministicEvaluationStatusV1, GenerationQualificationOperationPolicyV1Relations,
};
use rewrite_model_store::{
    ArtifactStateStore, CandidateDeterministicEvaluationV1ReadInput,
    GenerationQualificationPlanFoundationV1Input,
};

use super::*;
use crate::GenerationQualificationOperationDraft;
use crate::generation_qualification_preregistration::active::receipt_set::tests::positive::{
    prepare_active, seed_completed_parents,
};
use crate::verified_candidate_batch_set::tests::{
    active_subject::bound_synthetic_set,
    support::{Scenario, prepared_scenario_with_suffix},
};

mod adversarial;
pub(crate) mod material;
use material::MaterialFixture;

pub(crate) struct SyntheticPair {
    target: Scenario,
    baseline: Scenario,
}

impl SyntheticPair {
    pub(crate) fn new(
        foundation: GenerationQualificationPlanFoundationV1Input<'_>,
        relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        active: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
        suffix: &str,
    ) -> Self {
        let target = relations.target_system;
        let baseline = relations.baseline_system;
        Self {
            target: prepared_scenario_with_suffix(
                foundation,
                target.generation_system,
                target.relations.effective_package_evidence_v2,
                active.request_projection(),
                suffix,
            ),
            baseline: prepared_scenario_with_suffix(
                foundation,
                baseline.generation_system,
                baseline.relations.effective_package_evidence_v2,
                active.request_projection(),
                suffix,
            ),
        }
    }

    pub(crate) fn target_failure_control(
        &self,
    ) -> crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl {
        self.target.batches[0].failure_control()
    }

    pub(crate) fn bind(
        self,
        active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
        directory: &std::path::Path,
        evidence: &rewrite_app::CandidateGenerationEvidenceRepository,
        cancellation: &CancellationToken,
        seed: bool,
    ) -> (VerifiedCandidateBatchSet, VerifiedCandidateBatchSet) {
        if seed {
            seed_completed_parents(
                active,
                &self.target,
                directory,
                evidence.root_id(),
                cancellation,
            );
            seed_completed_parents(
                active,
                &self.baseline,
                directory,
                evidence.root_id(),
                cancellation,
            );
        }
        let (records, receipts) = self.target.portable_attempts();
        active.next_candidate_attempt = self.target.input.planned_attempts.len();
        active.target_attempt_records = records;
        active.target_attempt_receipts = receipts;
        active
            .seal_target_attempt_ledger()
            .expect("real target ledger sealing over synthetic complete attempts");
        (
            bound_synthetic_set(self.target, &active.subject),
            bound_synthetic_set(self.baseline, &active.subject),
        )
    }
}

#[test]
fn passed_and_failed_results_persist_replay_and_cold_read_back_exactly() {
    for passed in [false, true] {
        with_synthetic_generation_qualification_fixture(
            SyntheticGenerationQualificationScenario::TrafficEligible,
            |input, platform_owners, license_proof, license_policy, production_policy| {
                let source = input.case_authorities[0]
                    .source
                    .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                    .expect("exact source");
                let material_fixture =
                    MaterialFixture::new(input.foundation.plan_foundation, &source);
                let material = material_fixture.verify();
                prepare_active!(
                    input,
                    platform_owners,
                    license_proof,
                    license_policy,
                    production_policy,
                    directory,
                    repository,
                    active,
                    cancellation,
                    evidence
                );
                let suffix = if passed {
                    "qualification-closure-passing"
                } else {
                    "qualification-closure"
                };
                let pair = SyntheticPair::new(
                    input.foundation.plan_foundation,
                    input.operation_policy_relations,
                    &active,
                    suffix,
                );
                let (target, baseline) = pair.bind(
                    &mut active,
                    directory.path(),
                    &evidence,
                    &cancellation,
                    true,
                );
                let (record, disposition) = active
                    .persist_candidate_deterministic_evaluation(
                        &mut repository,
                        &target,
                        &baseline,
                        &material,
                        &cancellation,
                    )
                    .expect("settled result");
                assert_eq!(disposition, WriteDisposition::Inserted);
                assert_eq!(
                    record.status(),
                    if passed {
                        CandidateDeterministicEvaluationStatusV1::Passed
                    } else {
                        CandidateDeterministicEvaluationStatusV1::Failed
                    }
                );
                assert_eq!(
                    active
                        .persist_candidate_deterministic_evaluation(
                            &mut repository,
                            &target,
                            &baseline,
                            &material,
                            &cancellation
                        )
                        .expect("replay"),
                    (record.clone(), WriteDisposition::AlreadyPresent)
                );
                assert!(!active.terminal);
                let stored = ArtifactStateStore::open_existing_read_only(
                    &directory.path().join("qualification.db"),
                )
                .expect("cold store");
                assert_eq!(
                    stored
                        .candidate_deterministic_evaluation_v1(
                            CandidateDeterministicEvaluationV1ReadInput { record: &record }
                        )
                        .expect("cold exact result"),
                    Some(record)
                );
                assert_eq!(
                    row_count(directory.path(), "candidate_generation_receipt_sets"),
                    2
                );
                assert_eq!(
                    row_count(
                        directory.path(),
                        "candidate_deterministic_evaluation_records"
                    ),
                    1
                );
            },
        );
    }
}

#[test]
fn swapped_candidate_systems_and_missing_terminal_parents_cannot_settle() {
    for swapped in [false, true] {
        with_synthetic_generation_qualification_fixture(
            SyntheticGenerationQualificationScenario::TrafficEligible,
            |input, platform_owners, license_proof, license_policy, production_policy| {
                let source = input.case_authorities[0]
                    .source
                    .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                    .expect("exact source");
                let fixture = MaterialFixture::new(input.foundation.plan_foundation, &source);
                let material = fixture.verify();
                prepare_active!(
                    input,
                    platform_owners,
                    license_proof,
                    license_policy,
                    production_policy,
                    directory,
                    repository,
                    active,
                    cancellation,
                    evidence
                );
                let pair = SyntheticPair::new(
                    input.foundation.plan_foundation,
                    input.operation_policy_relations,
                    &active,
                    "qualification-closure-passing",
                );
                let (target, baseline) = pair.bind(
                    &mut active,
                    directory.path(),
                    &evidence,
                    &cancellation,
                    false,
                );
                let (target, baseline) = if swapped {
                    (&baseline, &target)
                } else {
                    (&target, &baseline)
                };
                assert_eq!(
                    active
                        .persist_candidate_deterministic_evaluation(
                            &mut repository,
                            target,
                            baseline,
                            &material,
                            &cancellation
                        )
                        .expect_err("settlement must fail closed"),
                    if swapped {
                        ActiveGenerationQualificationDeterministicSettlementError::OperationScope
                    } else {
                        ActiveGenerationQualificationDeterministicSettlementError::Publication
                    }
                );
                assert!(active.terminal);
                assert_eq!(
                    row_count(directory.path(), "candidate_generation_receipt_sets"),
                    0
                );
                assert_eq!(
                    row_count(
                        directory.path(),
                        "candidate_deterministic_evaluation_records"
                    ),
                    0
                );
            },
        );
    }
}

fn row_count(directory: &std::path::Path, table: &str) -> i64 {
    let connection =
        rusqlite::Connection::open(directory.join("qualification.db")).expect("inspect inert rows");
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("bounded fixture count")
}

#[test]
fn final_deadline_and_cancellation_precedence_retains_independent_finalization_failure() {
    use ActiveGenerationQualificationDeterministicSettlementError as Error;
    for primary in [
        Error::OperationScope,
        Error::Compilation,
        Error::Publication,
    ] {
        assert_eq!(
            finish_settlement::<()>(Err(primary), false, Ok(())),
            Err(primary)
        );
        assert_eq!(
            finish_settlement::<()>(Err(primary), true, Ok(())),
            Err(Error::PrimaryAndFinalization)
        );
        assert_eq!(
            finish_settlement::<()>(Err(primary), true, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineAndFinalization)
        );
        assert_eq!(
            finish_settlement::<()>(Err(primary), false, Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
        assert_eq!(
            finish_settlement::<()>(Err(primary), true, Err(Error::Cancelled)),
            Err(Error::CancelledAndFinalization)
        );
    }
    assert_eq!(
        finish_settlement(Ok(7), true, Ok(())),
        Err(Error::MandatoryFinalization)
    );
    assert_eq!(finish_settlement(Ok(7), false, Ok(())), Ok(7));
}

#[test]
fn settlement_error_debug_and_display_are_content_free() {
    let error = ActiveGenerationQualificationDeterministicSettlementError::OperationScope;
    assert!(!format!("{error:?} {error}").contains("Retain Acme"));
    assert_eq!(
        map_preparation_error(GenerationQualificationPreparationError::Cancelled),
        ActiveGenerationQualificationDeterministicSettlementError::Cancelled
    );
    assert_eq!(
        map_preparation_error(GenerationQualificationPreparationError::DeadlineExceeded),
        ActiveGenerationQualificationDeterministicSettlementError::DeadlineExceeded
    );
}
