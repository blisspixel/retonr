use rewrite_app::{
    ManagedOllamaIsolationLease, RuntimePackageLease, VerifiedManagedOllamaModelPackageLease,
};
use std::time::Instant;

use rewrite_types::CancellationToken;

use super::deadline::{OperationGateFailure, ensure_operation_active};
use super::error::{
    ManagedJudgeScheduleRunnerError, ManagedJudgeScheduleRunnerFinalizationFailures,
    ManagedJudgeScheduleRunnerPrimaryFailure,
};

pub(super) fn finalize_failed_live_schedule(
    primary: ManagedJudgeScheduleRunnerPrimaryFailure,
    managed_ollama: ManagedOllamaIsolationLease<'_>,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    runtime_package: &mut RuntimePackageLease,
    cancellation: &CancellationToken,
    operation_deadline: Instant,
) -> ManagedJudgeScheduleRunnerError {
    let (cleanup, model, runtime) = run_finalizers(
        || managed_ollama.close(&CancellationToken::new()),
        || model_package.revalidate(&CancellationToken::new()),
        || runtime_package.revalidate(&CancellationToken::new()),
    );
    let primary = terminal_primary(
        primary,
        ensure_operation_active(cancellation, operation_deadline),
    );
    ManagedJudgeScheduleRunnerError::PostLaunch {
        primary: Box::new(primary),
        finalization: ManagedJudgeScheduleRunnerFinalizationFailures {
            cleanup: cleanup.map(Box::new),
            model: model.map(Box::new),
            runtime: runtime.map(Box::new),
        },
    }
}

fn terminal_primary(
    primary: ManagedJudgeScheduleRunnerPrimaryFailure,
    terminal: Result<(), OperationGateFailure>,
) -> ManagedJudgeScheduleRunnerPrimaryFailure {
    match terminal {
        Err(OperationGateFailure::DeadlineExceeded) => {
            ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded
        }
        Err(OperationGateFailure::Cancelled) => ManagedJudgeScheduleRunnerPrimaryFailure::Cancelled,
        Ok(()) => primary,
    }
}

fn run_finalizers<C, M, R>(
    cleanup: impl FnOnce() -> Result<(), C>,
    model: impl FnOnce() -> Result<(), M>,
    runtime: impl FnOnce() -> Result<(), R>,
) -> (Option<C>, Option<M>, Option<R>) {
    (cleanup().err(), model().err(), runtime().err())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use std::time::Instant;

    use rewrite_types::CancellationToken;

    use super::{run_finalizers, terminal_primary};
    use crate::local_ollama_managed_preflight::generation::managed_schedule_runner::{
        OperationGateFailure, error::ManagedJudgeScheduleRunnerPrimaryFailure,
    };

    #[test]
    fn all_eight_failure_combinations_run_every_finalizer_in_order() {
        for mask in 0_u8..8 {
            let order = Rc::new(RefCell::new(Vec::new()));
            let cleanup_order = Rc::clone(&order);
            let model_order = Rc::clone(&order);
            let runtime_order = Rc::clone(&order);
            let failures = run_finalizers(
                || {
                    cleanup_order.borrow_mut().push("cleanup");
                    if mask & 1 == 0 {
                        Ok(())
                    } else {
                        Err("cleanup")
                    }
                },
                || {
                    model_order.borrow_mut().push("model");
                    if mask & 2 == 0 { Ok(()) } else { Err("model") }
                },
                || {
                    runtime_order.borrow_mut().push("runtime");
                    if mask & 4 == 0 {
                        Ok(())
                    } else {
                        Err("runtime")
                    }
                },
            );
            assert_eq!(&*order.borrow(), &["cleanup", "model", "runtime"]);
            assert_eq!(failures.0.is_some(), mask & 1 != 0);
            assert_eq!(failures.1.is_some(), mask & 2 != 0);
            assert_eq!(failures.2.is_some(), mask & 4 != 0);
        }
    }

    #[test]
    fn terminal_precedence_is_resampled_only_after_fresh_finalizers() {
        let cancellation = CancellationToken::new();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let cleanup = Rc::clone(&calls);
        let model = Rc::clone(&calls);
        let runtime = Rc::clone(&calls);
        let _ = run_finalizers(
            || {
                cleanup.borrow_mut().push("cleanup");
                cancellation.cancel();
                Ok::<(), ()>(())
            },
            || {
                model.borrow_mut().push("model");
                Ok::<(), ()>(())
            },
            || {
                runtime.borrow_mut().push("runtime");
                Ok::<(), ()>(())
            },
        );
        assert_eq!(&*calls.borrow(), &["cleanup", "model", "runtime"]);
        assert!(matches!(
            terminal_primary(
                ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority,
                Err(OperationGateFailure::DeadlineExceeded),
            ),
            ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded
        ));
        assert!(matches!(
            terminal_primary(
                ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority,
                Err(OperationGateFailure::Cancelled),
            ),
            ManagedJudgeScheduleRunnerPrimaryFailure::Cancelled
        ));
        assert!(matches!(
            terminal_primary(
                ManagedJudgeScheduleRunnerPrimaryFailure::SequenceAuthority,
                super::ensure_operation_active(&cancellation, Instant::now()),
            ),
            ManagedJudgeScheduleRunnerPrimaryFailure::DeadlineExceeded
        ));
    }
}
