use super::*;

#[test]
fn authority_accessors_and_debug_are_content_free() {
    let fixture = Fixture::new("process");
    let mut sequence = fixture.sequence(1);
    let sequence_debug = format!("{sequence:?}");
    assert!(sequence_debug.contains("attempt_count: 1"));
    assert!(!sequence_debug.contains(fixture.process.evidence_digest().as_str()));

    start_and_preflight(&mut sequence, &fixture);
    drive_attempt(&mut sequence, 0, &fixture);
    let observation = complete_observation(&mut sequence, 0, &fixture);
    assert_eq!(observation.schedule_cursor(), 0);
    assert_eq!(observation.process(), &fixture.process);
    assert_eq!(observation.native_load(), &fixture.native_load);
    assert_eq!(observation.initial_worker(), &fixture.worker);
    assert_eq!(observation.final_worker(), &fixture.worker);
    assert_eq!(
        observation.worker_native_load(),
        &fixture.worker_native_load
    );
    assert_eq!(observation.model_mapping(), &fixture.model_mapping);
    assert_eq!(observation.first_response_ordinal(), 8);
    assert_eq!(observation.last_response_ordinal(), 16);
    assert!(!format!("{observation:?}").contains(fixture.process.evidence_digest().as_str()));

    let request = fixture.structured_request();
    let receipt = Fixture::receipt(0, &request);
    let retained_request = request.binding_digest();
    let retained_response = receipt.execution().retained_response_id();
    let retained_preflight = receipt.execution().preflight_digest().clone();
    let retained_receipt = receipt.clone();
    let state = FakeState::new(&observation, &request, &receipt, "state");
    let state_digest = state.binding.clone();
    sequence
        .seal_attempt_with_subject(
            observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        )
        .expect("seal");
    let completed = sequence.finish(&CancellationToken::new()).expect("finish");
    assert_eq!(completed.schedule_id(), &schedule_id("schedule"));
    assert!(format!("{completed:?}").contains("attempt_count: 1"));
    let preflight = completed.preflight();
    assert_eq!(preflight.initial_process(), &fixture.process);
    assert_eq!(preflight.post_preflight_process(), &fixture.process);
    assert_eq!(preflight.final_process(), &fixture.process);
    assert_eq!(preflight.native_load(), &fixture.native_load);
    assert_eq!(preflight.connection_observations().len(), 8);
    assert_eq!(
        preflight.connection_witness(),
        preflight.connection_observations().last().expect("witness")
    );
    assert_eq!(
        preflight.connection_binding_digest(),
        completed.preflight_connection_binding_digest()
    );
    assert!(format!("{preflight:?}").contains("connection_observation_count: 8"));
    let binding = &completed.bindings()[0];
    assert_eq!(binding.effective_state_binding_digest(), &state_digest);
    assert_eq!(binding.request_binding_digest(), &retained_request);
    assert_eq!(binding.retained_response_id(), retained_response);
    assert_eq!(binding.retained_preflight_digest(), &retained_preflight);
    assert_eq!(binding.first_residency_ordinal(), 12);
    assert_eq!(binding.last_residency_ordinal(), 16);
    assert!(binding.binds_receipt(&retained_receipt));
    let other_receipt = Fixture::receipt(1, &fixture.structured_request());
    assert!(!binding.binds_receipt(&other_receipt));
    assert!(format!("{binding:?}").contains("schedule_cursor: 0"));
    assert!(!format!("{binding:?}").contains(state_digest.as_str()));
}

#[test]
fn invalid_counts_and_unclosed_phases_fail_at_the_exact_boundary() {
    let fixture = Fixture::new("process");
    for count in [0, 513] {
        assert!(matches!(
            ManagedJudgeObservationSequence::for_test(
                Box::new(FakeAuthority {
                    process: fixture.process.clone(),
                    native_load: fixture.native_load.clone(),
                    worker: fixture.worker.clone(),
                    worker_native_load: fixture.worker_native_load.clone(),
                    model_mapping: fixture.model_mapping.clone(),
                    process_error: false,
                    native_error: false,
                    worker_reobserve_error: false,
                }),
                schedule_id("schedule"),
                count,
                &CancellationToken::new(),
            ),
            Err(ManagedJudgeObservationError::InvalidCount)
        ));
    }
    assert_eq!(
        ManagedJudgeObservationSequence::supported_on_current_platform(),
        cfg!(target_os = "linux")
    );

    let mut duplicate_start = fixture.sequence(1);
    begin(&mut duplicate_start);
    assert!(matches!(
        observe(
            &mut duplicate_start,
            OllamaResponseObservationPhase::BeforeResponses,
        ),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));

    let mut without_preflight = fixture.sequence(1);
    assert!(matches!(
        complete_observation_result(&mut without_preflight, 0, &fixture),
        Err(ManagedJudgeObservationError::InvalidPreflightSpan)
    ));

    let mut duplicate_preflight = fixture.sequence(1);
    start_and_preflight(&mut duplicate_preflight, &fixture);
    assert!(matches!(
        duplicate_preflight
            .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new(),),
        Err(ManagedJudgeObservationError::InvalidPreflightSpan)
    ));

    let mut awaiting = fixture.sequence(1);
    start_and_preflight(&mut awaiting, &fixture);
    drive_attempt(&mut awaiting, 0, &fixture);
    let _observation = complete_observation(&mut awaiting, 0, &fixture);
    assert!(matches!(
        observe(
            &mut awaiting,
            OllamaResponseObservationPhase::AfterResponse { ordinal: 17 },
        ),
        Err(ManagedJudgeObservationError::AwaitingSeal)
    ));
}

#[test]
fn every_closed_error_kind_has_redacted_debug_and_display() {
    let cases = [
        ManagedJudgeObservationError::UnsupportedPlatform,
        ManagedJudgeObservationError::Cancelled,
        ManagedJudgeObservationError::DeadlineExceeded,
        ManagedJudgeObservationError::InvalidCount,
        ManagedJudgeObservationError::InvalidResponseSequence,
        ManagedJudgeObservationError::FailedResponseAttempt,
        ManagedJudgeObservationError::InvalidPreflightSpan,
        ManagedJudgeObservationError::InvalidAttemptSpan,
        ManagedJudgeObservationError::InvalidScheduleCursor,
        ManagedJudgeObservationError::AwaitingSeal,
        ManagedJudgeObservationError::AwaitingPreflightSeal,
        ManagedJudgeObservationError::InvalidWorkerObservationPhase,
        ManagedJudgeObservationError::IncompleteWorkerObservation,
        ManagedJudgeObservationError::ObservationSubstitution,
        ManagedJudgeObservationError::AttemptClosureMismatch,
        ManagedJudgeObservationError::EffectiveStateMismatch,
        ManagedJudgeObservationError::IncompleteSequence,
    ];
    for error in cases {
        assert!(format!("{error:?}").starts_with("ManagedJudgeObservationError::"));
        assert!(!error.to_string().is_empty());
    }
}
