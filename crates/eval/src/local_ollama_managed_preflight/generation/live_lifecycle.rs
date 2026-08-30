use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

const NO_LIVE_AUTHORITIES: u8 = 0;
const ONE_LIVE_AUTHORITY: u8 = 1;

/// Process-local single-slot lifecycle for one qualification operation.
///
/// This is deliberately an in-memory, non-cloneable authority. It neither
/// serializes nor accepts caller-provided lifecycle observations.
pub(crate) struct GenerationQualificationLiveLifecycle {
    reservation_active: AtomicBool,
    ever_acquired: AtomicBool,
    peak_live_authorities: AtomicU8,
}

impl GenerationQualificationLiveLifecycle {
    pub(crate) const fn new() -> Self {
        Self {
            reservation_active: AtomicBool::new(false),
            ever_acquired: AtomicBool::new(false),
            peak_live_authorities: AtomicU8::new(NO_LIVE_AUTHORITIES),
        }
    }

    pub(super) fn reserve(
        &self,
    ) -> Result<GenerationQualificationLiveReservation<'_>, LiveReservationError> {
        self.reservation_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_active| LiveReservationError::AlreadyReserved)?;
        Ok(GenerationQualificationLiveReservation {
            lifecycle: self,
            authority_acquired: false,
        })
    }

    pub(crate) fn ever_acquired(&self) -> bool {
        self.ever_acquired.load(Ordering::Acquire)
    }

    pub(crate) fn peak_live_authorities(&self) -> u8 {
        self.peak_live_authorities.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn mark_authority_acquired_for_test(&self) {
        self.ever_acquired.store(true, Ordering::Release);
        self.peak_live_authorities
            .store(ONE_LIVE_AUTHORITY, Ordering::Release);
    }

    #[cfg(test)]
    fn reservation_active(&self) -> bool {
        self.reservation_active.load(Ordering::Acquire)
    }
}

impl Default for GenerationQualificationLiveLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiveReservationError {
    AlreadyReserved,
}

pub(super) struct GenerationQualificationLiveReservation<'a> {
    lifecycle: &'a GenerationQualificationLiveLifecycle,
    authority_acquired: bool,
}

impl GenerationQualificationLiveReservation<'_> {
    pub(super) fn mark_authority_acquired(&mut self) {
        assert!(
            !self.authority_acquired,
            "a live reservation can acquire authority only once"
        );
        self.authority_acquired = true;
        self.lifecycle.ever_acquired.store(true, Ordering::Release);
        self.lifecycle
            .peak_live_authorities
            .store(ONE_LIVE_AUTHORITY, Ordering::Release);
    }
}

impl Drop for GenerationQualificationLiveReservation<'_> {
    fn drop(&mut self) {
        self.lifecycle
            .reservation_active
            .store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::{GenerationQualificationLiveLifecycle, LiveReservationError};

    fn assert_send_and_sync<T: Send + Sync>() {}

    fn simulated_lower_run(
        lifecycle: &GenerationQualificationLiveLifecycle,
        launch_succeeds: bool,
        fail_after_launch: bool,
    ) -> Result<(), &'static str> {
        let mut reservation = lifecycle.reserve().map_err(|_error| "overlap")?;
        if !launch_succeeds {
            return Err("prelaunch");
        }
        reservation.mark_authority_acquired();
        if fail_after_launch {
            return Err("postlaunch");
        }
        Ok(())
    }

    #[test]
    fn dropped_unacquired_reservation_leaves_peak_zero() {
        assert_send_and_sync::<GenerationQualificationLiveLifecycle>();
        let lifecycle = GenerationQualificationLiveLifecycle::new();
        {
            let _reservation = lifecycle.reserve().expect("exclusive reservation");
            assert!(lifecycle.reservation_active());
            assert!(!lifecycle.ever_acquired());
            assert_eq!(lifecycle.peak_live_authorities(), 0);
        }
        assert!(!lifecycle.reservation_active());
        assert!(!lifecycle.ever_acquired());
        assert_eq!(lifecycle.peak_live_authorities(), 0);
    }

    #[test]
    fn every_postlaunch_return_leaves_sticky_peak_one_and_releases_slot() {
        for fail_after_launch in [false, true] {
            let lifecycle = GenerationQualificationLiveLifecycle::new();
            let result = simulated_lower_run(&lifecycle, true, fail_after_launch);
            assert_eq!(result.is_err(), fail_after_launch);
            assert!(!lifecycle.reservation_active());
            assert!(lifecycle.ever_acquired());
            assert_eq!(lifecycle.peak_live_authorities(), 1);
            assert!(lifecycle.reserve().is_ok());
        }
    }

    #[test]
    fn prelaunch_failure_releases_slot_without_raising_peak() {
        let lifecycle = GenerationQualificationLiveLifecycle::new();
        assert_eq!(
            simulated_lower_run(&lifecycle, false, false),
            Err("prelaunch")
        );
        assert!(!lifecycle.reservation_active());
        assert!(!lifecycle.ever_acquired());
        assert_eq!(lifecycle.peak_live_authorities(), 0);
        assert!(lifecycle.reserve().is_ok());
    }

    #[test]
    fn overlap_is_refused_while_unacquired_or_live() {
        let lifecycle = GenerationQualificationLiveLifecycle::new();
        let mut reservation = lifecycle.reserve().expect("first reservation");
        assert!(matches!(
            lifecycle.reserve(),
            Err(LiveReservationError::AlreadyReserved)
        ));
        assert_eq!(lifecycle.peak_live_authorities(), 0);

        reservation.mark_authority_acquired();
        assert!(matches!(
            lifecycle.reserve(),
            Err(LiveReservationError::AlreadyReserved)
        ));
        assert_eq!(lifecycle.peak_live_authorities(), 1);
        drop(reservation);
        assert!(lifecycle.reserve().is_ok());
    }

    #[test]
    fn ever_acquired_stays_true_across_sequential_reservations() {
        let lifecycle = GenerationQualificationLiveLifecycle::new();
        simulated_lower_run(&lifecycle, true, false).expect("first live run");
        assert!(lifecycle.ever_acquired());
        assert_eq!(lifecycle.peak_live_authorities(), 1);

        assert_eq!(
            simulated_lower_run(&lifecycle, false, false),
            Err("prelaunch")
        );
        assert!(lifecycle.ever_acquired());
        assert_eq!(lifecycle.peak_live_authorities(), 1);
        simulated_lower_run(&lifecycle, true, false).expect("second live run");
        assert_eq!(lifecycle.peak_live_authorities(), 1);
    }

    #[test]
    fn concurrent_reservations_admit_exactly_one_slot_owner() {
        let lifecycle = GenerationQualificationLiveLifecycle::new();
        let ready = Arc::new(Barrier::new(3));
        let attempted = Arc::new(Barrier::new(2));
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let ready = Arc::clone(&ready);
                    let attempted = Arc::clone(&attempted);
                    let lifecycle = &lifecycle;
                    scope.spawn(move || {
                        ready.wait();
                        let reservation = lifecycle.reserve();
                        attempted.wait();
                        reservation.is_ok()
                    })
                })
                .collect();
            ready.wait();
            let admitted = handles
                .into_iter()
                .map(|handle| usize::from(handle.join().expect("reservation worker")))
                .sum::<usize>();
            assert_eq!(admitted, 1);
        });
        assert!(!lifecycle.reservation_active());
        assert_eq!(lifecycle.peak_live_authorities(), 0);
    }

    #[test]
    fn unwinding_after_launch_releases_the_slot_and_retains_peak() {
        let lifecycle = GenerationQualificationLiveLifecycle::default();
        let unwind = std::panic::catch_unwind(|| {
            let mut reservation = lifecycle.reserve().expect("exclusive reservation");
            reservation.mark_authority_acquired();
            panic!("postlaunch fixture");
        });
        assert!(unwind.is_err());
        assert!(!lifecycle.reservation_active());
        assert!(lifecycle.ever_acquired());
        assert_eq!(lifecycle.peak_live_authorities(), 1);
        assert!(lifecycle.reserve().is_ok());
    }
}
