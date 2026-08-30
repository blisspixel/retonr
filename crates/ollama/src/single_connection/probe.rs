use std::{error::Error, fmt, net::TcpStream};

use rewrite_inference::{InferenceError, OperationContext};

use super::{
    OllamaConnectionAddresses, OllamaObservedPreflightError, OllamaResponseObservation,
    OllamaResponseObservationPhase, transport::SingleConnectionTransport,
};
use crate::{
    OllamaEndpoint, OllamaLimits, OllamaVersion,
    response::{check_context, compatibility_error, malformed_error},
};

/// Hard maximum response-body bytes accepted by the single-request runtime probe.
pub const OLLAMA_RUNTIME_PROBE_MAX_BODY_BYTES: usize = 1_024;

/// Non-authoritative, content-free result of one exact-version runtime probe.
///
/// This value records only the validated runtime version. It grants no package,
/// model, generation, or admission authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaRuntimeProbeEvidence {
    runtime_version: OllamaVersion,
}

impl OllamaRuntimeProbeEvidence {
    /// Returns the exact validated runtime version.
    #[must_use]
    pub const fn runtime_version(self) -> OllamaVersion {
        self.runtime_version
    }
}

/// Failure from the bounded runtime probe or its caller-provided observer.
#[derive(Debug)]
pub enum OllamaObservedRuntimeProbeError<E> {
    /// The retained connection, protocol, response, version, or context failed closed.
    Probe(InferenceError),
    /// The caller-provided retained-connection observation failed closed.
    Observation(E),
}

impl<E> From<InferenceError> for OllamaObservedRuntimeProbeError<E> {
    fn from(error: InferenceError) -> Self {
        Self::Probe(error)
    }
}

impl<E> fmt::Display for OllamaObservedRuntimeProbeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe(_) => formatter.write_str("single-connection Ollama runtime probe failed"),
            Self::Observation(_) => {
                formatter.write_str("retained Ollama connection observation failed")
            }
        }
    }
}

impl<E: Error + 'static> Error for OllamaObservedRuntimeProbeError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Probe(error) => Some(error),
            Self::Observation(error) => Some(error),
        }
    }
}

/// Exact-version runtime probe over one caller-supplied retained HTTP/1 stream.
///
/// The probe has no connector, pool, retry, fallback, model, or generation input.
/// One run consumes one already-connected loopback stream and can issue only one
/// `GET /api/version` request.
pub struct OllamaSingleConnectionRuntimeProbe {
    endpoint: OllamaEndpoint,
    expected_version: OllamaVersion,
    limits: OllamaLimits,
}

impl OllamaSingleConnectionRuntimeProbe {
    /// Creates a bounded single-request probe for one exact runtime version.
    ///
    /// # Errors
    ///
    /// Returns a policy error when the shared Ollama limits are invalid.
    pub fn new(
        endpoint: OllamaEndpoint,
        expected_version: OllamaVersion,
        limits: OllamaLimits,
    ) -> Result<Self, InferenceError> {
        Ok(Self {
            endpoint,
            expected_version,
            limits: limits.validate()?,
        })
    }

    /// Probes one caller-supplied retained stream with exactly one version request.
    ///
    /// The stream is consumed and must already be connected from loopback to the
    /// configured endpoint. The observer runs after the HTTP handshake immediately
    /// before the request, then exactly once more after a successful response is
    /// fully drained. A callback error or context change prevents further protocol
    /// work. No failed response attempt receives an after-response callback.
    ///
    /// # Errors
    ///
    /// Returns [`OllamaObservedRuntimeProbeError::Probe`] for endpoint, protocol,
    /// limit, timeout, cancellation, persistence, response, or exact-version
    /// failure. Returns [`OllamaObservedRuntimeProbeError::Observation`] when the
    /// callback fails.
    pub async fn probe_on_connected_stream_with_observer<F, E>(
        &self,
        context: OperationContext<'_>,
        stream: TcpStream,
        mut observer: F,
    ) -> Result<OllamaRuntimeProbeEvidence, OllamaObservedRuntimeProbeError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let mut limits = self.limits;
        limits.discovery_body_bytes = limits
            .discovery_body_bytes
            .min(OLLAMA_RUNTIME_PROBE_MAX_BODY_BYTES);
        let mut transport = SingleConnectionTransport::from_connected_stream(
            &self.endpoint,
            limits,
            limits.discovery_body_bytes,
            context,
            stream,
        )
        .await
        .map_err(OllamaObservedRuntimeProbeError::Probe)?;
        let addresses = transport.addresses();
        observe_before_request(addresses, context, &mut observer)?;
        let runtime = transport
            .runtime_identity(context, &mut observer)
            .await
            .map_err(map_transport_error)?;
        let runtime_version = runtime.version.parse::<OllamaVersion>().map_err(|_error| {
            OllamaObservedRuntimeProbeError::Probe(malformed_error("invalid_runtime_version"))
        })?;
        if runtime_version != self.expected_version {
            return Err(OllamaObservedRuntimeProbeError::Probe(compatibility_error(
                "runtime_version_mismatch",
            )));
        }
        transport
            .ensure_open(context)
            .await
            .map_err(OllamaObservedRuntimeProbeError::Probe)?;
        Ok(OllamaRuntimeProbeEvidence { runtime_version })
    }
}

fn observe_before_request<F, E>(
    addresses: OllamaConnectionAddresses,
    context: OperationContext<'_>,
    observer: &mut F,
) -> Result<(), OllamaObservedRuntimeProbeError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    observer(OllamaResponseObservation {
        phase: OllamaResponseObservationPhase::BeforeResponses,
        addresses,
    })
    .map_err(OllamaObservedRuntimeProbeError::Observation)?;
    check_context(context).map_err(OllamaObservedRuntimeProbeError::Probe)
}

fn map_transport_error<E>(
    error: OllamaObservedPreflightError<E>,
) -> OllamaObservedRuntimeProbeError<E> {
    match error {
        OllamaObservedPreflightError::Preflight(error) => {
            OllamaObservedRuntimeProbeError::Probe(error)
        }
        OllamaObservedPreflightError::Observation(error) => {
            OllamaObservedRuntimeProbeError::Observation(error)
        }
    }
}
