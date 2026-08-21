use rewrite_ollama::OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES;
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{EvaluationSuite, TransformationCoverage};

mod deterministic;
mod normalize;

use deterministic::run_deterministic_gates;
use normalize::normalize_observations;

/// Current hybrid scorecard contract version.
pub const HYBRID_SCORECARD_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized plan or observation batch size.
pub const MAX_HYBRID_SCORECARD_BYTES: usize = 4 * 1024 * 1024;

const MAX_SCORECARD_CASES: usize = 10_000;
const MAX_RUBRIC_CLAUSES_PER_CASE: usize = 32;
const MAX_EXECUTED_JUDGE_CASES: usize = 256;
const MAX_JUDGE_INPUT_BYTES: u32 = OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES;
const MAX_JUDGE_SOURCE_BYTES: u32 = 1024 * 1024;
const MAX_JUDGE_CANDIDATE_BYTES: u32 = 1024 * 1024;
const MAX_JUDGE_CONTEXT_TOKENS: u32 = 131_072;
const MAX_JUDGE_OUTPUT_TOKENS: u32 = 8_192;
const MAX_JUDGE_ELAPSED_MILLIS: u32 = 60 * 60 * 1_000;
const MIN_JUDGE_RESPONSE_BYTES: u32 = 256;
const MAX_JUDGE_RESPONSE_BYTES: u32 = 64 * 1024;

/// Frozen scorecard plan for deterministic gates and local judge triage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HybridScorecardPlan {
    /// Scorecard schema version.
    pub schema_version: u32,
    /// Stable plan identifier.
    pub plan_id: String,
    /// Digest of the exact admitted corpus manifest.
    pub corpus_digest: Digest,
    /// Caller-declared digest of the intended rubric contract.
    ///
    /// Version 1 does not bind this declaration to an observed judge execution.
    pub rubric_digest: Digest,
    /// Digest of deterministic gate policy and thresholds.
    pub deterministic_policy_digest: Digest,
    /// Exact local judge execution policy.
    pub judge: LocalJudgePolicy,
    /// Ordered candidate-pair plans.
    pub cases: Vec<HybridScorecardCasePlan>,
}

/// Caller-declared intended judge identity and bounded execution policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalJudgePolicy {
    /// Judge execution class. Only local execution is admitted.
    pub execution: JudgeExecution,
    /// Judge authority. Only triage is admitted.
    pub authority: JudgeAuthority,
    /// Required two-order presentation policy.
    pub order_policy: JudgeOrderPolicy,
    /// Caller-declared digest joining the intended runtime and isolation state.
    ///
    /// Version 1 does not construct or attest this execution identity.
    pub judge_system_digest: Digest,
    /// Exact bounded runtime-local model reference admitted by the retained session.
    pub judge_model_reference: String,
    /// Exact immutable model artifact digest admitted by the retained session.
    pub judge_model_digest: Digest,
    /// Exact versioned canonical prompt-contract digest.
    pub judge_prompt_contract_digest: Digest,
    /// Exact neutral structured-output schema digest.
    pub judge_output_schema_digest: Digest,
    /// Frozen presentation schedule seed.
    pub presentation_seed: u64,
    /// Sampling temperature in thousandths. Version 1 requires zero.
    pub temperature_milli: u16,
    /// Nucleus sampling probability in thousandths. Version 1 requires 1000.
    pub top_p_milli: u16,
    /// Attempts per presentation order. Version 1 requires one.
    pub attempts_per_order: u8,
    /// Maximum cases admitted to one retained-session execution.
    pub max_judge_cases: u16,
    /// Maximum UTF-8 bytes in one source presented to the judge.
    pub max_source_bytes: u32,
    /// Maximum UTF-8 bytes in either candidate presented to the judge.
    pub max_candidate_bytes: u32,
    /// Maximum UTF-8 bytes in the complete canonical judge input.
    pub max_input_bytes: u32,
    /// Maximum context tokens requested from the runtime.
    pub context_token_limit: u32,
    /// Maximum output tokens requested from the runtime.
    pub output_token_limit: u32,
    /// Maximum accepted response bytes for one judge attempt.
    pub max_response_bytes: u32,
    /// Maximum wall-clock budget accepted from the operation context.
    pub maximum_elapsed_millis: u32,
}

/// Admitted judge execution class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeExecution {
    /// The plan requires local network-isolated execution under exact runtime evidence.
    ///
    /// Version 1 does not itself produce the execution receipt needed to prove this.
    LocalIsolated,
}

/// Admitted judge authority class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeAuthority {
    /// Output can route human review but cannot accept or reject a candidate.
    TriageOnly,
}

/// Required order-bias control.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeOrderPolicy {
    /// Present every pair once in each order.
    BothOrders,
}

/// One blinded candidate-pair plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HybridScorecardCasePlan {
    /// Stable case identifier.
    pub id: String,
    /// Dependence cluster used by later statistical reporting.
    pub cluster_id: String,
    /// Digest of the exact source bytes.
    pub source_digest: Digest,
    /// Digest of candidate A bytes.
    pub candidate_a_digest: Digest,
    /// Digest of candidate B bytes.
    pub candidate_b_digest: Digest,
    /// Caller-declared system identity intended to have produced candidate A.
    ///
    /// Version 1 carries no candidate-generation receipt.
    pub candidate_a_system_digest: Digest,
    /// Caller-declared system identity intended to have produced candidate B.
    ///
    /// Version 1 carries no candidate-generation receipt.
    pub candidate_b_system_digest: Digest,
    /// Sorted rubric clauses admitted for this case.
    pub rubric_clauses: Vec<String>,
}

/// One caller-declared, content-free batch of two-order judge observations.
///
/// Version 1 carries no candidate-generation or judge-execution receipt. The
/// observations can route human triage only and are never execution evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JudgeObservationBatch {
    /// Scorecard schema version.
    pub schema_version: u32,
    /// Exact scorecard plan identifier.
    pub plan_id: String,
    /// Digest of the complete validated plan, including judge system identity.
    pub plan_digest: Digest,
    /// One observation for each planned case and presentation order.
    pub observations: Vec<JudgeObservation>,
}

/// One bounded judge choice without raw prompt, candidate, or rationale content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JudgeObservation {
    /// Stable case identifier.
    pub case_id: String,
    /// Candidate presentation order.
    pub presentation: JudgePresentation,
    /// Choice relative to presentation order.
    pub choice: JudgeChoice,
    /// Sorted rubric clauses cited by the structured result.
    pub rubric_clauses: Vec<String>,
}

/// Candidate presentation order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgePresentation {
    /// Underlying candidate A is presented first.
    CandidateAFirst,
    /// Underlying candidate B is presented first.
    CandidateBFirst,
}

/// Judge choice relative to the presented order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeChoice {
    /// Prefer the first presented candidate.
    First,
    /// Prefer the second presented candidate.
    Second,
    /// The candidates are tied under the cited clauses.
    Tie,
    /// The evidence or rubric is insufficient.
    Abstain,
}

/// Content-free hybrid scorecard report.
///
/// Rubric and system digests remain caller declarations in version 1. The report
/// does not claim that those systems generated the candidates or observations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HybridScorecardReport {
    /// Scorecard schema version.
    pub schema_version: u32,
    /// Exact scorecard plan identifier.
    pub plan_id: String,
    /// Digest of the complete validated scorecard plan.
    pub plan_digest: Digest,
    /// Digest of the admitted corpus manifest.
    pub corpus_digest: Digest,
    /// Caller-declared digest of the intended rubric.
    pub declared_rubric_digest: Digest,
    /// Digest of deterministic gate policy and thresholds.
    pub deterministic_policy_digest: Digest,
    /// Caller-declared local judge system identity.
    pub declared_judge_system_digest: Digest,
    /// Digest of the deterministic report serialized by its versioned contract.
    pub deterministic_report_digest: Digest,
    /// Digest of the exact ordered judge batch, absent when hard gates block judging.
    pub judge_observation_batch_digest: Option<Digest>,
    /// Authority class of the supplied judge observations, absent when judging is blocked.
    pub judge_observation_evidence_class: Option<JudgeObservationEvidenceClass>,
    /// Total deterministic cases.
    pub deterministic_total: usize,
    /// Passing deterministic cases.
    pub deterministic_passed: usize,
    /// Transformation coverage with its explicit denominator.
    pub transformation_coverage: TransformationCoverage,
    /// Content-free judge triage summary.
    pub judge: JudgeTriageSummary,
    /// Release review disposition. Version 1 never represents qualification.
    pub release_review: ReleaseReviewDisposition,
}

/// Authority class retained for one judge observation batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeObservationEvidenceClass {
    /// Observations were supplied by the caller without an attested execution receipt.
    CallerDeclared,
}

impl HybridScorecardReport {
    /// Returns whether every deterministic gate passed.
    ///
    /// This does not represent release qualification. Human adjudication remains
    /// required for every successful version 1 scorecard.
    #[must_use]
    pub const fn hard_gates_passed(&self) -> bool {
        self.deterministic_total > 0 && self.deterministic_total == self.deterministic_passed
    }
}

/// Aggregate two-order judge results.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct JudgeTriageSummary {
    /// Planned judge cases.
    pub total: usize,
    /// Stable preference for underlying candidate A.
    pub stable_a: usize,
    /// Stable preference for underlying candidate B.
    pub stable_b: usize,
    /// Stable tie in both orders.
    pub stable_tie: usize,
    /// At least one order abstained.
    pub abstained: usize,
    /// Orders produced incompatible underlying choices.
    pub order_sensitive: usize,
    /// Per-case content-free results.
    pub cases: Vec<JudgeCaseResult>,
}

/// One normalized content-free judge result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct JudgeCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Normalized two-order outcome.
    pub outcome: JudgeCaseOutcome,
}

/// Normalized two-order outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgeCaseOutcome {
    /// Both orders prefer underlying candidate A.
    StableA,
    /// Both orders prefer underlying candidate B.
    StableB,
    /// Both orders return tie.
    StableTie,
    /// At least one order abstains.
    Abstained,
    /// The two orders are inconsistent.
    OrderSensitive,
}

/// Non-authoritative release-review state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseReviewDisposition {
    /// Deterministic gates failed, so judge execution is forbidden.
    BlockedByHardGate,
    /// Hard gates passed; human review remains required.
    RequiresHumanAdjudication,
}

/// Hybrid scorecard parsing or relationship failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HybridScorecardError {
    /// Serialized input exceeds the byte limit.
    #[error("hybrid scorecard input exceeds the supported byte limit")]
    TooLarge,
    /// JSON is malformed or contains an unknown value.
    #[error("invalid hybrid scorecard JSON")]
    InvalidJson,
    /// The schema version is unsupported.
    #[error("unsupported hybrid scorecard schema version {0}")]
    UnsupportedSchema(u32),
    /// Plan-level identity or judge policy is invalid.
    #[error("hybrid scorecard plan is invalid")]
    InvalidPlan,
    /// Deterministic report counters or coverage are inconsistent.
    #[error("deterministic evaluation report is invalid")]
    InvalidDeterministicReport,
    /// Deterministic suites are empty, malformed, reordered, or not comparable.
    #[error("deterministic evaluation suites are invalid")]
    InvalidDeterministicSuites,
    /// Deterministic suites do not match the corpus committed by the plan.
    #[error("deterministic evaluation suites do not match the selected corpus")]
    CorpusMismatch,
    /// Plan names a deterministic policy other than the fixed version 1 policy.
    #[error("deterministic evaluation policy does not match the version 1 runner")]
    DeterministicPolicyMismatch,
    /// A planned case is invalid, duplicated, or out of canonical order.
    #[error("hybrid scorecard case {index} is invalid")]
    InvalidCase {
        /// Zero-based position of the invalid planned case.
        index: usize,
    },
    /// Observation batch does not name the selected plan.
    #[error("judge observation batch does not match the selected plan")]
    PlanMismatch,
    /// Judge observations were supplied after a hard-gate failure.
    #[error("judge observations are forbidden after a deterministic hard-gate failure")]
    JudgeAfterHardGateFailure,
    /// An observation is missing, duplicated, undeclared, or cites an invalid clause.
    #[error("judge observation set is invalid")]
    InvalidObservations,
}

/// Parses and validates a frozen scorecard plan.
///
/// # Errors
///
/// Returns [`HybridScorecardError`] for size, JSON, version, policy, identity,
/// ordering, or relationship failures.
pub fn parse_hybrid_scorecard_plan(
    input: &str,
) -> Result<HybridScorecardPlan, HybridScorecardError> {
    if input.len() > MAX_HYBRID_SCORECARD_BYTES {
        return Err(HybridScorecardError::TooLarge);
    }
    let plan: HybridScorecardPlan =
        serde_json::from_str(input).map_err(|_| HybridScorecardError::InvalidJson)?;
    validate_plan(&plan)?;
    Ok(plan)
}

/// Parses a caller-declared, content-free judge observation batch.
///
/// # Errors
///
/// Returns [`HybridScorecardError`] for size, JSON, version, label, clause, or
/// observation-count failures.
pub fn parse_judge_observation_batch(
    input: &str,
) -> Result<JudgeObservationBatch, HybridScorecardError> {
    if input.len() > MAX_HYBRID_SCORECARD_BYTES {
        return Err(HybridScorecardError::TooLarge);
    }
    let batch: JudgeObservationBatch =
        serde_json::from_str(input).map_err(|_| HybridScorecardError::InvalidJson)?;
    validate_observation_batch(&batch)?;
    Ok(batch)
}

fn validate_observation_batch(batch: &JudgeObservationBatch) -> Result<(), HybridScorecardError> {
    if batch.schema_version != HYBRID_SCORECARD_SCHEMA_VERSION {
        return Err(HybridScorecardError::UnsupportedSchema(
            batch.schema_version,
        ));
    }
    if serde_json::to_vec(batch)
        .map_err(|_| HybridScorecardError::InvalidObservations)?
        .len()
        > MAX_HYBRID_SCORECARD_BYTES
    {
        return Err(HybridScorecardError::TooLarge);
    }
    if !valid_label(&batch.plan_id)
        || batch.observations.len() > MAX_SCORECARD_CASES.saturating_mul(2)
        || batch.observations.iter().any(|observation| {
            !valid_label(&observation.case_id)
                || observation.rubric_clauses.is_empty()
                || observation.rubric_clauses.len() > MAX_RUBRIC_CLAUSES_PER_CASE
                || observation
                    .rubric_clauses
                    .iter()
                    .any(|clause| !valid_label(clause))
                || !observation
                    .rubric_clauses
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
        })
    {
        return Err(HybridScorecardError::InvalidObservations);
    }
    Ok(())
}

/// Computes the canonical digest of two validated deterministic suites.
///
/// # Errors
///
/// Returns [`HybridScorecardError`] when either suite is invalid, empty,
/// oversized, out of canonical order, or not comparable to the other suite.
pub fn hybrid_scorecard_suite_pair_digest(
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<Digest, HybridScorecardError> {
    deterministic::suite_pair_digest(candidate_a, candidate_b)
}

/// Returns the fixed version 1 deterministic runner policy digest.
///
/// Any change to gate semantics, ordering, thresholds, suite execution, or
/// report validation requires a new scorecard contract and policy domain.
#[must_use]
pub fn hybrid_scorecard_deterministic_policy_digest() -> Digest {
    deterministic::policy_digest()
}

/// Computes the domain-separated digest of a validated scorecard plan.
///
/// # Errors
///
/// Returns [`HybridScorecardError`] when the plan is invalid or cannot be
/// serialized by its versioned contract.
pub fn hybrid_scorecard_plan_digest(
    plan: &HybridScorecardPlan,
) -> Result<Digest, HybridScorecardError> {
    validate_plan(plan)?;
    deterministic::plan_digest(plan)
}

/// Builds a redacted scorecard and normalizes two-order judge results.
///
/// # Errors
///
/// Returns [`HybridScorecardError`] when the plan or deterministic suites are
/// invalid, their content or policy does not match the plan, the batch names
/// another exact plan, judge execution follows a hard-gate failure, or the
/// two-order observation set is incomplete or inconsistent.
pub fn run_hybrid_scorecard(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
    batch: &JudgeObservationBatch,
) -> Result<HybridScorecardReport, HybridScorecardError> {
    validate_plan(plan)?;
    validate_observation_batch(batch)?;
    let plan_digest = deterministic::plan_digest(plan)?;
    if batch.plan_id != plan.plan_id || batch.plan_digest != plan_digest {
        return Err(HybridScorecardError::PlanMismatch);
    }
    let deterministic = run_deterministic_gates(plan, candidate_a, candidate_b)?;
    if !deterministic.success {
        if !batch.observations.is_empty() {
            return Err(HybridScorecardError::JudgeAfterHardGateFailure);
        }
        return Ok(report(
            plan,
            &deterministic,
            plan_digest,
            None,
            None,
            JudgeTriageSummary::default(),
            ReleaseReviewDisposition::BlockedByHardGate,
        ));
    }
    let judge = normalize_observations(plan, batch)?;
    let observation_batch_digest = deterministic::observation_batch_digest(batch)?;
    Ok(report(
        plan,
        &deterministic,
        plan_digest,
        Some(observation_batch_digest),
        Some(JudgeObservationEvidenceClass::CallerDeclared),
        judge,
        ReleaseReviewDisposition::RequiresHumanAdjudication,
    ))
}

pub(crate) fn run_hybrid_scorecard_hard_gates(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<Option<HybridScorecardReport>, HybridScorecardError> {
    validate_plan(plan)?;
    let plan_digest = deterministic::plan_digest(plan)?;
    let deterministic = run_deterministic_gates(plan, candidate_a, candidate_b)?;
    if deterministic.success {
        return Ok(None);
    }
    Ok(Some(report(
        plan,
        &deterministic,
        plan_digest,
        None,
        None,
        JudgeTriageSummary::default(),
        ReleaseReviewDisposition::BlockedByHardGate,
    )))
}

fn validate_plan(plan: &HybridScorecardPlan) -> Result<(), HybridScorecardError> {
    if plan.schema_version != HYBRID_SCORECARD_SCHEMA_VERSION {
        return Err(HybridScorecardError::UnsupportedSchema(plan.schema_version));
    }
    if serde_json::to_vec(plan)
        .map_err(|_| HybridScorecardError::InvalidPlan)?
        .len()
        > MAX_HYBRID_SCORECARD_BYTES
    {
        return Err(HybridScorecardError::TooLarge);
    }
    if !valid_label(&plan.plan_id)
        || plan.cases.is_empty()
        || plan.cases.len() > MAX_SCORECARD_CASES
        || !valid_model_reference(&plan.judge.judge_model_reference)
        || plan.judge.judge_prompt_contract_digest != crate::local_judge_prompt_contract_digest()
        || plan.judge.judge_output_schema_digest
            != rewrite_inference::local_judge_attempt_output_contract().schema_digest
        || plan.judge.temperature_milli != 0
        || plan.judge.top_p_milli != 1_000
        || plan.judge.attempts_per_order != 1
        || plan.judge.max_judge_cases == 0
        || usize::from(plan.judge.max_judge_cases) > MAX_EXECUTED_JUDGE_CASES
        || plan.cases.len() > usize::from(plan.judge.max_judge_cases)
        || !(1..=MAX_JUDGE_SOURCE_BYTES).contains(&plan.judge.max_source_bytes)
        || !(1..=MAX_JUDGE_CANDIDATE_BYTES).contains(&plan.judge.max_candidate_bytes)
        || !(1..=MAX_JUDGE_INPUT_BYTES).contains(&plan.judge.max_input_bytes)
        || plan.judge.max_input_bytes < plan.judge.max_source_bytes
        || plan.judge.max_input_bytes < plan.judge.max_candidate_bytes
        || !(1..=MAX_JUDGE_CONTEXT_TOKENS).contains(&plan.judge.context_token_limit)
        || !(1..=MAX_JUDGE_OUTPUT_TOKENS).contains(&plan.judge.output_token_limit)
        || !(MIN_JUDGE_RESPONSE_BYTES..=MAX_JUDGE_RESPONSE_BYTES)
            .contains(&plan.judge.max_response_bytes)
        || !(1..=MAX_JUDGE_ELAPSED_MILLIS).contains(&plan.judge.maximum_elapsed_millis)
    {
        return Err(HybridScorecardError::InvalidPlan);
    }
    let mut previous = None;
    for (index, case) in plan.cases.iter().enumerate() {
        if !valid_case(case, &plan.judge.judge_system_digest)
            || previous.is_some_and(|id: &str| id >= case.id.as_str())
        {
            return Err(HybridScorecardError::InvalidCase { index });
        }
        previous = Some(case.id.as_str());
    }
    Ok(())
}

fn valid_case(case: &HybridScorecardCasePlan, judge_system: &Digest) -> bool {
    valid_label(&case.id)
        && valid_label(&case.cluster_id)
        && case.candidate_a_digest != case.candidate_b_digest
        && &case.candidate_a_system_digest != judge_system
        && &case.candidate_b_system_digest != judge_system
        && !case.rubric_clauses.is_empty()
        && case.rubric_clauses.len() <= MAX_RUBRIC_CLAUSES_PER_CASE
        && case.rubric_clauses.iter().all(|clause| valid_label(clause))
        && case.rubric_clauses.windows(2).all(|pair| pair[0] < pair[1])
}

fn report(
    plan: &HybridScorecardPlan,
    deterministic: &deterministic::DeterministicGateReceipt,
    plan_digest: Digest,
    judge_observation_batch_digest: Option<Digest>,
    judge_observation_evidence_class: Option<JudgeObservationEvidenceClass>,
    judge: JudgeTriageSummary,
    release_review: ReleaseReviewDisposition,
) -> HybridScorecardReport {
    HybridScorecardReport {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        plan_digest,
        corpus_digest: plan.corpus_digest.clone(),
        declared_rubric_digest: plan.rubric_digest.clone(),
        deterministic_policy_digest: plan.deterministic_policy_digest.clone(),
        declared_judge_system_digest: plan.judge.judge_system_digest.clone(),
        deterministic_report_digest: deterministic.report_digest.clone(),
        judge_observation_batch_digest,
        judge_observation_evidence_class,
        deterministic_total: deterministic.total,
        deterministic_passed: deterministic.passed,
        transformation_coverage: deterministic.transformation_coverage,
        judge,
        release_review,
    }
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

fn valid_model_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/')
        })
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && value
            .bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && !value.contains("..")
        && !value.contains("//")
        && !value.contains("::")
}

#[cfg(test)]
mod tests;
