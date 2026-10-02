use std::time::{Duration, Instant};

use super::super::{
    linux_control::ControlError, linux_helper_setup::HelperFailure,
    linux_helper_support::control_failure,
};

/// One preparation allowance, followed by a separately bounded child handshake.
pub(super) struct PreparationDeadline {
    preparation: Instant,
    ready: Instant,
    startup: Duration,
}

impl PreparationDeadline {
    pub(super) fn new_at(now: Instant, startup: Duration) -> Self {
        let preparation = now + crate::CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT;
        Self {
            preparation,
            ready: preparation + startup,
            startup,
        }
    }

    pub(super) const fn expires_at(&self) -> Instant {
        self.preparation
    }

    pub(super) fn finish_at(self, now: Instant) -> Result<Instant, HelperFailure> {
        if now >= self.preparation {
            return Err(HelperFailure::ControlledBuildSnapshotTimeout);
        }
        Ok((now + self.startup).min(self.ready))
    }
}

pub(super) fn startup_control_failure(error: ControlError) -> HelperFailure {
    match error {
        ControlError::Deadline => HelperFailure::StartupTimeout,
        other => control_failure(other),
    }
}

#[cfg(test)]
mod tests;
