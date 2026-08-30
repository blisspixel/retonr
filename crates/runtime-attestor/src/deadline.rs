use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObservationBudget {
    started: Instant,
    maximum_elapsed: Duration,
}

impl ObservationBudget {
    pub(crate) fn fresh(
        maximum_elapsed: Duration,
        operation_deadline: Option<Instant>,
    ) -> Result<Self, ()> {
        let now = Instant::now();
        Self::at(now, now, maximum_elapsed, operation_deadline)
    }

    pub(crate) fn retained(
        started: Instant,
        maximum_elapsed: Duration,
        operation_deadline: Option<Instant>,
    ) -> Result<Self, ()> {
        Self::at(Instant::now(), started, maximum_elapsed, operation_deadline)
    }

    fn at(
        now: Instant,
        local_started: Instant,
        maximum_elapsed: Duration,
        operation_deadline: Option<Instant>,
    ) -> Result<Self, ()> {
        let local_deadline = local_started.checked_add(maximum_elapsed).ok_or(())?;
        let deadline =
            operation_deadline.map_or(local_deadline, |operation| operation.min(local_deadline));
        if now >= deadline {
            return Err(());
        }
        Ok(Self {
            started: now,
            maximum_elapsed: deadline.duration_since(now),
        })
    }

    pub(crate) const fn started(self) -> Instant {
        self.started
    }

    pub(crate) const fn maximum_elapsed(self) -> Duration {
        self.maximum_elapsed
    }
}

pub(crate) fn earliest_deadline(
    retained: Option<Instant>,
    supplied: Option<Instant>,
) -> Option<Instant> {
    match (retained, supplied) {
        (Some(retained), Some(supplied)) => Some(retained.min(supplied)),
        (Some(retained), None) => Some(retained),
        (None, supplied) => supplied,
    }
}

pub(crate) fn operation_expired(operation_deadline: Option<Instant>) -> bool {
    operation_deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{ObservationBudget, earliest_deadline};

    #[test]
    fn absolute_deadline_is_open_only_strictly_before_it() {
        let now = Instant::now();
        let below = now + Duration::from_nanos(1);
        let budget = ObservationBudget::at(now, now, Duration::from_secs(1), Some(below))
            .expect("strictly below");
        assert_eq!(budget.started(), now);
        assert_eq!(budget.maximum_elapsed(), Duration::from_nanos(1));
        assert!(ObservationBudget::at(now, now, Duration::from_secs(1), Some(now)).is_err());
        assert!(
            ObservationBudget::at(
                now,
                now,
                Duration::from_secs(1),
                Some(
                    now.checked_sub(Duration::from_nanos(1))
                        .expect("test instant supports one-nanosecond subtraction"),
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn retained_original_deadline_does_not_reset() {
        let local_started = Instant::now();
        let captured = local_started + Duration::from_secs(10);
        let first_now = local_started + Duration::from_secs(1);
        let later_now = local_started + Duration::from_secs(6);
        let first = ObservationBudget::at(
            first_now,
            local_started,
            Duration::from_secs(30),
            Some(captured),
        )
        .expect("first stage");
        let later = ObservationBudget::at(
            later_now,
            local_started,
            Duration::from_secs(30),
            Some(captured),
        )
        .expect("later stage");
        assert_eq!(first.started() + first.maximum_elapsed(), captured);
        assert_eq!(later.started() + later.maximum_elapsed(), captured);
    }

    #[test]
    fn retained_local_deadline_remains_tighter() {
        let local_started = Instant::now();
        let now = local_started + Duration::from_secs(2);
        let budget = ObservationBudget::at(
            now,
            local_started,
            Duration::from_secs(5),
            Some(local_started + Duration::from_secs(20)),
        )
        .expect("local deadline");
        assert_eq!(
            budget.started() + budget.maximum_elapsed(),
            local_started + Duration::from_secs(5)
        );
    }

    #[test]
    fn earliest_original_deadline_cannot_be_replaced_by_a_later_one() {
        let now = Instant::now();
        let original = now + Duration::from_secs(2);
        let later = now + Duration::from_secs(20);
        assert_eq!(
            earliest_deadline(Some(original), Some(later)),
            Some(original)
        );
        assert_eq!(earliest_deadline(Some(original), None), Some(original));
    }
}
