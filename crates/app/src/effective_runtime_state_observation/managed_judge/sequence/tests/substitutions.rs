use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct RetryAuthority {
    process: AttachedProcessEvidence,
    native_load: NativeLoadObservation,
    worker: ManagedGenerationWorkerEvidence,
    worker_native_load: ManagedGenerationWorkerNativeLoadEvidence,
    model_mapping: ManagedGenerationWorkerModelMappingEvidence,
    fail_process: Arc<AtomicBool>,
    fail_native: Arc<AtomicBool>,
}

impl ManagedJudgeProcessAuthority for RetryAuthority {
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.process
    }

    fn reobserve(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        if self.fail_process.load(Ordering::Acquire) {
            return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
        }
        Ok(self.process.clone())
    }

    fn observe_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        connection_evidence(&self.process)
    }

    fn reobserve_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _initial: &RetainedTcpConnectionEvidence,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        connection_evidence(&self.process)
    }

    fn observe_native_load(
        &mut self,
        _request: &NativeLoadObservationRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError> {
        if self.fail_native.load(Ordering::Acquire) {
            return Err(NativeLoadObserverError::ObservationChanged);
        }
        Ok(self.native_load.clone())
    }

    fn observe_generation_worker(
        &mut self,
        _request: &ManagedGenerationWorkerObservationRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<Box<dyn ManagedJudgeWorkerAuthority>, ManagedGenerationWorkerError> {
        Ok(Box::new(FakeWorkerAuthority {
            initial: self.worker.clone(),
            native_load: self.worker_native_load.clone(),
            model_mapping: self.model_mapping.clone(),
            reobserve_error: false,
        }))
    }
}

#[test]
fn process_native_state_and_sequence_substitution_are_rejected() {
    let fixture = Fixture::new("process");
    let substituted = Fixture::new("other-process");
    let mut process_substitution = ManagedJudgeObservationSequence::for_test(
        Box::new(FakeAuthority {
            process: substituted.process.clone(),
            native_load: fixture.native_load.clone(),
            worker: fixture.worker.clone(),
            worker_native_load: fixture.worker_native_load.clone(),
            model_mapping: fixture.model_mapping.clone(),
            process_error: false,
            native_error: false,
            worker_reobserve_error: false,
        }),
        schedule_id("schedule"),
        1,
        &CancellationToken::new(),
    )
    .expect("sequence");
    begin(&mut process_substitution);
    drive(
        &mut process_substitution,
        1,
        MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize,
    );
    assert!(matches!(
        process_substitution
            .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new(),),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));

    let mut native_substitution = ManagedJudgeObservationSequence::for_test(
        Box::new(FakeAuthority {
            process: fixture.process.clone(),
            native_load: substituted.native_load.clone(),
            worker: fixture.worker.clone(),
            worker_native_load: fixture.worker_native_load.clone(),
            model_mapping: fixture.model_mapping.clone(),
            process_error: false,
            native_error: false,
            worker_reobserve_error: false,
        }),
        schedule_id("schedule"),
        1,
        &CancellationToken::new(),
    )
    .expect("sequence");
    begin(&mut native_substitution);
    drive(
        &mut native_substitution,
        1,
        MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize,
    );
    assert!(matches!(
        native_substitution
            .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new(),),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));

    let mut state_substitution = fixture.sequence(1);
    start_and_preflight(&mut state_substitution, &fixture);
    drive_attempt(&mut state_substitution, 0, &fixture);
    let observation = complete_observation(&mut state_substitution, 0, &fixture);
    let request = fixture.structured_request();
    let receipt = Fixture::receipt(0, &request);
    let mut wrong_state = FakeState::new(&observation, &request, &receipt, "wrong");
    wrong_state.process = substituted.process.evidence_digest().clone();
    wrong_state.native_load = substituted
        .native_load
        .native_load_observation_id()
        .digest()
        .clone();
    assert!(matches!(
        state_substitution.seal_attempt_with_subject(
            observation,
            request,
            receipt,
            wrong_state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::EffectiveStateMismatch)
    ));

    let mut first = fixture.sequence(1);
    let mut second = fixture.sequence(1);
    start_and_preflight(&mut first, &fixture);
    start_and_preflight(&mut second, &fixture);
    drive_attempt(&mut first, 0, &fixture);
    drive_attempt(&mut second, 0, &fixture);
    let first_observation = complete_observation(&mut first, 0, &fixture);
    let _second_observation = complete_observation(&mut second, 0, &fixture);
    let request = fixture.structured_request();
    let receipt = Fixture::receipt(0, &request);
    let state = FakeState::new(&first_observation, &request, &receipt, "state");
    assert!(matches!(
        second.seal_attempt_with_subject(
            first_observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        ),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));
}

#[test]
fn worker_and_preflight_tokens_cannot_cross_sequences() {
    let fixture = Fixture::new("process");
    let substituted = Fixture::new("other-worker");
    let wrong_worker = ManagedGenerationWorkerEvidence::for_test(
        ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
        fixture.package_id.clone(),
        fixture.retained_worker.artifact_id().clone(),
        ArtifactId::from_digest(digest("wrong-model")),
        "wrong-worker",
    );
    let mut worker_substitution = ManagedJudgeObservationSequence::for_test(
        Box::new(FakeAuthority {
            process: fixture.process.clone(),
            native_load: fixture.native_load.clone(),
            worker: wrong_worker,
            worker_native_load: substituted.worker_native_load.clone(),
            model_mapping: substituted.model_mapping,
            process_error: false,
            native_error: false,
            worker_reobserve_error: false,
        }),
        schedule_id("schedule"),
        1,
        &CancellationToken::new(),
    )
    .expect("sequence");
    start_and_preflight(&mut worker_substitution, &fixture);
    drive(&mut worker_substitution, 8, 11);
    assert!(matches!(
        begin_worker_result(&mut worker_substitution, 0, &fixture),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));

    let mut first = fixture.sequence(1);
    let mut second = fixture.sequence(1);
    begin(&mut first);
    begin(&mut second);
    drive(
        &mut first,
        1,
        MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize,
    );
    drive(
        &mut second,
        1,
        MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize,
    );
    let first_token = first
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect("first token");
    let _second_token = second
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect("second token");
    assert!(matches!(
        second.seal_preflight(first_token, &CancellationToken::new()),
        Err(ManagedJudgeObservationError::ObservationSubstitution)
    ));
}

#[test]
fn cancellation_incomplete_sequence_and_observer_errors_are_explicit_and_redacted() {
    let fixture = Fixture::new("process");
    let cancelled = CancellationToken::new();
    cancelled.cancel();
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
            1,
            &cancelled,
        ),
        Err(ManagedJudgeObservationError::Cancelled)
    ));
    assert!(matches!(
        fixture.sequence(1).finish(&CancellationToken::new()),
        Err(ManagedJudgeObservationError::IncompleteSequence)
    ));

    let mut process_error = sequence_with_errors(&fixture, true, false);
    begin(&mut process_error);
    drive(
        &mut process_error,
        1,
        MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize,
    );
    let error = process_error
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect_err("process error");
    assert!(matches!(error, ManagedJudgeObservationError::Process(_)));
    assert_eq!(
        format!("{error:?}"),
        "ManagedJudgeObservationError::Process"
    );

    let mut native_error = sequence_with_errors(&fixture, false, true);
    begin(&mut native_error);
    drive(
        &mut native_error,
        1,
        MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT as usize,
    );
    let error = native_error
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect_err("native error");
    assert!(matches!(error, ManagedJudgeObservationError::NativeLoad(_)));
    assert_eq!(
        format!("{error:?}"),
        "ManagedJudgeObservationError::NativeLoad"
    );
}

#[test]
fn cancellation_and_recovered_observers_cannot_retry_a_failed_boundary() {
    let fixture = Fixture::new("process");
    let mut cancelled_sequence = fixture.sequence(1);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        cancelled_sequence.observe_phase(
            OllamaResponseObservationPhase::BeforeResponses,
            connection(),
            &cancelled,
        ),
        Err(ManagedJudgeObservationError::Cancelled)
    ));
    assert!(matches!(
        observe(
            &mut cancelled_sequence,
            OllamaResponseObservationPhase::BeforeResponses,
        ),
        Err(ManagedJudgeObservationError::InvalidResponseSequence)
    ));

    for fail_native in [false, true] {
        let fail_process = Arc::new(AtomicBool::new(false));
        let native_switch = Arc::new(AtomicBool::new(false));
        let mut sequence = ManagedJudgeObservationSequence::for_test(
            Box::new(RetryAuthority {
                process: fixture.process.clone(),
                native_load: fixture.native_load.clone(),
                worker: fixture.worker.clone(),
                worker_native_load: fixture.worker_native_load.clone(),
                model_mapping: fixture.model_mapping.clone(),
                fail_process: Arc::clone(&fail_process),
                fail_native: Arc::clone(&native_switch),
            }),
            schedule_id("schedule"),
            1,
            &CancellationToken::new(),
        )
        .expect("sequence");
        start_and_preflight(&mut sequence, &fixture);
        fail_process.store(!fail_native, Ordering::Release);
        native_switch.store(fail_native, Ordering::Release);
        drive_attempt(&mut sequence, 0, &fixture);
        assert!(complete_observation_result(&mut sequence, 0, &fixture).is_err());
        fail_process.store(false, Ordering::Release);
        native_switch.store(false, Ordering::Release);
        assert!(matches!(
            complete_observation_result(&mut sequence, 0, &fixture),
            Err(ManagedJudgeObservationError::InvalidResponseSequence)
        ));
        assert!(matches!(
            sequence.finish(&CancellationToken::new()),
            Err(ManagedJudgeObservationError::IncompleteSequence)
        ));
    }
}
