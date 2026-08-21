use std::{
    thread,
    time::{Duration, Instant},
};

use rewrite_types::CancellationToken;

use crate::{AttachedProcessWitnessError, AttachedProcessWitnessLimits, ensure_active};

const MANAGED_LISTENER_SNAPSHOT_ATTEMPTS: usize = 8;
const MANAGED_LISTENER_SNAPSHOT_LIMIT: Duration = Duration::from_millis(50);
const MANAGED_LISTENER_SNAPSHOT_RETRY_INTERVAL: Duration = Duration::from_millis(5);

pub(super) fn retry_incomplete_snapshot<T, F>(
    limits: AttachedProcessWitnessLimits,
    cancellation: &CancellationToken,
    started: Instant,
    mut snapshot: F,
) -> Result<T, AttachedProcessWitnessError>
where
    F: FnMut() -> Result<T, AttachedProcessWitnessError>,
{
    let retry_started = Instant::now();
    for attempt in 0..MANAGED_LISTENER_SNAPSHOT_ATTEMPTS {
        ensure_active(cancellation, started, limits)?;
        match snapshot() {
            Ok(value) => return Ok(value),
            Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
                if attempt + 1 < MANAGED_LISTENER_SNAPSHOT_ATTEMPTS
                    && retry_started.elapsed() < MANAGED_LISTENER_SNAPSHOT_LIMIT =>
            {
                let remaining =
                    MANAGED_LISTENER_SNAPSHOT_LIMIT.saturating_sub(retry_started.elapsed());
                thread::sleep(MANAGED_LISTENER_SNAPSHOT_RETRY_INTERVAL.min(remaining));
            }
            Err(error) => return Err(error),
        }
    }
    Err(AttachedProcessWitnessError::ListenerSnapshotIncomplete)
}
