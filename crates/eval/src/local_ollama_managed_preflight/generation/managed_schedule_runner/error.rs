use std::{convert::Infallible, fmt};

use rewrite_app::effective_runtime_state_observation::{
    EffectiveRuntimeStateObservationError, ManagedJudgeObservationError,
};
use rewrite_app::{
    ManagedJudgeEffectivePackagePlanError, ManagedJudgeEffectivePackageReleaseError,
    ManagedJudgeObservationAuthorityError, ManagedOllamaCloseError, ManagedOllamaLaunchError,
    ManagedOllamaModelPackageError, PackageAttestationError,
};
use rewrite_model::GenerationQualificationContractError;
use rewrite_ollama::OllamaObservedSessionError;

use crate::candidate_judge_preparation::CandidateJudgeRunnerRequestError;
use crate::local_judge_execution::normalization::CandidateJudgeAttemptNormalizationError;
use crate::{LocalOllamaManagedGenerationError, LocalOllamaManagedPreflightError};

use super::super::runner_configuration::ManagedJudgeRunnerConfigurationError;
use super::post_release::ManagedJudgeSchedulePostReleaseFailures;

/// Failure after a managed process was successfully launched.
pub(in crate::local_ollama_managed_preflight::generation) enum ManagedJudgeScheduleRunnerPrimaryFailure
{
    ManagedPreflight(LocalOllamaManagedPreflightError),
    Generation(LocalOllamaManagedGenerationError),
    Session(OllamaObservedSessionError<ManagedJudgeObservationError>),
    Observation(ManagedJudgeObservationError),
    EffectiveState(EffectiveRuntimeStateObservationError),
    Request(CandidateJudgeRunnerRequestError<Infallible>),
    Normalization(CandidateJudgeRunnerRequestError<CandidateJudgeAttemptNormalizationError>),
    PortableContract(GenerationQualificationContractError),
    ObservationAuthority(ManagedJudgeObservationAuthorityError),
    SequenceAuthority,
    RuntimeStateRelationship,
    ManagedBuildRelationship,
    Cancelled,
    DeadlineExceeded,
    PreflightObserverBinding,
    RetainedSessionPreflightBinding,
    PortableOutputRelationship,
}

/// Exhaustive post-launch failure accounting for the schedule runner.
pub(in crate::local_ollama_managed_preflight::generation) struct ManagedJudgeScheduleRunnerFinalizationFailures
{
    pub(super) cleanup: Option<Box<ManagedOllamaCloseError>>,
    pub(super) model: Option<Box<ManagedOllamaModelPackageError>>,
    pub(super) runtime: Option<Box<PackageAttestationError>>,
}

/// Content-redacted terminal failure of the managed schedule runner.
pub(in crate::local_ollama_managed_preflight::generation) enum ManagedJudgeScheduleRunnerError {
    Configuration(ManagedJudgeRunnerConfigurationError),
    DeadlineExceededBeforeLaunch,
    CancelledBeforeLaunch,
    LiveReservationRefused,
    Launch(ManagedOllamaLaunchError),
    PostLaunch {
        primary: Box<ManagedJudgeScheduleRunnerPrimaryFailure>,
        finalization: ManagedJudgeScheduleRunnerFinalizationFailures,
    },
    EffectivePackagePlan(ManagedJudgeEffectivePackagePlanError),
    EffectivePackageRelease {
        source: Box<ManagedJudgeEffectivePackageReleaseError>,
        cancelled: bool,
        deadline_exceeded: bool,
    },
    PostReleaseInvalidated {
        cancelled: bool,
        deadline_exceeded: bool,
        finalization: ManagedJudgeSchedulePostReleaseFailures,
    },
}

impl fmt::Debug for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ManagedPreflight(_) => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::ManagedPreflight"
            }
            Self::Generation(_) => "ManagedJudgeScheduleRunnerPrimaryFailure::Generation",
            Self::Session(_) => "ManagedJudgeScheduleRunnerPrimaryFailure::Session",
            Self::Observation(_) => "ManagedJudgeScheduleRunnerPrimaryFailure::Observation",
            Self::EffectiveState(_) => "ManagedJudgeScheduleRunnerPrimaryFailure::EffectiveState",
            Self::Request(_) => "ManagedJudgeScheduleRunnerPrimaryFailure::Request",
            Self::Normalization(_) => "ManagedJudgeScheduleRunnerPrimaryFailure::Normalization",
            Self::PortableContract(_) => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::PortableContract"
            }
            Self::ObservationAuthority(_) => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::ObservationAuthority"
            }
            Self::SequenceAuthority => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority"
            }
            Self::RuntimeStateRelationship => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::RuntimeStateRelationship"
            }
            Self::ManagedBuildRelationship => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::ManagedBuildRelationship"
            }
            Self::Cancelled => "ManagedJudgeScheduleRunnerPrimaryFailure::Cancelled",
            Self::DeadlineExceeded => "ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded",
            Self::PreflightObserverBinding => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::PreflightObserverBinding"
            }
            Self::RetainedSessionPreflightBinding => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::RetainedSessionPreflightBinding"
            }
            Self::PortableOutputRelationship => {
                "ManagedJudgeScheduleRunnerPrimaryFailure::PortableOutputRelationship"
            }
        })
    }
}

impl fmt::Display for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("managed judge schedule failed after launch")
    }
}

impl std::error::Error for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManagedPreflight(source) => Some(source),
            Self::Generation(source) => Some(source),
            Self::Session(source) => Some(source),
            Self::Observation(source) => Some(source),
            Self::EffectiveState(source) => Some(source),
            Self::Request(source) => Some(source),
            Self::Normalization(source) => Some(source),
            Self::PortableContract(source) => Some(source),
            Self::ObservationAuthority(source) => Some(source),
            Self::SequenceAuthority
            | Self::RuntimeStateRelationship
            | Self::ManagedBuildRelationship
            | Self::Cancelled
            | Self::DeadlineExceeded
            | Self::PreflightObserverBinding
            | Self::RetainedSessionPreflightBinding
            | Self::PortableOutputRelationship => None,
        }
    }
}

impl fmt::Debug for ManagedJudgeScheduleRunnerFinalizationFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeScheduleRunnerFinalizationFailures")
            .field("cleanup_failed", &self.cleanup.is_some())
            .field("model_failed", &self.model.is_some())
            .field("runtime_failed", &self.runtime.is_some())
            .finish()
    }
}

impl fmt::Debug for ManagedJudgeScheduleRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedJudgeScheduleRunnerError");
        match self {
            Self::Configuration(_) => value.field("kind", &"configuration"),
            Self::DeadlineExceededBeforeLaunch => {
                value.field("kind", &"deadline_exceeded_before_launch")
            }
            Self::CancelledBeforeLaunch => value.field("kind", &"cancelled_before_launch"),
            Self::LiveReservationRefused => value.field("kind", &"live_reservation_refused"),
            Self::Launch(_) => value.field("kind", &"launch"),
            Self::PostLaunch {
                primary,
                finalization,
            } => value
                .field("kind", &"post_launch")
                .field("primary", primary)
                .field("finalization", finalization),
            Self::EffectivePackagePlan(_) => value.field("kind", &"effective_package_plan"),
            Self::EffectivePackageRelease {
                cancelled,
                deadline_exceeded,
                ..
            } => value
                .field("kind", &"effective_package_release")
                .field("cancelled", cancelled)
                .field("deadline_exceeded", deadline_exceeded),
            Self::PostReleaseInvalidated {
                cancelled,
                deadline_exceeded,
                finalization,
            } => value
                .field("kind", &"post_release_invalidated")
                .field("cancelled", cancelled)
                .field("deadline_exceeded", deadline_exceeded)
                .field("finalization", finalization),
        };
        value.finish_non_exhaustive()
    }
}

impl fmt::Display for ManagedJudgeScheduleRunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration(_) => "managed judge runner configuration failed",
            Self::DeadlineExceededBeforeLaunch => {
                "managed judge runner deadline expired before launch"
            }
            Self::CancelledBeforeLaunch => "managed judge runner was cancelled before launch",
            Self::LiveReservationRefused => "managed judge runner live reservation was refused",
            Self::Launch(_) => "managed judge runner launch failed",
            Self::PostLaunch { .. } => "managed judge runner failed after launch",
            Self::EffectivePackagePlan(_) => "managed judge effective-package planning failed",
            Self::EffectivePackageRelease { .. } => {
                "managed judge effective-package release failed"
            }
            Self::PostReleaseInvalidated { .. } => {
                "managed judge execution was invalidated after package release"
            }
        })
    }
}

impl std::error::Error for ManagedJudgeScheduleRunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Configuration(source) => Some(source),
            Self::Launch(source) => Some(source),
            Self::PostLaunch { primary, .. } => Some(primary.as_ref()),
            Self::EffectivePackagePlan(source) => Some(source),
            Self::EffectivePackageRelease { source, .. } => Some(source.as_ref()),
            Self::PostReleaseInvalidated { .. }
            | Self::DeadlineExceededBeforeLaunch
            | Self::CancelledBeforeLaunch
            | Self::LiveReservationRefused => None,
        }
    }
}

impl From<LocalOllamaManagedPreflightError> for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn from(value: LocalOllamaManagedPreflightError) -> Self {
        Self::ManagedPreflight(value)
    }
}

impl From<LocalOllamaManagedGenerationError> for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn from(value: LocalOllamaManagedGenerationError) -> Self {
        Self::Generation(value)
    }
}

impl From<ManagedJudgeObservationError> for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn from(value: ManagedJudgeObservationError) -> Self {
        Self::Observation(value)
    }
}

impl From<EffectiveRuntimeStateObservationError> for ManagedJudgeScheduleRunnerPrimaryFailure {
    fn from(value: EffectiveRuntimeStateObservationError) -> Self {
        Self::EffectiveState(value)
    }
}
