use super::{
    GenerationClusterRecordV1, GenerationQualificationContractError,
    GenerationQualificationPlanLimitsV1, GenerationQualificationPlanV1,
    GenerationQualificationPlanV1Input, GenerationSuiteManifestV1,
    MAX_GENERATION_CANDIDATES_PER_COMPLETION, MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES, MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
    MAX_GENERATION_MACHINE_KEY_BYTES, MAX_GENERATION_RETAINED_INPUT_BYTES,
    MAX_GENERATION_SUITE_CASES, MAX_GENERATION_SYSTEMS_PER_PLAN, MAX_PLANNED_GENERATION_ATTEMPTS,
    attempt_records, case, digest, fixture, limits, system_records,
};

fn maximum_attempts_u32() -> u32 {
    u32::try_from(MAX_PLANNED_GENERATION_ATTEMPTS).expect("hard attempt maximum fits u32")
}

#[test]
fn machine_key_boundary_is_exact_and_canonical() {
    let maximum = "a".repeat(MAX_GENERATION_MACHINE_KEY_BYTES);
    assert!(GenerationClusterRecordV1::new(maximum, digest("policy")).is_ok());
    for invalid in [
        String::new(),
        "a".repeat(MAX_GENERATION_MACHINE_KEY_BYTES + 1),
        "Upper".to_owned(),
        "with space".to_owned(),
        "with/slash".to_owned(),
        "unicode-é".to_owned(),
        "-leading".to_owned(),
        "trailing_".to_owned(),
        "empty--component".to_owned(),
        "empty_-component".to_owned(),
    ] {
        assert_eq!(
            GenerationClusterRecordV1::new(invalid, digest("policy")),
            Err(GenerationQualificationContractError::InvalidMachineKey)
        );
    }
}

#[test]
fn suite_accepts_exact_maximum_and_rejects_one_more() {
    let fixture = fixture();
    let cases: Vec<_> = (0..MAX_GENERATION_SUITE_CASES)
        .map(|index| {
            case(
                &fixture.cluster,
                format!("case-{index}"),
                &format!("source {index}"),
            )
        })
        .collect();
    let suite =
        GenerationSuiteManifestV1::new(digest("maximum protocol"), &cases).expect("maximum suite");
    assert_eq!(suite.case_ids().len(), MAX_GENERATION_SUITE_CASES);
    let mut excessive = cases;
    excessive.push(case(&fixture.cluster, "case-extra", "extra source"));
    assert_eq!(
        GenerationSuiteManifestV1::new(digest("protocol"), &excessive),
        Err(GenerationQualificationContractError::InvalidCollectionSize)
    );
    assert_eq!(
        GenerationSuiteManifestV1::new(digest("protocol"), &[]),
        Err(GenerationQualificationContractError::InvalidCollectionSize)
    );
}

#[test]
fn plan_accepts_exact_attempt_and_system_maxima() {
    let fixture = fixture();
    let systems = system_records(MAX_GENERATION_SYSTEMS_PER_PLAN);
    let attempts = attempt_records(
        &fixture.suite,
        &fixture.cases[0],
        &fixture.cluster,
        &fixture.repetition,
        &systems,
        MAX_PLANNED_GENERATION_ATTEMPTS,
    );
    let plan = GenerationQualificationPlanV1::new(
        &fixture.suite,
        &fixture.repetitions,
        &systems,
        &attempts,
        GenerationQualificationPlanV1Input {
            limits: limits(maximum_attempts_u32()),
            selection_policy_digest: digest("selection"),
            failure_policy_digest: digest("failure"),
        },
    )
    .expect("maximum plan");
    assert_eq!(
        plan.generation_system_ids().len(),
        MAX_GENERATION_SYSTEMS_PER_PLAN
    );
    assert_eq!(
        plan.planned_attempt_ids().len(),
        MAX_PLANNED_GENERATION_ATTEMPTS
    );

    let excessive_systems = system_records(MAX_GENERATION_SYSTEMS_PER_PLAN + 1);
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &fixture.repetitions,
            &excessive_systems,
            &fixture.attempts,
            GenerationQualificationPlanV1Input {
                limits: limits(4),
                selection_policy_digest: digest("selection"),
                failure_policy_digest: digest("failure"),
            },
        ),
        Err(GenerationQualificationContractError::InvalidCollectionSize)
    );
}

#[test]
fn every_limit_accepts_its_maximum_and_rejects_zero_or_overflow_values() {
    let maximum = GenerationQualificationPlanLimitsV1::new(
        MAX_GENERATION_CANDIDATES_PER_COMPLETION,
        maximum_attempts_u32(),
        MAX_GENERATION_RETAINED_INPUT_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES,
        MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    )
    .expect("maximum limits");
    assert_eq!(maximum.maximum_candidates_per_completion(), 16);
    assert_eq!(maximum.maximum_predeclared_attempts(), 1_024);
    assert_eq!(maximum.maximum_retained_input_bytes(), 4 * 1_024 * 1_024);
    assert_eq!(maximum.maximum_evidence_bundle_entries(), 4_096);
    assert_eq!(maximum.maximum_evidence_relative_path_bytes(), 512);
    assert_eq!(maximum.maximum_evidence_bundle_bytes(), 256 * 1_024 * 1_024);

    let invalid = [
        (0, 1, 1, 1, 1, 1),
        (17, 1, 1, 1, 1, 1),
        (1, 0, 1, 1, 1, 1),
        (1, u32::MAX, 1, 1, 1, 1),
        (1, 1, 0, 1, 1, 1),
        (1, 1, u64::MAX, 1, 1, 1),
        (1, 1, 1, 0, 1, 1),
        (1, 1, 1, u32::MAX, 1, 1),
        (1, 1, 1, 1, 0, 1),
        (1, 1, 1, 1, u32::MAX, 1),
        (1, 1, 1, 1, 1, 0),
        (1, 1, 1, 1, 1, u64::MAX),
    ];
    for (candidates, attempts, input, entries, path, bundle) in invalid {
        assert_eq!(
            GenerationQualificationPlanLimitsV1::new(
                candidates, attempts, input, entries, path, bundle,
            ),
            Err(GenerationQualificationContractError::InvalidLimits)
        );
    }
}

#[test]
fn plan_requires_nonempty_system_and_attempt_arrays() {
    let fixture = fixture();
    for (systems, attempts) in [
        (Vec::new(), fixture.attempts.clone()),
        (fixture.systems.clone(), Vec::new()),
    ] {
        assert_eq!(
            GenerationQualificationPlanV1::new(
                &fixture.suite,
                &fixture.repetitions,
                &systems,
                &attempts,
                GenerationQualificationPlanV1Input {
                    limits: limits(4),
                    selection_policy_digest: digest("selection"),
                    failure_policy_digest: digest("failure"),
                },
            ),
            Err(GenerationQualificationContractError::InvalidCollectionSize)
        );
    }
}
