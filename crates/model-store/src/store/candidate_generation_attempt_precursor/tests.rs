use std::cell::Cell;

use rewrite_model::{
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationAttemptPrecursorV1Input,
};
use tempfile::tempdir;

use super::*;
use crate::store::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationV1Input,
    tests::support::{self, Fixture},
};

fn preregistration_input(fixture: &Fixture) -> GenerationQualificationPreregistrationV1Input<'_> {
    GenerationQualificationPreregistrationV1Input {
        operation_policy: &fixture.policy,
        operation_policy_relations: fixture.relations(),
        operation_policy_input: &fixture.policy_input,
        request_projection: &fixture.projection,
        request_projection_entry_inputs: &fixture.entry_inputs,
    }
}

fn read_input(fixture: &Fixture) -> GenerationQualificationPreregistrationReadInput<'_> {
    GenerationQualificationPreregistrationReadInput {
        operation_policy_id: fixture.policy.operation_policy_id(),
        request_projection_id: fixture.projection.request_projection_id(),
        operation_policy_relations: fixture.relations(),
        operation_policy_input: &fixture.policy_input,
        request_projection_entry_inputs: &fixture.entry_inputs,
    }
}

fn precursor(
    fixture: &Fixture,
    runtime_installation_generation: u64,
) -> CandidateGenerationAttemptPrecursorV1 {
    CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        &fixture.attempts[0],
        &fixture.systems[0],
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation,
            model_installation_generation: 1,
            structured_request_binding_id: fixture.entry_inputs[0]
                .structured_completion_request_binding_id
                .clone(),
        },
    )
    .expect("candidate precursor")
}

fn prepared_store(path: &std::path::Path, fixture: &Fixture) -> ArtifactStateStore {
    let mut store = ArtifactStateStore::open(path).expect("open store");
    support::persist_plan_foundation(&mut store, fixture);
    store
        .transact_generation_qualification_preregistration(
            preregistration_input(fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("persist preregistration");
    store
}

#[test]
fn checkpoint_inserts_cold_reads_and_reports_exact_replay() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store = prepared_store(&directory.path().join("checkpoint.db"), &fixture);
    let precursor = precursor(&fixture, 1);
    let gates = Cell::new(0);
    let (validated, first) = store
        .transact_candidate_generation_attempt_precursor_checkpoint_v1(
            CandidateGenerationAttemptPrecursorCheckpointV1Input {
                precursor: &precursor,
                preregistration: read_input(&fixture),
            },
            || {
                gates.set(gates.get() + 1);
                Ok::<_, ()>(())
            },
            |readback| {
                assert_eq!(readback.precursor(), &precursor);
                assert_eq!(
                    readback.preregistration().plan_foundation().plan(),
                    &fixture.plan,
                );
                Ok::<_, ()>("validated")
            },
        )
        .expect("insert checkpoint");
    assert_eq!(validated, "validated");
    assert_eq!(gates.get(), 3);
    assert_eq!(first.precursor, WriteDisposition::Inserted);
    assert_eq!(
        store
            .candidate_generation_attempt_precursor_checkpoint_v1(
                fixture.plan.qualification_plan_id(),
                fixture.attempts[0].planned_attempt_id(),
            )
            .expect("read checkpoint"),
        Some(precursor.clone()),
    );

    let ((), replay) = store
        .transact_candidate_generation_attempt_precursor_checkpoint_v1(
            CandidateGenerationAttemptPrecursorCheckpointV1Input {
                precursor: &precursor,
                preregistration: read_input(&fixture),
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("exact replay");
    assert_eq!(replay.precursor, WriteDisposition::AlreadyPresent);
}

#[test]
fn checkpoint_rejects_a_different_precursor_for_the_consumed_plan_slot() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store = prepared_store(&directory.path().join("conflict.db"), &fixture);
    let first = precursor(&fixture, 1);
    store
        .transact_candidate_generation_attempt_precursor_checkpoint_v1(
            CandidateGenerationAttemptPrecursorCheckpointV1Input {
                precursor: &first,
                preregistration: read_input(&fixture),
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("first checkpoint");
    let conflicting = precursor(&fixture, 2);
    assert!(matches!(
        store.transact_candidate_generation_attempt_precursor_checkpoint_v1(
            CandidateGenerationAttemptPrecursorCheckpointV1Input {
                precursor: &conflicting,
                preregistration: read_input(&fixture),
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        ),
        Err(
            CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
}

#[test]
fn gate_and_validation_rejection_roll_back_the_checkpoint() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store = prepared_store(&directory.path().join("rollback.db"), &fixture);
    let precursor = precursor(&fixture, 1);
    let gate_calls = Cell::new(0);
    let gate_rejection = store.transact_candidate_generation_attempt_precursor_checkpoint_v1(
        CandidateGenerationAttemptPrecursorCheckpointV1Input {
            precursor: &precursor,
            preregistration: read_input(&fixture),
        },
        || {
            gate_calls.set(gate_calls.get() + 1);
            if gate_calls.get() == 3 {
                Err("gate")
            } else {
                Ok(())
            }
        },
        |_| Ok(()),
    );
    assert!(matches!(
        gate_rejection,
        Err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Gate("gate"))
    ));
    assert!(
        store
            .candidate_generation_attempt_precursor_checkpoint_v1(
                fixture.plan.qualification_plan_id(),
                fixture.attempts[0].planned_attempt_id(),
            )
            .expect("read after gate rollback")
            .is_none()
    );

    let rejected = store.transact_candidate_generation_attempt_precursor_checkpoint_v1(
        CandidateGenerationAttemptPrecursorCheckpointV1Input {
            precursor: &precursor,
            preregistration: read_input(&fixture),
        },
        || Ok::<_, &'static str>(()),
        |_| Err::<(), _>("validation"),
    );
    assert!(matches!(
        rejected,
        Err(
            CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Validation(
                "validation"
            )
        )
    ));
    assert!(
        store
            .candidate_generation_attempt_precursor_checkpoint_v1(
                fixture.plan.qualification_plan_id(),
                fixture.attempts[0].planned_attempt_id(),
            )
            .expect("read after validation rollback")
            .is_none()
    );
}

#[test]
fn cold_read_rejects_corrupt_canonical_bytes() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store = prepared_store(&directory.path().join("corrupt.db"), &fixture);
    let precursor = precursor(&fixture, 1);
    store
        .transact_candidate_generation_attempt_precursor_checkpoint_v1(
            CandidateGenerationAttemptPrecursorCheckpointV1Input {
                precursor: &precursor,
                preregistration: read_input(&fixture),
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("checkpoint");
    store
        .connection()
        .execute(
            "UPDATE candidate_generation_attempt_precursors
             SET canonical_json = X'7b7d'",
            [],
        )
        .expect("corrupt precursor bytes");
    assert!(matches!(
        store.candidate_generation_attempt_precursor_checkpoint_v1(
            fixture.plan.qualification_plan_id(),
            fixture.attempts[0].planned_attempt_id(),
        ),
        Err(StoreError::CorruptRecord)
    ));
}
