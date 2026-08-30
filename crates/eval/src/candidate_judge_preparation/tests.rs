use rewrite_model::{
    CandidateJudgeCaseV1, CandidateJudgeLimitsV1, CandidateJudgeOrderPolicyV1,
    CandidateJudgePlanV1Input, CandidateJudgePresentationV1, GenerationQualificationContractError,
    GenerationSystemRecordV1, StructuredCompletionRequestBindingId,
};
use rewrite_types::{CancellationToken, Digest};

use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_judge_execution::prompt::{
    LocalJudgeAttemptLimits, build_local_judge_attempt_request,
};
use crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl;
use crate::verified_candidate_batch_set::tests::{
    OfflinePairedAuthorityFixture, offline_judge_system, offline_judge_system_with_model,
    offline_paired_authorities,
};
use crate::{
    CandidateDeterministicCompilerError, LocalJudgeRubricClause, LocalJudgeRubricError,
    VerifiedCandidateBatchSetError, VerifiedGenerationCaseMaterialError,
    local_judge_prompt_contract_digest, local_judge_rubric_digest,
};

use super::*;

#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/request_branches.rs"]
pub(crate) mod request_branches;

pub(crate) struct PreparationConfig {
    pub(super) case_indices: Vec<usize>,
    pub(super) case_clauses: Option<Vec<Vec<String>>>,
    pub(super) plan_rubric: LocalJudgeRubric,
    pub(super) supplied_rubric: LocalJudgeRubric,
    pub(super) prompt_digest: Digest,
    pub(super) output_schema_digest: Digest,
    pub(super) limits: CandidateJudgeLimitsV1,
    pub(super) foreign_relation_model: bool,
    pub(super) fail_candidate_a_on_call: Option<usize>,
    pub(super) fail_candidate_b_on_call: Option<usize>,
}

pub(crate) struct PreparationResult<'store> {
    pub(crate) result:
        Result<CandidateJudgePreparationOutcome<'store>, CandidateJudgePreparationError>,
    pub(super) candidate_a_calls: usize,
    pub(super) candidate_b_calls: usize,
    pub(super) candidate_a_control: OfflineBatchFailureControl,
    pub(super) candidate_b_control: OfflineBatchFailureControl,
}

#[test]
fn passed_authorities_prepare_frozen_exact_two_pass_requests() {
    let material_fixture = Fixture::judge_triple();
    let prepared = prepare(
        &material_fixture,
        "judge-ready",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let outcome = prepared.result.expect("ready outcome");
    let debug = format!("{outcome:?}");
    assert!(debug.contains("Ready"));
    assert!(!debug.contains("Acme"));
    let CandidateJudgePreparationOutcome::Ready(ready) = outcome else {
        panic!("passed deterministic record must prepare judge run")
    };
    assert_eq!(ready.deterministic_evaluation().passed(), 6);
    assert_eq!(ready.judge_plan().cases().len(), 2);
    assert_eq!(ready.judge_schedule().entry_count(), 4);
    assert_eq!(ready.request_aggregate().entry_count(), 4);
    assert_eq!(ready.eligible_semantic_indices, [0, 2]);
    assert!(
        ready
            .judge_schedule()
            .entries()
            .iter()
            .all(|entry| entry.case_id() != material_fixture.cases[1].case_id())
    );
    assert_eq!(
        ready
            .judge_plan()
            .candidate_judge_plan_id()
            .digest()
            .as_str(),
        "8328d2e775d0c1d0e3ef9efbd14445eec277d836a2bb6ff30464b5201a105647"
    );
    assert_eq!(
        ready
            .judge_schedule()
            .candidate_judge_schedule_id()
            .digest()
            .as_str(),
        "d731f94046451587e29b1877d84db15df6662d77b34f1cfbb37f7c2e0fd16e1a"
    );
    assert_eq!(
        ready
            .request_aggregate()
            .request_aggregate_id()
            .digest()
            .as_str(),
        "38472722c3ca91345523cea50ab86c7353c0cf54495b25befab37749bb12cd38"
    );
    let first_pass = ready
        .request_aggregate()
        .structured_request_binding_ids()
        .to_vec();
    assert_eq!(first_pass, rederive_second_pass_request_ids(&ready));
    assert_eq!(
        first_pass
            .iter()
            .map(|id| id.digest().as_str())
            .collect::<Vec<_>>(),
        [
            "5f21f0918fe1901269772780b476497841a4d2c830874bde0cd0c3976b8d7ad2",
            "65cf939eb34e395ec12ae5c2927876aa619881e9cf86319564c770ef04b7d81e",
            "c834b234fe00baa14e742398db57a6763636af4a1b04ea0700fc5878ec31c868",
            "4a094c477c70671d7e523b632fe3a8f679e75c77f21187623987d24d6bf71778",
        ]
    );
    assert_eq!(prepared.candidate_a_calls, 24);
    assert_eq!(prepared.candidate_b_calls, 24);
    ready
        .revalidate(&CancellationToken::new())
        .expect("ready authority revalidates");
}

#[test]
fn preparation_debug_views_cover_every_closed_variant_without_content() {
    let errors = vec![
        CandidateJudgePreparationError::Deterministic(
            CandidateDeterministicCompilerError::Cancelled,
        ),
        CandidateJudgePreparationError::CandidateBatchSet {
            side: CandidateJudgePreparationSide::CandidateA,
            source: VerifiedCandidateBatchSetError::Cancelled,
        },
        CandidateJudgePreparationError::CaseMaterial {
            source: VerifiedGenerationCaseMaterialError::Cancelled,
        },
        CandidateJudgePreparationError::AuthorityValidation {
            candidate_a: Some(Box::new(VerifiedCandidateBatchSetError::Cancelled)),
            candidate_b: Some(Box::new(VerifiedCandidateBatchSetError::Cancelled)),
            case_material: Some(Box::new(VerifiedGenerationCaseMaterialError::Cancelled)),
            cancelled: true,
        },
        CandidateJudgePreparationError::Rubric(LocalJudgeRubricError::UnsupportedSchema(99)),
        CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::JudgePolicyClosure,
        ),
        CandidateJudgePreparationError::Request {
            schedule_index: 7,
            failure: CandidateJudgePreparationRequestFailure::InvalidRequest,
        },
        CandidateJudgePreparationError::PortableContract {
            source: GenerationQualificationContractError::InvalidEncoding,
        },
        CandidateJudgePreparationError::PrimaryAndFinalValidation {
            primary: Box::new(CandidateJudgePreparationError::Relationship(
                CandidateJudgePreparationRelationship::ScheduleClosure,
            )),
            final_validation: Box::new(CandidateJudgePreparationError::AuthorityValidation {
                candidate_a: None,
                candidate_b: None,
                case_material: None,
                cancelled: true,
            }),
        },
    ];
    for error in errors {
        let debug = format!("{error:?}");
        assert!(debug.starts_with("CandidateJudgePreparationError"));
        assert!(!debug.contains("invalid prompt contract"));
    }
}

pub(crate) fn ready_config(case_indices: Vec<usize>) -> PreparationConfig {
    let rubric = rubric();
    PreparationConfig {
        case_indices,
        case_clauses: None,
        plan_rubric: rubric.clone(),
        supplied_rubric: rubric,
        prompt_digest: local_judge_prompt_contract_digest(),
        output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
            .schema_digest,
        limits: limits(16, 4_096, 4_096, 16_384),
        foreign_relation_model: false,
        fail_candidate_a_on_call: None,
        fail_candidate_b_on_call: None,
    }
}

pub(crate) fn prepare<'store>(
    material_fixture: &'store Fixture,
    suffix: &str,
    config: PreparationConfig,
    cancellation: &CancellationToken,
) -> PreparationResult<'store> {
    let material = material_fixture.verify().expect("material authority");
    let pair = offline_paired_authorities(&material_fixture.suite, &material_fixture.cases, suffix);
    if let Some(call) = config.fail_candidate_a_on_call {
        pair.candidate_a_control.fail_on_call(call);
    }
    if let Some(call) = config.fail_candidate_b_on_call {
        pair.candidate_b_control.fail_on_call(call);
    }
    let OfflinePairedAuthorityFixture {
        qualification_plan,
        suite,
        repetition,
        selection_policy,
        planned_attempts,
        candidate_a_system,
        candidate_b_system,
        candidate_a,
        candidate_b,
        candidate_a_control,
        candidate_b_control,
        ..
    } = pair;
    let judge_system = offline_judge_system(
        "exact judge",
        config.prompt_digest.clone(),
        config.output_schema_digest.clone(),
    );
    let relation = |judge_system| CandidateJudgePlanV1Relations {
        qualification_plan: &qualification_plan,
        suite: &suite,
        repetition: &repetition,
        selection_policy: &selection_policy,
        planned_attempts: &planned_attempts,
        candidate_a_system: &candidate_a_system,
        candidate_b_system: &candidate_b_system,
        judge_system,
    };
    let plan_relations = relation(&judge_system);
    let cases = config
        .case_indices
        .iter()
        .enumerate()
        .map(|(position, index)| {
            let clauses = config.case_clauses.as_ref().map_or_else(
                || {
                    material_fixture.contracts[*index]
                        .rubric_clause_ids()
                        .to_vec()
                },
                |values| values[position].clone(),
            );
            CandidateJudgeCaseV1::new(material_fixture.cases[*index].case_id().clone(), clauses)
                .expect("candidate judge case")
        })
        .collect();
    let judge_plan = CandidateJudgePlanV1::new(
        plan_relations,
        CandidateJudgePlanV1Input {
            case_material_set_digest: material.case_material_set_digest().clone(),
            rubric_digest: local_judge_rubric_digest(&config.plan_rubric).expect("plan rubric"),
            cases,
            order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
            presentation_seed: 29,
            attempts_per_order: 1,
            limits: config.limits,
            prompt_contract_digest: config.prompt_digest,
            output_schema_digest: config.output_schema_digest,
        },
    )
    .expect("candidate judge plan");
    let relation_judge_system = config.foreign_relation_model.then(|| {
        offline_judge_system_with_model(
            "substituted judge model",
            local_judge_prompt_contract_digest(),
            rewrite_inference::local_judge_attempt_output_contract().schema_digest,
            "foreign judge model bytes",
        )
    });
    let supplied_relations = relation(relation_judge_system.as_ref().unwrap_or(&judge_system));
    let input = CandidateJudgePreparationInput {
        plan_relations: supplied_relations,
        judge_plan: &judge_plan,
        rubric: &config.supplied_rubric,
        candidate_a,
        candidate_b,
        case_material: material,
    };
    let input_debug = format!("{input:?}");
    assert!(input_debug.contains("CandidateJudgePreparationInput"));
    assert!(!input_debug.contains("selected candidate"));
    let result = prepare_candidate_judge(input, cancellation);
    PreparationResult {
        result,
        candidate_a_calls: candidate_a_control.revalidation_calls(),
        candidate_b_calls: candidate_b_control.revalidation_calls(),
        candidate_a_control,
        candidate_b_control,
    }
}

pub(super) fn try_plan(
    material_fixture: &Fixture,
    case_indices: &[usize],
    clauses: Vec<Vec<String>>,
) -> Result<CandidateJudgePlanV1, GenerationQualificationContractError> {
    let material = material_fixture.verify().expect("material authority");
    let pair = offline_paired_authorities(
        &material_fixture.suite,
        &material_fixture.cases,
        "plan-only",
    );
    let judge_system = offline_judge_system(
        "plan-only judge",
        local_judge_prompt_contract_digest(),
        rewrite_inference::local_judge_attempt_output_contract().schema_digest,
    );
    CandidateJudgePlanV1::new(
        relations(&pair, &judge_system),
        CandidateJudgePlanV1Input {
            case_material_set_digest: material.case_material_set_digest().clone(),
            rubric_digest: local_judge_rubric_digest(&rubric()).expect("rubric"),
            cases: case_indices
                .iter()
                .zip(clauses)
                .map(|(index, clauses)| {
                    CandidateJudgeCaseV1::new(
                        material_fixture.cases[*index].case_id().clone(),
                        clauses,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
            presentation_seed: 29,
            attempts_per_order: 1,
            limits: limits(16, 4_096, 4_096, 16_384),
            prompt_contract_digest: local_judge_prompt_contract_digest(),
            output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
                .schema_digest,
        },
    )
}

fn relations<'a>(
    pair: &'a OfflinePairedAuthorityFixture,
    judge_system: &'a GenerationSystemRecordV1,
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

pub(super) fn rubric() -> LocalJudgeRubric {
    LocalJudgeRubric {
        schema_version: crate::LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION,
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

pub(super) fn limits(
    maximum_judge_cases: u32,
    maximum_source_bytes: u32,
    maximum_candidate_bytes: u32,
    maximum_complete_input_bytes: u32,
) -> CandidateJudgeLimitsV1 {
    CandidateJudgeLimitsV1::new(
        maximum_judge_cases,
        maximum_source_bytes,
        maximum_candidate_bytes,
        maximum_complete_input_bytes,
        8_192,
        512,
        4_096,
        30_000,
    )
    .expect("judge limits")
}

fn rederive_second_pass_request_ids(
    ready: &PreparedCandidateJudgeRun<'_>,
) -> Vec<StructuredCompletionRequestBindingId> {
    let cancellation = CancellationToken::new();
    let candidate_a = ready
        .candidate_a
        .selected_candidates(&cancellation)
        .expect("candidate A");
    let candidate_b = ready
        .candidate_b
        .selected_candidates(&cancellation)
        .expect("candidate B");
    let limits = ready.judge_plan.limits();
    let attempt_limits = LocalJudgeAttemptLimits {
        maximum_source_bytes: u64::from(limits.maximum_source_bytes()),
        maximum_candidate_bytes: u64::from(limits.maximum_candidate_bytes()),
        maximum_input_bytes: u64::from(limits.maximum_complete_input_bytes()),
        context_token_limit: limits.maximum_context_tokens(),
        output_token_limit: limits.maximum_output_tokens(),
        maximum_response_bytes: u64::from(limits.maximum_response_bytes()),
    };
    ready
        .case_material
        .with_all_case_source_bytes(&cancellation, |semantic_index, source| {
            let Ok(position) = ready
                .eligible_semantic_indices
                .binary_search(&semantic_index)
            else {
                return Vec::new();
            };
            let contract = &ready.case_material.contracts()[semantic_index];
            let source = std::str::from_utf8(source).expect("UTF-8 source");
            let rubric = contract
                .rubric_clause_ids()
                .iter()
                .map(|id| {
                    let index = ready
                        .rubric
                        .clauses
                        .binary_search_by(|clause| clause.id.as_str().cmp(id))
                        .expect("rubric clause");
                    &ready.rubric.clauses[index]
                })
                .collect::<Vec<_>>();
            ready.judge_schedule.entries()[position * 2..position * 2 + 2]
                .iter()
                .map(|entry| {
                    build_local_judge_attempt_request(
                        contract.case_key(),
                        source,
                        &candidate_a[semantic_index].text,
                        &candidate_b[semantic_index].text,
                        match entry.presentation() {
                            CandidateJudgePresentationV1::CandidateAFirst => {
                                crate::JudgePresentation::CandidateAFirst
                            }
                            CandidateJudgePresentationV1::CandidateBFirst => {
                                crate::JudgePresentation::CandidateBFirst
                            }
                        },
                        &rubric,
                        ready.judge_system.model_artifact_id(),
                        ready.judge_system.model_artifact_id().digest(),
                        entry.seed(),
                        attempt_limits,
                    )
                    .expect("second-pass request")
                    .structured_request_binding_id()
                })
                .collect()
        })
        .expect("source traversal")
        .into_iter()
        .flatten()
        .collect()
}
