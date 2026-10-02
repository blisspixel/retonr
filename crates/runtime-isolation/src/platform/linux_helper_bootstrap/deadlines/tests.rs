use std::{os::fd::AsFd as _, time::Duration};

use super::*;
use crate::platform::{
    linux_build_protocol::{decode_helper_failure, encode_helper_failure},
    linux_control::{pair, receive},
};

#[test]
fn preparation_longer_than_receipt_startup_uses_existing_snapshot_allowance() {
    let now = Instant::now();
    let startup = Duration::from_secs(30);
    let phases = PreparationDeadline::new_at(now, startup);
    assert_eq!(phases.expires_at(), now + Duration::from_mins(10));
    assert_eq!(
        phases.finish_at(now + Duration::from_secs(67)),
        Ok(now + Duration::from_secs(97))
    );
}

#[test]
fn expired_original_preparation_is_never_renewed_into_startup() {
    let now = Instant::now();
    for elapsed in [600, 601] {
        let phases = PreparationDeadline::new_at(now, Duration::from_secs(30));
        assert_eq!(
            phases.finish_at(now + Duration::from_secs(elapsed)),
            Err(HelperFailure::ControlledBuildSnapshotTimeout)
        );
    }
    let phases = PreparationDeadline::new_at(now, Duration::from_secs(30));
    assert_eq!(
        phases.finish_at(now + Duration::from_secs(599)),
        Ok(now + Duration::from_secs(629))
    );
}

#[test]
fn expired_handshake_preserves_startup_timeout_through_helper_wire_codec() {
    let (_sender, receiver) = pair().expect("control pair");
    let error = receive(receiver.as_fd(), Instant::now(), None).expect_err("expired handshake");
    let failure = startup_control_failure(error);
    assert_eq!(failure, HelperFailure::StartupTimeout);
    assert_eq!(
        decode_helper_failure(&encode_helper_failure(failure)),
        Some(crate::IsolationError::StartupTimeout)
    );
    for cause in [
        ControlError::Cancelled,
        ControlError::Closed,
        ControlError::Invalid,
        ControlError::Native,
    ] {
        assert_eq!(startup_control_failure(cause), HelperFailure::InvalidLaunch);
    }
}

#[test]
fn expired_capability_receipt_preserves_startup_timeout() {
    let (_sender, receiver) = pair().expect("control pair");
    assert!(matches!(
        super::super::receive_capabilities(receiver.as_fd(), Instant::now()),
        Err(HelperFailure::StartupTimeout)
    ));
}
