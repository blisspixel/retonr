use rewrite_inference::{
    LocalJudgeAttemptOutput, LocalJudgeByteSpan, LocalJudgeChoice, StructuredCompletionRequest,
    StructuredCompletionResponse, UsageObservation,
};
use rewrite_model::{
    CandidateJudgeCaseV1, CandidateJudgeLimitsV1, CandidateJudgeOrderPolicyV1,
    CandidateJudgePlanV1, CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations,
    CandidateJudgePresentationV1, CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1,
    OllamaRetainedSessionResponseId, RuntimeIdentity,
};
use rewrite_ollama::derive_ollama_retained_session_response_id;
use rewrite_types::{CancellationToken, Digest};

use crate::compile_candidate_deterministic_evaluation;
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_judge_execution::prompt::{
    LocalJudgeAttemptLimits, build_local_judge_attempt_request,
};
use crate::verified_candidate_batch_set::tests::{
    OfflinePairedAuthorityFixture, offline_judge_system, offline_paired_authorities,
};
use crate::{
    JudgePresentation, LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION, LocalJudgeRubric, LocalJudgeRubricClause,
    local_judge_prompt_contract_digest, local_judge_rubric_digest,
};

use super::super::CandidateJudgeAttemptNormalizationInput;

pub(super) const SOURCE: &str = "Acme 42 needs polish.";

pub(super) struct NormalizationFixture {
    pub(super) plan: CandidateJudgePlanV1,
    pub(super) schedule: CandidateJudgeScheduleV1,
    pub(super) requests: Vec<StructuredCompletionRequest>,
    pub(super) request_aggregate: CandidateJudgeRequestAggregateV1,
    pub(super) case_key: String,
    pub(super) admitted_clauses: Vec<String>,
    pub(super) candidate_a: String,
    pub(super) candidate_b: String,
}

impl NormalizationFixture {
    pub(super) fn new() -> Self {
        Self::named("normalization")
    }

    pub(super) fn foreign() -> Self {
        Self::named("foreign-normalization")
    }

    fn named(label: &str) -> Self {
        let material_fixture = Fixture::judge_pair();
        let material = material_fixture.verify().expect("material");
        let pair =
            offline_paired_authorities(&material_fixture.suite, &material_fixture.cases, label);
        let judge_system = offline_judge_system(
            &format!("{label} judge"),
            local_judge_prompt_contract_digest(),
            rewrite_inference::local_judge_attempt_output_contract().schema_digest,
        );
        let plan = CandidateJudgePlanV1::new(
            relations(&pair, &judge_system),
            CandidateJudgePlanV1Input {
                case_material_set_digest: material.case_material_set_digest().clone(),
                rubric_digest: local_judge_rubric_digest(&rubric()).expect("rubric"),
                cases: vec![
                    CandidateJudgeCaseV1::new(
                        material_fixture.cases[0].case_id().clone(),
                        material_fixture.contracts[0].rubric_clause_ids().to_vec(),
                    )
                    .expect("judge case"),
                ],
                order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
                presentation_seed: 29,
                attempts_per_order: 1,
                limits: limits(),
                prompt_contract_digest: local_judge_prompt_contract_digest(),
                output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
                    .schema_digest,
            },
        )
        .expect("judge plan");
        let deterministic = compile_candidate_deterministic_evaluation(
            &pair.candidate_a,
            &pair.candidate_b,
            &material,
            &CancellationToken::new(),
        )
        .expect("deterministic record");
        let schedule =
            CandidateJudgeScheduleV1::new(&plan, deterministic.candidate_receipt_pair_set_id())
                .expect("schedule");
        let candidate_a = pair
            .candidate_a
            .selected_candidates(&CancellationToken::new())
            .expect("candidate A")[0]
            .text
            .clone();
        let candidate_b = pair
            .candidate_b
            .selected_candidates(&CancellationToken::new())
            .expect("candidate B")[0]
            .text
            .clone();
        let rubric = rubric();
        let clauses = rubric.clauses.iter().collect::<Vec<_>>();
        let request_limits = request_limits(&plan);
        let requests = schedule
            .entries()
            .iter()
            .map(|entry| {
                build_local_judge_attempt_request(
                    material_fixture.contracts[0].case_key(),
                    SOURCE,
                    &candidate_a,
                    &candidate_b,
                    presentation(entry.presentation()),
                    &clauses,
                    judge_system.model_artifact_id(),
                    judge_system.model_artifact_id().digest(),
                    entry.seed(),
                    request_limits,
                )
                .expect("request")
            })
            .collect::<Vec<_>>();
        let request_aggregate = CandidateJudgeRequestAggregateV1::new(
            &plan,
            &schedule,
            requests
                .iter()
                .map(StructuredCompletionRequest::structured_request_binding_id)
                .collect(),
        )
        .expect("request aggregate");
        Self {
            plan,
            schedule,
            requests,
            request_aggregate,
            case_key: material_fixture.contracts[0].case_key().to_owned(),
            admitted_clauses: material_fixture.contracts[0].rubric_clause_ids().to_vec(),
            candidate_a,
            candidate_b,
        }
    }

    pub(super) fn output(&self, choice: LocalJudgeChoice) -> LocalJudgeAttemptOutput {
        LocalJudgeAttemptOutput {
            schema_version: rewrite_inference::LOCAL_JUDGE_ATTEMPT_OUTPUT_SCHEMA_VERSION,
            case_id: self.case_key.clone(),
            choice,
            rubric_clauses: vec!["fidelity".to_owned()],
            source_spans: vec![span(0, 4)],
            first_candidate_spans: vec![span(0, 4)],
            second_candidate_spans: vec![span(0, 4)],
        }
    }

    pub(super) fn response(
        &self,
        cursor: usize,
        output: &LocalJudgeAttemptOutput,
    ) -> StructuredCompletionResponse {
        response(&self.requests[cursor], output)
    }

    pub(super) fn input<'input>(
        &'input self,
        cursor: usize,
        response: &'input StructuredCompletionResponse,
        response_id: &'input OllamaRetainedSessionResponseId,
    ) -> CandidateJudgeAttemptNormalizationInput<'input> {
        let entry = &self.schedule.entries()[cursor];
        let (first, second) = match entry.presentation() {
            CandidateJudgePresentationV1::CandidateAFirst => {
                (self.candidate_a.as_str(), self.candidate_b.as_str())
            }
            CandidateJudgePresentationV1::CandidateBFirst => {
                (self.candidate_b.as_str(), self.candidate_a.as_str())
            }
        };
        CandidateJudgeAttemptNormalizationInput {
            plan: &self.plan,
            schedule: &self.schedule,
            request_aggregate: &self.request_aggregate,
            schedule_cursor: cursor,
            request: &self.requests[cursor],
            response,
            retained_response_id: response_id,
            case_key: &self.case_key,
            presentation: entry.presentation(),
            admitted_rubric_clause_ids: &self.admitted_clauses,
            source: SOURCE,
            presented_first: first,
            presented_second: second,
        }
    }
}

pub(super) fn response(
    request: &StructuredCompletionRequest,
    output: &LocalJudgeAttemptOutput,
) -> StructuredCompletionResponse {
    StructuredCompletionResponse::complete(
        request,
        RuntimeIdentity {
            backend: "normalization-test".to_owned(),
            version: "1".to_owned(),
            digest: Some(Digest::sha256(b"normalization runtime")),
        },
        request.artifact_id.clone(),
        request.artifact_digest.clone(),
        serde_json::to_string(output).expect("output JSON"),
        UsageObservation {
            input_tokens: Some(10),
            output_tokens: Some(5),
            generation_micros: Some(50),
        },
    )
    .expect("structured response")
}

pub(super) fn response_id(
    response: &StructuredCompletionResponse,
) -> OllamaRetainedSessionResponseId {
    derive_ollama_retained_session_response_id(response)
}

pub(super) const fn span(start: u32, end: u32) -> LocalJudgeByteSpan {
    LocalJudgeByteSpan { start, end }
}

fn relations<'a>(
    pair: &'a OfflinePairedAuthorityFixture,
    judge_system: &'a rewrite_model::GenerationSystemRecordV1,
) -> CandidateJudgePlanV1Relations<'a> {
    CandidateJudgePlanV1Relations {
        qualification_plan: &pair.qualification_plan,
        suite: &pair.suite,
        repetition: &pair.repetition,
        selection_policy: &pair.selection_policy,
        planned_attempts: &pair.planned_attempts,
        candidate_a_system: &pair.candidate_a_system,
        candidate_b_system: &pair.candidate_b_system,
        judge_system,
    }
}

fn rubric() -> LocalJudgeRubric {
    LocalJudgeRubric {
        schema_version: LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION,
        clauses: vec![
            LocalJudgeRubricClause {
                id: "fidelity".to_owned(),
                instruction: "Prefer exact meaning preservation.".to_owned(),
            },
            LocalJudgeRubricClause {
                id: "protected-values".to_owned(),
                instruction: "Require every protected value unchanged.".to_owned(),
            },
        ],
    }
}

fn limits() -> CandidateJudgeLimitsV1 {
    CandidateJudgeLimitsV1::new(1, 4_096, 4_096, 16_384, 8_192, 512, 4_096, 30_000).expect("limits")
}

fn request_limits(plan: &CandidateJudgePlanV1) -> LocalJudgeAttemptLimits {
    let limits = plan.limits();
    LocalJudgeAttemptLimits {
        maximum_source_bytes: u64::from(limits.maximum_source_bytes()),
        maximum_candidate_bytes: u64::from(limits.maximum_candidate_bytes()),
        maximum_input_bytes: u64::from(limits.maximum_complete_input_bytes()),
        context_token_limit: limits.maximum_context_tokens(),
        output_token_limit: limits.maximum_output_tokens(),
        maximum_response_bytes: u64::from(limits.maximum_response_bytes()),
    }
}

const fn presentation(value: CandidateJudgePresentationV1) -> JudgePresentation {
    match value {
        CandidateJudgePresentationV1::CandidateAFirst => JudgePresentation::CandidateAFirst,
        CandidateJudgePresentationV1::CandidateBFirst => JudgePresentation::CandidateBFirst,
    }
}
