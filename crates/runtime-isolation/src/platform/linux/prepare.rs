use std::{fs::File, path::Path, time::Instant};

use rewrite_types::{CancellationToken, Digest};

use crate::{IsolationError, IsolationPolicy, IsolationPreparationEvidence, IsolationResult};

use super::{Prepared, abort_helper, abort_helper_until, ensure_active, receive_ready};
use crate::platform::{
    PrepareOutput,
    linux_command::{apply_policy_environment, helper_command, spawn_helper},
    linux_helper_identity::{open_executable, snapshot_helper, validate_executable},
    linux_process::require_successful_exit,
};

pub(crate) fn prepare(
    helper_executable: &Path,
    expected_digest: &Digest,
    expected_bytes: u64,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> IsolationResult<PrepareOutput> {
    ensure_active(cancellation)?;
    let started = Instant::now();
    let helper = open_executable(helper_executable, true)?;
    prepare_retained_started(
        helper,
        expected_digest,
        expected_bytes,
        policy,
        cancellation,
        started,
    )
}

pub(crate) fn prepare_retained(
    helper: File,
    expected_digest: &Digest,
    expected_bytes: u64,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> IsolationResult<PrepareOutput> {
    ensure_active(cancellation)?;
    prepare_retained_started(
        helper,
        expected_digest,
        expected_bytes,
        policy,
        cancellation,
        Instant::now(),
    )
}

fn prepare_retained_started(
    helper: File,
    expected_digest: &Digest,
    expected_bytes: u64,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
    started: Instant,
) -> IsolationResult<PrepareOutput> {
    validate_executable(&helper, true)?;
    let helper = snapshot_helper(
        helper,
        expected_digest,
        expected_bytes,
        policy.startup_timeout(),
        cancellation,
        started,
    )?;
    let mut command = helper_command(&helper);
    command.arg("--stage1-probe");
    apply_policy_environment(&mut command, policy);
    let mut child = spawn_helper(&mut command)?;
    let remaining = policy.startup_timeout().saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err(abort_helper(
            &mut child,
            policy.shutdown_timeout(),
            IsolationError::StartupTimeout,
        ));
    }
    let ready = receive_ready(&mut child, remaining, cancellation)
        .map_err(|error| abort_helper(&mut child, policy.shutdown_timeout(), error))?;
    let shutdown_deadline = Instant::now() + policy.shutdown_timeout();
    let status = child
        .wait_for_exit(
            shutdown_deadline,
            Some(cancellation),
            "wait-isolation-helper",
        )
        .map_err(|error| abort_helper_until(&mut child, shutdown_deadline, error))?;
    require_successful_exit(status)?;
    if ready.device_boundary.policy() != policy.managed_device_visibility()
        || !ready.device_boundary.all_visibility_canaries_passed()
        || !ready.runtime_inputs.is_empty()
        || !ready.runtime_inputs.all_postconditions_passed()
    {
        return Err(IsolationError::EvidenceChanged);
    }
    let preparation = IsolationPreparationEvidence::verified(
        ready.loopback_index,
        ready.device_boundary.policy(),
        expected_digest.clone(),
        expected_bytes,
    );
    Ok((
        Prepared {
            helper,
            preparation: preparation.clone(),
        },
        preparation,
    ))
}
