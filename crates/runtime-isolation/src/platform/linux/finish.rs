use super::{
    CancellationToken, File, HelperProcess, Instant, IsolationError, IsolationEvidence,
    IsolationPolicy, IsolationPreparationEvidence, IsolationResult, Lease, OwnedFd, ReadyMessage,
    RetainedRuntimeInputMember, abort_helper, ensure_active_until, operation_error_precedence,
    verify,
};

pub(super) struct FinishLaunchRequest<'a> {
    pub(super) target: &'a File,
    pub(super) runtime_inputs: Vec<RetainedRuntimeInputMember>,
    pub(super) control: OwnedFd,
    pub(super) ready: ReadyMessage,
    pub(super) policy: IsolationPolicy,
    pub(super) preparation: &'a IsolationPreparationEvidence,
    pub(super) cancellation: &'a CancellationToken,
    pub(super) operation_deadline: Option<Instant>,
}

pub(super) fn finish_launch(
    mut child: HelperProcess,
    request: FinishLaunchRequest<'_>,
) -> IsolationResult<Lease> {
    let FinishLaunchRequest {
        target,
        runtime_inputs,
        control,
        ready,
        policy,
        preparation,
        cancellation,
        operation_deadline,
    } = request;
    let guardian_pid = child.id();
    if let Err(error) = ensure_active_until(cancellation, operation_deadline) {
        return Err(abort_helper(&mut child, policy.shutdown_timeout(), error));
    }
    if ready.guardian_pid != guardian_pid {
        let error = operation_error_precedence(
            IsolationError::HelperProtocol,
            cancellation,
            operation_deadline,
        );
        return Err(abort_helper(&mut child, policy.shutdown_timeout(), error));
    }
    let declarations = runtime_inputs
        .iter()
        .map(|input| input.declaration.clone())
        .collect::<Vec<_>>();
    let verified = match verify::launch(verify::LaunchRequest {
        guardian_pid,
        target,
        runtime_inputs: &declarations,
        ready: &ready,
        policy,
        preparation,
        cancellation,
        operation_deadline,
    }) {
        Ok(verified) => verified,
        Err(error) => {
            let error = operation_error_precedence(error, cancellation, operation_deadline);
            return Err(abort_helper(&mut child, policy.shutdown_timeout(), error));
        }
    };
    if let Err(error) = ensure_active_until(cancellation, operation_deadline) {
        return Err(abort_helper(&mut child, policy.shutdown_timeout(), error));
    }
    let initial = IsolationEvidence::new(
        guardian_pid,
        verified.namespaces,
        ready.device_boundary,
        ready.runtime_inputs.clone(),
        preparation.clone(),
        verified.target.evidence,
    );
    Ok(Lease {
        child,
        pidfd: verified.pidfd,
        namespace_init_pidfd: verified.namespace_init_pidfd,
        target_pidfd: verified.target.pidfd,
        namespace_init_pid: ready.namespace_init_pid,
        target_executable: verified.target.executable,
        network_namespace: verified.network_namespace,
        user_namespace: verified.user_namespace,
        process_namespace: verified.process_namespace,
        mount_namespace: verified.mount_namespace,
        runtime_inputs,
        initial,
        control,
        channel_timeout: policy.startup_timeout(),
        channel_requested: false,
        shutdown_timeout: policy.shutdown_timeout(),
        closed: false,
    })
}
