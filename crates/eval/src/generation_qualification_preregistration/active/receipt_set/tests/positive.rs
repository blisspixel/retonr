//! Synthetic same-subject batches exercise durable publication, not managed traffic.

use super::*;
use crate::GenerationQualificationOperationDraft;
use crate::verified_candidate_batch_set::tests::{
    active_subject::bound_synthetic_set, support::prepared_scenario,
};
use rewrite_app::{
    GenerationQualificationLicenseAssessmentCompiler, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};
use rewrite_model_store::{ArtifactStateStore, GenerationQualificationPreregistrationReadInput};

macro_rules! prepare_active {
    ($input:ident, $platform_owners:ident, $license_proof:ident, $license_policy:ident,
     $production_policy:ident, $directory:ident, $repository:ident, $active:ident,
     $cancellation:ident, $evidence:ident) => {
        // Provision fixture dependencies before capturing the operation clock.
        let $directory = tempfile::tempdir().expect("temporary repository");
        let mut $repository = GenerationQualificationPreregistrationRepository::open(
            &$directory.path().join("qualification.db"),
        )
        .expect("repository");
        let $evidence =
            rewrite_app::CandidateGenerationEvidenceRepository::initialize($directory.path())
                .expect("evidence root");
        let $cancellation = CancellationToken::new();
        let projected = GenerationQualificationOperationDraft::begin(
            $input.operation_policy_relations,
            $input.operation_policy_input,
            &$cancellation,
        )
        .expect("draft")
        .project($input.case_authorities, &$cancellation)
        .expect("projection");
        let platform = $platform_owners
            .assess(projected.platform_portable_relations(), &$cancellation)
            .expect("platform");
        let assessment_input = projected.license_assessment_input(
            &$license_policy,
            $license_proof.control(),
            $license_proof.selected_lease(),
            &$production_policy,
        );
        let license = GenerationQualificationLicenseAssessmentCompiler::compile(
            &assessment_input,
            &$cancellation,
        )
        .expect("license");
        let prepared = projected
            .finish(
                &mut $repository,
                $input.foundation,
                platform,
                license,
                $license_policy,
                $production_policy,
                &$cancellation,
            )
            .expect("prepared");
        let mut $active = prepared
            .activate(&$repository, &$evidence, &$cancellation)
            .expect("active");
    };
}
pub(crate) use prepare_active;

#[test]
fn same_subject_exact_ledger_publishes_reads_back_and_replays_without_duplicate_rows() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
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
            let target = input.operation_policy_relations.target_system;
            let fixture = prepared_scenario(
                input.foundation.plan_foundation,
                target.generation_system,
                target.relations.effective_package_evidence_v2,
                active.request_projection(),
            );
            let (records, receipts) = fixture.portable_attempts();
            seed_completed_parents(
                &mut active,
                &fixture,
                directory.path(),
                evidence.root_id(),
                &cancellation,
            );
            active.next_candidate_attempt = fixture.input.planned_attempts.len();
            active.target_attempt_records = records;
            active.target_attempt_receipts = receipts;
            active
                .seal_target_attempt_ledger()
                .expect("real ledger sealing over synthetic exact records");
            let set = bound_synthetic_set(fixture, &active.subject);
            assert_eq!(
                active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                Ok(WriteDisposition::Inserted)
            );
            assert_eq!(
                active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                Ok(WriteDisposition::AlreadyPresent)
            );
            assert!(!active.terminal);
            assert_eq!(active.peak_live_authorities(), 0);
            let stored = ArtifactStateStore::open_existing_read_only(
                &directory.path().join("qualification.db"),
            )
            .expect("cold read-only store");
            let expected = set
                .receipt_set(&cancellation)
                .expect("current synthetic batch record");
            assert_eq!(
                stored
                    .candidate_generation_receipt_set_v1(
                        rewrite_model_store::CandidateGenerationReceiptSetV1ReadInput {
                            record: expected
                        }
                    )
                    .expect("cold canonical readback")
                    .as_ref(),
                Some(expected)
            );
            let connection = rusqlite::Connection::open(directory.path().join("qualification.db"))
                .expect("inspect rows");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM candidate_generation_receipt_sets",
                    [],
                    |row| row.get(0),
                )
                .expect("receipt-set rows");
            assert_eq!(count, 1);
            connection
                .execute(
                    "UPDATE candidate_generation_receipt_sets SET canonical_json = ?1",
                    [b"{}".as_slice()],
                )
                .expect("corrupt inert fixture row");
            assert_eq!(
                active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                Err(ActiveGenerationQualificationReceiptSetError::Publication)
            );
            assert!(active.terminal);
        },
    );
}

pub(crate) fn seed_completed_parents(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    fixture: &Scenario,
    directory: &std::path::Path,
    storage_root: &rewrite_model_store::CandidateGenerationEvidenceStorageRootId,
    cancellation: &CancellationToken,
) {
    let mut store =
        ArtifactStateStore::open_existing_writable_exact(&directory.join("qualification.db"))
            .expect("existing exact fixture store");
    active
        .prepared
        .with_validated_view(cancellation, |view| {
            let preregistration = GenerationQualificationPreregistrationReadInput {
                operation_policy_id: view.operation_policy.operation_policy_id(),
                request_projection_id: view.request_projection.request_projection_id(),
                operation_policy_relations: view.operation_policy_relations,
                operation_policy_input: view.operation_policy_input,
                request_projection_entry_inputs: view.request_projection_entry_inputs,
            };
            for batch in &fixture.batches {
                batch.persist_synthetic_completed(&mut store, preregistration, storage_root);
            }
            Ok::<_, ()>(())
        })
        .expect("synthetic complete parent cohorts under Prepared validation");
}

#[test]
fn same_subject_baseline_and_missing_terminal_parents_cannot_publish() {
    for baseline in [false, true] {
        with_synthetic_generation_qualification_fixture(
            SyntheticGenerationQualificationScenario::TrafficEligible,
            |input, platform_owners, license_proof, license_policy, production_policy| {
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
                let target = input.operation_policy_relations.target_system;
                let fixture = prepared_scenario(
                    input.foundation.plan_foundation,
                    target.generation_system,
                    target.relations.effective_package_evidence_v2,
                    active.request_projection(),
                );
                let (records, receipts) = fixture.portable_attempts();
                active.next_candidate_attempt = fixture.input.planned_attempts.len();
                active.target_attempt_records = records;
                active.target_attempt_receipts = receipts;
                active
                    .seal_target_attempt_ledger()
                    .expect("synthetic target ledger");
                let fixture = if baseline {
                    let other = input.operation_policy_relations.baseline_system;
                    prepared_scenario(
                        input.foundation.plan_foundation,
                        other.generation_system,
                        other.relations.effective_package_evidence_v2,
                        active.request_projection(),
                    )
                } else {
                    fixture
                };
                let set = bound_synthetic_set(fixture, &active.subject);
                let expected = if baseline {
                    ActiveGenerationQualificationReceiptSetError::OperationScope
                } else {
                    ActiveGenerationQualificationReceiptSetError::Publication
                };
                assert_eq!(
                    active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                    Err(expected)
                );
                assert!(active.terminal);
                let connection =
                    rusqlite::Connection::open(directory.path().join("qualification.db"))
                        .expect("inspect rows");
                let count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM candidate_generation_receipt_sets",
                        [],
                        |row| row.get(0),
                    )
                    .expect("count rows");
                assert_eq!(count, 0);
            },
        );
    }
}
