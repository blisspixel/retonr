//! Versioned, non-generative Ollama preflight for local development evidence.

use std::collections::BTreeSet;

use rewrite_inference::InferenceError;
use rewrite_inference::OperationContext;
use rewrite_ollama::{
    OllamaBackend, OllamaEndpoint, OllamaLimits, OllamaModelDetails, OllamaPreflight,
    OllamaPreflightTarget,
};
use rewrite_types::{CancellationToken, Digest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current local Ollama preflight plan contract version.
pub const LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION: u32 = 1;
/// Current local Ollama preflight report contract version.
pub const LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded plan bytes admitted by the preflight parser.
pub const MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES: usize = 64 * 1024;
/// Maximum exact runtime model bindings in one preflight.
pub const MAX_LOCAL_OLLAMA_MODELS: usize = 8;
const MAX_PLAN_LABEL_BYTES: usize = 64;
const MAX_RUNTIME_VERSION_BYTES: usize = 128;
const MAX_REFERENCE_BYTES: usize = 256;
const MAX_METADATA_BYTES: usize = 256;

/// Whether a preflight records a new observation or verifies frozen expected details.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaPreflightMode {
    /// Record bounded runtime evidence without treating it as a frozen verification.
    Observe,
    /// Require every observed model-description field and digest to match the plan.
    Verify,
}

/// One runtime-local model selection in a local preflight plan.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalOllamaModelPlan {
    /// Runtime-local mutable model reference used only as an address.
    pub reference: String,
    /// Exact Ollama inventory digest expected for that reference.
    pub inventory_digest: Digest,
    /// Frozen content-redacted model-description evidence required in verify mode.
    #[serde(default)]
    pub expected_details: Option<OllamaModelDetails>,
}

/// Bounded plan for one read-only Ollama preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalOllamaPreflightPlan {
    /// Plan contract version.
    pub schema_version: u32,
    /// Stable lowercase machine label.
    pub plan_id: String,
    /// Observation or frozen-verification mode.
    pub mode: LocalOllamaPreflightMode,
    /// Explicit IP-literal loopback endpoint.
    pub endpoint: String,
    /// Exact runtime-reported version expected before and after inspection.
    pub expected_runtime_version: String,
    /// Refuse success when any model is already resident.
    pub require_idle: bool,
    /// Canonically ordered exact runtime model bindings.
    pub models: Vec<LocalOllamaModelPlan>,
}

/// Redacted report from one stable read-only preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocalOllamaPreflightReport {
    /// Report contract version.
    pub schema_version: u32,
    /// Stable plan label.
    pub plan_id: String,
    /// Digest of the canonical parsed plan.
    pub plan_digest: Digest,
    /// Observation or verification mode.
    pub mode: LocalOllamaPreflightMode,
    /// Stable adapter evidence captured without generation.
    pub observed: OllamaPreflight,
    /// Always false. This development preflight cannot qualify a runtime or model.
    pub qualified: bool,
}

impl LocalOllamaPreflightReport {
    /// Returns whether this report verified frozen model details.
    #[must_use]
    pub const fn is_verified(&self) -> bool {
        matches!(self.mode, LocalOllamaPreflightMode::Verify)
    }
}

/// Opaque proof that the local Ollama preflight runner produced one exact report.
///
/// This capability is neither cloneable nor serializable. Its fields are private,
/// and it is issued only after the runner completes every bounded backend request
/// and report validation. Consuming code must still validate the plan and report.
#[derive(Debug, Eq, PartialEq)]
pub struct LocalOllamaPreflightExecutionReceipt {
    plan_digest: Digest,
    report_digest: Digest,
}

/// One executed local Ollama preflight report and its opaque provenance capability.
///
/// The outcome is neither cloneable nor serializable. Use [`Self::report`] to
/// inspect or serialize the unchanged v1 report, or [`Self::into_parts`] when an
/// exact downstream binding must consume the receipt.
#[derive(Debug, Eq, PartialEq)]
pub struct LocalOllamaPreflightExecutionOutcome {
    report: LocalOllamaPreflightReport,
    receipt: LocalOllamaPreflightExecutionReceipt,
}

impl LocalOllamaPreflightExecutionOutcome {
    /// Returns the unchanged v1 preflight report.
    #[must_use]
    pub const fn report(&self) -> &LocalOllamaPreflightReport {
        &self.report
    }

    /// Splits this single-use outcome into its report and provenance capability.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        LocalOllamaPreflightReport,
        LocalOllamaPreflightExecutionReceipt,
    ) {
        (self.report, self.receipt)
    }

    fn into_report(self) -> LocalOllamaPreflightReport {
        self.report
    }
}

impl LocalOllamaPreflightExecutionReceipt {
    fn issue(
        plan: &LocalOllamaPreflightPlan,
        report: &LocalOllamaPreflightReport,
    ) -> Result<Self, LocalOllamaPreflightError> {
        Ok(Self {
            plan_digest: execution_value_digest(
                b"retonr:local-ollama-preflight-execution-plan:v1\0",
                plan,
            )?,
            report_digest: execution_value_digest(
                b"retonr:local-ollama-preflight-execution-report:v1\0",
                report,
            )?,
        })
    }

    pub(crate) fn validates(
        &self,
        plan: &LocalOllamaPreflightPlan,
        report: &LocalOllamaPreflightReport,
    ) -> Result<bool, LocalOllamaPreflightError> {
        Ok(self.plan_digest
            == execution_value_digest(b"retonr:local-ollama-preflight-execution-plan:v1\0", plan)?
            && self.report_digest
                == execution_value_digest(
                    b"retonr:local-ollama-preflight-execution-report:v1\0",
                    report,
                )?)
    }
}

/// Local preflight plan, transport, or evidence mismatch.
#[derive(Debug, Error)]
pub enum LocalOllamaPreflightError {
    /// Encoded plan exceeds the fixed parser ceiling.
    #[error("local Ollama preflight plan exceeds the byte limit")]
    TooLarge,
    /// Plan JSON is malformed or contains unknown fields.
    #[error("invalid local Ollama preflight plan JSON")]
    InvalidJson,
    /// Plan version is not supported.
    #[error("unsupported local Ollama preflight plan schema")]
    UnsupportedSchema,
    /// Plan values or ordering violate the bounded contract.
    #[error("invalid local Ollama preflight plan")]
    InvalidPlan,
    /// Endpoint is not an explicit IP-literal loopback URL.
    #[error("invalid local Ollama preflight endpoint")]
    InvalidEndpoint,
    /// Adapter setup or a bounded read-only request failed.
    #[error("local Ollama preflight request failed: {0}")]
    Backend(#[source] InferenceError),
    /// Runtime version differs from the frozen plan.
    #[error("local Ollama runtime version does not match the plan")]
    RuntimeVersionMismatch,
    /// Verify mode observed model-description evidence that differs from the plan.
    #[error("local Ollama model details do not match the plan")]
    ModelDetailsMismatch,
    /// The plan required an idle runtime but a model was resident.
    #[error("local Ollama preflight requires an idle runtime")]
    RuntimeNotIdle,
}

/// Parses and validates one byte-bounded local Ollama preflight plan.
///
/// # Errors
///
/// Returns [`LocalOllamaPreflightError`] for an oversized, malformed, unsupported,
/// unbounded, noncanonical, or internally inconsistent plan.
pub fn parse_local_ollama_preflight_plan(
    bytes: &[u8],
) -> Result<LocalOllamaPreflightPlan, LocalOllamaPreflightError> {
    if bytes.len() > MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES {
        return Err(LocalOllamaPreflightError::TooLarge);
    }
    let plan: LocalOllamaPreflightPlan =
        serde_json::from_slice(bytes).map_err(|_error| LocalOllamaPreflightError::InvalidJson)?;
    validate_local_ollama_preflight_plan(&plan)?;
    Ok(plan)
}

/// Executes a non-generative, read-only Ollama preflight.
///
/// This operation never loads or runs a model. It cannot produce qualification
/// evidence because an Ollama inventory digest is not a complete artifact-set or
/// runtime-build identity.
///
/// # Errors
///
/// Returns [`LocalOllamaPreflightError`] when the plan, endpoint, bounded adapter
/// requests, runtime version, model details, or idle requirement fail closed.
pub async fn run_local_ollama_preflight(
    plan: &LocalOllamaPreflightPlan,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaPreflightReport, LocalOllamaPreflightError> {
    run_local_ollama_preflight_with_receipt(plan, cancellation)
        .await
        .map(LocalOllamaPreflightExecutionOutcome::into_report)
}

/// Executes a non-generative local Ollama preflight and retains runner provenance.
///
/// The returned receipt is a single-use, in-process capability for joining this
/// exact plan and report to stronger static package evidence. It does not prove
/// model loading, model use, handler execution, effective identity, or qualification.
///
/// # Errors
///
/// Returns [`LocalOllamaPreflightError`] when the plan, endpoint, bounded adapter
/// requests, runtime version, model details, idle requirement, or receipt encoding
/// fail closed.
pub async fn run_local_ollama_preflight_with_receipt(
    plan: &LocalOllamaPreflightPlan,
    cancellation: &CancellationToken,
) -> Result<LocalOllamaPreflightExecutionOutcome, LocalOllamaPreflightError> {
    validate_local_ollama_preflight_plan(plan)?;
    let endpoint = OllamaEndpoint::parse(&plan.endpoint)
        .map_err(|_error| LocalOllamaPreflightError::InvalidEndpoint)?;
    let targets = local_ollama_preflight_targets(plan)?;
    let backend = OllamaBackend::new_preflight(endpoint, targets, OllamaLimits::default())
        .map_err(LocalOllamaPreflightError::Backend)?;
    let observed = backend
        .preflight(OperationContext::new(cancellation, None))
        .await
        .map_err(LocalOllamaPreflightError::Backend)?;
    let report = local_ollama_preflight_report(plan, observed)?;
    let receipt = LocalOllamaPreflightExecutionReceipt::issue(plan, &report)?;
    Ok(LocalOllamaPreflightExecutionOutcome { report, receipt })
}

#[cfg(test)]
pub(crate) fn issue_local_ollama_preflight_test_receipt(
    plan: &LocalOllamaPreflightPlan,
    report: &LocalOllamaPreflightReport,
) -> Result<LocalOllamaPreflightExecutionReceipt, LocalOllamaPreflightError> {
    LocalOllamaPreflightExecutionReceipt::issue(plan, report)
}

pub(crate) fn local_ollama_preflight_targets(
    plan: &LocalOllamaPreflightPlan,
) -> Result<Vec<OllamaPreflightTarget>, LocalOllamaPreflightError> {
    plan.models
        .iter()
        .map(|model| {
            OllamaPreflightTarget::new(model.reference.clone(), model.inventory_digest.clone())
                .map_err(|_error| LocalOllamaPreflightError::InvalidPlan)
        })
        .collect()
}

pub(crate) fn local_ollama_preflight_report(
    plan: &LocalOllamaPreflightPlan,
    observed: OllamaPreflight,
) -> Result<LocalOllamaPreflightReport, LocalOllamaPreflightError> {
    if observed.runtime.version != plan.expected_runtime_version {
        return Err(LocalOllamaPreflightError::RuntimeVersionMismatch);
    }
    if plan.require_idle && !observed.running.is_empty() {
        return Err(LocalOllamaPreflightError::RuntimeNotIdle);
    }
    if matches!(plan.mode, LocalOllamaPreflightMode::Verify)
        && !plan
            .models
            .iter()
            .zip(&observed.bindings)
            .all(|(expected, actual)| {
                expected.reference == actual.reference
                    && expected
                        .expected_details
                        .as_ref()
                        .is_some_and(|details| details == &actual.details)
            })
    {
        return Err(LocalOllamaPreflightError::ModelDetailsMismatch);
    }
    let canonical =
        serde_json::to_vec(plan).map_err(|_error| LocalOllamaPreflightError::InvalidPlan)?;
    Ok(LocalOllamaPreflightReport {
        schema_version: LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        plan_digest: Digest::sha256(&canonical),
        mode: plan.mode,
        observed,
        qualified: false,
    })
}

pub(crate) fn validate_local_ollama_preflight_plan(
    plan: &LocalOllamaPreflightPlan,
) -> Result<(), LocalOllamaPreflightError> {
    if plan.schema_version != LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION {
        return Err(LocalOllamaPreflightError::UnsupportedSchema);
    }
    if !valid_label(&plan.plan_id)
        || !valid_text(&plan.expected_runtime_version, MAX_RUNTIME_VERSION_BYTES)
        || plan.models.is_empty()
        || plan.models.len() > MAX_LOCAL_OLLAMA_MODELS
    {
        return Err(LocalOllamaPreflightError::InvalidPlan);
    }
    OllamaEndpoint::parse(&plan.endpoint)
        .map_err(|_error| LocalOllamaPreflightError::InvalidEndpoint)?;
    let mut references = BTreeSet::new();
    let mut digests = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for model in &plan.models {
        if !valid_text(&model.reference, MAX_REFERENCE_BYTES)
            || !references.insert(model.reference.as_str())
            || !digests.insert(model.inventory_digest.as_str())
            || previous.is_some_and(|prior| prior.as_bytes() >= model.reference.as_bytes())
            || !valid_expected_details(model.expected_details.as_ref())
        {
            return Err(LocalOllamaPreflightError::InvalidPlan);
        }
        previous = Some(&model.reference);
    }
    let details_match_mode = match plan.mode {
        LocalOllamaPreflightMode::Observe => plan
            .models
            .iter()
            .all(|model| model.expected_details.is_none()),
        LocalOllamaPreflightMode::Verify => plan
            .models
            .iter()
            .all(|model| model.expected_details.is_some()),
    };
    if !details_match_mode {
        return Err(LocalOllamaPreflightError::InvalidPlan);
    }
    Ok(())
}

fn valid_expected_details(details: Option<&OllamaModelDetails>) -> bool {
    let Some(details) = details else {
        return true;
    };
    valid_text(&details.format, MAX_METADATA_BYTES)
        && valid_text(&details.family, MAX_METADATA_BYTES)
        && details.quantization.len() <= MAX_METADATA_BYTES
        && !details.quantization.chars().any(char::is_control)
        && details.capabilities.len() <= 64
        && details
            .capabilities
            .iter()
            .all(|value| valid_text(value, MAX_METADATA_BYTES))
        && details
            .capabilities
            .windows(2)
            .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PLAN_LABEL_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn execution_value_digest(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<Digest, LocalOllamaPreflightError> {
    let encoded =
        serde_json::to_vec(value).map_err(|_error| LocalOllamaPreflightError::InvalidPlan)?;
    let mut bytes = Vec::with_capacity(domain.len().saturating_add(encoded.len()));
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&encoded);
    Ok(Digest::sha256(&bytes))
}

#[cfg(test)]
#[path = "local_ollama_preflight/tests.rs"]
mod tests;
