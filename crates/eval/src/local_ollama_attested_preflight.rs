//! Native listener-owner witness joined to the read-only Ollama preflight.

use rewrite_ollama::OllamaEndpoint;
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessLease, AttachedProcessObserver,
    AttachedProcessWitnessError, AttachedProcessWitnessLimits, ListenerEndpoint,
    MAXIMUM_ENTRYPOINT_BYTES, NativeAttachedProcessObserver,
};
use rewrite_types::{CancellationToken, Digest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    LocalOllamaPreflightError, LocalOllamaPreflightMode, LocalOllamaPreflightPlan,
    LocalOllamaPreflightReport, MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES,
    parse_local_ollama_preflight_plan, run_local_ollama_preflight,
};

/// Current attached Ollama preflight plan contract version.
pub const LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_SCHEMA_VERSION: u32 = 1;
/// Current attached Ollama preflight report contract version.
pub const LOCAL_OLLAMA_ATTESTED_PREFLIGHT_REPORT_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded attached-preflight plan bytes.
pub const MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES: usize =
    MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES + 16 * 1024;

/// Bounded plan for native listener-owner observation around one Ollama preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalOllamaAttestedPreflightPlan {
    /// Attached-preflight contract version.
    pub schema_version: u32,
    /// Existing versioned read-only Ollama preflight.
    pub preflight: LocalOllamaPreflightPlan,
    /// Maximum executable bytes hashed before and after HTTP observation.
    pub maximum_entrypoint_bytes: u64,
    /// Frozen executable digest required in verify mode and forbidden in observe mode.
    #[serde(default)]
    pub expected_entrypoint_digest: Option<Digest>,
}

/// Evidence strength represented by the first attached-preflight report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaProcessEvidenceLevel {
    /// Native listener ownership was observed before and after the HTTP preflight.
    ObservedNativeListener,
}

/// Redacted inert report from one bracketed process and Ollama preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocalOllamaAttestedPreflightReport {
    /// Attached-preflight report version.
    pub schema_version: u32,
    /// Digest of the canonical attached-preflight plan.
    pub plan_digest: Digest,
    /// Exact evidence strength. This is not response attestation.
    pub process_evidence_level: LocalOllamaProcessEvidenceLevel,
    /// Stable native listener-owner and executable evidence.
    pub process_witness: AttachedProcessEvidence,
    /// Stable read-only Ollama API observation.
    pub preflight: LocalOllamaPreflightReport,
    /// Always false because the HTTP connection is not bound to a server-side socket owner.
    pub response_bound: bool,
    /// Always false. This development report cannot qualify a runtime or model.
    pub qualified: bool,
}

/// Attached-preflight plan, native witness, or Ollama observation failure.
#[derive(Debug, Error)]
pub enum LocalOllamaAttestedPreflightError {
    /// Encoded plan exceeds the parser ceiling.
    #[error("attached Ollama preflight plan exceeds the byte limit")]
    TooLarge,
    /// JSON is malformed or contains unknown fields.
    #[error("invalid attached Ollama preflight plan JSON")]
    InvalidJson,
    /// Plan schema is unsupported.
    #[error("unsupported attached Ollama preflight plan schema")]
    UnsupportedSchema,
    /// Plan values or nested preflight are invalid.
    #[error("invalid attached Ollama preflight plan")]
    InvalidPlan,
    /// Native listener-owner observation failed closed.
    #[error("attached Ollama process witness failed: {0}")]
    Witness(#[source] AttachedProcessWitnessError),
    /// The read-only Ollama observation failed closed.
    #[error("attached Ollama API preflight failed: {0}")]
    Preflight(#[source] LocalOllamaPreflightError),
}

/// Parses and validates one byte-bounded attached-preflight plan.
///
/// # Errors
///
/// Returns [`LocalOllamaAttestedPreflightError`] for oversized, malformed,
/// unsupported, unbounded, or internally inconsistent input.
pub fn parse_local_ollama_attested_preflight_plan(
    bytes: &[u8],
) -> Result<LocalOllamaAttestedPreflightPlan, LocalOllamaAttestedPreflightError> {
    if bytes.len() > MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES {
        return Err(LocalOllamaAttestedPreflightError::TooLarge);
    }
    let plan: LocalOllamaAttestedPreflightPlan = serde_json::from_slice(bytes)
        .map_err(|_error| LocalOllamaAttestedPreflightError::InvalidJson)?;
    validate_plan(&plan)?;
    Ok(plan)
}

/// Runs native listener-owner observation around the existing read-only preflight.
///
/// The result remains inert and unqualified. It does not bind the HTTP responses
/// to the observed process and does not construct runtime-build or effective-state
/// identity.
///
/// # Errors
///
/// Returns [`LocalOllamaAttestedPreflightError`] for any invalid plan, native
/// observation failure, executable mismatch, API failure, or drift.
pub async fn run_local_ollama_attested_preflight(
    plan: &LocalOllamaAttestedPreflightPlan,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaAttestedPreflightReport, LocalOllamaAttestedPreflightError> {
    run_with_observer(plan, cancellation, &NativeAttachedProcessObserver).await
}

async fn run_with_observer<O: AttachedProcessObserver>(
    plan: &LocalOllamaAttestedPreflightPlan,
    cancellation: &CancellationToken,
    observer: &O,
) -> Result<LocalOllamaAttestedPreflightReport, LocalOllamaAttestedPreflightError> {
    validate_plan(plan)?;
    let endpoint = OllamaEndpoint::parse(&plan.preflight.endpoint)
        .map_err(|_error| LocalOllamaAttestedPreflightError::InvalidPlan)?;
    let endpoint = ListenerEndpoint::new(endpoint.socket_addr())
        .map_err(LocalOllamaAttestedPreflightError::Witness)?;
    let limits = AttachedProcessWitnessLimits {
        maximum_entrypoint_bytes: plan.maximum_entrypoint_bytes,
        ..AttachedProcessWitnessLimits::default()
    };
    let mut lease = observer
        .attach(endpoint, limits, cancellation)
        .map_err(LocalOllamaAttestedPreflightError::Witness)?;
    if plan
        .expected_entrypoint_digest
        .as_ref()
        .is_some_and(|expected| expected != lease.initial_evidence().entrypoint_digest())
    {
        return Err(LocalOllamaAttestedPreflightError::Witness(
            AttachedProcessWitnessError::EntrypointDigestMismatch,
        ));
    }
    let preflight = run_local_ollama_preflight(&plan.preflight, cancellation).await;
    let final_evidence = lease
        .reobserve(cancellation)
        .map_err(LocalOllamaAttestedPreflightError::Witness)?;
    let preflight = preflight.map_err(LocalOllamaAttestedPreflightError::Preflight)?;
    if plan
        .expected_entrypoint_digest
        .as_ref()
        .is_some_and(|expected| expected != final_evidence.entrypoint_digest())
    {
        return Err(LocalOllamaAttestedPreflightError::Witness(
            AttachedProcessWitnessError::EntrypointDigestMismatch,
        ));
    }
    let canonical = serde_json::to_vec(plan)
        .map_err(|_error| LocalOllamaAttestedPreflightError::InvalidPlan)?;
    Ok(LocalOllamaAttestedPreflightReport {
        schema_version: LOCAL_OLLAMA_ATTESTED_PREFLIGHT_REPORT_SCHEMA_VERSION,
        plan_digest: Digest::sha256(&canonical),
        process_evidence_level: LocalOllamaProcessEvidenceLevel::ObservedNativeListener,
        process_witness: final_evidence,
        preflight,
        response_bound: false,
        qualified: false,
    })
}

fn validate_plan(
    plan: &LocalOllamaAttestedPreflightPlan,
) -> Result<(), LocalOllamaAttestedPreflightError> {
    if plan.schema_version != LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_SCHEMA_VERSION {
        return Err(LocalOllamaAttestedPreflightError::UnsupportedSchema);
    }
    let nested = serde_json::to_vec(&plan.preflight)
        .map_err(|_error| LocalOllamaAttestedPreflightError::InvalidPlan)?;
    parse_local_ollama_preflight_plan(&nested)
        .map_err(|_error| LocalOllamaAttestedPreflightError::InvalidPlan)?;
    let digest_matches_mode = match plan.preflight.mode {
        LocalOllamaPreflightMode::Observe => plan.expected_entrypoint_digest.is_none(),
        LocalOllamaPreflightMode::Verify => plan.expected_entrypoint_digest.is_some(),
    };
    let limits = AttachedProcessWitnessLimits {
        maximum_entrypoint_bytes: plan.maximum_entrypoint_bytes,
        ..AttachedProcessWitnessLimits::default()
    };
    if !digest_matches_mode
        || limits.maximum_entrypoint_bytes == 0
        || limits.maximum_entrypoint_bytes > MAXIMUM_ENTRYPOINT_BYTES
    {
        return Err(LocalOllamaAttestedPreflightError::InvalidPlan);
    }
    Ok(())
}

#[cfg(test)]
#[path = "local_ollama_attested_preflight/tests.rs"]
mod tests;
