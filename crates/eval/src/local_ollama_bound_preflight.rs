//! Native exact-connection witness joined to one retained Ollama preflight transport.

use rewrite_inference::OperationContext;
use rewrite_ollama::{
    OllamaEndpoint, OllamaLimits, OllamaObservedPreflightError, OllamaResponseObservation,
    OllamaResponseObservationPhase, OllamaSingleConnectionPreflight,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessLease, AttachedProcessObserver,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint,
    MAXIMUM_ENTRYPOINT_BYTES, NativeAttachedProcessObserver, RetainedTcpConnection,
    RetainedTcpConnectionEvidence,
};
use rewrite_types::{CancellationToken, Digest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    LocalOllamaPreflightError, LocalOllamaPreflightMode, LocalOllamaPreflightPlan,
    LocalOllamaPreflightReport, MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES,
    local_ollama_preflight::{
        local_ollama_preflight_report, local_ollama_preflight_targets,
        validate_local_ollama_preflight_plan,
    },
};

/// Current bound Ollama preflight plan contract version.
pub const LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION: u32 = 1;
/// Current bound Ollama preflight report contract version.
pub const LOCAL_OLLAMA_BOUND_PREFLIGHT_REPORT_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded bound-preflight plan bytes.
pub const MAX_LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_BYTES: usize =
    MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES + 16 * 1024;

/// Bounded plan for native process and exact-connection observation around one preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalOllamaBoundPreflightPlan {
    /// Bound-preflight contract version.
    pub schema_version: u32,
    /// Existing versioned read-only Ollama preflight.
    pub preflight: LocalOllamaPreflightPlan,
    /// Maximum executable bytes hashed before and after HTTP observation.
    pub maximum_entrypoint_bytes: u64,
    /// Maximum aggregate response-body bytes on the retained session.
    pub maximum_session_body_bytes: u64,
    /// Frozen executable digest required in verify mode and forbidden in observe mode.
    #[serde(default)]
    pub expected_entrypoint_digest: Option<Digest>,
}

/// Exact process evidence strength represented by a bound-preflight report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaBoundProcessEvidenceLevel {
    /// Native attribution matched the retained process before and after every response.
    ObservedNativeConnectionAttribution,
}

/// Redacted inert report from one retained transport and native attribution bracket.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the versioned report exposes four explicit security claim flags"
)]
pub struct LocalOllamaBoundPreflightReport {
    /// Bound-preflight report version.
    pub schema_version: u32,
    /// Digest of the canonical bound-preflight plan.
    pub plan_digest: Digest,
    /// Domain-separated digest binding the plan, API report, and native evidence sequence.
    pub binding_digest: Digest,
    /// Exact native process evidence strength.
    pub process_evidence_level: LocalOllamaBoundProcessEvidenceLevel,
    /// Final stable listener-owner, process-incarnation, and executable evidence.
    pub process_witness: AttachedProcessEvidence,
    /// Final redacted exact-connection attribution evidence.
    pub connection_witness: RetainedTcpConnectionEvidence,
    /// Initial and post-response redacted connection observations in execution order.
    pub connection_observations: Vec<RetainedTcpConnectionEvidence>,
    /// Always true for a successful report because the transport has no reconnect path.
    pub all_responses_used_retained_transport: bool,
    /// Always true for a successful report after exact count and ordinal validation.
    pub kernel_attribution_checked_after_each_response: bool,
    /// Always false because admitted platform APIs do not prove exclusive ownership.
    pub exclusive_socket_owner_proven: bool,
    /// Always false because socket attribution does not identify application handlers.
    pub application_handler_proven: bool,
    /// Existing stable read-only Ollama API observation.
    pub preflight: LocalOllamaPreflightReport,
    /// Always false. This development report cannot qualify a runtime or model.
    pub qualified: bool,
}

/// Bound-preflight plan, process witness, connection witness, or Ollama failure.
#[derive(Debug, Error)]
pub enum LocalOllamaBoundPreflightError {
    /// Encoded plan exceeds the parser ceiling.
    #[error("bound Ollama preflight plan exceeds the byte limit")]
    TooLarge,
    /// JSON is malformed or contains unknown fields.
    #[error("invalid bound Ollama preflight plan JSON")]
    InvalidJson,
    /// Plan schema is unsupported.
    #[error("unsupported bound Ollama preflight plan schema")]
    UnsupportedSchema,
    /// Plan values or nested preflight are invalid.
    #[error("invalid bound Ollama preflight plan")]
    InvalidPlan,
    /// The adapter produced an invalid callback sequence.
    #[error("bound Ollama connection observation sequence is invalid")]
    InvalidObservationSequence,
    /// Native listener, process, executable, or connection observation failed closed.
    #[error("bound Ollama native witness failed: {0}")]
    Witness(#[source] AttachedProcessWitnessError),
    /// The retained read-only Ollama observation failed closed.
    #[error("bound Ollama API preflight failed: {0}")]
    Preflight(#[source] LocalOllamaPreflightError),
}

/// Parses and validates one byte-bounded bound-preflight plan.
///
/// # Errors
///
/// Returns [`LocalOllamaBoundPreflightError`] for oversized, malformed, unsupported,
/// unbounded, or internally inconsistent input.
pub fn parse_local_ollama_bound_preflight_plan(
    bytes: &[u8],
) -> Result<LocalOllamaBoundPreflightPlan, LocalOllamaBoundPreflightError> {
    if bytes.len() > MAX_LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_BYTES {
        return Err(LocalOllamaBoundPreflightError::TooLarge);
    }
    let plan: LocalOllamaBoundPreflightPlan = serde_json::from_slice(bytes)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidJson)?;
    validate_bound_plan(&plan)?;
    Ok(plan)
}

/// Runs one exact retained transport under native process and connection observation.
///
/// The result remains inert and unqualified. It proves neither exclusive socket
/// ownership nor application-handler execution.
///
/// # Errors
///
/// Returns [`LocalOllamaBoundPreflightError`] for any invalid plan, native
/// observation failure, executable mismatch, retained HTTP failure, or drift.
pub async fn run_local_ollama_bound_preflight(
    plan: &LocalOllamaBoundPreflightPlan,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaBoundPreflightReport, LocalOllamaBoundPreflightError> {
    run_with_observer(plan, cancellation, &NativeAttachedProcessObserver).await
}

async fn run_with_observer<O: AttachedProcessObserver>(
    plan: &LocalOllamaBoundPreflightPlan,
    cancellation: &CancellationToken,
    process_observer: &O,
) -> Result<LocalOllamaBoundPreflightReport, LocalOllamaBoundPreflightError> {
    validate_bound_plan(plan)?;
    let endpoint = OllamaEndpoint::parse(&plan.preflight.endpoint)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let listener = ListenerEndpoint::new(endpoint.socket_addr())
        .map_err(LocalOllamaBoundPreflightError::Witness)?;
    let limits = AttachedProcessWitnessLimits {
        maximum_entrypoint_bytes: plan.maximum_entrypoint_bytes,
        ..AttachedProcessWitnessLimits::default()
    };
    let mut lease = process_observer
        .attach(listener, limits, cancellation)
        .map_err(LocalOllamaBoundPreflightError::Witness)?;
    enforce_entrypoint_digest(plan, lease.initial_evidence())?;

    let targets = local_ollama_preflight_targets(&plan.preflight)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let expected_responses = targets.len().saturating_add(6);
    let maximum_session_body_bytes = usize::try_from(plan.maximum_session_body_bytes)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let preflight = OllamaSingleConnectionPreflight::new(
        endpoint,
        targets,
        OllamaLimits::default(),
        maximum_session_body_bytes,
    )
    .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let mut observations = ConnectionObservationSequence::new(expected_responses);
    let transport_result = preflight
        .preflight_with_observer(OperationContext::new(cancellation, None), |observation| {
            observations.observe(&mut lease, cancellation, observation)
        })
        .await;

    let process_witness = lease
        .reobserve(cancellation)
        .map_err(LocalOllamaBoundPreflightError::Witness)?;
    enforce_entrypoint_digest(plan, &process_witness)?;

    let api_observation = match transport_result {
        Ok(api_observation) => api_observation,
        Err(OllamaObservedPreflightError::Observation(error)) => return Err(error),
        Err(OllamaObservedPreflightError::Preflight(error)) => {
            return Err(LocalOllamaBoundPreflightError::Preflight(
                LocalOllamaPreflightError::Backend(error),
            ));
        }
    };
    observations.validate_complete()?;
    let preflight = local_ollama_preflight_report(&plan.preflight, api_observation)
        .map_err(LocalOllamaBoundPreflightError::Preflight)?;
    let connection_observations = observations.into_evidence();
    let connection_witness = connection_observations
        .last()
        .cloned()
        .ok_or(LocalOllamaBoundPreflightError::InvalidObservationSequence)?;
    let canonical =
        serde_json::to_vec(plan).map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let plan_digest = Digest::sha256(&canonical);
    let binding_digest = binding_digest(
        &plan_digest,
        &process_witness,
        &connection_observations,
        &preflight,
    )?;

    Ok(LocalOllamaBoundPreflightReport {
        schema_version: LOCAL_OLLAMA_BOUND_PREFLIGHT_REPORT_SCHEMA_VERSION,
        plan_digest,
        binding_digest,
        process_evidence_level:
            LocalOllamaBoundProcessEvidenceLevel::ObservedNativeConnectionAttribution,
        process_witness,
        connection_witness,
        connection_observations,
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_after_each_response: true,
        exclusive_socket_owner_proven: false,
        application_handler_proven: false,
        preflight,
        qualified: false,
    })
}

pub(crate) fn validate_bound_plan(
    plan: &LocalOllamaBoundPreflightPlan,
) -> Result<(), LocalOllamaBoundPreflightError> {
    if plan.schema_version != LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION {
        return Err(LocalOllamaBoundPreflightError::UnsupportedSchema);
    }
    validate_local_ollama_preflight_plan(&plan.preflight)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let digest_matches_mode = match plan.preflight.mode {
        LocalOllamaPreflightMode::Observe => plan.expected_entrypoint_digest.is_none(),
        LocalOllamaPreflightMode::Verify => plan.expected_entrypoint_digest.is_some(),
    };
    let maximum_session_body_bytes = usize::try_from(plan.maximum_session_body_bytes)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let endpoint = OllamaEndpoint::parse(&plan.preflight.endpoint)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let targets = local_ollama_preflight_targets(&plan.preflight)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let session_valid = OllamaSingleConnectionPreflight::new(
        endpoint,
        targets,
        OllamaLimits::default(),
        maximum_session_body_bytes,
    )
    .is_ok();
    if !digest_matches_mode
        || plan.maximum_entrypoint_bytes == 0
        || plan.maximum_entrypoint_bytes > MAXIMUM_ENTRYPOINT_BYTES
        || !session_valid
    {
        return Err(LocalOllamaBoundPreflightError::InvalidPlan);
    }
    Ok(())
}

fn enforce_entrypoint_digest(
    plan: &LocalOllamaBoundPreflightPlan,
    evidence: &AttachedProcessEvidence,
) -> Result<(), LocalOllamaBoundPreflightError> {
    if plan
        .expected_entrypoint_digest
        .as_ref()
        .is_some_and(|expected| expected != evidence.entrypoint_digest())
    {
        return Err(LocalOllamaBoundPreflightError::Witness(
            AttachedProcessWitnessError::EntrypointDigestMismatch,
        ));
    }
    Ok(())
}

pub(crate) struct ConnectionObservationSequence {
    expected_responses: usize,
    completed_responses: usize,
    connection: Option<RetainedTcpConnection>,
    initial: Option<RetainedTcpConnectionEvidence>,
    evidence: Vec<RetainedTcpConnectionEvidence>,
    failed_attempt_observed: bool,
}

impl ConnectionObservationSequence {
    pub(crate) fn new(expected_responses: usize) -> Self {
        Self {
            expected_responses,
            completed_responses: 0,
            connection: None,
            initial: None,
            evidence: Vec::with_capacity(expected_responses.saturating_add(1)),
            failed_attempt_observed: false,
        }
    }

    pub(crate) fn observe<L: AttachedProcessLease>(
        &mut self,
        lease: &mut L,
        cancellation: &CancellationToken,
        observation: OllamaResponseObservation,
    ) -> Result<(), LocalOllamaBoundPreflightError> {
        let addresses = observation.addresses();
        let connection = RetainedTcpConnection::new(addresses.client(), addresses.server())
            .map_err(LocalOllamaBoundPreflightError::Witness)?;
        match observation.phase() {
            OllamaResponseObservationPhase::BeforeResponses => {
                if self.connection.is_some()
                    || self.initial.is_some()
                    || self.completed_responses != 0
                    || self.failed_attempt_observed
                {
                    return Err(LocalOllamaBoundPreflightError::InvalidObservationSequence);
                }
                let evidence = lease
                    .observe_connection(connection, cancellation)
                    .map_err(LocalOllamaBoundPreflightError::Witness)?;
                self.connection = Some(connection);
                self.initial = Some(evidence.clone());
                self.evidence.push(evidence);
            }
            OllamaResponseObservationPhase::AfterResponse { ordinal } => {
                if self.failed_attempt_observed
                    || ordinal != self.completed_responses.saturating_add(1)
                    || ordinal > self.expected_responses
                    || self.connection != Some(connection)
                {
                    return Err(LocalOllamaBoundPreflightError::InvalidObservationSequence);
                }
                let initial = self
                    .initial
                    .as_ref()
                    .ok_or(LocalOllamaBoundPreflightError::InvalidObservationSequence)?;
                let evidence = lease
                    .reobserve_connection(connection, initial, cancellation)
                    .map_err(LocalOllamaBoundPreflightError::Witness)?;
                self.completed_responses = ordinal;
                self.evidence.push(evidence);
            }
            OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses,
            } => {
                if self.failed_attempt_observed
                    || completed_responses != self.completed_responses
                    || self.connection != Some(connection)
                {
                    return Err(LocalOllamaBoundPreflightError::InvalidObservationSequence);
                }
                let initial = self
                    .initial
                    .as_ref()
                    .ok_or(LocalOllamaBoundPreflightError::InvalidObservationSequence)?;
                lease
                    .reobserve_connection(connection, initial, cancellation)
                    .map_err(LocalOllamaBoundPreflightError::Witness)?;
                self.failed_attempt_observed = true;
            }
        }
        Ok(())
    }

    pub(crate) fn validate_complete(&self) -> Result<(), LocalOllamaBoundPreflightError> {
        self.validate_progress(self.expected_responses)
    }

    pub(crate) fn validate_progress(
        &self,
        completed_responses: usize,
    ) -> Result<(), LocalOllamaBoundPreflightError> {
        if completed_responses > self.expected_responses
            || self.failed_attempt_observed
            || self.completed_responses != completed_responses
            || self.initial.is_none()
            || self.evidence.len() != completed_responses.saturating_add(1)
        {
            return Err(LocalOllamaBoundPreflightError::InvalidObservationSequence);
        }
        Ok(())
    }

    pub(crate) fn evidence(&self) -> &[RetainedTcpConnectionEvidence] {
        &self.evidence
    }

    pub(crate) fn into_evidence(self) -> Vec<RetainedTcpConnectionEvidence> {
        self.evidence
    }
}

fn binding_digest(
    plan_digest: &Digest,
    process: &AttachedProcessEvidence,
    connections: &[RetainedTcpConnectionEvidence],
    preflight: &LocalOllamaPreflightReport,
) -> Result<Digest, LocalOllamaBoundPreflightError> {
    let preflight_bytes = serde_json::to_vec(preflight)
        .map_err(|_error| LocalOllamaBoundPreflightError::InvalidPlan)?;
    let mut material = Vec::with_capacity(256 + connections.len().saturating_mul(72));
    material.extend_from_slice(b"retonr:local-ollama-bound-preflight-binding:v1\0");
    material.extend_from_slice(plan_digest.as_str().as_bytes());
    material.extend_from_slice(process.evidence_digest().as_str().as_bytes());
    material.extend_from_slice(
        &u64::try_from(connections.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    for (index, evidence) in connections.iter().enumerate() {
        material.extend_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_be_bytes());
        material.extend_from_slice(evidence.evidence_digest().as_str().as_bytes());
    }
    material.extend_from_slice(Digest::sha256(&preflight_bytes).as_str().as_bytes());
    Ok(Digest::sha256(&material))
}

#[cfg(test)]
#[path = "local_ollama_bound_preflight/tests.rs"]
mod tests;
