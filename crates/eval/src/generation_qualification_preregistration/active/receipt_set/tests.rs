use super::*;
use crate::verified_candidate_batch_set::tests::support::{Scenario, scenario};

pub(crate) mod positive;

fn relations(fixture: &Scenario) -> CandidateGenerationReceiptSetV1Relations<'_> {
    CandidateGenerationReceiptSetV1Relations {
        qualification_plan: &fixture.input.qualification_plan,
        suite: &fixture.input.suite,
        repetition: &fixture.input.repetition,
        generation_system: &fixture.input.generation_system,
        selection_policy: &fixture.input.selection_policy,
        planned_attempts: &fixture.input.planned_attempts,
        attempt_records: &[],
        receipts: &[],
    }
}

#[test]
fn ledger_projection_uses_suite_order_and_exact_full_receipt_set() {
    let fixture = scenario("durable-ledger");
    let (mut records, mut receipts) = fixture.portable_attempts();
    let expected = CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
        attempt_records: &records,
        receipts: &receipts,
        ..relations(&fixture)
    })
    .expect("independent complete set");
    records.reverse();
    receipts.reverse();
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &records, &receipts).expect("suite ordering"),
        expected
    );
}

#[test]
fn ledger_projection_rejects_missing_duplicate_and_foreign_receipts_or_records() {
    let fixture = scenario("durable-ledger");
    let foreign = scenario("foreign-durable-ledger");
    let (records, receipts) = fixture.portable_attempts();
    let (foreign_records, foreign_receipts) = foreign.portable_attempts();
    let scope = Err(ActiveGenerationQualificationReceiptSetError::OperationScope);
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &records[..1], &receipts),
        scope
    );
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &records, &receipts[..1]),
        scope
    );
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &foreign_records, &receipts),
        scope
    );
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &records, &foreign_receipts),
        scope
    );
    let mut duplicates = receipts.clone();
    duplicates.push(receipts[0].clone());
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &records, &duplicates),
        scope
    );
    let mut duplicates = records.clone();
    duplicates.push(records[0].clone());
    assert_eq!(
        rederive_from_ledger(relations(&fixture), &duplicates, &receipts),
        scope
    );
}

#[test]
fn ledger_projection_cannot_change_the_selected_system() {
    let fixture = scenario("durable-ledger");
    let foreign = crate::verified_candidate_batch_set::tests::support::judge_system(
        "another-system",
        rewrite_types::Digest::sha256(b"foreign prompt"),
        rewrite_types::Digest::sha256(b"foreign output"),
    );
    let (records, receipts) = fixture.portable_attempts();
    let mut changed = relations(&fixture);
    changed.generation_system = &foreign;
    assert_eq!(
        rederive_from_ledger(changed, &records, &receipts),
        Err(ActiveGenerationQualificationReceiptSetError::OperationScope)
    );
}

#[test]
fn publication_finalization_never_masks_independent_failure() {
    use ActiveGenerationQualificationReceiptSetError as Error;
    assert_eq!(combine_finalization(Ok(7), false), Ok(7));
    assert_eq!(
        combine_finalization(Ok(7), true),
        Err(Error::MandatoryFinalization)
    );
    for error in [
        Error::NotReady,
        Error::OperationScope,
        Error::BatchAuthority,
        Error::Publication,
    ] {
        assert_eq!(combine_finalization::<()>(Err(error), false), Err(error));
        assert_eq!(
            combine_finalization::<()>(Err(error), true),
            Err(Error::PrimaryAndFinalization)
        );
        let display = error.to_string();
        assert!(!display.contains("durable-ledger"));
    }
}

#[test]
fn cancellation_and_deadline_remain_typed_and_finalization_is_independent() {
    use ActiveGenerationQualificationReceiptSetError as Error;
    use GenerationQualificationPreparationError as Preparation;
    assert_eq!(
        map_preparation_error(Preparation::Cancelled),
        Error::Cancelled
    );
    assert_eq!(
        map_preparation_error(Preparation::DeadlineExceeded),
        Error::DeadlineExceeded
    );
    assert_eq!(
        map_preparation_error(Preparation::ReadbackMismatch),
        Error::Publication
    );
    assert_eq!(
        combine_finalization::<()>(Err(Error::Cancelled), false),
        Err(Error::Cancelled)
    );
    assert_eq!(
        combine_finalization::<()>(Err(Error::DeadlineExceeded), true),
        Err(Error::PrimaryAndFinalization)
    );
}

#[test]
fn terminal_gate_has_precedence_after_failed_publication_and_retains_finalizer_failure() {
    use ActiveGenerationQualificationReceiptSetError as Error;
    for primary in [
        Error::Publication,
        Error::OperationScope,
        Error::BatchAuthority,
    ] {
        assert_eq!(
            finish_publication::<()>(Err(primary), false, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineExceeded)
        );
        assert_eq!(
            finish_publication::<()>(Err(primary), true, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineAndFinalization)
        );
        assert_eq!(
            finish_publication::<()>(Err(primary), false, Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
        assert_eq!(
            finish_publication::<()>(Err(primary), true, Err(Error::Cancelled)),
            Err(Error::CancelledAndFinalization)
        );
    }
    assert_eq!(finish_publication(Ok(7), false, Ok(())), Ok(7));
    assert_eq!(
        finish_publication::<()>(Err(Error::Publication), true, Ok(())),
        Err(Error::PrimaryAndFinalization)
    );
}

#[test]
fn active_publication_requires_a_sealed_ledger_and_same_process_subject() {
    use crate::GenerationQualificationOperationDraft;
    use rewrite_app::{
        GenerationQualificationLicenseAssessmentCompiler, SyntheticGenerationQualificationScenario,
        with_synthetic_generation_qualification_fixture,
    };
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            let cancellation = CancellationToken::new();
            let projected = GenerationQualificationOperationDraft::begin(
                input.operation_policy_relations,
                input.operation_policy_input,
                &cancellation,
            )
            .expect("draft")
            .project(input.case_authorities, &cancellation)
            .expect("projection");
            let platform = platform_owners
                .assess(projected.platform_portable_relations(), &cancellation)
                .expect("platform");
            let assessment_input = projected.license_assessment_input(
                &license_policy,
                license_proof.control(),
                license_proof.selected_lease(),
                &production_policy,
            );
            let license = GenerationQualificationLicenseAssessmentCompiler::compile(
                &assessment_input,
                &cancellation,
            )
            .expect("license");
            let directory = tempfile::tempdir().expect("temporary repository");
            let mut repository = GenerationQualificationPreregistrationRepository::open(
                &directory.path().join("qualification.db"),
            )
            .expect("repository");
            let prepared = projected
                .finish(
                    &mut repository,
                    input.foundation,
                    platform,
                    license,
                    license_policy,
                    production_policy,
                    &cancellation,
                )
                .expect("prepared");
            let evidence =
                rewrite_app::CandidateGenerationEvidenceRepository::initialize(directory.path())
                    .expect("evidence root");
            let mut active = prepared
                .activate(&repository, &evidence, &cancellation)
                .expect("active");
            let fixture = scenario("durable-ledger");
            let ledger = offline_ledger(&fixture, &active);
            let set = fixture.into_offline_set();
            assert_eq!(
                active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                Err(ActiveGenerationQualificationReceiptSetError::NotReady)
            );
            assert!(!active.terminal);
            active.attempt_ledger = Some(ledger);
            assert_eq!(
                active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                Err(ActiveGenerationQualificationReceiptSetError::OperationScope)
            );
            assert!(active.terminal);
            assert_eq!(
                active.persist_target_receipt_set(&mut repository, &set, &cancellation),
                Err(ActiveGenerationQualificationReceiptSetError::NotReady)
            );
            let connection = rusqlite::Connection::open(directory.path().join("qualification.db"))
                .expect("inspect store");
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

fn offline_ledger(
    fixture: &Scenario,
    active: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
) -> super::super::attempt_ledger::ActiveGenerationQualificationAttemptLedgerClosure {
    use rewrite_model::{
        GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
        GenerationQualificationPhaseScopeV1,
    };
    let (records, receipts) = fixture.portable_attempts();
    let manifest =
        GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: &fixture.input.generation_system,
                qualification_plan: &fixture.input.qualification_plan,
                suite: &fixture.input.suite,
            },
            phase_policy_digest: active.operation_policy().attempt_ledger_policy_digest(),
            planned_attempts: &fixture.input.planned_attempts,
            attempt_records: &records,
            status: GenerationQualificationPhaseStatusV1::Passed,
        })
        .expect("synthetic Passed ledger");
    super::super::attempt_ledger::ActiveGenerationQualificationAttemptLedgerClosure::new(
        active.subject.binding(),
        manifest,
        records,
        receipts,
    )
}

#[test]
fn durable_publication_rejects_cancellation_deadline_and_missing_parents_without_rows() {
    use crate::generation_qualification_preregistration::GenerationQualificationPreparationError as Error;
    let fixture = scenario("durable-ledger");
    let (records, receipts) = fixture.portable_attempts();
    let record = rederive_from_ledger(relations(&fixture), &records, &receipts)
        .expect("complete ledger set");
    let directory = tempfile::tempdir().expect("temporary repository");
    let path = directory.path().join("state.db");
    let mut repository =
        GenerationQualificationPreregistrationRepository::open(&path).expect("repository");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let later = std::time::Instant::now() + std::time::Duration::from_mins(1);
    assert_eq!(
        repository.persist_receipt_set(&record, later, &cancelled),
        Err(Error::Cancelled)
    );
    assert_eq!(
        repository.persist_receipt_set(
            &record,
            std::time::Instant::now(),
            &CancellationToken::new()
        ),
        Err(Error::DeadlineExceeded)
    );
    assert!(
        repository
            .persist_receipt_set(&record, later, &CancellationToken::new())
            .is_err()
    );
    let connection = rusqlite::Connection::open(&path).expect("inspect store");
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM candidate_generation_receipt_sets",
            [],
            |row| row.get(0),
        )
        .expect("count rows");
    assert_eq!(count, 0);
}
