use rewrite_model::{
    CandidateGenerationAttemptRecordV1, GenerationAttemptLedgerManifestV1,
    GenerationAttemptLedgerManifestV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1,
};

use super::super::CandidateGenerationExecutionV1Input;
use super::{failed_attempt, prereg_support, support};
use crate::StoreError;

#[test]
fn skipped_ledger_matches_the_constructor_and_does_not_write() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let store = support::prepared_store(&directory.path().join("skipped.db"), &fixture);
    let before = store.connection().total_changes();
    let manifest = store
        .rederive_attempt_ledger_manifest_v1(support::read_input(&fixture), &[])
        .expect("skipped ledger");
    assert_eq!(store.connection().total_changes(), before);
    assert_eq!(
        manifest,
        expected(&fixture, &[], GenerationQualificationPhaseStatusV1::Skipped)
    );
    assert!(matches!(
        store.rederive_attempt_ledger_manifest_v1(
            support::read_input(&fixture),
            std::slice::from_ref(&support::completed(&fixture).managed_input),
        ),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn failed_prefix_matches_the_constructor_and_does_not_write() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let mut store = support::prepared_store(&directory.path().join("failed.db"), &fixture);
    let attempt = failed_attempt(&fixture, None);
    store
        .transact_candidate_generation_execution_v1(
            CandidateGenerationExecutionV1Input::Failed {
                preregistration: support::read_input(&fixture),
                precursor: None,
                attempt: &attempt,
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("failed attempt");
    let before = store.connection().total_changes();
    let manifest = store
        .rederive_attempt_ledger_manifest_v1(support::read_input(&fixture), &[])
        .expect("failed ledger");
    assert_eq!(store.connection().total_changes(), before);
    assert_eq!(
        manifest,
        expected(
            &fixture,
            std::slice::from_ref(&attempt),
            GenerationQualificationPhaseStatusV1::Failed,
        )
    );
}

#[test]
fn completed_target_matches_the_constructor_and_does_not_write() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let completed = support::completed(&fixture);
    let mut store = support::prepared_store(&directory.path().join("completed.db"), &fixture);
    support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
    store
        .transact_candidate_generation_execution_v1(
            support::completed_input(&fixture, &completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("completed attempt");
    let before = store.connection().total_changes();
    let manifest = store
        .rederive_attempt_ledger_manifest_v1(
            support::read_input(&fixture),
            std::slice::from_ref(&completed.managed_input),
        )
        .expect("completed ledger");
    assert_eq!(store.connection().total_changes(), before);
    assert_eq!(
        manifest,
        expected(
            &fixture,
            std::slice::from_ref(&completed.attempt),
            GenerationQualificationPhaseStatusV1::Passed,
        )
    );
    assert_eq!(
        manifest.status(),
        GenerationQualificationPhaseStatusV1::Passed
    );
}

fn expected(
    fixture: &prereg_support::Fixture,
    records: &[CandidateGenerationAttemptRecordV1],
    status: GenerationQualificationPhaseStatusV1,
) -> GenerationAttemptLedgerManifestV1 {
    GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system: &fixture.systems[0],
            qualification_plan: &fixture.plan,
            suite: &fixture.suite,
        },
        phase_policy_digest: fixture.policy.attempt_ledger_policy_digest(),
        planned_attempts: &fixture.attempts,
        attempt_records: records,
        status,
    })
    .expect("model ledger")
}
