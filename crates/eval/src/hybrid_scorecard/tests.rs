use rewrite_types::{Digest, ReasonCode, RewriteStatus};

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationSuite, ExpectedOutput, ReferenceJudgment,
    hybrid_scorecard::{
        HYBRID_SCORECARD_SCHEMA_VERSION, HybridScorecardCasePlan, HybridScorecardError,
        HybridScorecardPlan, JudgeAuthority, JudgeCaseOutcome, JudgeChoice, JudgeExecution,
        JudgeObservation, JudgeObservationBatch, JudgeObservationEvidenceClass, JudgeOrderPolicy,
        JudgePresentation, LocalJudgePolicy, MAX_HYBRID_SCORECARD_BYTES, ReleaseReviewDisposition,
        hybrid_scorecard_deterministic_policy_digest, hybrid_scorecard_plan_digest,
        hybrid_scorecard_suite_pair_digest, parse_hybrid_scorecard_plan,
        parse_judge_observation_batch, run_hybrid_scorecard,
    },
};

mod limits;

fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}

fn evaluation_case(id: &str, source: &str, candidate: &str) -> EvaluationCase {
    EvaluationCase {
        id: id.to_owned(),
        category: "positive_literal".to_owned(),
        source: source.to_owned(),
        candidate: candidate.to_owned(),
        protected_terms: Vec::new(),
        reference_judgment: ReferenceJudgment::Acceptable,
        expected_status: RewriteStatus::Rewritten,
        expected_reason: None,
        expected_output: ExpectedOutput::Candidate,
    }
}

fn suites() -> (EvaluationSuite, EvaluationSuite) {
    let sources = [
        ("case-a", "Hello world", "Hello, world!", "Hello world."),
        (
            "case-b",
            "First line\r\nSecond line\r\n",
            "First line.\r\nSecond line.\r\n",
            "First line!\r\nSecond line!\r\n",
        ),
        ("case-c", "Cafe deja vu", "Cafe, deja vu.", "Cafe deja vu!"),
        (
            "case-d",
            "Plain clear text",
            "Plain, clear text.",
            "Plain clear text!",
        ),
        (
            "case-e",
            "Ready for review",
            "Ready, for review.",
            "Ready for review!",
        ),
    ];
    let candidate_a = EvaluationSuite {
        schema_version: EVALUATION_SCHEMA_VERSION,
        cases: sources
            .iter()
            .map(|(id, source, candidate_a, _)| evaluation_case(id, source, candidate_a))
            .collect(),
    };
    let candidate_b = EvaluationSuite {
        schema_version: EVALUATION_SCHEMA_VERSION,
        cases: sources
            .iter()
            .map(|(id, source, _, candidate_b)| evaluation_case(id, source, candidate_b))
            .collect(),
    };
    (candidate_a, candidate_b)
}

fn plan(candidate_a: &EvaluationSuite, candidate_b: &EvaluationSuite) -> HybridScorecardPlan {
    let cases = candidate_a
        .cases
        .iter()
        .zip(&candidate_b.cases)
        .map(|(case_a, case_b)| HybridScorecardCasePlan {
            id: case_a.id.clone(),
            cluster_id: format!("cluster-{}", case_a.id),
            source_digest: Digest::sha256(case_a.source.as_bytes()),
            candidate_a_digest: Digest::sha256(case_a.candidate.as_bytes()),
            candidate_b_digest: Digest::sha256(case_b.candidate.as_bytes()),
            candidate_a_system_digest: digest("generator-a"),
            candidate_b_system_digest: digest("generator-b"),
            rubric_clauses: vec!["clarity".to_owned(), "meaning".to_owned()],
        })
        .collect();
    HybridScorecardPlan {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: "locked-scorecard-v1".to_owned(),
        corpus_digest: hybrid_scorecard_suite_pair_digest(candidate_a, candidate_b)
            .expect("suites are valid"),
        rubric_digest: digest("rubric"),
        deterministic_policy_digest: hybrid_scorecard_deterministic_policy_digest(),
        judge: LocalJudgePolicy {
            execution: JudgeExecution::LocalIsolated,
            authority: JudgeAuthority::TriageOnly,
            order_policy: JudgeOrderPolicy::BothOrders,
            judge_system_digest: digest("judge"),
            judge_model_reference: "judge-fixture:latest".to_owned(),
            judge_model_digest: digest("judge-model"),
            judge_prompt_contract_digest: crate::local_judge_prompt_contract_digest(),
            judge_output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
                .schema_digest,
            presentation_seed: 7,
            temperature_milli: 0,
            top_p_milli: 1_000,
            attempts_per_order: 1,
            max_judge_cases: 32,
            max_source_bytes: 4_096,
            max_candidate_bytes: 4_096,
            max_input_bytes: 16_384,
            context_token_limit: 8_192,
            output_token_limit: 512,
            max_response_bytes: 4096,
            maximum_elapsed_millis: 30_000,
        },
        cases,
    }
}

fn observation(
    case_id: &str,
    presentation: JudgePresentation,
    choice: JudgeChoice,
) -> JudgeObservation {
    JudgeObservation {
        case_id: case_id.to_owned(),
        presentation,
        choice,
        rubric_clauses: vec!["meaning".to_owned()],
    }
}

fn batch(plan: &HybridScorecardPlan) -> JudgeObservationBatch {
    use JudgeChoice::{Abstain, First, Second, Tie};
    use JudgePresentation::{CandidateAFirst, CandidateBFirst};

    JudgeObservationBatch {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        plan_digest: hybrid_scorecard_plan_digest(plan).expect("plan is valid"),
        observations: vec![
            observation("case-a", CandidateAFirst, First),
            observation("case-a", CandidateBFirst, Second),
            observation("case-b", CandidateAFirst, Second),
            observation("case-b", CandidateBFirst, First),
            observation("case-c", CandidateAFirst, Tie),
            observation("case-c", CandidateBFirst, Tie),
            observation("case-d", CandidateAFirst, Abstain),
            observation("case-d", CandidateBFirst, Tie),
            observation("case-e", CandidateAFirst, First),
            observation("case-e", CandidateBFirst, First),
        ],
    }
}

#[test]
fn plan_and_observation_batch_round_trip_under_strict_schema() {
    let (candidate_a, candidate_b) = suites();
    let plan = plan(&candidate_a, &candidate_b);
    let batch = batch(&plan);
    let plan_json = serde_json::to_string(&plan).expect("plan serializes");
    assert_eq!(
        parse_hybrid_scorecard_plan(&plan_json).expect("plan parses"),
        plan
    );
    let batch_json = serde_json::to_string(&batch).expect("batch serializes");
    assert_eq!(
        parse_judge_observation_batch(&batch_json).expect("batch parses"),
        batch
    );

    let with_unknown = plan_json.replacen('{', "{\"unexpected\":true,", 1);
    assert_eq!(
        parse_hybrid_scorecard_plan(&with_unknown),
        Err(HybridScorecardError::InvalidJson)
    );
}

#[test]
fn runs_bound_hard_gates_and_normalizes_every_two_order_outcome() {
    let (candidate_a, candidate_b) = suites();
    let plan = plan(&candidate_a, &candidate_b);
    let result = run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &batch(&plan))
        .expect("scorecard runs");
    assert_eq!(result.deterministic_total, 10);
    assert_eq!(result.deterministic_passed, 10);
    assert!(result.hard_gates_passed());
    assert_eq!(result.transformation_coverage.acceptable, 10);
    assert_eq!(result.transformation_coverage.rewritten, 10);
    assert_eq!(result.judge.total, 5);
    assert_eq!(result.judge.stable_a, 1);
    assert_eq!(result.judge.stable_b, 1);
    assert_eq!(result.judge.stable_tie, 1);
    assert_eq!(result.judge.abstained, 1);
    assert_eq!(result.judge.order_sensitive, 1);
    assert!(result.judge_observation_batch_digest.is_some());
    assert_eq!(
        result.judge_observation_evidence_class,
        Some(JudgeObservationEvidenceClass::CallerDeclared)
    );
    assert_eq!(result.declared_rubric_digest, plan.rubric_digest);
    assert_eq!(
        result.declared_judge_system_digest,
        plan.judge.judge_system_digest
    );
    assert_eq!(
        result
            .judge
            .cases
            .iter()
            .map(|case| case.outcome)
            .collect::<Vec<_>>(),
        vec![
            JudgeCaseOutcome::StableA,
            JudgeCaseOutcome::StableB,
            JudgeCaseOutcome::StableTie,
            JudgeCaseOutcome::Abstained,
            JudgeCaseOutcome::OrderSensitive,
        ]
    );
    assert_eq!(
        result.release_review,
        ReleaseReviewDisposition::RequiresHumanAdjudication
    );
    let serialized = serde_json::to_string(&result).expect("report serializes");
    assert!(!serialized.contains("Hello world"));
    assert!(!serialized.contains("candidate"));
    assert!(!serialized.contains("rationale"));

    let mut reordered_batch = batch(&plan);
    reordered_batch.observations.swap(0, 1);
    let reordered = run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &reordered_batch)
        .expect("observation order remains valid");
    assert_ne!(
        result.judge_observation_batch_digest,
        reordered.judge_observation_batch_digest
    );
    assert_eq!(result.judge, reordered.judge);
}

#[test]
fn hard_gate_failure_blocks_judge_execution() {
    let (mut candidate_a, candidate_b) = suites();
    candidate_a.cases[0].candidate.clear();
    let plan = plan(&candidate_a, &candidate_b);
    let empty = JudgeObservationBatch {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        plan_digest: hybrid_scorecard_plan_digest(&plan).expect("plan is valid"),
        observations: Vec::new(),
    };
    let blocked =
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &empty).expect("blocked report");
    assert_eq!(
        blocked.release_review,
        ReleaseReviewDisposition::BlockedByHardGate
    );
    assert_eq!(blocked.judge.total, 0);
    assert!(!blocked.hard_gates_passed());
    assert_eq!(blocked.judge_observation_batch_digest, None);
    assert_eq!(blocked.judge_observation_evidence_class, None);

    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &batch(&plan)),
        Err(HybridScorecardError::JudgeAfterHardGateFailure)
    );
}

#[test]
fn expected_abstention_cases_are_not_judge_eligible() {
    let (mut candidate_a, mut candidate_b) = suites();
    for suite in [&mut candidate_a, &mut candidate_b] {
        suite.cases[0].reference_judgment = ReferenceJudgment::Unacceptable;
        suite.cases[0].expected_status = RewriteStatus::Abstained;
        suite.cases[0].expected_reason = Some(ReasonCode::SemanticUncertain);
        suite.cases[0].expected_output = ExpectedOutput::Source;
    }
    let plan = plan(&candidate_a, &candidate_b);
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &batch(&plan)),
        Err(HybridScorecardError::CorpusMismatch)
    );
}

#[test]
fn rejects_replayed_batches_and_changed_corpus_or_policy() {
    let (candidate_a, candidate_b) = suites();
    let original = plan(&candidate_a, &candidate_b);
    let original_batch = batch(&original);

    let mut changed_plan = original.clone();
    changed_plan.judge.presentation_seed += 1;
    assert_eq!(
        run_hybrid_scorecard(&changed_plan, &candidate_a, &candidate_b, &original_batch),
        Err(HybridScorecardError::PlanMismatch)
    );

    let current_batch = batch(&original);
    let mut changed_candidate = candidate_b.clone();
    changed_candidate.cases[0].candidate.push('?');
    assert_eq!(
        run_hybrid_scorecard(&original, &candidate_a, &changed_candidate, &current_batch),
        Err(HybridScorecardError::CorpusMismatch)
    );

    let mut wrong_policy = original.clone();
    wrong_policy.deterministic_policy_digest = digest("other-policy");
    assert_eq!(
        run_hybrid_scorecard(
            &wrong_policy,
            &candidate_a,
            &candidate_b,
            &batch(&wrong_policy)
        ),
        Err(HybridScorecardError::DeterministicPolicyMismatch)
    );
}

#[test]
fn rejects_invalid_or_noncomparable_deterministic_suites() {
    let (candidate_a, candidate_b) = suites();
    let plan = plan(&candidate_a, &candidate_b);
    let empty = EvaluationSuite {
        schema_version: EVALUATION_SCHEMA_VERSION,
        cases: Vec::new(),
    };
    assert_eq!(
        hybrid_scorecard_suite_pair_digest(&empty, &empty),
        Err(HybridScorecardError::InvalidDeterministicSuites)
    );

    let mut reordered = candidate_b.clone();
    reordered.cases.swap(0, 1);
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &reordered, &batch(&plan)),
        Err(HybridScorecardError::InvalidDeterministicSuites)
    );

    let mut changed_source = candidate_b.clone();
    changed_source.cases[0].source.push('.');
    assert_eq!(
        hybrid_scorecard_suite_pair_digest(&candidate_a, &changed_source),
        Err(HybridScorecardError::InvalidDeterministicSuites)
    );

    let mut wrong_schema = candidate_b;
    wrong_schema.schema_version += 1;
    assert_eq!(
        hybrid_scorecard_suite_pair_digest(&candidate_a, &wrong_schema),
        Err(HybridScorecardError::InvalidDeterministicSuites)
    );
}

#[test]
fn rejects_invalid_plan_policy_and_case_relationships() {
    let (candidate_a, candidate_b) = suites();
    let base = plan(&candidate_a, &candidate_b);
    let mut fixture = base.clone();
    fixture.schema_version += 1;
    assert!(matches!(
        parse_hybrid_scorecard_plan(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::UnsupportedSchema(_))
    ));

    for mutate in [
        |plan: &mut HybridScorecardPlan| plan.judge.temperature_milli = 1,
        |plan: &mut HybridScorecardPlan| plan.judge.top_p_milli = 999,
        |plan: &mut HybridScorecardPlan| plan.judge.attempts_per_order = 2,
        |plan: &mut HybridScorecardPlan| plan.judge.judge_model_reference = "Bad Ref".to_owned(),
        |plan: &mut HybridScorecardPlan| plan.judge.max_judge_cases = 0,
        |plan: &mut HybridScorecardPlan| plan.judge.max_judge_cases = 1,
        |plan: &mut HybridScorecardPlan| plan.judge.max_source_bytes = 0,
        |plan: &mut HybridScorecardPlan| plan.judge.max_candidate_bytes = 0,
        |plan: &mut HybridScorecardPlan| plan.judge.max_input_bytes = 0,
        |plan: &mut HybridScorecardPlan| plan.judge.context_token_limit = 0,
        |plan: &mut HybridScorecardPlan| plan.judge.output_token_limit = 0,
        |plan: &mut HybridScorecardPlan| plan.judge.max_response_bytes = 255,
        |plan: &mut HybridScorecardPlan| plan.judge.maximum_elapsed_millis = 0,
        |plan: &mut HybridScorecardPlan| plan.cases.clear(),
    ] {
        let mut fixture = base.clone();
        mutate(&mut fixture);
        assert_eq!(
            parse_hybrid_scorecard_plan(&serde_json::to_string(&fixture).expect("serialize")),
            Err(HybridScorecardError::InvalidPlan)
        );
    }

    let mut fixture = base.clone();
    fixture.cases[0].candidate_b_digest = fixture.cases[0].candidate_a_digest.clone();
    assert!(matches!(
        parse_hybrid_scorecard_plan(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::InvalidCase { index: 0 })
    ));

    let mut fixture = base.clone();
    fixture.cases[0].candidate_a_system_digest = fixture.judge.judge_system_digest.clone();
    assert!(matches!(
        parse_hybrid_scorecard_plan(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::InvalidCase { index: 0 })
    ));

    let mut fixture = base.clone();
    fixture.cases[0].rubric_clauses.reverse();
    assert!(matches!(
        parse_hybrid_scorecard_plan(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::InvalidCase { index: 0 })
    ));

    let mut fixture = base;
    fixture.cases.swap(0, 1);
    assert!(matches!(
        parse_hybrid_scorecard_plan(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::InvalidCase { .. })
    ));
}

#[test]
fn rejects_missing_duplicate_unknown_and_unbound_observations() {
    let (candidate_a, candidate_b) = suites();
    let plan = plan(&candidate_a, &candidate_b);
    let mut fixture = batch(&plan);
    fixture.observations.pop();
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &fixture),
        Err(HybridScorecardError::InvalidObservations)
    );

    let mut fixture = batch(&plan);
    fixture.observations[1] = fixture.observations[0].clone();
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &fixture),
        Err(HybridScorecardError::InvalidObservations)
    );

    let mut fixture = batch(&plan);
    fixture.observations[0].rubric_clauses = vec!["undeclared".to_owned()];
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &fixture),
        Err(HybridScorecardError::InvalidObservations)
    );

    let mut fixture = batch(&plan);
    fixture.observations[0].case_id = "unknown-case".to_owned();
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &fixture),
        Err(HybridScorecardError::InvalidObservations)
    );

    let mut fixture = batch(&plan);
    fixture.plan_id = "other-plan".to_owned();
    assert_eq!(
        run_hybrid_scorecard(&plan, &candidate_a, &candidate_b, &fixture),
        Err(HybridScorecardError::PlanMismatch)
    );
}

#[test]
fn parsers_enforce_bounds_versions_and_batch_shape() {
    let oversized = "x".repeat(MAX_HYBRID_SCORECARD_BYTES + 1);
    assert_eq!(
        parse_hybrid_scorecard_plan(&oversized),
        Err(HybridScorecardError::TooLarge)
    );
    assert_eq!(
        parse_judge_observation_batch(&oversized),
        Err(HybridScorecardError::TooLarge)
    );
    assert_eq!(
        parse_judge_observation_batch("not-json"),
        Err(HybridScorecardError::InvalidJson)
    );

    let (candidate_a, candidate_b) = suites();
    let plan = plan(&candidate_a, &candidate_b);
    let mut fixture = batch(&plan);
    fixture.schema_version += 1;
    assert!(matches!(
        parse_judge_observation_batch(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::UnsupportedSchema(_))
    ));

    let mut fixture = batch(&plan);
    fixture.plan_id = "not a label".to_owned();
    assert_eq!(
        parse_judge_observation_batch(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::InvalidObservations)
    );

    let mut fixture = batch(&plan);
    fixture.observations[0].rubric_clauses.clear();
    assert_eq!(
        parse_judge_observation_batch(&serde_json::to_string(&fixture).expect("serialize")),
        Err(HybridScorecardError::InvalidObservations)
    );
}
