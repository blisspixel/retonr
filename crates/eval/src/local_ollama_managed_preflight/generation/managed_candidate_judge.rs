use std::{error::Error, fmt, future::Future, time::Instant};

#[cfg(test)]
use std::time::Duration;

use rewrite_ollama::OllamaModelBinding;
use rewrite_runtime_attestor::ManagedGenerationWorkerLimits;
use rewrite_types::CancellationToken;

use super::live_lifecycle::GenerationQualificationLiveLifecycle;
use super::managed_local_judge_receipt::{
    ManagedLocalJudgeReceiptCompiler, ManagedLocalJudgeReceiptCompilerError,
};
use super::managed_schedule_runner::{
    ManagedJudgeScheduleRunnerError, OperationGateFailure, OperationPrecedenceError,
    ensure_operation_active, operation_precedence, run_managed_judge_schedule,
};
use super::runner_configuration::{
    ManagedJudgeRunnerConfigurationError, ManagedJudgeRunnerConfigurationInput,
    configure_managed_judge_runner,
};
use super::verified_candidate_judge_join::{
    VerifiedCandidateJudgeJoin, VerifiedCandidateJudgeJoinCompiler,
    VerifiedCandidateJudgeJoinCompilerError,
};
use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightLimits,
    LocalOllamaModelBindingEvidence, PairedCandidateJudgeRunnerHandoff,
};

/// Stable stage at which a complete managed candidate-judge run failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedCandidateJudgeRunErrorKind {
    /// No-launch eval and app configuration failed.
    Configuration,
    /// Managed schedule launch, traffic, or finalization failed.
    ScheduleExecution,
    /// Durable managed local-judge receipt compilation failed.
    ReceiptCompilation,
    /// Exact compatibility triage or candidate-to-judge join compilation failed.
    JoinCompilation,
}

/// Content-redacted failure from a complete managed candidate-judge run.
pub struct ManagedCandidateJudgeRunError {
    kind: ManagedCandidateJudgeRunErrorKind,
    detail: ManagedCandidateJudgeRunErrorDetail,
}

enum ManagedCandidateJudgeRunErrorDetail {
    Configuration(ManagedJudgeRunnerConfigurationError),
    ScheduleExecution(ManagedJudgeScheduleRunnerError),
    ReceiptCompilation(ManagedLocalJudgeReceiptCompilerError),
    JoinCompilation(VerifiedCandidateJudgeJoinCompilerError),
    Terminal,
}

impl ManagedCandidateJudgeRunError {
    /// Returns the stable categorical failure without exposing live authority detail.
    #[must_use]
    pub const fn kind(&self) -> ManagedCandidateJudgeRunErrorKind {
        self.kind
    }
}

impl fmt::Display for ManagedCandidateJudgeRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "managed candidate judge run failed: {:?}",
            self.kind
        )
    }
}

impl fmt::Debug for ManagedCandidateJudgeRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedCandidateJudgeRunError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl Error for ManagedCandidateJudgeRunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.detail {
            ManagedCandidateJudgeRunErrorDetail::Configuration(source) => Some(source),
            ManagedCandidateJudgeRunErrorDetail::ScheduleExecution(source) => Some(source),
            ManagedCandidateJudgeRunErrorDetail::ReceiptCompilation(source) => Some(source),
            ManagedCandidateJudgeRunErrorDetail::JoinCompilation(source) => Some(source),
            ManagedCandidateJudgeRunErrorDetail::Terminal => None,
        }
    }
}

/// Runs qualification under Prepared's original absolute operation deadline.
///
/// No judge-local deadline is calculated or retained. Every stage consumes the
/// exact supplied instant and cannot extend it.
#[expect(
    clippy::too_many_arguments,
    reason = "the compatibility entry's exact authorities remain explicit beside Prepared's deadline"
)]
pub(crate) async fn run_verified_managed_candidate_judge_until<
    'store,
    'records,
    'model,
    'runtime,
>(
    paired: PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, '_>,
    preflight_plan: LocalOllamaBoundPreflightPlan,
    preflight_limits: LocalOllamaManagedPreflightLimits,
    worker_limits: ManagedGenerationWorkerLimits,
    model_evidence: LocalOllamaModelBindingEvidence,
    model: OllamaModelBinding,
    operation_deadline: Instant,
    lifecycle: &GenerationQualificationLiveLifecycle,
    cancellation: &CancellationToken,
) -> Result<
    VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>,
    ManagedCandidateJudgeRunError,
> {
    let input = ManagedJudgeRunnerConfigurationInput {
        preflight_plan,
        preflight_limits,
        worker_limits,
        model_evidence,
        model,
    };
    Box::pin(run_pipeline_until(
        cancellation,
        operation_deadline,
        || configure_managed_judge_runner(paired, input, cancellation),
        |configuration| {
            run_managed_judge_schedule(configuration, operation_deadline, lifecycle, cancellation)
        },
        |execution| {
            std::future::ready(ManagedLocalJudgeReceiptCompiler::compile(
                execution,
                cancellation,
            ))
        },
        |receipt| {
            std::future::ready(VerifiedCandidateJudgeJoinCompiler::compile(
                receipt,
                cancellation,
            ))
        },
    ))
    .await
    .map_err(map_pipeline_error)
}

#[cfg(test)]
fn joined_run_deadline(started: Instant, maximum_elapsed_milliseconds: u32) -> Option<Instant> {
    started.checked_add(Duration::from_millis(u64::from(
        maximum_elapsed_milliseconds,
    )))
}

enum ManagedCandidateJudgePipelineError<C, E, R, J> {
    Terminal(OperationGateFailure),
    Configuration(C),
    ScheduleExecution(E),
    ReceiptCompilation(R),
    JoinCompilation(J),
}

async fn run_pipeline_until<
    C,
    E,
    R,
    J,
    CE,
    EE,
    RE,
    JE,
    Configure,
    Execute,
    Receipt,
    Join,
    EF,
    RF,
    JF,
>(
    cancellation: &CancellationToken,
    operation_deadline: Instant,
    configure: Configure,
    execute: Execute,
    compile_receipt: Receipt,
    compile_join: Join,
) -> Result<J, ManagedCandidateJudgePipelineError<CE, EE, RE, JE>>
where
    Configure: FnOnce() -> Result<C, CE>,
    Execute: FnOnce(C) -> EF,
    Receipt: FnOnce(E) -> RF,
    Join: FnOnce(R) -> JF,
    EF: Future<Output = Result<E, EE>>,
    RF: Future<Output = Result<R, RE>>,
    JF: Future<Output = Result<J, JE>>,
{
    ensure_pipeline_active(cancellation, operation_deadline)?;
    let configuration = map_pipeline_boundary(
        configure(),
        cancellation,
        operation_deadline,
        ManagedCandidateJudgePipelineError::Configuration,
    )?;
    ensure_pipeline_active(cancellation, operation_deadline)?;
    let execution = map_pipeline_boundary(
        execute(configuration).await,
        cancellation,
        operation_deadline,
        ManagedCandidateJudgePipelineError::ScheduleExecution,
    )?;
    ensure_pipeline_active(cancellation, operation_deadline)?;
    let receipt = map_pipeline_boundary(
        compile_receipt(execution).await,
        cancellation,
        operation_deadline,
        ManagedCandidateJudgePipelineError::ReceiptCompilation,
    )?;
    ensure_pipeline_active(cancellation, operation_deadline)?;
    map_pipeline_boundary(
        compile_join(receipt).await,
        cancellation,
        operation_deadline,
        ManagedCandidateJudgePipelineError::JoinCompilation,
    )
}

fn ensure_pipeline_active<C, E, R, J>(
    cancellation: &CancellationToken,
    operation_deadline: Instant,
) -> Result<(), ManagedCandidateJudgePipelineError<C, E, R, J>> {
    ensure_operation_active(cancellation, operation_deadline)
        .map_err(ManagedCandidateJudgePipelineError::Terminal)
}

fn map_pipeline_boundary<T, E, C, EE, R, J>(
    result: Result<T, E>,
    cancellation: &CancellationToken,
    operation_deadline: Instant,
    map_underlying: impl FnOnce(E) -> ManagedCandidateJudgePipelineError<C, EE, R, J>,
) -> Result<T, ManagedCandidateJudgePipelineError<C, EE, R, J>> {
    match operation_precedence(result, cancellation, operation_deadline) {
        Ok(value) => Ok(value),
        Err(OperationPrecedenceError::DeadlineExceeded) => Err(
            ManagedCandidateJudgePipelineError::Terminal(OperationGateFailure::DeadlineExceeded),
        ),
        Err(OperationPrecedenceError::Cancelled) => Err(
            ManagedCandidateJudgePipelineError::Terminal(OperationGateFailure::Cancelled),
        ),
        Err(OperationPrecedenceError::Underlying(error)) => Err(map_underlying(error)),
    }
}

#[cfg(test)]
async fn run_pipeline<C, E, R, J, CE, EE, RE, JE, Configure, Execute, Receipt, Join, EF, RF, JF>(
    configure: Configure,
    execute: Execute,
    compile_receipt: Receipt,
    compile_join: Join,
) -> Result<J, ManagedCandidateJudgePipelineError<CE, EE, RE, JE>>
where
    Configure: FnOnce() -> Result<C, CE>,
    Execute: FnOnce(C) -> EF,
    Receipt: FnOnce(E) -> RF,
    Join: FnOnce(R) -> JF,
    EF: Future<Output = Result<E, EE>>,
    RF: Future<Output = Result<R, RE>>,
    JF: Future<Output = Result<J, JE>>,
{
    let configuration = configure().map_err(ManagedCandidateJudgePipelineError::Configuration)?;
    let execution = execute(configuration)
        .await
        .map_err(ManagedCandidateJudgePipelineError::ScheduleExecution)?;
    let receipt = compile_receipt(execution)
        .await
        .map_err(ManagedCandidateJudgePipelineError::ReceiptCompilation)?;
    compile_join(receipt)
        .await
        .map_err(ManagedCandidateJudgePipelineError::JoinCompilation)
}

fn map_pipeline_error(
    error: ManagedCandidateJudgePipelineError<
        ManagedJudgeRunnerConfigurationError,
        ManagedJudgeScheduleRunnerError,
        ManagedLocalJudgeReceiptCompilerError,
        VerifiedCandidateJudgeJoinCompilerError,
    >,
) -> ManagedCandidateJudgeRunError {
    let (kind, detail) = match error {
        ManagedCandidateJudgePipelineError::Terminal(_failure) => (
            ManagedCandidateJudgeRunErrorKind::ScheduleExecution,
            ManagedCandidateJudgeRunErrorDetail::Terminal,
        ),
        ManagedCandidateJudgePipelineError::Configuration(source) => (
            ManagedCandidateJudgeRunErrorKind::Configuration,
            ManagedCandidateJudgeRunErrorDetail::Configuration(source),
        ),
        ManagedCandidateJudgePipelineError::ScheduleExecution(source) => (
            ManagedCandidateJudgeRunErrorKind::ScheduleExecution,
            ManagedCandidateJudgeRunErrorDetail::ScheduleExecution(source),
        ),
        ManagedCandidateJudgePipelineError::ReceiptCompilation(source) => (
            ManagedCandidateJudgeRunErrorKind::ReceiptCompilation,
            ManagedCandidateJudgeRunErrorDetail::ReceiptCompilation(source),
        ),
        ManagedCandidateJudgePipelineError::JoinCompilation(source) => (
            ManagedCandidateJudgeRunErrorKind::JoinCompilation,
            ManagedCandidateJudgeRunErrorDetail::JoinCompilation(source),
        ),
    };
    ManagedCandidateJudgeRunError { kind, detail }
}

#[cfg(test)]
mod tests;
