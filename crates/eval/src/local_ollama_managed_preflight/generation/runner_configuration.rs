//! No-launch configuration join for the exact managed candidate-judge runner.

use std::fmt;

use rewrite_app::{
    ManagedJudgePrecursorCompilationError, ManagedJudgePrecursorRunnerHandoff,
    PackageAttestationError,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::candidate_judge_preparation::{
    CandidateJudgePreparationError, CandidateJudgeRunnerHandoff, PairedCandidateJudgeRunnerHandoff,
};
mod context;
mod kernel;
mod validation;

pub(in crate::local_ollama_managed_preflight::generation) use context::ManagedJudgeRunnerContext;
use context::named_context;
pub(in crate::local_ollama_managed_preflight::generation) use context::{
    ManagedJudgeRunnerConfigurationInput, VerifiedManagedJudgeRunnerConfiguration,
};
use kernel::{ConfigurationSubject, validate_configuration};
use validation::{validate_caller_configuration, validate_exact_context};

pub(super) const MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT: usize = 7;

/// Authority-validation bracket around caller-supplied runner configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ManagedJudgeRunnerConfigurationValidationPhase {
    /// Validation before inspecting caller-supplied configuration.
    Initial,
    /// Validation after inspecting caller-supplied configuration.
    Final,
}

/// Exact no-launch relationship rejected by runner configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ManagedJudgeRunnerConfigurationRelationship {
    /// The bound preflight was not the exact one-model managed profile.
    PreflightProfile,
    /// Caller-selected process or native-load limits were invalid.
    PreflightLimits,
    /// Caller-selected generation-worker limits were invalid.
    WorkerLimits,
    /// Runtime records and retained runtime authorities did not form one closure.
    Runtime,
    /// The prepared isolation did not match the expected runtime state and helper.
    Isolation,
    /// Static model, runtime model, launch input, and judge system did not agree.
    Model,
    /// Complete portable judge records changed across the paired handoffs.
    PortableJudge,
    /// A captured installation generation was zero, stale, or substituted.
    InstallationGeneration,
}

/// Failure to configure the exact managed candidate-judge runner without launch.
#[derive(Error)]
pub(super) enum ManagedJudgeRunnerConfigurationError {
    /// Work was cancelled outside a retained authority validation operation.
    #[error("managed judge runner configuration was cancelled")]
    Cancelled,
    /// One validation bracket observed one or both authority families failing.
    #[error("managed judge runner authority validation failed during {phase:?}")]
    AuthorityValidation {
        /// Exact validation bracket.
        phase: ManagedJudgeRunnerConfigurationValidationPhase,
        /// Eval-owned validation failure, when observed before cancellation.
        eval: Option<Box<CandidateJudgePreparationError>>,
        /// App-owned validation failure, when observed before cancellation.
        app: Option<Box<ManagedJudgePrecursorCompilationError>>,
        /// Whether cancellation stopped or invalidated this bracket.
        cancelled: bool,
    },
    /// Retained eval or runtime-package authority failed after deterministic joins.
    #[error("managed judge runner retained authority validation failed")]
    RetainedAuthorityValidation {
        /// Eval-owned authority failure.
        eval: Option<Box<CandidateJudgePreparationError>>,
        /// Retained runtime-package failure.
        runtime: Option<Box<PackageAttestationError>>,
        /// Whether cancellation stopped or invalidated the validation.
        cancelled: bool,
    },
    /// One caller or retained relationship did not match.
    #[error("managed judge runner configuration relationship is invalid: {0:?}")]
    Relationship(ManagedJudgeRunnerConfigurationRelationship),
    /// Caller configuration and mandatory final authority validation both failed.
    #[error("managed judge runner configuration and final authority validation both failed")]
    PrimaryAndFinalValidation {
        /// Primary cancellation or relationship failure.
        primary: Box<Self>,
        /// Independent final authority-validation failure.
        final_validation: Box<Self>,
    },
}

impl fmt::Debug for ManagedJudgeRunnerConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedJudgeRunnerConfigurationError");
        match self {
            Self::Cancelled => value.field("kind", &"cancelled"),
            Self::AuthorityValidation {
                phase,
                eval,
                app,
                cancelled,
            } => value
                .field("kind", &"authority_validation")
                .field("phase", phase)
                .field("eval_failed", &eval.is_some())
                .field("app_failed", &app.is_some())
                .field("cancelled", cancelled),
            Self::RetainedAuthorityValidation {
                eval,
                runtime,
                cancelled,
            } => value
                .field("kind", &"retained_authority_validation")
                .field("eval_failed", &eval.is_some())
                .field("runtime_failed", &runtime.is_some())
                .field("cancelled", cancelled),
            Self::Relationship(relationship) => value
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::PrimaryAndFinalValidation { .. } => {
                value.field("kind", &"primary_and_final_validation")
            }
        };
        value.finish_non_exhaustive()
    }
}

/// Compiles the exact paired handoff and inert configuration without launching.
pub(super) fn configure_managed_judge_runner<'store, 'records, 'model, 'runtime, 'characterized>(
    paired: PairedCandidateJudgeRunnerHandoff<'store, 'records, 'model, 'runtime, 'characterized>,
    input: ManagedJudgeRunnerConfigurationInput,
    cancellation: &CancellationToken,
) -> Result<
    VerifiedManagedJudgeRunnerConfiguration<'store, 'records, 'model, 'runtime, 'characterized>,
    ManagedJudgeRunnerConfigurationError,
> {
    let (eval, app) = paired.into_parts();
    validate_configuration(
        ConcreteConfigurationSubject {
            eval: Some(eval),
            app: Some(app),
            input: Some(input),
        },
        cancellation,
    )
}

struct ConcreteConfigurationSubject<'store, 'records, 'model, 'runtime, 'characterized> {
    eval: Option<CandidateJudgeRunnerHandoff<'store>>,
    app: Option<ManagedJudgePrecursorRunnerHandoff<'records, 'model, 'runtime, 'characterized>>,
    input: Option<ManagedJudgeRunnerConfigurationInput>,
}

impl<'store, 'records, 'model, 'runtime, 'characterized> ConfigurationSubject
    for ConcreteConfigurationSubject<'store, 'records, 'model, 'runtime, 'characterized>
{
    type Output =
        VerifiedManagedJudgeRunnerConfiguration<'store, 'records, 'model, 'runtime, 'characterized>;

    fn revalidate_eval(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        self.eval
            .as_ref()
            .ok_or(CandidateJudgePreparationError::Relationship(
                crate::candidate_judge_preparation::CandidateJudgePreparationRelationship::JudgePlan,
            ))?
            .revalidate(cancellation)
    }

    fn revalidate_app(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgePrecursorCompilationError> {
        self.app
            .as_mut()
            .ok_or(ManagedJudgePrecursorCompilationError::Cancelled)?
            .revalidate(cancellation)
    }

    fn validate_caller_configuration(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeRunnerConfigurationError> {
        let input =
            self.input
                .as_ref()
                .ok_or(ManagedJudgeRunnerConfigurationError::Relationship(
                    ManagedJudgeRunnerConfigurationRelationship::PortableJudge,
                ))?;
        validate_caller_configuration(input, cancellation)
    }

    fn release(
        mut self,
        cancellation: &CancellationToken,
    ) -> Result<Self::Output, ManagedJudgeRunnerConfigurationError> {
        ensure_not_cancelled(cancellation)?;
        let eval = self.eval.take().ok_or_else(|| {
            relationship(ManagedJudgeRunnerConfigurationRelationship::PortableJudge)
        })?;
        let app = self.app.take().ok_or_else(|| {
            relationship(ManagedJudgeRunnerConfigurationRelationship::PortableJudge)
        })?;
        let input = self.input.take().ok_or_else(|| {
            relationship(ManagedJudgeRunnerConfigurationRelationship::PortableJudge)
        })?;
        let mut context = named_context(eval, app, input);
        validate_exact_context(&mut context, cancellation)?;
        Ok(VerifiedManagedJudgeRunnerConfiguration { context })
    }
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    if cancellation.is_cancelled() {
        Err(ManagedJudgeRunnerConfigurationError::Cancelled)
    } else {
        Ok(())
    }
}

const fn relationship(
    value: ManagedJudgeRunnerConfigurationRelationship,
) -> ManagedJudgeRunnerConfigurationError {
    ManagedJudgeRunnerConfigurationError::Relationship(value)
}

#[cfg(test)]
mod tests;
