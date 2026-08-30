use std::fmt;

use rewrite_app::{
    ManagedJudgeEffectivePackageFinalValidationError, ManagedOllamaModelPackageError,
    PackageAttestationError, ReleasedManagedJudgeEffectivePackageV2, RuntimePackageLease,
    VerifiedManagedOllamaModelPackageLease,
};
use rewrite_types::CancellationToken;

use crate::candidate_judge_preparation::{
    CandidateJudgePreparationError, CandidateJudgeRunnerHandoff,
};

pub(in crate::local_ollama_managed_preflight::generation) struct ManagedJudgeSchedulePostReleaseFailures
{
    eval: Option<Box<CandidateJudgePreparationError>>,
    app: Option<Box<ManagedJudgeEffectivePackageFinalValidationError>>,
    model: Option<Box<ManagedOllamaModelPackageError>>,
    runtime: Option<Box<PackageAttestationError>>,
}

pub(super) enum ReleaseDisposition<T, E> {
    Released(T),
    Failed {
        source: E,
        cancelled: bool,
        deadline_exceeded: bool,
    },
    Invalidated {
        released: T,
        cancelled: bool,
        deadline_exceeded: bool,
    },
}

pub(super) fn classify_release<T, E>(
    release: Result<T, E>,
    cancelled: bool,
    deadline_exceeded: bool,
) -> ReleaseDisposition<T, E> {
    match release {
        Err(source) => ReleaseDisposition::Failed {
            source,
            cancelled,
            deadline_exceeded,
        },
        Ok(released) if cancelled || deadline_exceeded => ReleaseDisposition::Invalidated {
            released,
            cancelled,
            deadline_exceeded,
        },
        Ok(released) => ReleaseDisposition::Released(released),
    }
}

pub(super) fn finalize_invalid_released_execution(
    eval: &CandidateJudgeRunnerHandoff<'_>,
    released: &ReleasedManagedJudgeEffectivePackageV2,
    model: &VerifiedManagedOllamaModelPackageLease,
    runtime: &mut RuntimePackageLease,
) -> ManagedJudgeSchedulePostReleaseFailures {
    let (eval, app, model, runtime) = run_fresh_finalizers(
        |token| eval.revalidate(token),
        |token| released.revalidate_retained_authority(token),
        |token| model.revalidate(token),
        |token| runtime.revalidate(token),
    );
    ManagedJudgeSchedulePostReleaseFailures {
        eval: eval.map(Box::new),
        app: app.map(Box::new),
        model: model.map(Box::new),
        runtime: runtime.map(Box::new),
    }
}

fn run_fresh_finalizers<E, A, M, R>(
    eval: impl FnOnce(&CancellationToken) -> Result<(), E>,
    app: impl FnOnce(&CancellationToken) -> Result<(), A>,
    model: impl FnOnce(&CancellationToken) -> Result<(), M>,
    runtime: impl FnOnce(&CancellationToken) -> Result<(), R>,
) -> (Option<E>, Option<A>, Option<M>, Option<R>) {
    (
        eval(&CancellationToken::new()).err(),
        app(&CancellationToken::new()).err(),
        model(&CancellationToken::new()).err(),
        runtime(&CancellationToken::new()).err(),
    )
}

impl fmt::Debug for ManagedJudgeSchedulePostReleaseFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeSchedulePostReleaseFailures")
            .field("eval_failed", &self.eval.is_some())
            .field("app_failed", &self.app.is_some())
            .field("model_failed", &self.model.is_some())
            .field("runtime_failed", &self.runtime.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use rewrite_types::CancellationToken;
    use std::cell::RefCell;

    use super::{ReleaseDisposition, classify_release, run_fresh_finalizers};

    #[test]
    fn every_post_release_failure_combination_runs_all_finalizers_in_order() {
        for mask in 0_u8..16 {
            let calls = RefCell::new(Vec::new());
            let result = run_fresh_finalizers(
                |token| step(&calls, "eval", mask & 1 != 0, token),
                |token| step(&calls, "app", mask & 2 != 0, token),
                |token| step(&calls, "model", mask & 4 != 0, token),
                |token| step(&calls, "runtime", mask & 8 != 0, token),
            );

            assert_eq!(*calls.borrow(), ["eval", "app", "model", "runtime"]);
            assert_eq!(result.0.is_some(), mask & 1 != 0);
            assert_eq!(result.1.is_some(), mask & 2 != 0);
            assert_eq!(result.2.is_some(), mask & 4 != 0);
            assert_eq!(result.3.is_some(), mask & 8 != 0);
        }
    }

    fn step(
        calls: &RefCell<Vec<&'static str>>,
        name: &'static str,
        fail: bool,
        token: &CancellationToken,
    ) -> Result<(), &'static str> {
        assert!(!token.is_cancelled());
        token.cancel();
        calls.borrow_mut().push(name);
        if fail { Err(name) } else { Ok(()) }
    }

    #[test]
    fn cancellation_and_deadline_flags_form_a_closed_cross_product() {
        for cancelled in [false, true] {
            for deadline_exceeded in [false, true] {
                for release_failed in [false, true] {
                    let release = if release_failed {
                        Err("release")
                    } else {
                        Ok("released")
                    };
                    match classify_release(release, cancelled, deadline_exceeded) {
                        ReleaseDisposition::Failed {
                            source,
                            cancelled: actual_cancelled,
                            deadline_exceeded: actual_deadline,
                        } => {
                            assert!(release_failed);
                            assert_eq!(source, "release");
                            assert_eq!(actual_cancelled, cancelled);
                            assert_eq!(actual_deadline, deadline_exceeded);
                        }
                        ReleaseDisposition::Invalidated {
                            released,
                            cancelled: actual_cancelled,
                            deadline_exceeded: actual_deadline,
                        } => {
                            assert!(!release_failed);
                            assert!(cancelled || deadline_exceeded);
                            assert_eq!(released, "released");
                            assert_eq!(actual_cancelled, cancelled);
                            assert_eq!(actual_deadline, deadline_exceeded);
                        }
                        ReleaseDisposition::Released(released) => {
                            assert!(!release_failed);
                            assert!(!cancelled);
                            assert!(!deadline_exceeded);
                            assert_eq!(released, "released");
                        }
                    }
                }
            }
        }
    }
}
