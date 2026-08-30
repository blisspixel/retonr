use std::time::{Duration, Instant};

use crate::{IsolationError, IsolationResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeadlineSource {
    Operation,
    Local,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeadlineBudget {
    at: Instant,
    source: DeadlineSource,
}

impl DeadlineBudget {
    #[cfg(target_os = "linux")]
    pub(crate) fn from_now(
        operation_deadline: Option<Instant>,
        local_ceiling: Duration,
    ) -> IsolationResult<Self> {
        Self::at(Instant::now(), operation_deadline, local_ceiling)
    }

    fn at(
        now: Instant,
        operation_deadline: Option<Instant>,
        local_ceiling: Duration,
    ) -> IsolationResult<Self> {
        if operation_deadline.is_some_and(|deadline| now >= deadline) {
            return Err(IsolationError::OperationDeadlineExceeded);
        }
        let local_deadline = now
            .checked_add(local_ceiling)
            .ok_or(IsolationError::StartupTimeout)?;
        match operation_deadline {
            Some(operation_deadline) if operation_deadline <= local_deadline => Ok(Self {
                at: operation_deadline,
                source: DeadlineSource::Operation,
            }),
            _ => Ok(Self {
                at: local_deadline,
                source: DeadlineSource::Local,
            }),
        }
    }

    pub(crate) const fn deadline(self) -> Instant {
        self.at
    }

    pub(crate) const fn source(self) -> DeadlineSource {
        self.source
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{DeadlineBudget, DeadlineSource};
    use crate::IsolationError;

    #[test]
    fn operation_deadline_is_admitted_only_strictly_before_it() {
        let now = Instant::now();
        let below = now + Duration::from_nanos(1);
        assert_eq!(
            DeadlineBudget::at(now, Some(below), Duration::from_secs(1))
                .expect("strictly future deadline")
                .deadline(),
            below
        );
        assert_eq!(
            DeadlineBudget::at(now, Some(now), Duration::from_secs(1)),
            Err(IsolationError::OperationDeadlineExceeded)
        );
        assert_eq!(
            DeadlineBudget::at(
                now,
                Some(
                    now.checked_sub(Duration::from_nanos(1))
                        .expect("test instant supports one-nanosecond subtraction"),
                ),
                Duration::from_secs(1),
            ),
            Err(IsolationError::OperationDeadlineExceeded)
        );
    }

    #[test]
    fn original_operation_deadline_is_not_reset_between_stages() {
        let captured = Instant::now() + Duration::from_secs(10);
        let first_now = captured
            .checked_sub(Duration::from_secs(9))
            .expect("test instant supports subtraction");
        let later_now = captured
            .checked_sub(Duration::from_secs(4))
            .expect("test instant supports subtraction");
        let first = DeadlineBudget::at(first_now, Some(captured), Duration::from_secs(30))
            .expect("first stage");
        let later = DeadlineBudget::at(later_now, Some(captured), Duration::from_secs(30))
            .expect("later stage");
        assert_eq!(first.deadline(), captured);
        assert_eq!(later.deadline(), captured);
        assert_eq!(first.source(), DeadlineSource::Operation);
        assert_eq!(later.source(), DeadlineSource::Operation);
    }

    #[test]
    fn local_ceiling_remains_earlier_when_it_is_tighter() {
        let now = Instant::now();
        let budget = DeadlineBudget::at(
            now,
            Some(now + Duration::from_secs(10)),
            Duration::from_secs(2),
        )
        .expect("local bound");
        assert_eq!(budget.deadline(), now + Duration::from_secs(2));
        assert_eq!(budget.source(), DeadlineSource::Local);
    }
}
