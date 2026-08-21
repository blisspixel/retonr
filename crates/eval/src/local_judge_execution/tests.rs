use std::time::Duration;

use rewrite_inference::OperationContext;
use rewrite_types::{CancellationToken, Digest, RewriteStatus};
use serde_json::{Value, json};

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, ExpectedOutput, HybridScorecardCasePlan,
    JudgeAuthority, JudgeExecution, JudgeOrderPolicy, LocalJudgePolicy, ReferenceJudgment,
    hybrid_scorecard_deterministic_policy_digest, hybrid_scorecard_suite_pair_digest,
};

use super::*;

mod fixture;

use fixture::{JudgeServer, assert_judge_wire_policy, model_binding, output, preflighted_session};

fn suites() -> (EvaluationSuite, EvaluationSuite) {
    let case = |candidate: &str| EvaluationCase {
        id: "case-a".to_owned(),
        category: "positive_literal".to_owned(),
        source: "Hello world".to_owned(),
        candidate: candidate.to_owned(),
        protected_terms: Vec::new(),
        reference_judgment: ReferenceJudgment::Acceptable,
        expected_status: RewriteStatus::Rewritten,
        expected_reason: None,
        expected_output: ExpectedOutput::Candidate,
    };
    (
        EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: vec![case("Hello, world!")],
        },
        EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: vec![case("Hello world.")],
        },
    )
}

fn rubric() -> LocalJudgeRubric {
    LocalJudgeRubric {
        schema_version: LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION,
        clauses: vec![LocalJudgeRubricClause {
            id: "meaning".to_owned(),
            instruction: "Prefer the candidate that preserves the source meaning.".to_owned(),
        }],
    }
}

fn plan(candidate_a: &EvaluationSuite, candidate_b: &EvaluationSuite) -> HybridScorecardPlan {
    let rubric = rubric();
    let model = model_binding();
    HybridScorecardPlan {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: "local-judge-v1".to_owned(),
        corpus_digest: hybrid_scorecard_suite_pair_digest(candidate_a, candidate_b)
            .expect("suite pair"),
        rubric_digest: local_judge_rubric_digest(&rubric).expect("rubric digest"),
        deterministic_policy_digest: hybrid_scorecard_deterministic_policy_digest(),
        judge: LocalJudgePolicy {
            execution: JudgeExecution::LocalIsolated,
            authority: JudgeAuthority::TriageOnly,
            order_policy: JudgeOrderPolicy::BothOrders,
            judge_system_digest: Digest::sha256(b"declared-judge-system"),
            judge_model_reference: model.reference().to_owned(),
            judge_model_digest: model.artifact_digest().clone(),
            judge_prompt_contract_digest: local_judge_prompt_contract_digest(),
            judge_output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
                .schema_digest,
            presentation_seed: 17,
            temperature_milli: 0,
            top_p_milli: 1_000,
            attempts_per_order: 1,
            max_judge_cases: 8,
            max_source_bytes: 4_096,
            max_candidate_bytes: 4_096,
            max_input_bytes: 16_384,
            context_token_limit: 8_192,
            output_token_limit: 512,
            max_response_bytes: 4_096,
            maximum_elapsed_millis: 30_000,
        },
        cases: vec![HybridScorecardCasePlan {
            id: "case-a".to_owned(),
            cluster_id: "cluster-a".to_owned(),
            source_digest: Digest::sha256(candidate_a.cases[0].source.as_bytes()),
            candidate_a_digest: Digest::sha256(candidate_a.cases[0].candidate.as_bytes()),
            candidate_b_digest: Digest::sha256(candidate_b.cases[0].candidate.as_bytes()),
            candidate_a_system_digest: Digest::sha256(b"generator-a"),
            candidate_b_system_digest: Digest::sha256(b"generator-b"),
            rubric_clauses: vec!["meaning".to_owned()],
        }],
    }
}

fn context(token: &CancellationToken) -> OperationContext<'_> {
    OperationContext::new(
        token,
        Some(std::time::Instant::now() + Duration::from_secs(10)),
    )
}

fn executed(outcome: LocalJudgeExecutionOutcome) -> LocalJudgeExecution {
    match outcome {
        LocalJudgeExecutionOutcome::Executed(execution) => *execution,
        LocalJudgeExecutionOutcome::BlockedByHardGate(_) => panic!("unexpected hard gate block"),
    }
}

#[tokio::test]
async fn executes_both_blinded_orders_once_on_one_stream() {
    let server =
        JudgeServer::start(vec![output("case-a", "first"), output("case-a", "second")]).await;
    let token = CancellationToken::new();
    let mut session = preflighted_session(&server, &token).await;
    let (candidate_a, candidate_b) = suites();
    let execution = executed(
        run_local_judge_execution(
            &plan(&candidate_a, &candidate_b),
            &candidate_a,
            &candidate_b,
            &rubric(),
            &model_binding(),
            &mut session,
            context(&token),
        )
        .await
        .expect("execute judge"),
    );
    assert_eq!(execution.observations().observations.len(), 2);
    assert_ne!(
        execution.observations().observations[0].presentation,
        execution.observations().observations[1].presentation
    );
    assert!(execution.report().hard_gates_passed());
    assert_eq!(execution.receipt().attempt_count(), 2);
    assert_eq!(execution.receipt().first_response_ordinal(), 8);
    assert_eq!(execution.receipt().last_response_ordinal(), 21);
    assert_eq!(
        execution.receipt().evidence_class(),
        LocalJudgeExecutionEvidenceClass::RetainedTransportBindingOnly
    );

    let result = server.finish().await;
    assert_eq!(result.accepts, 1);
    assert_eq!(result.generate_requests.len(), 2);
    assert_eq!(result.paths.len(), 21);
    assert_judge_wire_policy(&result.generate_requests);
    let prompts = result
        .generate_requests
        .iter()
        .map(|request| {
            serde_json::from_str::<Value>(request["prompt"].as_str().expect("prompt string"))
                .expect("canonical prompt")
        })
        .collect::<Vec<_>>();
    assert_eq!(prompts[0]["source"], "Hello world");
    assert_eq!(prompts[0]["case_id"], "case-a");
    assert_eq!(prompts[0]["rubric"][0]["id"], "meaning");
    assert_eq!(prompts[0].get("candidate_a"), None);
    assert_eq!(prompts[0].get("candidate_b"), None);
    assert_eq!(
        prompts[0]["first_candidate"],
        prompts[1]["second_candidate"]
    );
    assert_eq!(
        prompts[0]["second_candidate"],
        prompts[1]["first_candidate"]
    );
    assert_ne!(
        result.generate_requests[0]["options"]["seed"],
        result.generate_requests[1]["options"]["seed"]
    );
    let rerun = run_local_judge_execution(
        &plan(&candidate_a, &candidate_b),
        &candidate_a,
        &candidate_b,
        &rubric(),
        &model_binding(),
        &mut session,
        context(&token),
    )
    .await;
    assert!(matches!(rerun, Err(LocalJudgeExecutionError::Session(_))));
}

#[tokio::test]
async fn deterministic_failure_makes_no_judge_call() {
    let server = JudgeServer::start(Vec::new()).await;
    let token = CancellationToken::new();
    let mut session = preflighted_session(&server, &token).await;
    let (mut candidate_a, candidate_b) = suites();
    candidate_a.cases[0].candidate.clear();
    let outcome = run_local_judge_execution(
        &plan(&candidate_a, &candidate_b),
        &candidate_a,
        &candidate_b,
        &rubric(),
        &model_binding(),
        &mut session,
        context(&token),
    )
    .await
    .expect("blocked outcome");
    assert!(matches!(
        outcome,
        LocalJudgeExecutionOutcome::BlockedByHardGate(_)
    ));
    session.invalidate();
    let result = server.finish().await;
    assert_eq!(result.paths.len(), 7);
    assert!(result.generate_requests.is_empty());
}

#[tokio::test]
async fn output_contract_relationship_failures_poison_without_retry() {
    let invalid_outputs = [
        "{}".to_owned(),
        output("wrong-case", "first"),
        json!({
            "schema_version": 1,
            "case_id": "case-a",
            "choice": "first",
            "rubric_clauses": ["clarity"],
            "source_spans": [],
            "first_candidate_spans": [],
            "second_candidate_spans": []
        })
        .to_string(),
        json!({
            "schema_version": 1,
            "case_id": "case-a",
            "choice": "first",
            "rubric_clauses": ["meaning"],
            "source_spans": [{"start": 0, "end": 999}],
            "first_candidate_spans": [],
            "second_candidate_spans": []
        })
        .to_string(),
    ];
    for (index, invalid) in invalid_outputs.into_iter().enumerate() {
        let server = JudgeServer::start(vec![invalid]).await;
        let token = CancellationToken::new();
        let mut session = preflighted_session(&server, &token).await;
        let (candidate_a, candidate_b) = suites();
        let plan = plan(&candidate_a, &candidate_b);
        let first = run_local_judge_execution(
            &plan,
            &candidate_a,
            &candidate_b,
            &rubric(),
            &model_binding(),
            &mut session,
            context(&token),
        )
        .await;
        assert!(first.is_err());
        let second = run_local_judge_execution(
            &plan,
            &candidate_a,
            &candidate_b,
            &rubric(),
            &model_binding(),
            &mut session,
            context(&token),
        )
        .await;
        assert!(matches!(second, Err(LocalJudgeExecutionError::Session(_))));
        let result = server.finish().await;
        assert_eq!(result.accepts, 1);
        assert_eq!(result.generate_requests.len(), 1);
        assert_eq!(result.paths.len(), if index == 0 { 11 } else { 14 });
    }
}

#[tokio::test]
async fn input_limit_failure_precedes_judge_traffic() {
    let server = JudgeServer::start(Vec::new()).await;
    let token = CancellationToken::new();
    let mut session = preflighted_session(&server, &token).await;
    let (candidate_a, candidate_b) = suites();

    let mut limited = plan(&candidate_a, &candidate_b);
    limited.judge.max_source_bytes = 16;
    limited.judge.max_candidate_bytes = 16;
    limited.judge.max_input_bytes = 16;
    assert!(matches!(
        run_local_judge_execution(
            &limited,
            &candidate_a,
            &candidate_b,
            &rubric(),
            &model_binding(),
            &mut session,
            context(&token)
        )
        .await,
        Err(LocalJudgeExecutionError::InputLimitExceeded)
    ));

    session.invalidate();
    let result = server.finish().await;
    assert_eq!(result.paths.len(), 7);
    assert!(result.generate_requests.is_empty());
}

#[tokio::test]
async fn rubric_model_and_contract_drift_fail_before_judge_traffic() {
    let server = JudgeServer::start(Vec::new()).await;
    let token = CancellationToken::new();
    let mut session = preflighted_session(&server, &token).await;
    let (candidate_a, candidate_b) = suites();

    let mut wrong_rubric = rubric();
    wrong_rubric.clauses[0].instruction.push_str(" Exactly.");
    assert!(matches!(
        run_local_judge_execution(
            &plan(&candidate_a, &candidate_b),
            &candidate_a,
            &candidate_b,
            &wrong_rubric,
            &model_binding(),
            &mut session,
            context(&token)
        )
        .await,
        Err(LocalJudgeExecutionError::RubricMismatch)
    ));

    let wrong_digest = Digest::sha256(b"wrong-model");
    let wrong_model = rewrite_ollama::OllamaModelBinding::new(
        "wrong:latest",
        rewrite_model::ArtifactId::from_digest(wrong_digest.clone()),
        wrong_digest,
    )
    .expect("wrong binding");
    assert!(matches!(
        run_local_judge_execution(
            &plan(&candidate_a, &candidate_b),
            &candidate_a,
            &candidate_b,
            &rubric(),
            &wrong_model,
            &mut session,
            context(&token)
        )
        .await,
        Err(LocalJudgeExecutionError::ModelBindingMismatch)
    ));

    let expected_model = model_binding();
    let alias_model = rewrite_ollama::OllamaModelBinding::new(
        "alias:latest",
        expected_model.artifact_id().clone(),
        expected_model.artifact_digest().clone(),
    )
    .expect("alias binding");
    assert!(matches!(
        run_local_judge_execution(
            &plan(&candidate_a, &candidate_b),
            &candidate_a,
            &candidate_b,
            &rubric(),
            &alias_model,
            &mut session,
            context(&token)
        )
        .await,
        Err(LocalJudgeExecutionError::ModelBindingMismatch)
    ));

    for drift in 0..3 {
        let mut drifted = plan(&candidate_a, &candidate_b);
        match drift {
            0 => drifted.judge.judge_prompt_contract_digest = Digest::sha256(b"wrong-prompt"),
            1 => drifted.judge.judge_output_schema_digest = Digest::sha256(b"wrong-output"),
            _ => drifted.judge.top_p_milli = 999,
        }
        assert!(matches!(
            run_local_judge_execution(
                &drifted,
                &candidate_a,
                &candidate_b,
                &rubric(),
                &model_binding(),
                &mut session,
                context(&token)
            )
            .await,
            Err(LocalJudgeExecutionError::InvalidPolicy)
        ));
    }
    session.invalidate();
    let result = server.finish().await;
    assert_eq!(result.paths.len(), 7);
    assert!(result.generate_requests.is_empty());
}

#[tokio::test]
async fn cancellation_and_deadline_fail_before_judge_traffic() {
    let server = JudgeServer::start(Vec::new()).await;
    let token = CancellationToken::new();
    let mut session = preflighted_session(&server, &token).await;
    let (candidate_a, candidate_b) = suites();

    for invalid_context in [
        OperationContext::new(&token, None),
        OperationContext::new(
            &token,
            Some(std::time::Instant::now() + Duration::from_secs(31)),
        ),
        OperationContext::new(
            &token,
            Some(
                std::time::Instant::now()
                    .checked_sub(Duration::from_millis(1))
                    .expect("one millisecond is representable"),
            ),
        ),
    ] {
        assert!(matches!(
            run_local_judge_execution(
                &plan(&candidate_a, &candidate_b),
                &candidate_a,
                &candidate_b,
                &rubric(),
                &model_binding(),
                &mut session,
                invalid_context
            )
            .await,
            Err(LocalJudgeExecutionError::InvalidOperationContext)
        ));
    }

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        run_local_judge_execution(
            &plan(&candidate_a, &candidate_b),
            &candidate_a,
            &candidate_b,
            &rubric(),
            &model_binding(),
            &mut session,
            context(&cancelled)
        )
        .await,
        Err(LocalJudgeExecutionError::InvalidOperationContext)
    ));
    session.invalidate();
    let result = server.finish().await;
    assert_eq!(result.paths.len(), 7);
    assert!(result.generate_requests.is_empty());
}

#[tokio::test]
async fn receipt_is_redacted_and_sensitive_to_exact_responses() {
    async fn execute(choice: &str) -> LocalJudgeExecutionReceipt {
        let server =
            JudgeServer::start(vec![output("case-a", choice), output("case-a", choice)]).await;
        let token = CancellationToken::new();
        let mut session = preflighted_session(&server, &token).await;
        let (candidate_a, candidate_b) = suites();
        let execution = executed(
            run_local_judge_execution(
                &plan(&candidate_a, &candidate_b),
                &candidate_a,
                &candidate_b,
                &rubric(),
                &model_binding(),
                &mut session,
                context(&token),
            )
            .await
            .expect("judge execution"),
        );
        let receipt = execution.into_parts().2;
        let _result = server.finish().await;
        receipt
    }

    let first = execute("first").await;
    let second = execute("second").await;
    assert_ne!(
        first.response_receipts_digest(),
        second.response_receipts_digest()
    );
    assert_ne!(first.binding_digest(), second.binding_digest());
    let debug = format!("{first:?}");
    for private in ["Hello world", "Hello, world!", "Hello world."] {
        assert!(!debug.contains(private));
    }
}

#[test]
fn span_validation_requires_bounds_and_utf8_boundaries() {
    assert!(valid_spans(&[LocalJudgeByteSpan { start: 0, end: 2 }], "é"));
    assert!(!valid_spans(
        &[LocalJudgeByteSpan { start: 1, end: 2 }],
        "é"
    ));
    assert!(!valid_spans(
        &[LocalJudgeByteSpan { start: 0, end: 3 }],
        "é"
    ));
}

#[test]
fn evidence_class_is_explicitly_limited() {
    assert_eq!(
        local_judge_prompt_contract_digest(),
        Digest::from_sha256_hex("918f12eed8459c8c6d72587740c3fe94d925b2d7f4e511befae4f95801565988")
            .expect("golden prompt contract digest")
    );
    assert_eq!(
        LocalJudgeExecutionEvidenceClass::RetainedTransportBindingOnly,
        LocalJudgeExecutionEvidenceClass::RetainedTransportBindingOnly
    );
}
