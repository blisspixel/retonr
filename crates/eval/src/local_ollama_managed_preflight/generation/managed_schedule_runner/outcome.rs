use std::fmt;

use rewrite_app::{
    ManagedJudgeEffectivePackageFinalValidationError, ManagedOllamaModelPackageError,
    PackageAttestationError, ReleasedManagedJudgeEffectivePackageV2, RuntimePackageLease,
    VerifiedManagedOllamaModelPackageLease,
};
use rewrite_model::{
    CandidateJudgeObservationBatchV1, CandidateJudgeResponseAggregateV1, RuntimePackageManifest,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_types::CancellationToken;

use crate::candidate_judge_preparation::{
    CandidateJudgePreparationError, CandidateJudgeRunnerHandoff,
};
use crate::{
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightError,
    LocalOllamaManagedPreflightLimits, LocalOllamaManagedPreflightOutcome,
    local_ollama_managed_preflight::revalidate_managed_preflight_outcome,
};

/// Cleanup-gated retained authority for the next durable receipt compiler.
///
/// This capability owns the exact eval handoff and retains both concrete package
/// authorities. A portable copy of its content-free records cannot recreate it.
pub(in crate::local_ollama_managed_preflight::generation) struct VerifiedManagedJudgeScheduleExecution<
    'store,
    'records,
    'model,
    'runtime,
> {
    eval: CandidateJudgeRunnerHandoff<'store>,
    runtime_manifest: &'records RuntimePackageManifest,
    frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
    preflight_plan: LocalOllamaBoundPreflightPlan,
    preflight_limits: LocalOllamaManagedPreflightLimits,
    model_package: &'model VerifiedManagedOllamaModelPackageLease,
    runtime_package: &'runtime mut RuntimePackageLease,
    released_package: ReleasedManagedJudgeEffectivePackageV2,
    response_aggregate: CandidateJudgeResponseAggregateV1,
    observation_batch: CandidateJudgeObservationBatchV1,
    managed_preflight: LocalOllamaManagedPreflightOutcome,
}

impl<'store, 'records, 'model, 'runtime>
    VerifiedManagedJudgeScheduleExecution<'store, 'records, 'model, 'runtime>
{
    #[expect(
        clippy::too_many_arguments,
        reason = "every retained authority and owner-produced record remains explicit"
    )]
    pub(super) const fn new(
        eval: CandidateJudgeRunnerHandoff<'store>,
        runtime_manifest: &'records RuntimePackageManifest,
        frozen_components: &'records VerifiedFrozenExternalNativeComponentSet,
        preflight_plan: LocalOllamaBoundPreflightPlan,
        preflight_limits: LocalOllamaManagedPreflightLimits,
        model_package: &'model VerifiedManagedOllamaModelPackageLease,
        runtime_package: &'runtime mut RuntimePackageLease,
        released_package: ReleasedManagedJudgeEffectivePackageV2,
        response_aggregate: CandidateJudgeResponseAggregateV1,
        observation_batch: CandidateJudgeObservationBatchV1,
        managed_preflight: LocalOllamaManagedPreflightOutcome,
    ) -> Self {
        Self {
            eval,
            runtime_manifest,
            frozen_components,
            preflight_plan,
            preflight_limits,
            model_package,
            runtime_package,
            released_package,
            response_aggregate,
            observation_batch,
            managed_preflight,
        }
    }

    pub(in crate::local_ollama_managed_preflight::generation) fn with_revalidated_authorities<
        T,
        E,
    >(
        &mut self,
        cancellation: &CancellationToken,
        use_authorities: impl FnOnce(ManagedJudgeScheduleExecutionView<'_, 'store>) -> Result<T, E>,
    ) -> Result<T, ManagedJudgeScheduleExecutionAuthorityError<E>> {
        if let Err(initial) = self.revalidate_all(cancellation, false) {
            return finish_failed_initial_validation(initial, || {
                self.revalidate_all(cancellation, true)
            });
        }
        let primary = use_authorities(self.view());
        let final_validation = self.revalidate_all(cancellation, true);
        combine_bracket(primary, final_validation)
    }

    fn revalidate_all(
        &mut self,
        cancellation: &CancellationToken,
        fresh_tokens: bool,
    ) -> Result<(), ManagedJudgeScheduleAuthorityFailures> {
        let Self {
            eval,
            runtime_manifest,
            frozen_components,
            preflight_plan,
            preflight_limits,
            model_package,
            runtime_package,
            released_package,
            managed_preflight,
            ..
        } = self;
        let failures = run_revalidation_kernel(
            cancellation,
            fresh_tokens,
            |token| eval.revalidate(token),
            |token| released_package.revalidate_retained_authority(token),
            |_token| {
                revalidate_managed_preflight_outcome(
                    runtime_manifest,
                    preflight_plan,
                    frozen_components.expected_components(),
                    *preflight_limits,
                    managed_preflight,
                )
            },
            |token| model_package.revalidate(token),
            |token| runtime_package.revalidate(token),
        );
        let eval = failures.eval.err().map(Box::new);
        let app = failures.app.err().map(Box::new);
        let preflight = failures.preflight.err().map(Box::new);
        let model = failures.model.err().map(Box::new);
        let runtime = failures.runtime.err().map(Box::new);
        if eval.is_some()
            || app.is_some()
            || preflight.is_some()
            || model.is_some()
            || runtime.is_some()
            || failures.operation_cancelled
        {
            Err(ManagedJudgeScheduleAuthorityFailures {
                eval,
                app,
                preflight,
                model,
                runtime,
                cancelled: failures.operation_cancelled,
            })
        } else {
            Ok(())
        }
    }

    fn view(&self) -> ManagedJudgeScheduleExecutionView<'_, 'store> {
        ManagedJudgeScheduleExecutionView {
            eval: &self.eval,
            released_package: &self.released_package,
            response_aggregate: &self.response_aggregate,
            observation_batch: &self.observation_batch,
            managed_preflight: &self.managed_preflight,
        }
    }
}

pub(in crate::local_ollama_managed_preflight::generation) struct ManagedJudgeScheduleExecutionView<
    'borrow,
    'store,
> {
    pub(in crate::local_ollama_managed_preflight::generation) eval:
        &'borrow CandidateJudgeRunnerHandoff<'store>,
    pub(in crate::local_ollama_managed_preflight::generation) released_package:
        &'borrow ReleasedManagedJudgeEffectivePackageV2,
    pub(in crate::local_ollama_managed_preflight::generation) response_aggregate:
        &'borrow CandidateJudgeResponseAggregateV1,
    pub(in crate::local_ollama_managed_preflight::generation) observation_batch:
        &'borrow CandidateJudgeObservationBatchV1,
    pub(in crate::local_ollama_managed_preflight::generation) managed_preflight:
        &'borrow LocalOllamaManagedPreflightOutcome,
}

pub(in crate::local_ollama_managed_preflight::generation) struct ManagedJudgeScheduleAuthorityFailures
{
    eval: Option<Box<CandidateJudgePreparationError>>,
    app: Option<Box<ManagedJudgeEffectivePackageFinalValidationError>>,
    preflight: Option<Box<LocalOllamaManagedPreflightError>>,
    model: Option<Box<ManagedOllamaModelPackageError>>,
    runtime: Option<Box<PackageAttestationError>>,
    cancelled: bool,
}

impl ManagedJudgeScheduleAuthorityFailures {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next receipt error surface forwards this typed cause"
        )
    )]
    pub(in crate::local_ollama_managed_preflight::generation) fn eval(
        &self,
    ) -> Option<&CandidateJudgePreparationError> {
        self.eval.as_deref()
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next receipt error surface forwards this typed cause"
        )
    )]
    pub(in crate::local_ollama_managed_preflight::generation) fn app(
        &self,
    ) -> Option<&ManagedJudgeEffectivePackageFinalValidationError> {
        self.app.as_deref()
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next receipt error surface forwards this typed cause"
        )
    )]
    pub(in crate::local_ollama_managed_preflight::generation) fn preflight(
        &self,
    ) -> Option<&LocalOllamaManagedPreflightError> {
        self.preflight.as_deref()
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next receipt error surface forwards this typed cause"
        )
    )]
    pub(in crate::local_ollama_managed_preflight::generation) fn model(
        &self,
    ) -> Option<&ManagedOllamaModelPackageError> {
        self.model.as_deref()
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next receipt error surface forwards this typed cause"
        )
    )]
    pub(in crate::local_ollama_managed_preflight::generation) fn runtime(
        &self,
    ) -> Option<&PackageAttestationError> {
        self.runtime.as_deref()
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the next receipt error surface forwards this typed cause"
        )
    )]
    pub(in crate::local_ollama_managed_preflight::generation) const fn cancelled(&self) -> bool {
        self.cancelled
    }
}

#[cfg(test)]
impl ManagedJudgeScheduleAuthorityFailures {
    pub(in crate::local_ollama_managed_preflight::generation) const fn cancelled_for_test() -> Self
    {
        Self {
            eval: None,
            app: None,
            preflight: None,
            model: None,
            runtime: None,
            cancelled: true,
        }
    }

    pub(in crate::local_ollama_managed_preflight::generation) fn eval_for_test(
        source: CandidateJudgePreparationError,
        cancelled: bool,
    ) -> Self {
        Self {
            eval: Some(Box::new(source)),
            app: None,
            preflight: None,
            model: None,
            runtime: None,
            cancelled,
        }
    }
}

struct RevalidationKernelResult<E, A, P, M, R> {
    eval: Result<(), E>,
    app: Result<(), A>,
    preflight: Result<(), P>,
    model: Result<(), M>,
    runtime: Result<(), R>,
    operation_cancelled: bool,
}

fn run_revalidation_kernel<E, A, P, M, R>(
    operation: &CancellationToken,
    fresh_tokens: bool,
    eval: impl FnOnce(&CancellationToken) -> Result<(), E>,
    app: impl FnOnce(&CancellationToken) -> Result<(), A>,
    preflight: impl FnOnce(&CancellationToken) -> Result<(), P>,
    model: impl FnOnce(&CancellationToken) -> Result<(), M>,
    runtime: impl FnOnce(&CancellationToken) -> Result<(), R>,
) -> RevalidationKernelResult<E, A, P, M, R> {
    let eval = if fresh_tokens {
        eval(&CancellationToken::new())
    } else {
        eval(operation)
    };
    let app = if fresh_tokens {
        app(&CancellationToken::new())
    } else {
        app(operation)
    };
    let preflight = if fresh_tokens {
        preflight(&CancellationToken::new())
    } else {
        preflight(operation)
    };
    let model = if fresh_tokens {
        model(&CancellationToken::new())
    } else {
        model(operation)
    };
    let runtime = if fresh_tokens {
        runtime(&CancellationToken::new())
    } else {
        runtime(operation)
    };
    RevalidationKernelResult {
        eval,
        app,
        preflight,
        model,
        runtime,
        operation_cancelled: operation.is_cancelled(),
    }
}

pub(in crate::local_ollama_managed_preflight::generation) enum ManagedJudgeScheduleExecutionAuthorityError<
    E,
> {
    Initial(ManagedJudgeScheduleAuthorityFailures),
    InitialAndFinal {
        initial: ManagedJudgeScheduleAuthorityFailures,
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
    Callback(E),
    Final(ManagedJudgeScheduleAuthorityFailures),
    CallbackAndFinal {
        callback: E,
        final_validation: ManagedJudgeScheduleAuthorityFailures,
    },
}

fn combine_bracket<T, E>(
    primary: Result<T, E>,
    final_validation: Result<(), ManagedJudgeScheduleAuthorityFailures>,
) -> Result<T, ManagedJudgeScheduleExecutionAuthorityError<E>> {
    match (primary, final_validation) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(callback), Ok(())) => Err(ManagedJudgeScheduleExecutionAuthorityError::Callback(
            callback,
        )),
        (Ok(value), Err(final_validation)) => {
            drop(value);
            Err(ManagedJudgeScheduleExecutionAuthorityError::Final(
                final_validation,
            ))
        }
        (Err(callback), Err(final_validation)) => Err(
            ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
                callback,
                final_validation,
            },
        ),
    }
}

fn finish_failed_initial_validation<T, E>(
    initial: ManagedJudgeScheduleAuthorityFailures,
    final_validation: impl FnOnce() -> Result<(), ManagedJudgeScheduleAuthorityFailures>,
) -> Result<T, ManagedJudgeScheduleExecutionAuthorityError<E>> {
    match final_validation() {
        Ok(()) => Err(ManagedJudgeScheduleExecutionAuthorityError::Initial(
            initial,
        )),
        Err(final_validation) => Err(
            ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
                initial,
                final_validation,
            },
        ),
    }
}

impl fmt::Debug for VerifiedManagedJudgeScheduleExecution<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedJudgeScheduleExecution")
            .field("judge_plan_id", self.released_package.judge_plan_id())
            .field(
                "judge_schedule_id",
                self.released_package.judge_schedule_id(),
            )
            .field("attempt_count", &self.released_package.attempt_count())
            .field(
                "response_aggregate_id",
                self.response_aggregate.response_aggregate_id(),
            )
            .field(
                "observation_batch_id",
                self.observation_batch.observation_batch_id(),
            )
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for ManagedJudgeScheduleAuthorityFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeScheduleAuthorityFailures")
            .field("eval_failed", &self.eval.is_some())
            .field("app_failed", &self.app.is_some())
            .field("preflight_failed", &self.preflight.is_some())
            .field("model_failed", &self.model.is_some())
            .field("runtime_failed", &self.runtime.is_some())
            .field("cancelled", &self.cancelled)
            .finish()
    }
}

impl fmt::Display for ManagedJudgeScheduleAuthorityFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("managed judge schedule authority revalidation failed")
    }
}

impl std::error::Error for ManagedJudgeScheduleAuthorityFailures {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.eval
            .as_deref()
            .map(|error| error as &(dyn std::error::Error + 'static))
            .or_else(|| {
                self.app
                    .as_deref()
                    .map(|error| error as &(dyn std::error::Error + 'static))
            })
            .or_else(|| {
                self.preflight
                    .as_deref()
                    .map(|error| error as &(dyn std::error::Error + 'static))
            })
            .or_else(|| {
                self.model
                    .as_deref()
                    .map(|error| error as &(dyn std::error::Error + 'static))
            })
            .or_else(|| {
                self.runtime
                    .as_deref()
                    .map(|error| error as &(dyn std::error::Error + 'static))
            })
    }
}

impl<E> fmt::Debug for ManagedJudgeScheduleExecutionAuthorityError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedJudgeScheduleExecutionAuthorityError");
        match self {
            Self::Initial(failures) => value.field("kind", &"initial").field("failures", failures),
            Self::InitialAndFinal {
                initial,
                final_validation,
            } => value
                .field("kind", &"initial_and_final")
                .field("initial", initial)
                .field("final_validation", final_validation),
            Self::Callback(callback) => {
                let _ = callback;
                value.field("kind", &"callback")
            }
            Self::Final(failures) => value.field("kind", &"final").field("failures", failures),
            Self::CallbackAndFinal {
                callback,
                final_validation,
            } => {
                let _ = callback;
                value
                    .field("kind", &"callback_and_final")
                    .field("final_validation", final_validation)
            }
        };
        value.finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
