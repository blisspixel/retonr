use super::*;
use std::sync::Arc;

fn open_attempt(
    fixture: &Fixture,
    cursor: u32,
    attempt_count: u32,
) -> (
    ManagedJudgeObservationSequence,
    ManagedJudgeAttemptObservation,
    StructuredCompletionRequest,
    OllamaResidentSessionExecutionReceipt,
) {
    let mut sequence = fixture.sequence(attempt_count);
    start_and_preflight(&mut sequence, fixture);
    drive_attempt(&mut sequence, cursor, fixture);
    let observation = complete_observation(&mut sequence, cursor, fixture);
    let request = fixture.structured_request();
    let receipt = Fixture::receipt(cursor, &request);
    (sequence, observation, request, receipt)
}

fn expect_receipt_rejected(
    fixture: &Fixture,
    receipt: OllamaResidentSessionExecutionReceipt,
    expected: &ManagedJudgeObservationError,
) {
    let (mut sequence, observation, request, original) = open_attempt(fixture, 0, 1);
    let state = FakeState::new(&observation, &request, &original, "state");
    let error = sequence
        .seal_attempt_with_subject(
            observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        )
        .expect_err("substituted receipt");
    assert_eq!(
        std::mem::discriminant(&error),
        std::mem::discriminant(expected)
    );
    assert!(matches!(
        sequence.finish(&CancellationToken::new()),
        Err(ManagedJudgeObservationError::IncompleteSequence)
    ));
}

#[test]
fn request_response_and_complete_receipt_substitution_fail_closed() {
    let fixture = Fixture::new("process");
    let (mut sequence, observation, mut request, receipt) = open_attempt(&fixture, 0, 1);
    let state = FakeState::new(&observation, &request, &receipt, "state");
    request.input.push_str(" substituted");
    assert!(matches!(
        sequence.seal_attempt_with_subject(
            observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::AttemptClosureMismatch)
    ));

    let (_, _, request, _) = open_attempt(&fixture, 0, 1);
    let running = running_model();
    let preflight = preflight(&running);
    let changed_response = Fixture::receipt_from(
        &request,
        &preflight,
        &running,
        r#"{"candidates":[{"text":"different"}]}"#,
        [8, 16, 12, 16],
    );
    expect_receipt_rejected(
        &fixture,
        changed_response,
        &ManagedJudgeObservationError::EffectiveStateMismatch,
    );

    let mut changed_running = running_model();
    changed_running.context_tokens += 1;
    let changed_receipt = Fixture::receipt_from(
        &request,
        &preflight,
        &changed_running,
        r#"{"candidates":[{"text":"ok"}]}"#,
        [8, 16, 12, 16],
    );
    expect_receipt_rejected(
        &fixture,
        changed_receipt,
        &ManagedJudgeObservationError::EffectiveStateMismatch,
    );
}

#[test]
fn every_execution_and_residency_ordinal_is_checked_independently() {
    let fixture = Fixture::new("process");
    let request = fixture.structured_request();
    let running = running_model();
    let preflight = preflight(&running);
    for ordinals in [
        [9, 16, 12, 16],
        [8, 15, 12, 16],
        [8, 16, 13, 16],
        [8, 16, 12, 15],
        [8, 17, 12, 17],
    ] {
        let receipt = Fixture::receipt_from(
            &request,
            &preflight,
            &running,
            r#"{"candidates":[{"text":"ok"}]}"#,
            ordinals,
        );
        expect_receipt_rejected(
            &fixture,
            receipt,
            &ManagedJudgeObservationError::AttemptClosureMismatch,
        );
    }
}

#[test]
fn shared_preflight_and_exact_state_subject_are_mandatory() {
    let fixture = Fixture::new("process");
    let mut sequence = fixture.sequence(2);
    start_and_preflight(&mut sequence, &fixture);

    drive_attempt(&mut sequence, 0, &fixture);
    let first_observation = complete_observation(&mut sequence, 0, &fixture);
    let first_request = fixture.structured_request();
    let first_receipt = Fixture::receipt(0, &first_request);
    let first_state = FakeState::new(
        &first_observation,
        &first_request,
        &first_receipt,
        "first-state",
    );
    sequence
        .seal_attempt_with_subject(
            first_observation,
            first_request,
            first_receipt,
            first_state,
            &CancellationToken::new(),
        )
        .expect("first seal");

    drive_attempt(&mut sequence, 1, &fixture);
    let second_observation = complete_observation(&mut sequence, 1, &fixture);
    let second_request = fixture.structured_request();
    let running = running_model();
    let mut changed_preflight = preflight(&running);
    changed_preflight.runtime.version = "0.32.16".to_owned();
    let changed_receipt = Fixture::receipt_from(
        &second_request,
        &changed_preflight,
        &running,
        r#"{"candidates":[{"text":"ok"}]}"#,
        [17, 25, 21, 25],
    );
    let changed_state = FakeState::new(
        &second_observation,
        &second_request,
        &changed_receipt,
        "second-state",
    );
    assert!(matches!(
        sequence.seal_attempt_with_subject(
            second_observation,
            second_request,
            changed_receipt,
            changed_state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::AttemptClosureMismatch)
    ));

    let (mut sequence, observation, request, receipt) = open_attempt(&fixture, 0, 1);
    let mut changed_state = FakeState::new(&observation, &request, &receipt, "state");
    changed_state.response = digest("substituted-state-response");
    assert!(matches!(
        sequence.seal_attempt_with_subject(
            observation,
            request,
            receipt,
            changed_state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::EffectiveStateMismatch)
    ));
}

#[test]
fn cursor_sequence_token_and_cancellation_are_terminal_at_seal() {
    let fixture = Fixture::new("process");
    let (mut sequence, mut observation, request, receipt) = open_attempt(&fixture, 0, 1);
    let state = FakeState::new(&observation, &request, &receipt, "state");
    observation.schedule_cursor = 1;
    assert!(matches!(
        sequence.seal_attempt_with_subject(
            observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));

    let (mut sequence, mut observation, request, receipt) = open_attempt(&fixture, 0, 1);
    let state = FakeState::new(&observation, &request, &receipt, "state");
    observation.sequence_token = Arc::new(());
    assert!(matches!(
        sequence.seal_attempt_with_subject(
            observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));

    let (mut sequence, observation, request, receipt) = open_attempt(&fixture, 0, 1);
    let state = FakeState::new(&observation, &request, &receipt, "state");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        sequence.seal_attempt_with_subject(observation, request, receipt, state, &cancelled,),
        Err(ManagedJudgeObservationError::Cancelled)
    ));
    assert!(matches!(
        sequence.finish(&CancellationToken::new()),
        Err(ManagedJudgeObservationError::IncompleteSequence)
    ));
}
