use std::time::{Duration, Instant};

use super::*;

fn expire(sequence: &mut ManagedJudgeObservationSequence) {
    sequence.operation_deadline = Some(Instant::now());
}

fn assert_deadline<T>(result: &Result<T, ManagedJudgeObservationError>) {
    match result {
        Err(ManagedJudgeObservationError::DeadlineExceeded) => {}
        _ => panic!("expected deadline precedence"),
    }
}

#[test]
fn construction_and_every_sequence_stage_enforce_the_retained_deadline() {
    let fixture = Fixture::new("deadline-stage");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        ManagedJudgeObservationSequence::for_test_until(
            fixture.authority(),
            schedule_id("deadline-construction"),
            1,
            &cancellation,
            Instant::now(),
        ),
        Err(ManagedJudgeObservationError::DeadlineExceeded)
    ));

    let mut response = fixture.sequence(1);
    expire(&mut response);
    assert_deadline(&response.observe_phase(
        OllamaResponseObservationPhase::BeforeResponses,
        connection(),
        &CancellationToken::new(),
    ));

    let mut preflight = fixture.sequence(1);
    expire(&mut preflight);
    assert_deadline(&preflight.complete_preflight_observation_until(
        &fixture.native_request(),
        &cancellation,
        Instant::now() + Duration::from_secs(30),
    ));

    let mut source = fixture.sequence(1);
    begin(&mut source);
    drive(
        &mut source,
        1,
        usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT).expect("preflight count"),
    );
    let preflight_observation = source
        .complete_preflight_observation(&fixture.native_request(), &CancellationToken::new())
        .expect("foreign preflight observation");
    let mut preflight_seal = fixture.sequence(1);
    expire(&mut preflight_seal);
    assert_deadline(&preflight_seal.seal_preflight_until(
        preflight_observation,
        &CancellationToken::new(),
        Instant::now() + Duration::from_secs(30),
    ));

    let mut worker = fixture.sequence(1);
    expire(&mut worker);
    assert_deadline(&worker.begin_attempt_worker_observation_until(
        7,
        &fixture.worker_request(),
        &fixture.worker_native_request(),
        &CancellationToken::new(),
        Instant::now() + Duration::from_secs(30),
    ));

    let mut completion = fixture.sequence(1);
    expire(&mut completion);
    assert_deadline(&completion.complete_attempt_observation_until(
        7,
        &fixture.native_request(),
        &CancellationToken::new(),
        Instant::now() + Duration::from_secs(30),
    ));

    let mut attempt_source = fixture.sequence(1);
    start_and_preflight(&mut attempt_source, &fixture);
    drive_attempt(&mut attempt_source, 0, &fixture);
    let attempt_observation = complete_observation(&mut attempt_source, 0, &fixture);
    let request = fixture.structured_request();
    let receipt = Fixture::receipt(0, &request);
    let state = FakeState::new(&attempt_observation, &request, &receipt, "deadline-state");
    let mut attempt_seal = fixture.sequence(1);
    expire(&mut attempt_seal);
    assert_deadline(&attempt_seal.seal_attempt_with_subject_until(
        attempt_observation,
        request,
        receipt,
        state,
        &CancellationToken::new(),
        Instant::now() + Duration::from_secs(30),
    ));

    let mut finish = fixture.sequence(1);
    expire(&mut finish);
    assert_deadline(&finish.finish_until(
        &CancellationToken::new(),
        Instant::now() + Duration::from_secs(30),
    ));
}

#[test]
fn retained_deadline_allows_complete_compatibility_sequence_but_cannot_be_extended() {
    let fixture = Fixture::new("deadline-complete");
    let retained = Instant::now() + Duration::from_secs(30);
    let mut sequence = ManagedJudgeObservationSequence::for_test_until(
        fixture.authority(),
        schedule_id("schedule"),
        1,
        &CancellationToken::new(),
        retained,
    )
    .expect("retained deadline sequence");
    start_and_preflight(&mut sequence, &fixture);
    drive_attempt(&mut sequence, 0, &fixture);
    let observation = complete_observation(&mut sequence, 0, &fixture);
    let request = fixture.structured_request();
    let receipt = Fixture::receipt(0, &request);
    let state = FakeState::new(&observation, &request, &receipt, "deadline-complete-state");
    sequence
        .seal_attempt_with_subject(
            observation,
            request,
            receipt,
            state,
            &CancellationToken::new(),
        )
        .expect("seal under retained deadline");
    let completed = sequence
        .finish_until(
            &CancellationToken::new(),
            retained + Duration::from_secs(30),
        )
        .expect("later caller deadline cannot replace retained deadline");
    assert_eq!(completed.bindings().len(), 1);

    let mut expired = fixture.sequence(1);
    expire(&mut expired);
    assert_deadline(&expired.complete_preflight_observation_until(
        &fixture.native_request(),
        &CancellationToken::new(),
        Instant::now() + Duration::from_mins(5),
    ));
}
