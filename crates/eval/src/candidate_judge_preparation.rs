//! Offline exact preparation for deterministic-gated candidate judging.

use std::fmt;

use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationStatusV1,
    CandidateJudgePlanV1, CandidateJudgePlanV1Relations, CandidateJudgeRequestAggregateV1,
    CandidateJudgeScheduleV1, GenerationQualificationContractError, GenerationSystemRecordV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::generation_case_material::{
    VerifiedGenerationCaseMaterial, VerifiedGenerationCaseMaterialError,
};
use crate::{
    CandidateDeterministicCompilerError, LocalJudgeRubric, LocalJudgeRubricError,
    VerifiedCandidateBatchSet, VerifiedCandidateBatchSetError,
    compile_candidate_deterministic_evaluation,
};

mod handoff;
mod pairing;
mod projection;
mod request;
mod validation;

#[cfg(test)]
pub(crate) use handoff::CandidateJudgeJoinCompilationRelationship;
pub use handoff::CandidateJudgeRunnerHandoff;
pub(crate) use handoff::{
    CandidateJudgeJoinCompilationError, CandidateJudgeRunnerRequestError,
    CandidateJudgeTriageCompilationError, CompiledCandidateJudgeTriage,
};
pub(crate) use pairing::CandidateJudgeOperationScopeError;
pub use pairing::{
    CandidateJudgeRunnerPairingError, CandidateJudgeRunnerPairingRelationship,
    CandidateJudgeRunnerPairingValidationPhase, PairedCandidateJudgeRunnerHandoff,
    pair_candidate_judge_runner_handoffs,
};
use projection::{PreparedCompatibilityProjection, compile_compatibility_projection};
use request::derive_request_aggregate;
use validation::validate_pre_output_closure;

/// Candidate position in the identity-significant exact judge pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateJudgePreparationSide {
    /// First candidate authority.
    CandidateA,
    /// Second candidate authority.
    CandidateB,
}

/// Exact relationship rejected before any judge runtime can be acquired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateJudgePreparationRelationship {
    /// The portable judge plan did not reload against the supplied records.
    JudgePlan,
    /// Candidate receipt sets did not close over the judge plan.
    CandidatePlanClosure,
    /// The judge plan did not name the exact verified material authority.
    CaseMaterialClosure,
    /// The plan's eligible cases were not the exact semantic-order contract subset.
    EligibleCaseClosure,
    /// A plan case did not name its exact contract rubric clauses.
    RubricClauseClosure,
    /// The rubric, prompt contract, or output schema differed from the frozen plan.
    JudgePolicyClosure,
    /// The derived schedule did not close over the deterministic candidate pair.
    ScheduleClosure,
    /// The exact eligible-subset compatibility projection did not close.
    CompatibilityProjection,
    /// Candidate authorities did not share one process-local Active subject.
    ActiveSubjectClosure,
}

/// Closed content-free failure from one pure request construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateJudgePreparationRequestFailure {
    /// The exact rubric subset was inconsistent.
    RubricMismatch,
    /// Source, candidate, or complete prompt bytes exceeded a fixed limit.
    InputLimitExceeded,
    /// Canonical prompt encoding failed.
    PromptEncoding,
    /// The resulting structured request violated its frozen contract.
    InvalidRequest,
    /// Exact source bytes were not UTF-8.
    SourceNotUtf8,
}

/// Failure to prepare the deterministic-gated exact candidate judge path.
#[derive(Error)]
pub enum CandidateJudgePreparationError {
    /// The authoritative deterministic compiler failed.
    #[error("candidate judge deterministic compilation failed")]
    Deterministic(#[source] CandidateDeterministicCompilerError),
    /// One retained candidate authority failed after deterministic compilation.
    #[error("candidate judge candidate authority failed for {side:?}")]
    CandidateBatchSet {
        /// Identity-significant candidate side.
        side: CandidateJudgePreparationSide,
        /// Exact typed authority failure, omitted from debug output.
        #[source]
        source: VerifiedCandidateBatchSetError,
    },
    /// The retained case-material authority failed.
    #[error("candidate judge case-material authority failed")]
    CaseMaterial {
        /// Exact typed authority failure, omitted from debug output.
        #[source]
        source: VerifiedGenerationCaseMaterialError,
    },
    /// One read-only authority or cancellation bracket found independent failures.
    #[error("candidate judge authority validation failed")]
    AuthorityValidation {
        /// Candidate A failure, when observed before cancellation stopped checks.
        candidate_a: Option<Box<VerifiedCandidateBatchSetError>>,
        /// Candidate B failure, when observed before cancellation stopped checks.
        candidate_b: Option<Box<VerifiedCandidateBatchSetError>>,
        /// Case-material failure, when observed before cancellation stopped checks.
        case_material: Option<Box<VerifiedGenerationCaseMaterialError>>,
        /// Whether cancellation stopped remaining read-only checks.
        cancelled: bool,
    },
    /// The canonical local-judge rubric failed validation.
    #[error("candidate judge rubric validation failed")]
    Rubric(#[source] LocalJudgeRubricError),
    /// One fixed pre-output relationship did not close.
    #[error("candidate judge preparation relationship does not match: {0:?}")]
    Relationship(CandidateJudgePreparationRelationship),
    /// One pure schedule attempt could not be constructed.
    #[error("candidate judge request preparation failed at schedule index {schedule_index}")]
    Request {
        /// Exact schedule position that failed.
        schedule_index: usize,
        /// Closed content-free request failure.
        failure: CandidateJudgePreparationRequestFailure,
    },
    /// The portable model contract rejected compiler-derived values.
    #[error("candidate judge portable contract failed")]
    PortableContract {
        /// Typed portable contract failure, omitted from debug output.
        #[source]
        source: GenerationQualificationContractError,
    },
    /// Preparation and mandatory final validation both failed.
    #[error("candidate judge preparation and final authority validation both failed")]
    PrimaryAndFinalValidation {
        /// Primary preparation failure.
        primary: Box<CandidateJudgePreparationError>,
        /// Independent final validation failure.
        final_validation: Box<CandidateJudgePreparationError>,
    },
}

impl fmt::Debug for CandidateJudgePreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateJudgePreparationError");
        match self {
            Self::Deterministic(_) => debug.field("kind", &"deterministic"),
            Self::CandidateBatchSet { side, .. } => debug
                .field("kind", &"candidate_batch_set")
                .field("side", side),
            Self::CaseMaterial { .. } => debug.field("kind", &"case_material"),
            Self::AuthorityValidation {
                candidate_a,
                candidate_b,
                case_material,
                cancelled,
            } => debug
                .field("kind", &"authority_validation")
                .field("candidate_a_failed", &candidate_a.is_some())
                .field("candidate_b_failed", &candidate_b.is_some())
                .field("case_material_failed", &case_material.is_some())
                .field("cancelled", cancelled),
            Self::Rubric(_) => debug.field("kind", &"rubric"),
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::Request {
                schedule_index,
                failure,
            } => debug
                .field("kind", &"request")
                .field("schedule_index", schedule_index)
                .field("failure", failure),
            Self::PortableContract { .. } => debug.field("kind", &"portable_contract"),
            Self::PrimaryAndFinalValidation { .. } => {
                debug.field("kind", &"primary_and_final_validation")
            }
        };
        debug.finish_non_exhaustive()
    }
}

/// Complete offline inputs for one exact candidate-judge preparation operation.
///
/// The candidate and material authorities are consumed. Runtime leases, launchers,
/// sessions, and connectors cannot be supplied at this boundary.
pub struct CandidateJudgePreparationInput<'records, 'store> {
    /// Complete portable records that rederive the pre-output judge plan.
    pub plan_relations: CandidateJudgePlanV1Relations<'records>,
    /// Exact pre-output judge plan.
    pub judge_plan: &'records CandidateJudgePlanV1,
    /// Exact canonical local-judge rubric.
    pub rubric: &'records LocalJudgeRubric,
    /// Candidate A's retained complete batch-set authority.
    pub candidate_a: VerifiedCandidateBatchSet,
    /// Candidate B's retained complete batch-set authority.
    pub candidate_b: VerifiedCandidateBatchSet,
    /// Complete retained semantic-order case-material authority.
    pub case_material: VerifiedGenerationCaseMaterial<'store>,
}

impl fmt::Debug for CandidateJudgePreparationInput<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgePreparationInput")
            .field("judge_plan_id", self.judge_plan.candidate_judge_plan_id())
            .field("candidate_a", &self.candidate_a)
            .field("candidate_b", &self.candidate_b)
            .field("case_material", &self.case_material)
            .finish_non_exhaustive()
    }
}

/// Terminal offline result of exact candidate-judge preparation.
pub enum CandidateJudgePreparationOutcome<'store> {
    /// Deterministic hard gates failed, so no ready capability exists.
    DeterministicFailed(Box<CandidateDeterministicEvaluationRecordV1>),
    /// Deterministic gates passed and exact second-pass execution is authorized.
    Ready(Box<PreparedCandidateJudgeRun<'store>>),
}

impl fmt::Debug for CandidateJudgePreparationOutcome<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeterministicFailed(record) => formatter
                .debug_tuple("DeterministicFailed")
                .field(record.deterministic_evaluation_id())
                .finish(),
            Self::Ready(ready) => formatter.debug_tuple("Ready").field(ready).finish(),
        }
    }
}

/// Noncloneable, nonserializable capability for the exact managed judge runner.
///
/// This value retains the live candidate and case-material authorities. It stores
/// only content-free first-pass request identities, never raw prompts.
///
/// ```compile_fail
/// use rewrite_eval::PreparedCandidateJudgeRun;
///
/// fn clone_capability(value: PreparedCandidateJudgeRun<'_>) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::PreparedCandidateJudgeRun;
///
/// fn serialize_capability(value: &PreparedCandidateJudgeRun<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct PreparedCandidateJudgeRun<'store> {
    candidate_a: VerifiedCandidateBatchSet,
    candidate_b: VerifiedCandidateBatchSet,
    case_material: VerifiedGenerationCaseMaterial<'store>,
    deterministic_evaluation: CandidateDeterministicEvaluationRecordV1,
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    compatibility_projection: PreparedCompatibilityProjection,
    rubric: LocalJudgeRubric,
    judge_system: GenerationSystemRecordV1,
    eligible_semantic_indices: Vec<usize>,
}

impl PreparedCandidateJudgeRun<'_> {
    /// Returns the passed deterministic hard-gate record.
    #[must_use]
    pub const fn deterministic_evaluation(&self) -> &CandidateDeterministicEvaluationRecordV1 {
        &self.deterministic_evaluation
    }

    /// Returns the exact reloaded pre-output judge plan.
    #[must_use]
    pub const fn judge_plan(&self) -> &CandidateJudgePlanV1 {
        &self.judge_plan
    }

    /// Returns the exact derived two-order schedule.
    #[must_use]
    pub const fn judge_schedule(&self) -> &CandidateJudgeScheduleV1 {
        &self.judge_schedule
    }

    /// Returns the content-free first-pass request aggregate.
    #[must_use]
    pub const fn request_aggregate(&self) -> &CandidateJudgeRequestAggregateV1 {
        &self.request_aggregate
    }

    /// Revalidates every retained authority and fixed judge-policy relationship.
    ///
    /// This check does not launch a runtime or rebuild raw prompts. The managed
    /// runner performs the exact second request pass after consuming this value.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateJudgePreparationError`] for cancellation, authority
    /// drift, invalid rubric state, or a substituted judge-system relationship.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        revalidate_prepared_authorities(
            &self.candidate_a,
            &self.candidate_b,
            &self.case_material,
            &self.judge_plan,
            &self.rubric,
            &self.judge_system,
            cancellation,
        )
    }
}

impl fmt::Debug for PreparedCandidateJudgeRun<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCandidateJudgeRun")
            .field(
                "deterministic_evaluation_id",
                self.deterministic_evaluation.deterministic_evaluation_id(),
            )
            .field("judge_plan_id", self.judge_plan.candidate_judge_plan_id())
            .field(
                "judge_schedule_id",
                self.judge_schedule.candidate_judge_schedule_id(),
            )
            .field(
                "request_aggregate_id",
                self.request_aggregate.request_aggregate_id(),
            )
            .field("compatibility_projection", &self.compatibility_projection)
            .field("eligible_case_count", &self.eligible_semantic_indices.len())
            .finish_non_exhaustive()
    }
}

struct PreparedPortableClosure {
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    compatibility_projection: PreparedCompatibilityProjection,
    eligible_semantic_indices: Vec<usize>,
}

/// Runs deterministic gates and prepares the exact two-pass judge capability.
///
/// A deterministic `Failed` record returns immediately without validating judge
/// policy, deriving a schedule, or constructing a request. Only a deterministic
/// `Passed` record can produce [`CandidateJudgePreparationOutcome::Ready`].
///
/// # Errors
///
/// Returns [`CandidateJudgePreparationError`] for authority drift, cancellation,
/// plan, material, rubric, schedule, request, or portable-contract mismatch.
pub fn prepare_candidate_judge<'store>(
    input: CandidateJudgePreparationInput<'_, 'store>,
    cancellation: &CancellationToken,
) -> Result<CandidateJudgePreparationOutcome<'store>, CandidateJudgePreparationError> {
    let deterministic_evaluation = compile_candidate_deterministic_evaluation(
        &input.candidate_a,
        &input.candidate_b,
        &input.case_material,
        cancellation,
    )
    .map_err(CandidateJudgePreparationError::Deterministic)?;
    if deterministic_evaluation.status() == CandidateDeterministicEvaluationStatusV1::Failed {
        return Ok(CandidateJudgePreparationOutcome::DeterministicFailed(
            Box::new(deterministic_evaluation),
        ));
    }

    let primary = prepare_pass(&input, &deterministic_evaluation, cancellation);
    let final_validation = revalidate_all(&input, cancellation);
    let closure = match (primary, final_validation) {
        (Ok(closure), Ok(())) => closure,
        (Err(primary), Ok(())) => return Err(primary),
        (Ok(_), Err(final_validation)) => return Err(final_validation),
        (Err(primary), Err(final_validation)) => {
            return Err(CandidateJudgePreparationError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(final_validation),
            });
        }
    };

    Ok(CandidateJudgePreparationOutcome::Ready(Box::new(
        PreparedCandidateJudgeRun {
            candidate_a: input.candidate_a,
            candidate_b: input.candidate_b,
            case_material: input.case_material,
            deterministic_evaluation,
            judge_plan: closure.judge_plan,
            judge_schedule: closure.judge_schedule,
            request_aggregate: closure.request_aggregate,
            compatibility_projection: closure.compatibility_projection,
            rubric: input.rubric.clone(),
            judge_system: input.plan_relations.judge_system.clone(),
            eligible_semantic_indices: closure.eligible_semantic_indices,
        },
    )))
}

fn prepare_pass(
    input: &CandidateJudgePreparationInput<'_, '_>,
    deterministic: &CandidateDeterministicEvaluationRecordV1,
    cancellation: &CancellationToken,
) -> Result<PreparedPortableClosure, CandidateJudgePreparationError> {
    ensure_active(cancellation)?;
    let validation = validate_pre_output_closure(input, deterministic, cancellation)?;
    let compatibility_projection = compile_compatibility_projection(
        input,
        &validation.judge_plan,
        &validation.eligible_semantic_indices,
        cancellation,
    )?;
    ensure_active(cancellation)?;
    let judge_schedule = CandidateJudgeScheduleV1::new(
        &validation.judge_plan,
        deterministic.candidate_receipt_pair_set_id(),
    )
    .map_err(portable_error)?;
    if judge_schedule.candidate_judge_plan_id() != validation.judge_plan.candidate_judge_plan_id()
        || judge_schedule.candidate_receipt_pair_set_id()
            != deterministic.candidate_receipt_pair_set_id()
    {
        return Err(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::ScheduleClosure,
        ));
    }
    let request_aggregate = derive_request_aggregate(
        input,
        &validation.judge_plan,
        &judge_schedule,
        &validation.eligible_semantic_indices,
        cancellation,
    )?;
    Ok(PreparedPortableClosure {
        judge_plan: validation.judge_plan,
        judge_schedule,
        request_aggregate,
        compatibility_projection,
        eligible_semantic_indices: validation.eligible_semantic_indices,
    })
}

fn revalidate_all(
    input: &CandidateJudgePreparationInput<'_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgePreparationError> {
    let mut candidate_a = input
        .candidate_a
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(candidate_a, None, None, true);
    }
    let candidate_b = input
        .candidate_b
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(candidate_a.take(), candidate_b, None, true);
    }
    let case_material = input
        .case_material
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    authority_validation_result(candidate_a, candidate_b, case_material, false)
}

#[expect(
    clippy::similar_names,
    reason = "candidate A and B authorities are identity-significant ordered inputs"
)]
pub(super) fn revalidate_prepared_authorities(
    candidate_a_authority: &VerifiedCandidateBatchSet,
    candidate_b_authority: &VerifiedCandidateBatchSet,
    case_material_authority: &VerifiedGenerationCaseMaterial<'_>,
    judge_plan: &CandidateJudgePlanV1,
    rubric: &LocalJudgeRubric,
    judge_system: &GenerationSystemRecordV1,
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgePreparationError> {
    let mut candidate_a = candidate_a_authority
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(candidate_a, None, None, true);
    }
    let candidate_b = candidate_b_authority
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    if cancellation.is_cancelled() {
        return authority_validation_result(candidate_a.take(), candidate_b, None, true);
    }
    let case_material = case_material_authority
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    authority_validation_result(candidate_a, candidate_b, case_material, false)?;
    let rubric_digest =
        crate::local_judge_rubric_digest(rubric).map_err(CandidateJudgePreparationError::Rubric)?;
    if judge_plan.rubric_digest() != &rubric_digest
        || judge_plan.judge_generation_system_id() != judge_system.generation_system_id()
        || judge_plan.prompt_contract_digest() != &crate::local_judge_prompt_contract_digest()
        || judge_plan.output_schema_digest()
            != &rewrite_inference::local_judge_attempt_output_contract().schema_digest
    {
        return Err(CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::JudgePolicyClosure,
        ));
    }
    ensure_active(cancellation)
}

fn authority_validation_result(
    candidate_a: Option<Box<VerifiedCandidateBatchSetError>>,
    candidate_b: Option<Box<VerifiedCandidateBatchSetError>>,
    case_material: Option<Box<VerifiedGenerationCaseMaterialError>>,
    cancelled: bool,
) -> Result<(), CandidateJudgePreparationError> {
    if candidate_a.is_none() && candidate_b.is_none() && case_material.is_none() && !cancelled {
        Ok(())
    } else {
        Err(CandidateJudgePreparationError::AuthorityValidation {
            candidate_a,
            candidate_b,
            case_material,
            cancelled,
        })
    }
}

pub(super) fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgePreparationError> {
    authority_validation_result(None, None, None, cancellation.is_cancelled())
}

fn portable_error(source: GenerationQualificationContractError) -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::PortableContract { source }
}

#[cfg(test)]
#[path = "candidate_judge_preparation/tests.rs"]
pub(crate) mod tests;
