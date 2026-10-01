use super::*;

#[test]
fn source_drift_and_independent_batch_finalizer_failures_never_publish_results() {
    for source_drift in [false, true] {
        with_synthetic_generation_qualification_fixture(
            SyntheticGenerationQualificationScenario::TrafficEligible,
            |input, platform_owners, license_proof, license_policy, production_policy| {
                let source = input.case_authorities[0]
                    .source
                    .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                    .expect("source");
                let fixture = MaterialFixture::new(input.foundation.plan_foundation, &source);
                let material = fixture.verify();
                prepare_active!(
                    input,
                    platform_owners,
                    license_proof,
                    license_policy,
                    production_policy,
                    directory,
                    repository,
                    active,
                    cancellation,
                    evidence
                );
                let pair = SyntheticPair::new(
                    input.foundation.plan_foundation,
                    input.operation_policy_relations,
                    &active,
                    "qualification-closure-passing",
                );
                let target_control = pair.target.batches[0].failure_control();
                let baseline_control = pair.baseline.batches[0].failure_control();
                let (target, baseline) = pair.bind(
                    &mut active,
                    directory.path(),
                    &evidence,
                    &cancellation,
                    true,
                );
                let target_calls = target_control.revalidation_calls();
                let baseline_calls = baseline_control.revalidation_calls();
                if source_drift {
                    fixture.invalidate_source_installation();
                } else {
                    target_control.fail_on_call(target_calls + 1);
                    baseline_control.fail_on_call(baseline_calls + 1);
                }
                assert_eq!(active.persist_candidate_deterministic_evaluation(&mut repository, &target, &baseline, &material, &cancellation).expect_err("settlement must fail closed"), ActiveGenerationQualificationDeterministicSettlementError::PrimaryAndFinalization);
                assert!(target_control.revalidation_calls() > target_calls);
                assert!(baseline_control.revalidation_calls() > baseline_calls);
                assert!(active.terminal);
                assert_eq!(
                    row_count(directory.path(), "candidate_generation_receipt_sets"),
                    0
                );
                assert_eq!(
                    row_count(
                        directory.path(),
                        "candidate_deterministic_evaluation_records"
                    ),
                    0
                );
            },
        );
    }
}

#[test]
fn evaluation_transaction_abort_leaves_only_inert_committed_parent_receipt_sets() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            let source = input.case_authorities[0]
                .source
                .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                .expect("source");
            let fixture = MaterialFixture::new(input.foundation.plan_foundation, &source);
            let material = fixture.verify();
            prepare_active!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                directory,
                repository,
                active,
                cancellation,
                evidence
            );
            let pair = SyntheticPair::new(
                input.foundation.plan_foundation,
                input.operation_policy_relations,
                &active,
                "qualification-closure-passing",
            );
            let (target, baseline) = pair.bind(
                &mut active,
                directory.path(),
                &evidence,
                &cancellation,
                true,
            );
            let connection = rusqlite::Connection::open(directory.path().join("qualification.db"))
                .expect("fixture connection");
            connection.execute_batch("CREATE TRIGGER refuse_synthetic_evaluation BEFORE INSERT ON candidate_deterministic_evaluation_records BEGIN SELECT RAISE(ABORT, 'synthetic evaluation refusal'); END;").expect("bounded fixture commit refusal");
            assert_eq!(
                active
                    .persist_candidate_deterministic_evaluation(
                        &mut repository,
                        &target,
                        &baseline,
                        &material,
                        &cancellation
                    )
                    .expect_err("settlement must fail closed"),
                ActiveGenerationQualificationDeterministicSettlementError::Publication
            );
            assert!(active.terminal);
            assert_eq!(
                row_count(directory.path(), "candidate_generation_receipt_sets"),
                2
            );
            assert_eq!(
                row_count(
                    directory.path(),
                    "candidate_deterministic_evaluation_records"
                ),
                0
            );
            assert_eq!(
                active
                    .persist_candidate_deterministic_evaluation(
                        &mut repository,
                        &target,
                        &baseline,
                        &material,
                        &cancellation
                    )
                    .expect_err("settlement must fail closed"),
                ActiveGenerationQualificationDeterministicSettlementError::NotReady
            );
        },
    );
}

#[test]
fn foreign_baseline_subject_cannot_write_even_with_identical_portable_scope() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            let source = input.case_authorities[0]
                .source
                .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                .expect("source");
            let fixture = MaterialFixture::new(input.foundation.plan_foundation, &source);
            let material = fixture.verify();
            prepare_active!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                directory,
                repository,
                active,
                cancellation,
                evidence
            );
            let pair = SyntheticPair::new(
                input.foundation.plan_foundation,
                input.operation_policy_relations,
                &active,
                "qualification-closure-passing",
            );
            let another = SyntheticPair::new(
                input.foundation.plan_foundation,
                input.operation_policy_relations,
                &active,
                "qualification-closure-passing",
            );
            let (target, _) = pair.bind(
                &mut active,
                directory.path(),
                &evidence,
                &cancellation,
                true,
            );
            let foreign_subject = crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject::new();
            let baseline = bound_synthetic_set(another.baseline, &foreign_subject);
            assert_eq!(
                active
                    .persist_candidate_deterministic_evaluation(
                        &mut repository,
                        &target,
                        &baseline,
                        &material,
                        &cancellation
                    )
                    .expect_err("settlement must fail closed"),
                ActiveGenerationQualificationDeterministicSettlementError::OperationScope
            );
            assert_eq!(
                row_count(directory.path(), "candidate_generation_receipt_sets"),
                0
            );
            assert_eq!(
                row_count(
                    directory.path(),
                    "candidate_deterministic_evaluation_records"
                ),
                0
            );
        },
    );
}

#[test]
fn corrupted_evaluation_readback_fails_closed_after_an_initial_success() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            let source = input.case_authorities[0]
                .source
                .with_source_bytes(&CancellationToken::new(), <[u8]>::to_vec)
                .expect("source");
            let fixture = MaterialFixture::new(input.foundation.plan_foundation, &source);
            let material = fixture.verify();
            prepare_active!(
                input,
                platform_owners,
                license_proof,
                license_policy,
                production_policy,
                directory,
                repository,
                active,
                cancellation,
                evidence
            );
            let pair = SyntheticPair::new(
                input.foundation.plan_foundation,
                input.operation_policy_relations,
                &active,
                "qualification-closure-passing",
            );
            let (target, baseline) = pair.bind(
                &mut active,
                directory.path(),
                &evidence,
                &cancellation,
                true,
            );
            active
                .persist_candidate_deterministic_evaluation(
                    &mut repository,
                    &target,
                    &baseline,
                    &material,
                    &cancellation,
                )
                .expect("initial exact settlement");
            let connection = rusqlite::Connection::open(directory.path().join("qualification.db"))
                .expect("fixture connection");
            connection
                .execute(
                    "UPDATE candidate_deterministic_evaluation_records SET canonical_json = ?1",
                    [b"{}".as_slice()],
                )
                .expect("corrupt fixture row");
            assert_eq!(
                active
                    .persist_candidate_deterministic_evaluation(
                        &mut repository,
                        &target,
                        &baseline,
                        &material,
                        &cancellation
                    )
                    .expect_err("settlement must fail closed"),
                ActiveGenerationQualificationDeterministicSettlementError::Publication
            );
            assert!(active.terminal);
            assert_eq!(
                row_count(
                    directory.path(),
                    "candidate_deterministic_evaluation_records"
                ),
                1
            );
        },
    );
}
