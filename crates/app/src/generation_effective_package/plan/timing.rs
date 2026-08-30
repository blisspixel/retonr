use std::time::{Duration, Instant};

use super::GenerationEffectivePackageCleanupTimingError;

type TimedReleaseResult<C, M, R, E> = (
    Option<C>,
    Result<Duration, GenerationEffectivePackageCleanupTimingError>,
    Instant,
    Option<M>,
    Option<R>,
    Option<E>,
);

pub(super) fn run_timed_release_and_validation_steps<C, M, R, E>(
    cleanup: impl FnOnce() -> Result<(), C>,
    model: impl FnOnce() -> Result<(), M>,
    runtime: impl FnOnce() -> Result<(), R>,
    validate: impl FnOnce() -> Result<(), E>,
    mut now: impl FnMut() -> Instant,
) -> TimedReleaseResult<C, M, R, E> {
    let cleanup_started = now();
    let cleanup = cleanup().err();
    let cleanup_completed_at = now();
    let cleanup_elapsed = cleanup_completed_at
        .checked_duration_since(cleanup_started)
        .ok_or(GenerationEffectivePackageCleanupTimingError::ReversedMonotonicCheckpoints);
    let model = model().err();
    let runtime = runtime().err();
    let validation = if cleanup.is_none() && model.is_none() && runtime.is_none() {
        validate().err()
    } else {
        None
    };
    (
        cleanup,
        cleanup_elapsed,
        cleanup_completed_at,
        model,
        runtime,
        validation,
    )
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn cleanup_timing_excludes_every_later_validation_step() {
        let base = Instant::now();
        let checkpoints = [base, base + Duration::from_nanos(17)];
        let mut checkpoint = checkpoints.into_iter();
        let order = Rc::new(RefCell::new(Vec::new()));
        let event = |name| {
            let order = Rc::clone(&order);
            move || {
                order.borrow_mut().push(name);
                Ok::<_, ()>(())
            }
        };
        let result = run_timed_release_and_validation_steps(
            event("cleanup"),
            event("model"),
            event("runtime"),
            event("evidence"),
            || checkpoint.next().expect("exact timing checkpoint"),
        );
        assert_eq!(result.1, Ok(Duration::from_nanos(17)));
        assert_eq!(result.2, checkpoints[1]);
        assert_eq!(
            order.borrow().as_slice(),
            ["cleanup", "model", "runtime", "evidence"]
        );
        assert!(checkpoint.next().is_none());
    }

    #[test]
    fn timed_release_preserves_independent_failures_and_skips_only_evidence_validation() {
        let base = Instant::now();
        let mut checkpoint = [base, base + Duration::from_nanos(3)].into_iter();
        let result = run_timed_release_and_validation_steps(
            || Err::<(), _>("cleanup"),
            || Err::<(), _>("model"),
            || Err::<(), _>("runtime"),
            || Err::<(), _>("evidence"),
            || checkpoint.next().expect("exact timing checkpoint"),
        );
        assert_eq!(result.0, Some("cleanup"));
        assert_eq!(result.1, Ok(Duration::from_nanos(3)));
        assert_eq!(result.3, Some("model"));
        assert_eq!(result.4, Some("runtime"));
        assert_eq!(result.5, None);
    }

    #[test]
    fn cleanup_timing_accepts_equal_checkpoints() {
        let base = Instant::now();
        let mut checkpoint = [base, base].into_iter();
        let result = run_timed_release_and_validation_steps(
            || Ok::<_, ()>(()),
            || Ok::<_, ()>(()),
            || Ok::<_, ()>(()),
            || Ok::<_, ()>(()),
            || checkpoint.next().expect("exact timing checkpoint"),
        );
        assert_eq!(result.1, Ok(Duration::ZERO));
        assert!(result.5.is_none());
    }

    #[test]
    fn cleanup_timing_rejects_reversed_checkpoints_and_preserves_later_steps() {
        let base = Instant::now();
        let reversed = base
            .checked_sub(Duration::from_secs(1))
            .expect("test checkpoint remains representable");
        let mut checkpoint = [base, reversed].into_iter();
        let order = Rc::new(RefCell::new(Vec::new()));
        let event = |name| {
            let order = Rc::clone(&order);
            move || {
                order.borrow_mut().push(name);
                Ok::<_, ()>(())
            }
        };
        let result = run_timed_release_and_validation_steps(
            event("cleanup"),
            event("model"),
            event("runtime"),
            event("evidence"),
            || checkpoint.next().expect("exact timing checkpoint"),
        );
        assert_eq!(
            result.1,
            Err(GenerationEffectivePackageCleanupTimingError::ReversedMonotonicCheckpoints)
        );
        assert_eq!(
            order.borrow().as_slice(),
            ["cleanup", "model", "runtime", "evidence"]
        );
        assert!(result.5.is_none());
    }
}
