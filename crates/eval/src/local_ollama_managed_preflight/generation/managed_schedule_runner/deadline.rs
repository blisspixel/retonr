use std::time::{Duration, Instant};

use rewrite_runtime_attestor::{ManagedGenerationWorkerLimits, NativeLoadObservationLimits};
use rewrite_types::CancellationToken;

use crate::LocalOllamaManagedPreflightLimits;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_ollama_managed_preflight::generation) struct JoinedRunDeadlineExceeded;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::local_ollama_managed_preflight::generation) enum OperationGateFailure {
    Cancelled,
    DeadlineExceeded,
}

pub(in crate::local_ollama_managed_preflight::generation) fn ensure_operation_active(
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<(), OperationGateFailure> {
    operation_failure_at(cancellation, deadline, Instant::now()).map_or(Ok(()), Err)
}

pub(in crate::local_ollama_managed_preflight::generation) fn operation_precedence<T, E>(
    result: Result<T, E>,
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<T, OperationPrecedenceError<E>> {
    operation_precedence_at(result, cancellation, deadline, Instant::now())
}

fn operation_precedence_at<T, E>(
    result: Result<T, E>,
    cancellation: &CancellationToken,
    deadline: Instant,
    observed: Instant,
) -> Result<T, OperationPrecedenceError<E>> {
    match operation_failure_at(cancellation, deadline, observed) {
        Some(OperationGateFailure::DeadlineExceeded) => {
            Err(OperationPrecedenceError::DeadlineExceeded)
        }
        Some(OperationGateFailure::Cancelled) => Err(OperationPrecedenceError::Cancelled),
        None => result.map_err(OperationPrecedenceError::Underlying),
    }
}

fn operation_failure_at(
    cancellation: &CancellationToken,
    deadline: Instant,
    observed: Instant,
) -> Option<OperationGateFailure> {
    if observed >= deadline {
        Some(OperationGateFailure::DeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Some(OperationGateFailure::Cancelled)
    } else {
        None
    }
}

pub(in crate::local_ollama_managed_preflight::generation) enum OperationPrecedenceError<E> {
    DeadlineExceeded,
    Cancelled,
    Underlying(E),
}

pub(super) fn ensure_before_deadline(deadline: Instant) -> Result<(), JoinedRunDeadlineExceeded> {
    ensure_before_deadline_at(deadline, Instant::now())
}

pub(super) fn remaining_before_deadline(
    deadline: Instant,
) -> Result<Duration, JoinedRunDeadlineExceeded> {
    remaining_before_deadline_at(deadline, Instant::now())
}

pub(super) fn cap_preflight_limits(
    mut limits: LocalOllamaManagedPreflightLimits,
    remaining: Duration,
) -> LocalOllamaManagedPreflightLimits {
    limits.process.maximum_elapsed = limits.process.maximum_elapsed.min(remaining);
    limits.native_load.maximum_elapsed = limits.native_load.maximum_elapsed.min(remaining);
    limits
}

pub(super) fn cap_native_load_limits(
    mut limits: NativeLoadObservationLimits,
    remaining: Duration,
) -> NativeLoadObservationLimits {
    limits.maximum_elapsed = limits.maximum_elapsed.min(remaining);
    limits
}

pub(super) fn cap_worker_limits(
    mut limits: ManagedGenerationWorkerLimits,
    remaining: Duration,
) -> ManagedGenerationWorkerLimits {
    limits.maximum_elapsed = limits.maximum_elapsed.min(remaining);
    limits.native_load.maximum_elapsed = limits.native_load.maximum_elapsed.min(remaining);
    limits
}

fn ensure_before_deadline_at(
    deadline: Instant,
    now: Instant,
) -> Result<(), JoinedRunDeadlineExceeded> {
    remaining_before_deadline_at(deadline, now).map(drop)
}

pub(in crate::local_ollama_managed_preflight::generation) fn remaining_before_deadline_at(
    deadline: Instant,
    now: Instant,
) -> Result<Duration, JoinedRunDeadlineExceeded> {
    deadline
        .checked_duration_since(now)
        .filter(|remaining| !remaining.is_zero())
        .ok_or(JoinedRunDeadlineExceeded)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rewrite_runtime_attestor::{ManagedGenerationWorkerLimits, NativeLoadObservationLimits};

    use super::{
        OperationGateFailure, cap_native_load_limits, cap_preflight_limits, cap_worker_limits,
        ensure_before_deadline_at, ensure_operation_active, operation_precedence_at,
        remaining_before_deadline, remaining_before_deadline_at,
    };
    use crate::LocalOllamaManagedPreflightLimits;
    use rewrite_types::CancellationToken;

    #[test]
    fn deadline_boundary_rejects_equality_and_expiry() {
        let now = Instant::now();
        assert_eq!(
            remaining_before_deadline_at(now + Duration::from_millis(7), now),
            Ok(Duration::from_millis(7))
        );
        assert!(ensure_before_deadline_at(now, now).is_err());
        assert!(ensure_before_deadline_at(now, now + Duration::from_nanos(1)).is_err());
    }

    #[test]
    fn operation_gate_preserves_deadline_then_cancellation_precedence() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            ensure_operation_active(&cancellation, Instant::now()),
            Err(OperationGateFailure::DeadlineExceeded)
        );
        assert_eq!(
            ensure_operation_active(&cancellation, Instant::now() + Duration::from_secs(1)),
            Err(OperationGateFailure::Cancelled)
        );

        let active = CancellationToken::new();
        let future = Instant::now() + Duration::from_secs(1);
        assert_eq!(ensure_operation_active(&active, future), Ok(()));
        assert!(remaining_before_deadline(future).is_ok());
        assert_eq!(
            ensure_operation_active(&active, Instant::now()),
            Err(OperationGateFailure::DeadlineExceeded)
        );
    }

    #[test]
    fn equality_and_simultaneous_underlying_failure_obey_terminal_precedence() {
        let observed = Instant::now();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            operation_precedence_at::<(), _>(Err("underlying"), &cancellation, observed, observed,),
            Err(super::OperationPrecedenceError::DeadlineExceeded)
        ));
        assert!(matches!(
            operation_precedence_at::<(), _>(
                Err("underlying"),
                &cancellation,
                observed + Duration::from_secs(1),
                observed,
            ),
            Err(super::OperationPrecedenceError::Cancelled)
        ));
        assert!(matches!(
            operation_precedence_at::<(), _>(
                Err("underlying"),
                &CancellationToken::new(),
                observed + Duration::from_secs(1),
                observed,
            ),
            Err(super::OperationPrecedenceError::Underlying("underlying"))
        ));
    }

    #[test]
    fn every_observer_elapsed_limit_is_capped_by_joined_remaining_time() {
        let remaining = Duration::from_millis(3);
        let preflight =
            cap_preflight_limits(LocalOllamaManagedPreflightLimits::default(), remaining);
        let native = cap_native_load_limits(NativeLoadObservationLimits::default(), remaining);
        let worker = cap_worker_limits(ManagedGenerationWorkerLimits::default(), remaining);

        assert_eq!(preflight.process.maximum_elapsed, remaining);
        assert_eq!(preflight.native_load.maximum_elapsed, remaining);
        assert_eq!(native.maximum_elapsed, remaining);
        assert_eq!(worker.maximum_elapsed, remaining);
        assert_eq!(worker.native_load.maximum_elapsed, remaining);
    }

    #[test]
    fn tighter_caller_limit_is_preserved() {
        let limits = ManagedGenerationWorkerLimits {
            maximum_elapsed: Duration::from_millis(2),
            native_load: NativeLoadObservationLimits {
                maximum_elapsed: Duration::from_millis(1),
                ..NativeLoadObservationLimits::default()
            },
            ..ManagedGenerationWorkerLimits::default()
        };
        let capped = cap_worker_limits(limits, Duration::from_millis(3));

        assert_eq!(capped.maximum_elapsed, Duration::from_millis(2));
        assert_eq!(capped.native_load.maximum_elapsed, Duration::from_millis(1));
    }

    #[test]
    fn multiple_attempts_recap_against_one_absolute_deadline() {
        let started = Instant::now();
        let deadline = started + Duration::from_millis(10);
        let original = ManagedGenerationWorkerLimits::default();
        let observed = [1_u64, 4, 9].map(|elapsed| {
            let remaining =
                remaining_before_deadline_at(deadline, started + Duration::from_millis(elapsed))
                    .expect("attempt begins before deadline");
            cap_worker_limits(original, remaining).maximum_elapsed
        });

        assert_eq!(
            observed,
            [
                Duration::from_millis(9),
                Duration::from_millis(6),
                Duration::from_millis(1),
            ]
        );
        assert!(
            remaining_before_deadline_at(deadline, started + Duration::from_millis(10)).is_err()
        );
    }
}
