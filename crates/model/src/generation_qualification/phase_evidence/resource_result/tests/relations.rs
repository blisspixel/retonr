use super::*;
use crate::generation_qualification::*;
use rewrite_types::Digest;

#[test]
fn constructor_rejects_failed_attempt_and_direct_relation_substitution() {
    let fixture = fixture();
    let input = input();
    let relations = fixture.relations();
    let failed = CandidateGenerationAttemptRecordV1::failed(
        relations.planned_attempt,
        None,
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
            failure_category: CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
            traffic_observed: false,
            output_observed: false,
            cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
        },
    )
    .expect("failed attempt");
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            attempt_record: &failed,
            ..relations
        },
        &input,
    );
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            planned_attempt: &fixture.base.attempts[1],
            ..relations
        },
        &input,
    );
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            case: &fixture.base.cases[1],
            ..relations
        },
        &input,
    );
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            repetition: &fixture.base.repetitions[1],
            ..relations
        },
        &input,
    );
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            candidate_generation_receipt: &fixture.alternate_receipt,
            ..relations
        },
        &input,
    );
}

#[test]
fn constructor_rejects_baseline_cross_scope_and_operation_policy_substitution() {
    let fixture = fixture();
    let input = input();
    let relations = fixture.relations();
    let baseline_scope = GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.base.systems[1],
        qualification_plan: &fixture.base.plan,
        suite: &fixture.base.suite,
    };
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            scope: baseline_scope,
            planned_attempt: &fixture.base.attempts[1],
            ..relations
        },
        &input,
    );

    let foreign_suite =
        GenerationSuiteManifestV1::new(digest("foreign suite protocol"), &fixture.base.cases)
            .expect("foreign suite");
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            scope: GenerationQualificationPhaseScopeV1 {
                suite: &foreign_suite,
                ..relations.scope
            },
            ..relations
        },
        &input,
    );

    let foreign_plan = GenerationQualificationPlanV1::new(
        &fixture.base.suite,
        &fixture.base.repetitions,
        &fixture.base.systems,
        &fixture.base.attempts,
        GenerationQualificationPlanV1Input {
            limits: fixture.base.plan.limits(),
            selection_policy_digest: digest("foreign selection policy"),
            failure_policy_digest: fixture.base.plan.failure_policy_digest().clone(),
        },
    )
    .expect("foreign plan");
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            scope: GenerationQualificationPhaseScopeV1 {
                qualification_plan: &foreign_plan,
                ..relations.scope
            },
            ..relations
        },
        &input,
    );

    let policy_relations = fixture.base.relations();
    let swapped_policy = GenerationQualificationOperationPolicyV1::new(
        GenerationQualificationOperationPolicyV1Relations {
            target_system: policy_relations.baseline_system,
            baseline_system: policy_relations.target_system,
            ..policy_relations
        },
        fixture.base.policy_input.clone(),
    )
    .expect("swapped target policy");
    assert_rejected(
        GenerationResourceAttemptResultRecordV1Relations {
            operation_policy: &swapped_policy,
            ..relations
        },
        &input,
    );
}

#[test]
fn observations_reject_overflow_reversed_intervals_and_bad_limit_order() {
    let fixture = fixture();
    let relations = fixture.relations();
    let invalid = [
        changed(input(), |value| value.installed_footprint_bytes += 1),
        changed(input(), |value| {
            value.runtime_installed_payload_bytes = u64::MAX;
            value.model_installed_payload_bytes = 1;
            value.installed_footprint_bytes = u64::MAX;
        }),
        changed(input(), |value| {
            value.load_duration_nanoseconds = u64::MAX;
            value.prompt_evaluation_duration_nanoseconds = 1;
            value.total_duration_nanoseconds = u64::MAX;
        }),
        changed(input(), |value| {
            value.total_duration_nanoseconds = 25_000_000;
        }),
        changed(input(), |value| {
            value.attempt_elapsed_nanoseconds = 29_999_999;
        }),
        changed(input(), |value| {
            value.first_response_elapsed_nanoseconds = 40_000_001;
        }),
        changed(input(), |value| {
            value.cleanup_elapsed_nanoseconds = 40_000_001;
        }),
        changed(input(), |value| {
            value.exceeded_limits = vec![
                GenerationResourceExceededLimitV1::WorkerHighWaterResident,
                GenerationResourceExceededLimitV1::FirstResponse,
            ];
        }),
        changed(input(), |value| {
            value.exceeded_limits = vec![
                GenerationResourceExceededLimitV1::FirstResponse,
                GenerationResourceExceededLimitV1::FirstResponse,
            ];
        }),
    ];
    for input in invalid {
        assert!(
            GenerationResourceAttemptResultRecordV1::new(relations, input).is_err(),
            "invalid observation accepted"
        );
    }
}

#[test]
fn zero_observations_and_exact_interval_equality_are_representable() {
    let zero = super::support::zero_fixture();
    let zero_input = GenerationResourceAttemptResultRecordV1Input {
        prompt_token_count: 0,
        generated_token_count: 0,
        total_duration_nanoseconds: 0,
        load_duration_nanoseconds: 0,
        prompt_evaluation_duration_nanoseconds: 0,
        evaluation_duration_nanoseconds: 0,
        attempt_elapsed_nanoseconds: 0,
        first_response_elapsed_nanoseconds: 0,
        cleanup_elapsed_nanoseconds: 0,
        worker_high_water_resident_bytes: 0,
        runtime_installed_payload_bytes: 0,
        model_installed_payload_bytes: 0,
        installed_footprint_bytes: 0,
        exceeded_limits: Vec::new(),
    };
    let result = GenerationResourceAttemptResultRecordV1::new(zero.relations(), zero_input)
        .expect("zero observations");
    assert_eq!(result.installed_footprint_bytes(), 0);

    let fixture = fixture();
    let equal = changed(input(), |value| {
        value.attempt_elapsed_nanoseconds = value.total_duration_nanoseconds;
        value.first_response_elapsed_nanoseconds = value.attempt_elapsed_nanoseconds;
        value.cleanup_elapsed_nanoseconds = value.attempt_elapsed_nanoseconds;
    });
    GenerationResourceAttemptResultRecordV1::new(fixture.relations(), equal)
        .expect("exact equality remains within a limit");
}

#[test]
fn all_exceeded_limit_identities_are_accepted_only_in_closed_order() {
    let fixture = fixture();
    let all = changed(input(), |value| {
        value.exceeded_limits = vec![
            GenerationResourceExceededLimitV1::AttemptElapsed,
            GenerationResourceExceededLimitV1::FirstResponse,
            GenerationResourceExceededLimitV1::Cleanup,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
            GenerationResourceExceededLimitV1::InstalledFootprint,
        ];
    });
    let result = GenerationResourceAttemptResultRecordV1::new(fixture.relations(), all)
        .expect("closed order");
    assert_eq!(
        serde_json::to_value(result.exceeded_limits()).expect("limit JSON"),
        serde_json::json!([
            "attempt_elapsed",
            "first_response",
            "cleanup",
            "worker_high_water_resident",
            "installed_footprint"
        ])
    );
}

#[test]
fn receipt_usage_is_required_and_exact_at_available_precision() {
    let missing = super::support::fixture_with_usage(CandidateGenerationUsageObservationV1::new(
        None, None, None,
    ));
    assert_rejected(missing.relations(), &input());
    let fixture = fixture();
    for changed_input in [
        changed(input(), |value| value.prompt_token_count += 1),
        changed(input(), |value| value.generated_token_count += 1),
        changed(input(), |value| {
            value.evaluation_duration_nanoseconds += 1_000;
        }),
    ] {
        assert_rejected(fixture.relations(), &changed_input);
    }
}

fn assert_rejected(
    relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
    input: &GenerationResourceAttemptResultRecordV1Input,
) {
    assert!(
        GenerationResourceAttemptResultRecordV1::new(relations, input.clone()).is_err(),
        "substituted relationship accepted"
    );
}

fn changed(
    mut input: GenerationResourceAttemptResultRecordV1Input,
    change: impl FnOnce(&mut GenerationResourceAttemptResultRecordV1Input),
) -> GenerationResourceAttemptResultRecordV1Input {
    change(&mut input);
    input
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
