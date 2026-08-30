use super::*;

fn assert_finish_error(
    result: &Result<
        VerifiedGenerationQualificationResourceAttemptObservation,
        GenerationQualificationResourceAttemptObservationError,
    >,
    expected: GenerationQualificationResourceAttemptObservationError,
) {
    assert!(matches!(result, Err(error) if *error == expected));
}

#[test]
fn portable_identical_foreign_session_is_rejected_at_finish() {
    with_fixture(|fixture| {
        let mut packages = PackageFixture::new();
        let launch_plan = launch(&packages.model_lease, "session-substitution");
        let approved = fixture.resource_policy(true);
        let clock = GenerationQualificationResourceAttemptClock::start(
            &approved,
            &fixture.operation,
            &CancellationToken::new(),
        )
        .expect("approved clock");
        let artifact_id = launch_plan.model_target().artifact_id().clone();
        let exact = completion(
            artifact_id.clone(),
            "same request",
            r#"{"candidates":[{"text":"same"}]}"#,
        );
        let foreign = completion(
            artifact_id.clone(),
            "same request",
            r#"{"candidates":[{"text":"same"}]}"#,
        );
        let worker = worker(&packages, artifact_id, "same worker");
        let worker_observation =
            ManagedGenerationWorkerResourceObservation::for_test(&worker, 4_096);
        let mut released = characterized(&packages, &launch_plan, &packages.runtime.runtime_state);
        released.retain_resource_attempt_test_subject(
            &exact.completion,
            &worker,
            &packages.runtime.runtime_package,
            &packages.model_lease,
        );

        assert_finish_error(
            &clock.finish(GenerationQualificationResourceAttemptObservationInput {
                policy: &approved,
                operation_policy: &fixture.operation,
                completion: foreign.completion,
                worker_observation: &worker_observation,
                worker_evidence: &worker,
                released_package: &released,
                runtime_package: &mut packages.runtime.runtime_package,
                model_package: &packages.model_lease,
                cancellation: &CancellationToken::new(),
            }),
            GenerationQualificationResourceAttemptObservationError::ResponseBindingMismatch,
        );
    });
}

#[test]
fn internally_consistent_foreign_worker_pair_is_rejected_at_finish() {
    with_fixture(|fixture| {
        let mut packages = PackageFixture::new();
        let launch_plan = launch(&packages.model_lease, "worker-substitution");
        let approved = fixture.resource_policy(true);
        let clock = GenerationQualificationResourceAttemptClock::start(
            &approved,
            &fixture.operation,
            &CancellationToken::new(),
        )
        .expect("approved clock");
        let artifact_id = launch_plan.model_target().artifact_id().clone();
        let completion = completion(
            artifact_id.clone(),
            "worker request",
            r#"{"candidates":[{"text":"same"}]}"#,
        );
        let exact_worker = worker(&packages, artifact_id.clone(), "exact worker");
        let foreign_worker = worker(&packages, artifact_id, "foreign worker");
        let foreign_observation =
            ManagedGenerationWorkerResourceObservation::for_test(&foreign_worker, 4_096);
        let mut released = characterized(&packages, &launch_plan, &packages.runtime.runtime_state);
        released.retain_resource_attempt_test_subject(
            &completion.completion,
            &exact_worker,
            &packages.runtime.runtime_package,
            &packages.model_lease,
        );

        assert_finish_error(
            &clock.finish(GenerationQualificationResourceAttemptObservationInput {
                policy: &approved,
                operation_policy: &fixture.operation,
                completion: completion.completion,
                worker_observation: &foreign_observation,
                worker_evidence: &foreign_worker,
                released_package: &released,
                runtime_package: &mut packages.runtime.runtime_package,
                model_package: &packages.model_lease,
                cancellation: &CancellationToken::new(),
            }),
            GenerationQualificationResourceAttemptObservationError::WorkerBindingMismatch,
        );
    });
}

#[test]
fn byte_identical_foreign_installation_leases_are_rejected_at_finish() {
    with_fixture(|fixture| {
        let first = PackageFixture::new();
        let mut foreign = PackageFixture::new();
        let launch_plan = launch(&first.model_lease, "package-substitution");
        let approved = fixture.resource_policy(true);
        let clock = GenerationQualificationResourceAttemptClock::start(
            &approved,
            &fixture.operation,
            &CancellationToken::new(),
        )
        .expect("approved clock");
        let artifact_id = launch_plan.model_target().artifact_id().clone();
        let completion = completion(
            artifact_id.clone(),
            "package request",
            r#"{"candidates":[{"text":"same"}]}"#,
        );
        let worker = worker(&first, artifact_id, "package worker");
        let worker_observation =
            ManagedGenerationWorkerResourceObservation::for_test(&worker, 4_096);
        let mut released = characterized(&first, &launch_plan, &first.runtime.runtime_state);
        released.retain_resource_attempt_test_subject(
            &completion.completion,
            &worker,
            &first.runtime.runtime_package,
            &first.model_lease,
        );
        assert!(
            validate_package_binding(
                &released,
                &foreign.runtime.runtime_package,
                &foreign.model_lease,
            )
            .is_ok(),
            "portable identities are intentionally identical"
        );

        assert_finish_error(
            &clock.finish(GenerationQualificationResourceAttemptObservationInput {
                policy: &approved,
                operation_policy: &fixture.operation,
                completion: completion.completion,
                worker_observation: &worker_observation,
                worker_evidence: &worker,
                released_package: &released,
                runtime_package: &mut foreign.runtime.runtime_package,
                model_package: &foreign.model_lease,
                cancellation: &CancellationToken::new(),
            }),
            GenerationQualificationResourceAttemptObservationError::PackageBindingMismatch,
        );
    });
}
