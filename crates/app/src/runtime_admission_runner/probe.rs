use super::{
    AttachedProcessLease, CancellationToken, OllamaObservedRuntimeProbeError,
    OllamaResponseObservationPhase, OllamaRuntimeProbeEvidence, OllamaSingleConnectionRuntimeProbe,
    OperationContext, RetainedTcpConnection, RetainedTcpConnectionEvidence,
    RuntimeAdmissionRunnerError, TcpStream,
};

pub(super) async fn run_exact_probe<L: AttachedProcessLease + ?Sized>(
    probe: &OllamaSingleConnectionRuntimeProbe,
    stream: TcpStream,
    process: &mut L,
    cancellation: &CancellationToken,
) -> Result<
    (
        OllamaRuntimeProbeEvidence,
        RetainedTcpConnectionEvidence,
        RetainedTcpConnectionEvidence,
    ),
    RuntimeAdmissionRunnerError,
> {
    let mut retained_connection = None;
    let mut initial_evidence = None;
    let mut final_evidence = None;
    let context = OperationContext::new(cancellation, None);
    let runtime_probe = probe
        .probe_on_connected_stream_with_observer(context, stream, |observation| {
            let addresses = observation.addresses();
            let connection = RetainedTcpConnection::new(addresses.client(), addresses.server())
                .map_err(RuntimeAdmissionRunnerError::Witness)?;
            match observation.phase() {
                OllamaResponseObservationPhase::BeforeResponses
                    if retained_connection.is_none()
                        && initial_evidence.is_none()
                        && final_evidence.is_none() =>
                {
                    let evidence = process
                        .observe_connection(connection, cancellation)
                        .map_err(RuntimeAdmissionRunnerError::Witness)?;
                    retained_connection = Some(connection);
                    initial_evidence = Some(evidence);
                    Ok(())
                }
                OllamaResponseObservationPhase::AfterResponse { ordinal: 1 }
                    if retained_connection == Some(connection)
                        && initial_evidence.is_some()
                        && final_evidence.is_none() =>
                {
                    let evidence = process
                        .reobserve_connection(
                            connection,
                            initial_evidence
                                .as_ref()
                                .expect("guard requires initial connection evidence"),
                            cancellation,
                        )
                        .map_err(RuntimeAdmissionRunnerError::Witness)?;
                    final_evidence = Some(evidence);
                    Ok(())
                }
                _ => Err(RuntimeAdmissionRunnerError::InvalidResponseObservationSequence),
            }
        })
        .await
        .map_err(|error| match error {
            OllamaObservedRuntimeProbeError::Probe(error) => {
                RuntimeAdmissionRunnerError::Probe(error)
            }
            OllamaObservedRuntimeProbeError::Observation(error) => error,
        })?;
    let initial =
        initial_evidence.ok_or(RuntimeAdmissionRunnerError::InvalidResponseObservationSequence)?;
    let final_evidence =
        final_evidence.ok_or(RuntimeAdmissionRunnerError::InvalidResponseObservationSequence)?;
    Ok((runtime_probe, initial, final_evidence))
}
