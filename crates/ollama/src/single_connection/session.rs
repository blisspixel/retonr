use std::{collections::BTreeSet, error::Error, fmt, net::TcpStream};

use rewrite_inference::{
    InferenceError, OperationContext, StructuredCompletionRequest, StructuredCompletionResponse,
};

use super::{
    MAX_PREFLIGHT_SESSION_BODY_BYTES, OllamaObservedPreflightError, OllamaResponseObservation,
    OllamaResponseObservationPhase, SingleConnectionTransport, run_preflight,
};
use crate::{
    OllamaEndpoint, OllamaLimits, OllamaModelBinding, OllamaPreflight, OllamaPreflightTarget,
    contract::MAX_PREFLIGHT_TARGETS,
    response::{check_context, compatibility_error, malformed_error, policy_error},
};

use super::receipt::OllamaSessionExecutionReceipt;
use completion::{run_completion, validate_completion_request};

mod completion;
mod residency;

const COMPLETION_RESPONSE_COUNT: usize = 7;

/// Absolute UTF-8 input ceiling for one retained-session completion request.
pub const OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES: u32 = 4 * 1024 * 1024;

/// Failure from a retained session exchange or its retained observer.
#[derive(Debug)]
pub enum OllamaObservedSessionError<E> {
    /// The session contract, transport, or Ollama response failed closed.
    Session(InferenceError),
    /// The retained connection observer failed closed.
    Observation(E),
}

impl<E> fmt::Display for OllamaObservedSessionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(_) => formatter.write_str("retained Ollama session failed"),
            Self::Observation(_) => {
                formatter.write_str("retained Ollama connection observation failed")
            }
        }
    }
}

impl<E: Error + 'static> Error for OllamaObservedSessionError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Session(error) => Some(error),
            Self::Observation(error) => Some(error),
        }
    }
}

/// Validated configuration for a caller-supplied retained Ollama stream.
#[derive(Clone, Debug)]
pub struct OllamaRetainedStreamSessionConfig {
    endpoint: OllamaEndpoint,
    bindings: Vec<OllamaModelBinding>,
    targets: Vec<OllamaPreflightTarget>,
    limits: OllamaLimits,
    session_body_bytes: usize,
    completion_input_bytes: u32,
}

impl OllamaRetainedStreamSessionConfig {
    /// Creates a bounded session configuration with exact artifact bindings.
    ///
    /// # Errors
    ///
    /// Returns a policy error for invalid limits, an empty or oversized binding
    /// set, duplicate identity, or an invalid aggregate response ceiling.
    pub fn new(
        endpoint: OllamaEndpoint,
        bindings: Vec<OllamaModelBinding>,
        limits: OllamaLimits,
        session_body_bytes: usize,
    ) -> Result<Self, InferenceError> {
        let limits = limits.validate()?;
        if bindings.is_empty() || bindings.len() > MAX_PREFLIGHT_TARGETS {
            return Err(policy_error("invalid_session_bindings"));
        }
        if session_body_bytes == 0 || session_body_bytes > MAX_PREFLIGHT_SESSION_BODY_BYTES {
            return Err(policy_error("invalid_session_body_limit"));
        }
        let mut references = BTreeSet::new();
        let mut artifacts = BTreeSet::new();
        let mut inventories = BTreeSet::new();
        let mut targets = Vec::with_capacity(bindings.len());
        for binding in &bindings {
            if !references.insert(binding.reference())
                || !artifacts.insert(binding.artifact_digest().as_str())
                || !inventories.insert(binding.inventory_digest().as_str())
            {
                return Err(policy_error("duplicate_session_binding"));
            }
            targets.push(OllamaPreflightTarget::new(
                binding.reference(),
                binding.inventory_digest().clone(),
            )?);
        }
        Ok(Self {
            endpoint,
            bindings,
            targets,
            limits,
            session_body_bytes,
            completion_input_bytes: OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES,
        })
    }

    /// Consumes one already-connected loopback stream and retains one observer.
    ///
    /// The observer receives the only initial checkpoint before any HTTP request.
    /// The returned session has no connector, pool, retry, or reconnect path.
    ///
    /// # Errors
    ///
    /// Returns a session error when the stream is not the exact configured
    /// loopback connection, the HTTP handshake fails, the initial observation
    /// fails, or the operation context becomes inactive.
    pub async fn open<F, E>(
        &self,
        stream: TcpStream,
        context: OperationContext<'_>,
        mut observer: F,
    ) -> Result<OllamaRetainedStreamSession<F>, OllamaObservedSessionError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let transport = SingleConnectionTransport::from_connected_stream(
            &self.endpoint,
            self.limits,
            self.session_body_bytes,
            context,
            stream,
        )
        .await
        .map_err(OllamaObservedSessionError::Session)?;
        observer(OllamaResponseObservation {
            phase: OllamaResponseObservationPhase::BeforeResponses,
            addresses: transport.addresses(),
        })
        .map_err(OllamaObservedSessionError::Observation)?;
        check_context(context).map_err(OllamaObservedSessionError::Session)?;
        Ok(OllamaRetainedStreamSession {
            transport: Some(transport),
            observer,
            bindings: self.bindings.clone(),
            targets: self.targets.clone(),
            preflight: None,
            preflight_attempted: false,
            completion_input_bytes: self.completion_input_bytes,
        })
    }
}

/// One caller-supplied HTTP/1 connection retained across preflight and inference.
///
/// Any failed operation permanently poisons the session and drops its only
/// transport. The session cannot construct or obtain another connection.
pub struct OllamaRetainedStreamSession<F> {
    transport: Option<SingleConnectionTransport>,
    observer: F,
    bindings: Vec<OllamaModelBinding>,
    targets: Vec<OllamaPreflightTarget>,
    preflight: Option<OllamaPreflight>,
    preflight_attempted: bool,
    completion_input_bytes: u32,
}

impl<F> OllamaRetainedStreamSession<F> {
    /// Irreversibly drops the retained transport and preflight evidence.
    ///
    /// This operation is idempotent and performs no network request. It lets a
    /// caller fail closed after input-specific validation of a returned payload.
    pub fn invalidate(&mut self) {
        self.poison();
    }

    /// Runs the exact N+6 preflight on the retained stream.
    ///
    /// This operation may be attempted exactly once. Its response ordinals start
    /// at one and later completion ordinals continue monotonically.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed session or observation error. Every error poisons
    /// the session and drops the only transport.
    pub async fn preflight<E>(
        &mut self,
        context: OperationContext<'_>,
    ) -> Result<OllamaPreflight, OllamaObservedSessionError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        if self.preflight_attempted {
            return self.fail(policy_error("session_preflight_already_attempted"));
        }
        self.preflight_attempted = true;
        let result = run_preflight(
            self.transport
                .as_mut()
                .ok_or_else(|| OllamaObservedSessionError::Session(session_closed()))?,
            &self.targets,
            context,
            &mut self.observer,
        )
        .await;
        let report = match result {
            Ok(report) => report,
            Err(error) => return Err(self.poison_observed(error, context)),
        };
        let expected = self.targets.len().checked_add(6).ok_or_else(|| {
            OllamaObservedSessionError::Session(malformed_error("response_ordinal_overflow"))
        })?;
        if self.completed_responses() != Some(expected) {
            return self.fail(malformed_error("preflight_response_count_mismatch"));
        }
        if report.bindings.iter().any(|binding| {
            !binding
                .details
                .capabilities
                .iter()
                .any(|capability| capability == "completion")
        }) {
            return self.fail(compatibility_error("completion_not_supported"));
        }
        self.preflight = Some(report.clone());
        Ok(report)
    }

    /// Runs one bounded structured completion on the preflighted transport.
    ///
    /// The result contains the validated structured response and a content-free
    /// receipt binding the preflight, request, response, and exact ordinal span.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed session or observation error for use before
    /// preflight, invalid identity or policy, drift, response failure, callback
    /// failure, cancellation, deadline, or connection closure. Every error
    /// permanently poisons the session.
    pub async fn complete_structured<E>(
        &mut self,
        request: StructuredCompletionRequest,
        context: OperationContext<'_>,
    ) -> Result<
        (StructuredCompletionResponse, OllamaSessionExecutionReceipt),
        OllamaObservedSessionError<E>,
    >
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        if self.transport.is_none() {
            return self.fail(session_closed());
        }
        let Some(preflight) = self.preflight.clone() else {
            return self.fail(policy_error("session_preflight_required"));
        };
        let (binding, profile) = match validate_completion_request(
            &request,
            &self.bindings,
            self.completion_input_bytes,
        ) {
            Ok(validated) => validated,
            Err(error) => return self.fail(error),
        };
        let binding = binding.clone();
        let completed_before = self
            .completed_responses()
            .ok_or_else(|| OllamaObservedSessionError::Session(session_closed()))?;
        let first_ordinal = completed_before.checked_add(1).ok_or_else(|| {
            OllamaObservedSessionError::Session(malformed_error("response_ordinal_overflow"))
        })?;
        let result = run_completion(
            self.transport
                .as_mut()
                .ok_or_else(|| OllamaObservedSessionError::Session(session_closed()))?,
            &mut self.observer,
            &preflight,
            &binding,
            profile,
            &request,
            context,
        )
        .await;
        let response = match result {
            Ok(response) => response,
            Err(error) => return Err(self.poison_observed(error, context)),
        };
        let completed_after = self
            .completed_responses()
            .ok_or_else(|| OllamaObservedSessionError::Session(session_closed()))?;
        if completed_before.checked_add(COMPLETION_RESPONSE_COUNT) != Some(completed_after) {
            return self.fail(malformed_error("completion_response_count_mismatch"));
        }
        let receipt = match OllamaSessionExecutionReceipt::new(
            &preflight,
            &response,
            first_ordinal,
            completed_after,
        ) {
            Ok(receipt) => receipt,
            Err(error) => return self.fail(error),
        };
        Ok((response, receipt))
    }

    fn completed_responses(&self) -> Option<usize> {
        self.transport
            .as_ref()
            .map(SingleConnectionTransport::completed_responses)
    }

    fn poison_observed<E>(
        &mut self,
        error: OllamaObservedPreflightError<E>,
        context: OperationContext<'_>,
    ) -> OllamaObservedSessionError<E>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let error = match error {
            OllamaObservedPreflightError::Observation(error) => {
                OllamaObservedSessionError::Observation(error)
            }
            OllamaObservedPreflightError::Preflight(error) => {
                if self
                    .transport
                    .as_ref()
                    .is_some_and(SingleConnectionTransport::response_attempt_in_progress)
                {
                    let observation = OllamaResponseObservation {
                        phase: OllamaResponseObservationPhase::AfterFailedAttempt {
                            completed_responses: self.completed_responses().unwrap_or_default(),
                        },
                        addresses: self
                            .transport
                            .as_ref()
                            .expect("checked retained transport")
                            .addresses(),
                    };
                    if let Err(observer_error) = (self.observer)(observation) {
                        self.poison();
                        return OllamaObservedSessionError::Observation(observer_error);
                    }
                    if let Err(context_error) = check_context(context) {
                        self.poison();
                        return OllamaObservedSessionError::Session(context_error);
                    }
                }
                OllamaObservedSessionError::Session(error)
            }
        };
        self.poison();
        error
    }

    fn fail<T, E>(&mut self, error: InferenceError) -> Result<T, OllamaObservedSessionError<E>> {
        self.poison();
        Err(OllamaObservedSessionError::Session(error))
    }

    fn poison(&mut self) {
        self.transport.take();
        self.preflight = None;
    }
}

fn session_closed() -> InferenceError {
    policy_error("retained_session_closed")
}

#[cfg(test)]
mod binding_tests {
    use rewrite_model::ArtifactId;
    use rewrite_types::Digest;

    use super::OllamaRetainedStreamSessionConfig;
    use crate::{OllamaEndpoint, OllamaLimits, OllamaModelBinding};

    #[test]
    fn preflight_uses_inventory_identity_not_model_artifact_identity() {
        let artifact = Digest::sha256(b"immutable model bytes");
        let inventory = Digest::sha256(b"mutable Ollama manifest");
        let binding = OllamaModelBinding::new_with_inventory(
            "model:exact",
            ArtifactId::from_digest(artifact.clone()),
            artifact.clone(),
            inventory.clone(),
        )
        .expect("distinct identities");
        let alias_artifact = Digest::sha256(b"different immutable model bytes");
        let alias = OllamaModelBinding::new_with_inventory(
            "model:alias",
            ArtifactId::from_digest(alias_artifact.clone()),
            alias_artifact,
            inventory.clone(),
        )
        .expect("structurally valid duplicate inventory");
        let duplicate_inventory = OllamaRetainedStreamSessionConfig::new(
            OllamaEndpoint::parse("http://127.0.0.1:11434").expect("endpoint"),
            vec![binding.clone(), alias],
            OllamaLimits::default(),
            1024,
        )
        .expect_err("one inventory cannot have two session identities");
        assert_eq!(duplicate_inventory.code, "duplicate_session_binding");
        let config = OllamaRetainedStreamSessionConfig::new(
            OllamaEndpoint::parse("http://127.0.0.1:11434").expect("endpoint"),
            vec![binding],
            OllamaLimits::default(),
            1024,
        )
        .expect("session config");

        assert_eq!(config.bindings[0].artifact_digest(), &artifact);
        assert_eq!(config.bindings[0].inventory_digest(), &inventory);
        assert_eq!(config.targets[0].inventory_digest, inventory);
    }
}
