use std::time::{Duration, Instant};

use rewrite_types::CancellationToken;

use crate::{
    IsolationError, IsolationResult,
    deadline::{DeadlineBudget, DeadlineSource},
};

use super::super::linux_control::{self, map_error as map_control_error};

#[derive(Clone, Debug)]
pub(super) struct StageDeadline {
    budget: DeadlineBudget,
    pub(super) timeout_error: IsolationError,
}

impl StageDeadline {
    pub(super) fn new(
        operation_deadline: Option<Instant>,
        local_ceiling: Duration,
    ) -> IsolationResult<Self> {
        let budget = DeadlineBudget::from_now(operation_deadline, local_ceiling)?;
        let timeout_error = match budget.source() {
            DeadlineSource::Operation => IsolationError::OperationDeadlineExceeded,
            DeadlineSource::Local => IsolationError::StartupTimeout,
        };
        Ok(Self {
            budget,
            timeout_error,
        })
    }

    pub(super) const fn at(&self) -> Instant {
        self.budget.deadline()
    }

    pub(super) fn map_control_error(&self, error: linux_control::ControlError) -> IsolationError {
        if self.operation_expired() {
            IsolationError::OperationDeadlineExceeded
        } else if error == linux_control::ControlError::Deadline {
            self.timeout_error.clone()
        } else {
            map_control_error(error)
        }
    }

    pub(super) fn map_isolation_error(&self, error: IsolationError) -> IsolationError {
        self.map_isolation_error_at(error, Instant::now())
    }

    fn map_isolation_error_at(&self, error: IsolationError, now: Instant) -> IsolationError {
        if self.operation_expired_at(now) {
            IsolationError::OperationDeadlineExceeded
        } else {
            error
        }
    }

    pub(super) fn ensure_not_expired(&self) -> IsolationResult<()> {
        if Instant::now() >= self.at() {
            Err(self.timeout_error.clone())
        } else {
            Ok(())
        }
    }

    fn operation_expired(&self) -> bool {
        self.operation_expired_at(Instant::now())
    }

    fn operation_expired_at(&self, now: Instant) -> bool {
        self.budget.source() == DeadlineSource::Operation && now >= self.at()
    }
}

pub(super) fn operation_result_precedence<T>(
    result: IsolationResult<T>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> IsolationResult<T> {
    operation_result_precedence_at(
        result,
        cancellation.is_cancelled(),
        operation_deadline,
        Instant::now(),
    )
}

pub(super) fn operation_error_precedence(
    error: IsolationError,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> IsolationError {
    if operation_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        IsolationError::OperationDeadlineExceeded
    } else if operation_deadline.is_some() && cancellation.is_cancelled() {
        IsolationError::Cancelled
    } else {
        error
    }
}

fn operation_result_precedence_at<T>(
    result: IsolationResult<T>,
    cancelled: bool,
    operation_deadline: Option<Instant>,
    now: Instant,
) -> IsolationResult<T> {
    if operation_deadline.is_some_and(|deadline| now >= deadline) {
        Err(IsolationError::OperationDeadlineExceeded)
    } else if operation_deadline.is_some() && cancelled {
        Err(IsolationError::Cancelled)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{StageDeadline, operation_result_precedence_at};
    use crate::IsolationError;

    #[test]
    fn operation_deadline_overrides_simultaneous_cancellation() {
        let deadline = StageDeadline::new(
            Some(Instant::now() + Duration::from_millis(1)),
            Duration::from_secs(1),
        )
        .expect("operation deadline");
        assert_eq!(
            deadline.map_isolation_error_at(IsolationError::Cancelled, deadline.at()),
            IsolationError::OperationDeadlineExceeded
        );
    }

    #[test]
    fn operation_boundary_precedence_is_deadline_then_cancellation_then_error() {
        let now = Instant::now();
        assert_eq!(
            operation_result_precedence_at::<()>(
                Err(IsolationError::EvidenceChanged),
                true,
                Some(now),
                now,
            ),
            Err(IsolationError::OperationDeadlineExceeded)
        );
        assert_eq!(
            operation_result_precedence_at::<()>(
                Err(IsolationError::EvidenceChanged),
                true,
                Some(now + Duration::from_secs(1)),
                now,
            ),
            Err(IsolationError::Cancelled)
        );
    }
}
