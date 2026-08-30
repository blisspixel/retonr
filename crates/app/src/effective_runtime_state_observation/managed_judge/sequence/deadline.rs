use std::time::Instant;

use rewrite_types::CancellationToken;

use super::super::error::ManagedJudgeObservationError;

pub(super) fn earliest_deadline(
    retained: Option<Instant>,
    supplied: Option<Instant>,
) -> Option<Instant> {
    match (retained, supplied) {
        (Some(retained), Some(supplied)) => Some(retained.min(supplied)),
        (Some(retained), None) => Some(retained),
        (None, supplied) => supplied,
    }
}

pub(super) fn ensure_active_until(
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<(), ManagedJudgeObservationError> {
    deadline_precedence(Ok(()), cancellation, operation_deadline)
}

pub(super) fn deadline_precedence<T>(
    result: Result<T, ManagedJudgeObservationError>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<T, ManagedJudgeObservationError> {
    deadline_precedence_at(result, Instant::now(), cancellation, operation_deadline)
}

fn deadline_precedence_at<T>(
    result: Result<T, ManagedJudgeObservationError>,
    observed: Instant,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<T, ManagedJudgeObservationError> {
    if operation_deadline.is_some_and(|deadline| observed >= deadline) {
        Err(ManagedJudgeObservationError::DeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Err(ManagedJudgeObservationError::Cancelled)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rewrite_types::CancellationToken;

    use super::{deadline_precedence_at, earliest_deadline};
    use crate::effective_runtime_state_observation::ManagedJudgeObservationError;

    #[test]
    fn equality_and_simultaneous_failures_preserve_closed_precedence() {
        let now = Instant::now();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            deadline_precedence_at::<()>(
                Err(ManagedJudgeObservationError::InvalidCount),
                now,
                &cancellation,
                Some(now),
            ),
            Err(ManagedJudgeObservationError::DeadlineExceeded)
        ));
        assert!(matches!(
            deadline_precedence_at::<()>(
                Err(ManagedJudgeObservationError::InvalidCount),
                now,
                &cancellation,
                Some(now + Duration::from_secs(1)),
            ),
            Err(ManagedJudgeObservationError::Cancelled)
        ));
        assert!(matches!(
            deadline_precedence_at::<()>(
                Err(ManagedJudgeObservationError::InvalidCount),
                now,
                &CancellationToken::new(),
                Some(now + Duration::from_secs(1)),
            ),
            Err(ManagedJudgeObservationError::InvalidCount)
        ));
    }

    #[test]
    fn later_deadline_cannot_extend_the_retained_original() {
        let now = Instant::now();
        let retained = now + Duration::from_secs(1);
        let earlier = now + Duration::from_millis(500);
        let later = now + Duration::from_secs(10);
        assert_eq!(
            earliest_deadline(Some(retained), Some(later)),
            Some(retained)
        );
        assert_eq!(
            earliest_deadline(Some(retained), Some(earlier)),
            Some(earlier)
        );
        assert_eq!(earliest_deadline(Some(retained), None), Some(retained));
        assert_eq!(earliest_deadline(None, Some(later)), Some(later));
    }
}
