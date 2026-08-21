use std::{collections::BTreeSet, error::Error, fmt, net::SocketAddr};

use rewrite_inference::{InferenceError, OperationContext};

use crate::{
    OllamaEndpoint,
    contract::{
        MAX_PREFLIGHT_TARGETS, OllamaLimits, OllamaPreflight, OllamaPreflightBinding,
        OllamaPreflightTarget,
    },
    response::{
        check_context, compatibility_error, confirm_inventory_digest, parse_ollama_inventory,
        policy_error,
    },
    wire::TagsResponse,
};

use self::transport::SingleConnectionTransport;

#[cfg(test)]
mod tests;
mod transport;

const MAX_PREFLIGHT_SESSION_BODY_BYTES: usize = 256 * 1024 * 1024;

/// Exact addresses of one retained loopback connection.
///
/// This type deliberately has no serialization or debug implementation. It is intended only for
/// immediate native ownership observation while the connection remains open.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OllamaConnectionAddresses {
    client: SocketAddr,
    server: SocketAddr,
}

impl OllamaConnectionAddresses {
    /// Returns the exact client-side address assigned to the retained connection.
    #[must_use]
    pub const fn client(self) -> SocketAddr {
        self.client
    }

    /// Returns the exact server-side address of the retained connection.
    #[must_use]
    pub const fn server(self) -> SocketAddr {
        self.server
    }
}

/// Position of a native connection observation around the HTTP response sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OllamaResponseObservationPhase {
    /// The connection exists, but no HTTP request has been sent.
    BeforeResponses,
    /// One response has been fully drained on the retained connection.
    AfterResponse {
        /// One-based ordinal of the fully drained response.
        ordinal: usize,
    },
    /// A response attempt failed before another response was fully drained.
    AfterFailedAttempt {
        /// Number of responses fully drained before the failed attempt.
        completed_responses: usize,
    },
}

/// One non-serializable observation request for the retained connection.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OllamaResponseObservation {
    phase: OllamaResponseObservationPhase,
    addresses: OllamaConnectionAddresses,
}

impl OllamaResponseObservation {
    /// Returns whether this observation occurs before or after the response sequence.
    #[must_use]
    pub const fn phase(self) -> OllamaResponseObservationPhase {
        self.phase
    }

    /// Returns the exact retained client and server addresses.
    #[must_use]
    pub const fn addresses(self) -> OllamaConnectionAddresses {
        self.addresses
    }
}

/// Failure from either the bounded Ollama exchange or its caller-provided observation callback.
#[derive(Debug)]
pub enum OllamaObservedPreflightError<E> {
    /// The connection or Ollama response sequence failed closed.
    Preflight(InferenceError),
    /// The caller-provided connection observation failed closed.
    Observation(E),
}

impl<E> From<InferenceError> for OllamaObservedPreflightError<E> {
    fn from(error: InferenceError) -> Self {
        Self::Preflight(error)
    }
}

impl<E> fmt::Display for OllamaObservedPreflightError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(_) => formatter.write_str("single-connection Ollama preflight failed"),
            Self::Observation(_) => {
                formatter.write_str("retained Ollama connection observation failed")
            }
        }
    }
}

impl<E: Error + 'static> Error for OllamaObservedPreflightError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Preflight(error) => Some(error),
            Self::Observation(error) => Some(error),
        }
    }
}

/// Read-only Ollama preflight that owns exactly one HTTP/1 connection per run.
///
/// The transport performs one direct loopback TCP connect and has no connector, pool, retry, or
/// reconnect path. A server-side close therefore fails the preflight instead of moving later
/// requests to another connection.
pub struct OllamaSingleConnectionPreflight {
    endpoint: OllamaEndpoint,
    targets: Vec<OllamaPreflightTarget>,
    limits: OllamaLimits,
    session_body_bytes: usize,
}

impl OllamaSingleConnectionPreflight {
    /// Creates a validated one-connection preflight configuration.
    ///
    /// `session_body_bytes` is an aggregate ceiling across every response in the N+6 request
    /// sequence. The existing discovery ceiling still applies independently to each response.
    ///
    /// # Errors
    ///
    /// Returns a policy error for invalid limits, targets, duplicate target identity, or an
    /// invalid aggregate response ceiling.
    pub fn new(
        endpoint: OllamaEndpoint,
        targets: Vec<OllamaPreflightTarget>,
        limits: OllamaLimits,
        session_body_bytes: usize,
    ) -> Result<Self, InferenceError> {
        let limits = limits.validate()?;
        validate_targets(&targets)?;
        if session_body_bytes == 0 || session_body_bytes > MAX_PREFLIGHT_SESSION_BODY_BYTES {
            return Err(policy_error("invalid_preflight_session_body_limit"));
        }
        Ok(Self {
            endpoint,
            targets,
            limits,
            session_body_bytes,
        })
    }

    /// Runs the exact read-only N+6 request sequence on one retained HTTP/1 connection.
    ///
    /// The callback runs once before any request and after each fully drained response. A failed
    /// response attempt also receives one terminal observation that does not advance the response
    /// ordinal. Every callback runs before another request can be sent and while the connection
    /// driver is still retained. Callbacks must enforce their own internal execution ceiling. The
    /// operation context is checked immediately after every callback so an overrun or cancellation
    /// cannot admit another request. An observation error takes priority over an HTTP error.
    ///
    /// # Errors
    ///
    /// Returns [`OllamaObservedPreflightError::Preflight`] for connection, protocol, limit,
    /// cancellation, deadline, response, or drift failure. Returns
    /// [`OllamaObservedPreflightError::Observation`] when the callback fails.
    pub async fn preflight_with_observer<F, E>(
        &self,
        context: OperationContext<'_>,
        mut observer: F,
    ) -> Result<OllamaPreflight, OllamaObservedPreflightError<E>>
    where
        F: FnMut(OllamaResponseObservation) -> Result<(), E>,
    {
        let mut transport = SingleConnectionTransport::connect(
            &self.endpoint,
            self.limits,
            self.session_body_bytes,
            context,
        )
        .await?;
        let addresses = transport.addresses();
        observer(OllamaResponseObservation {
            phase: OllamaResponseObservationPhase::BeforeResponses,
            addresses,
        })
        .map_err(OllamaObservedPreflightError::Observation)?;
        check_context(context).map_err(OllamaObservedPreflightError::Preflight)?;

        let result = run_preflight(&mut transport, &self.targets, context, &mut observer).await;
        if matches!(result, Err(OllamaObservedPreflightError::Preflight(_)))
            && transport.response_attempt_in_progress()
        {
            observer(OllamaResponseObservation {
                phase: OllamaResponseObservationPhase::AfterFailedAttempt {
                    completed_responses: transport.completed_responses(),
                },
                addresses,
            })
            .map_err(OllamaObservedPreflightError::Observation)?;
            check_context(context).map_err(OllamaObservedPreflightError::Preflight)?;
        }
        result
    }
}

fn validate_targets(targets: &[OllamaPreflightTarget]) -> Result<(), InferenceError> {
    if targets.is_empty() || targets.len() > MAX_PREFLIGHT_TARGETS {
        return Err(policy_error("invalid_preflight_targets"));
    }
    let mut references = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for target in targets {
        if !references.insert(target.reference.as_str())
            || !digests.insert(target.inventory_digest.as_str())
        {
            return Err(policy_error("duplicate_preflight_target"));
        }
    }
    Ok(())
}

async fn run_preflight<F, E>(
    transport: &mut SingleConnectionTransport,
    targets: &[OllamaPreflightTarget],
    context: OperationContext<'_>,
    observer: &mut F,
) -> Result<OllamaPreflight, OllamaObservedPreflightError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    let runtime_before = transport.runtime_identity(context, observer).await?;
    let tags_before = transport.tags(context, observer).await?;
    let mut inventory =
        parse_ollama_inventory(&tags_before).map_err(OllamaObservedPreflightError::Preflight)?;
    inventory.sort_unstable_by(|left, right| left.reference.cmp(&right.reference));
    let running_before = transport.running_models(context, observer).await?;
    let mut bindings = inspect_targets(transport, targets, &tags_before, context, observer).await?;
    bindings.sort_unstable_by(|left, right| left.reference.cmp(&right.reference));
    let tags_after = transport.tags(context, observer).await?;
    let runtime_after = transport.runtime_identity(context, observer).await?;
    let running_after = transport.running_models(context, observer).await?;
    if runtime_before != runtime_after
        || tags_before != tags_after
        || running_before != running_after
    {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("runtime_changed_during_preflight"),
        ));
    }
    for target in targets {
        confirm_inventory_digest(target.reference(), target.inventory_digest(), &tags_after)
            .map_err(OllamaObservedPreflightError::Preflight)?;
    }
    transport
        .ensure_open(context)
        .await
        .map_err(OllamaObservedPreflightError::Preflight)?;
    Ok(OllamaPreflight {
        runtime: runtime_after,
        inventory,
        bindings,
        running: running_after,
    })
}

async fn inspect_targets<F, E>(
    transport: &mut SingleConnectionTransport,
    targets: &[OllamaPreflightTarget],
    tags: &TagsResponse,
    context: OperationContext<'_>,
    observer: &mut F,
) -> Result<Vec<OllamaPreflightBinding>, OllamaObservedPreflightError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    let mut bindings = Vec::with_capacity(targets.len());
    for target in targets {
        confirm_inventory_digest(target.reference(), target.inventory_digest(), tags)
            .map_err(OllamaObservedPreflightError::Preflight)?;
        bindings.push(OllamaPreflightBinding {
            reference: target.reference.clone(),
            inventory_digest: target.inventory_digest.clone(),
            details: transport
                .show_details(target.reference(), context, observer)
                .await?,
        });
    }
    Ok(bindings)
}
