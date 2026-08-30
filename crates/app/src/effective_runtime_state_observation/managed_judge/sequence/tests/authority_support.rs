use super::*;

pub(super) struct BatchEffectiveStateFixture<'a, 'model> {
    pub(super) state: rewrite_model::EffectiveRuntimeState,
    pub(super) admitted_runtime: &'a crate::VerifiedAdmittedRuntime,
    pub(super) generation_path: &'a crate::VerifiedManagedGenerationPath,
    pub(super) runtime_package: &'a crate::RuntimePackageLease,
    pub(super) managed_ollama: &'a crate::ManagedOllamaIsolationLease<'model>,
    pub(super) tag: &'a str,
}

pub(crate) enum CompletedSequenceMutation {
    RemoveLast,
    SwapFirstTwo,
    Cursor { index: usize, value: u32 },
    FirstResponse { index: usize, value: u64 },
    LastResponse { index: usize, value: u64 },
    ExecutionFirstResponse { index: usize },
    ExecutionLastResponse { index: usize },
    FirstResidency { index: usize },
    LastResidency { index: usize },
    Process { index: usize, value: Digest },
    NativeLoad { index: usize, value: Digest },
    Connection { index: usize, value: Digest },
    Receipt { index: usize },
    Response { index: usize },
    ReceiptBinding { index: usize },
    EffectiveState { index: usize, value: Digest },
    RetainedPreflight { index: usize },
    PreflightInitialProcess,
    PreflightPostProcess,
    PreflightFinalProcess,
    PreflightNativeLoad,
    PreflightConnection,
}

pub(crate) fn completed_sequence_for_observation_authority(
    schedule_id: &CandidateJudgeScheduleId,
    attempts: u32,
) -> super::super::CompletedManagedJudgeObservationSequence {
    completed_sequence_with_unsealed_preflight_binding(schedule_id.clone(), schedule_id, attempts).0
}

pub(crate) fn completed_sequence_for_effective_package(
    schedule_id: &CandidateJudgeScheduleId,
    attempts: u32,
    state: &rewrite_model::EffectiveRuntimeState,
    admitted_runtime: &crate::VerifiedAdmittedRuntime,
    generation_path: &crate::VerifiedManagedGenerationPath,
    runtime_package: &crate::RuntimePackageLease,
    managed_ollama: &crate::ManagedOllamaIsolationLease<'_>,
) -> super::super::CompletedManagedJudgeObservationSequence {
    let fixture = Fixture::new("batch-effective-package");
    let mut sequence = ManagedJudgeObservationSequence::for_test(
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
        schedule_id.clone(),
        attempts,
        &CancellationToken::new(),
    )
    .expect("batch effective-package sequence");
    begin(&mut sequence);
    drive(
        &mut sequence,
        1,
        usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).expect("preflight count"),
    );
    let preflight = sequence
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect("batch preflight observation");
    sequence
        .seal_preflight(preflight, &CancellationToken::new())
        .expect("seal batch preflight");
    for cursor in 0..attempts {
        drive_attempt(&mut sequence, cursor, &fixture);
        let observation = complete_observation(&mut sequence, cursor, &fixture);
        let mut request = fixture.structured_request();
        write!(request.input, " {cursor}").expect("write request cursor");
        request.source_byte_count = u64::try_from(request.input.len()).expect("request bytes");
        let receipt = Fixture::receipt(cursor, &request);
        let retained = FakeState::new_for_batch(
            &observation,
            &request,
            &receipt,
            BatchEffectiveStateFixture {
                state: state.clone(),
                admitted_runtime,
                generation_path,
                runtime_package,
                managed_ollama,
                tag: "repeated batch state",
            },
        );
        sequence
            .seal_attempt_with_subject(
                observation,
                request,
                receipt,
                retained,
                &CancellationToken::new(),
            )
            .expect("seal batch attempt");
    }
    sequence
        .finish(&CancellationToken::new())
        .expect("complete batch effective-package sequence")
}

pub(crate) fn completed_sequence_with_unsealed_preflight_binding(
    schedule_id: CandidateJudgeScheduleId,
    binding_schedule_id: &CandidateJudgeScheduleId,
    attempts: u32,
) -> (
    super::super::CompletedManagedJudgeObservationSequence,
    Digest,
) {
    let fixture = Fixture::new("aggregate-authority");
    let mut sequence = ManagedJudgeObservationSequence::for_test(
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
        schedule_id,
        attempts,
        &CancellationToken::new(),
    )
    .expect("aggregate sequence");
    begin(&mut sequence);
    drive(
        &mut sequence,
        1,
        usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).expect("preflight count"),
    );
    let preflight = sequence
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect("aggregate preflight observation");
    let preflight_binding = crate::derive_managed_judge_preflight_observer_binding_digest(
        binding_schedule_id,
        &preflight,
    );
    sequence
        .seal_preflight(preflight, &CancellationToken::new())
        .expect("seal aggregate preflight");
    for cursor in 0..attempts {
        drive_attempt(&mut sequence, cursor, &fixture);
        let observation = complete_observation(&mut sequence, cursor, &fixture);
        let request = fixture.structured_request();
        let receipt = Fixture::receipt(cursor, &request);
        let state = FakeState::new(&observation, &request, &receipt, "repeated state");
        sequence
            .seal_attempt_with_subject(
                observation,
                request,
                receipt,
                state,
                &CancellationToken::new(),
            )
            .expect("aggregate attempt");
    }
    (
        sequence
            .finish(&CancellationToken::new())
            .expect("complete aggregate sequence"),
        preflight_binding,
    )
}

pub(crate) fn mutate_completed_sequence(
    sequence: &mut super::super::CompletedManagedJudgeObservationSequence,
    mutation: CompletedSequenceMutation,
) {
    match mutation {
        CompletedSequenceMutation::RemoveLast => {
            sequence.bindings.pop();
        }
        CompletedSequenceMutation::SwapFirstTwo => sequence.bindings.swap(0, 1),
        CompletedSequenceMutation::Cursor { index, value } => {
            sequence.bindings[index].schedule_cursor = value;
        }
        CompletedSequenceMutation::FirstResponse { index, value } => {
            sequence.bindings[index].first_response_ordinal = value;
        }
        CompletedSequenceMutation::LastResponse { index, value } => {
            sequence.bindings[index].last_response_ordinal = value;
        }
        CompletedSequenceMutation::FirstResidency { index } => {
            replace_receipt(sequence, index, ReceiptMutation::FirstResidency);
        }
        CompletedSequenceMutation::LastResidency { index } => {
            replace_receipt(sequence, index, ReceiptMutation::LastResidency);
        }
        CompletedSequenceMutation::ExecutionFirstResponse { index } => {
            replace_receipt(sequence, index, ReceiptMutation::ExecutionFirst);
        }
        CompletedSequenceMutation::ExecutionLastResponse { index } => {
            replace_receipt(sequence, index, ReceiptMutation::ExecutionLast);
        }
        CompletedSequenceMutation::Process { index, value } => {
            sequence.bindings[index].process_binding_digest = value;
        }
        CompletedSequenceMutation::NativeLoad { index, value } => {
            sequence.bindings[index].native_load_binding_digest = value;
        }
        CompletedSequenceMutation::Connection { index, value } => {
            sequence.bindings[index].connection_binding_digest = value;
        }
        CompletedSequenceMutation::Receipt { index } => {
            replace_receipt(sequence, index, ReceiptMutation::Output);
        }
        CompletedSequenceMutation::Response { index } => {
            replace_batch_response(sequence, index);
        }
        CompletedSequenceMutation::ReceiptBinding { index } => {
            sequence.bindings[index].receipt_binding_digest = digest("other complete receipt");
        }
        CompletedSequenceMutation::EffectiveState { index, value } => {
            sequence.bindings[index].effective_state =
                super::super::super::contract::RetainedManagedJudgeEffectiveState::Offline(value);
        }
        CompletedSequenceMutation::RetainedPreflight { index } => {
            replace_receipt(sequence, index, ReceiptMutation::Preflight);
        }
        CompletedSequenceMutation::PreflightInitialProcess => {
            sequence.preflight.evidence.initial_process = process("other initial process");
        }
        CompletedSequenceMutation::PreflightPostProcess => {
            sequence.preflight.evidence.post_preflight_process = process("other post process");
        }
        CompletedSequenceMutation::PreflightFinalProcess => {
            sequence.preflight.evidence.final_process = process("other final process");
        }
        CompletedSequenceMutation::PreflightNativeLoad => {
            sequence.preflight.evidence.native_load = Fixture::new("other native load").native_load;
        }
        CompletedSequenceMutation::PreflightConnection => {
            sequence.preflight.evidence.connection_binding_digest = digest("other connection");
        }
    }
}

fn replace_batch_response(
    sequence: &mut super::super::CompletedManagedJudgeObservationSequence,
    index: usize,
) {
    let fixture = Fixture::new("batch-effective-package");
    let cursor = sequence.bindings[index].schedule_cursor;
    let mut request = fixture.structured_request();
    write!(request.input, " {cursor}").expect("write request cursor");
    request.source_byte_count = u64::try_from(request.input.len()).expect("request bytes");
    let first = 8 + usize::try_from(cursor).expect("cursor") * 9;
    let running = running_model();
    let receipt = Fixture::receipt_from(
        &request,
        &preflight(&running),
        &running,
        r#"{"candidates":[{"text":"different response"}]}"#,
        [first, first + 8, first + 4, first + 8],
    );
    sequence.bindings[index].receipt_binding_digest = receipt.complete_binding_digest();
    sequence.bindings[index].receipt = receipt;
}

fn replace_receipt(
    sequence: &mut super::super::CompletedManagedJudgeObservationSequence,
    index: usize,
    mutation: ReceiptMutation,
) {
    let fixture = Fixture::new("other receipt");
    let request = fixture.structured_request();
    let running = running_model();
    let mut retained_preflight = preflight(&running);
    if matches!(mutation, ReceiptMutation::Preflight) {
        retained_preflight.runtime.version = "0.32.15-other".to_owned();
    }
    let cursor = sequence.bindings[index].schedule_cursor;
    let first = 8 + usize::try_from(cursor).expect("cursor") * 9;
    let execution_first = if matches!(mutation, ReceiptMutation::ExecutionFirst) {
        first + 1
    } else {
        first
    };
    let execution_last = if matches!(mutation, ReceiptMutation::ExecutionLast) {
        first + 7
    } else {
        first + 8
    };
    let first_residency = if matches!(mutation, ReceiptMutation::FirstResidency) {
        first + 5
    } else {
        first + 4
    };
    let last_residency = if matches!(mutation, ReceiptMutation::LastResidency) {
        first + 7
    } else {
        first + 8
    };
    let output = if matches!(mutation, ReceiptMutation::Output) {
        "different"
    } else {
        "ok"
    };
    let response = format!(r#"{{"candidates":[{{"text":"{output}"}}]}}"#);
    let receipt = Fixture::receipt_from(
        &request,
        &retained_preflight,
        &running,
        &response,
        [
            execution_first,
            execution_last,
            first_residency,
            last_residency,
        ],
    );
    sequence.bindings[index].receipt_binding_digest = receipt.complete_binding_digest();
    sequence.bindings[index].receipt = receipt;
}

#[derive(Clone, Copy)]
enum ReceiptMutation {
    Output,
    ExecutionFirst,
    ExecutionLast,
    FirstResidency,
    LastResidency,
    Preflight,
}
