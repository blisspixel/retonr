use super::*;

#[test]
fn complete_sequence_retains_every_exact_span_in_schedule_order() {
    let fixture = Fixture::new("process");
    let mut sequence = fixture.sequence(2);
    start_and_preflight(&mut sequence, &fixture);
    let expected_preflight = expected_preflight_digest(&fixture.process);

    for cursor in 0..2 {
        drive_attempt(&mut sequence, cursor, &fixture);
        let observation = complete_observation(&mut sequence, cursor, &fixture);
        let request = fixture.structured_request();
        let receipt = Fixture::receipt(cursor, &request);
        let state = FakeState::new(&observation, &request, &receipt, "state");
        sequence
            .seal_attempt_with_subject(
                observation,
                request,
                receipt,
                state,
                &CancellationToken::new(),
            )
            .expect("seal attempt");
    }

    let completed = sequence
        .finish(&CancellationToken::new())
        .expect("finish sequence");
    assert_eq!(completed.bindings().len(), 2);
    assert_eq!(
        completed.preflight_connection_binding_digest(),
        &expected_preflight
    );
    assert_eq!(completed.bindings()[0].schedule_cursor(), 0);
    assert_eq!(completed.bindings()[0].first_response_ordinal(), 8);
    assert_eq!(completed.bindings()[0].last_response_ordinal(), 16);
    assert_eq!(completed.bindings()[1].first_response_ordinal(), 17);
    assert_eq!(completed.bindings()[1].last_response_ordinal(), 25);
    assert_eq!(completed.bindings()[0].first_residency_ordinal(), 12);
    assert_eq!(completed.bindings()[0].last_residency_ordinal(), 16);
    assert_eq!(completed.bindings()[1].first_residency_ordinal(), 21);
    assert_eq!(completed.bindings()[1].last_residency_ordinal(), 25);
    assert_eq!(
        completed.bindings()[0].process_binding_digest(),
        fixture.process.evidence_digest()
    );
    assert_eq!(
        completed.bindings()[0].native_load_binding_digest(),
        fixture.native_load.native_load_observation_id().digest()
    );
    assert_ne!(
        completed.bindings()[0].connection_binding_digest(),
        completed.preflight_connection_binding_digest()
    );
    assert_eq!(
        completed.bindings()[0].request_binding_digest(),
        completed.bindings()[1].request_binding_digest()
    );
    assert_eq!(
        completed.bindings()[0].retained_response_id(),
        completed.bindings()[1].retained_response_id()
    );
    assert_eq!(
        completed.bindings()[0].retained_preflight_digest(),
        completed.bindings()[1].retained_preflight_digest()
    );
    assert_ne!(
        completed.bindings()[0].complete_receipt_binding_digest(),
        completed.bindings()[1].complete_receipt_binding_digest()
    );
}

#[test]
fn reordered_missing_and_extra_response_spans_fail_closed() {
    let fixture = Fixture::new("process");
    let mut reordered = fixture.sequence(1);
    begin(&mut reordered);
    assert!(matches!(
        observe(
            &mut reordered,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 2 }
        ),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));

    let mut missing_preflight = fixture.sequence(1);
    begin(&mut missing_preflight);
    drive(&mut missing_preflight, 1, 6);
    assert!(matches!(
        missing_preflight
            .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new(),),
        Err(ManagedJudgeObservationError::InvalidPreflightSpan)
    ));

    let mut missing_attempt = fixture.sequence(1);
    start_and_preflight(&mut missing_attempt, &fixture);
    drive(&mut missing_attempt, 8, 11);
    begin_worker(&mut missing_attempt, 0, &fixture);
    drive(&mut missing_attempt, 12, 15);
    assert!(matches!(
        complete_observation_result(&mut missing_attempt, 0, &fixture),
        Err(ManagedJudgeObservationError::InvalidAttemptSpan)
    ));

    let mut extra = fixture.sequence(1);
    start_and_preflight(&mut extra, &fixture);
    drive_attempt(&mut extra, 0, &fixture);
    assert!(matches!(
        observe(
            &mut extra,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 17 }
        ),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));
}

#[test]
fn wrong_cursor_response_count_drift_and_failed_attempt_are_rejected() {
    let fixture = Fixture::new("process");
    let mut wrong_cursor = fixture.sequence(1);
    start_and_preflight(&mut wrong_cursor, &fixture);
    drive_attempt(&mut wrong_cursor, 0, &fixture);
    assert!(matches!(
        complete_observation_result(&mut wrong_cursor, 1, &fixture),
        Err(ManagedJudgeObservationError::InvalidScheduleCursor)
    ));

    let mut count_drift = fixture.sequence(1);
    begin(&mut count_drift);
    drive(&mut count_drift, 1, 2);
    assert!(matches!(
        observe(
            &mut count_drift,
            OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses: 1,
            },
        ),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));

    let mut failed = fixture.sequence(1);
    begin(&mut failed);
    drive(&mut failed, 1, 2);
    assert!(matches!(
        observe(
            &mut failed,
            OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses: 2,
            },
        ),
        Err(ManagedJudgeObservationError::FailedResponseAttempt)
    ));
}

#[test]
fn worker_protocol_rejects_wrong_phase_skip_cancellation_and_incomplete_close() {
    let fixture = Fixture::new("process");

    let mut early = fixture.sequence(1);
    start_and_preflight(&mut early, &fixture);
    assert!(matches!(
        early.begin_attempt_worker_observation(
            0,
            &fixture.worker_request(),
            &fixture.worker_native_request(),
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::InvalidWorkerObservationPhase)
    ));

    let mut skipped = fixture.sequence(1);
    start_and_preflight(&mut skipped, &fixture);
    drive(&mut skipped, 8, 11);
    assert!(matches!(
        observe(
            &mut skipped,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 12 },
        ),
        Err(ManagedJudgeObservationError::IncompleteWorkerObservation)
    ));

    let mut cancelled = fixture.sequence(1);
    start_and_preflight(&mut cancelled, &fixture);
    drive(&mut cancelled, 8, 11);
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        cancelled.begin_attempt_worker_observation(
            0,
            &fixture.worker_request(),
            &fixture.worker_native_request(),
            &token,
        ),
        Err(ManagedJudgeObservationError::Cancelled)
    ));
    assert!(matches!(
        begin_worker_result(&mut cancelled, 0, &fixture),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));

    let mut incomplete = fixture.sequence(1);
    start_and_preflight(&mut incomplete, &fixture);
    drive(&mut incomplete, 8, 11);
    begin_worker(&mut incomplete, 0, &fixture);
    assert!(matches!(
        incomplete.finish(&CancellationToken::new()),
        Err(ManagedJudgeObservationError::IncompleteSequence)
    ));
}

#[test]
fn worker_reobservation_failure_is_terminal() {
    let fixture = Fixture::new("process");
    let mut sequence = ManagedJudgeObservationSequence::for_test(
        Box::new(FakeAuthority {
            process: fixture.process.clone(),
            native_load: fixture.native_load.clone(),
            worker: fixture.worker.clone(),
            worker_native_load: fixture.worker_native_load.clone(),
            model_mapping: fixture.model_mapping.clone(),
            process_error: false,
            native_error: false,
            worker_reobserve_error: true,
        }),
        schedule_id("schedule"),
        1,
        &CancellationToken::new(),
    )
    .expect("sequence");
    start_and_preflight(&mut sequence, &fixture);
    drive_attempt(&mut sequence, 0, &fixture);
    assert!(matches!(
        complete_observation_result(&mut sequence, 0, &fixture),
        Err(ManagedJudgeObservationError::Worker(
            ManagedGenerationWorkerError::ObservationChanged
        ))
    ));
    assert!(matches!(
        complete_observation_result(&mut sequence, 0, &fixture),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));
}
