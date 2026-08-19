use rewrite_app::{AppError, CandidateCheckRequest, CandidateCheckService};
use rewrite_inference::{
    GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, InferenceBackend, InferenceErrorKind,
    OperationContext, OutputContract, ReasoningPolicy, SamplingParameters,
};
use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::{CancellationToken, Digest, ReasonCode, RewriteStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{EvaluationCase, EvaluationSuite};

mod offline;

pub use offline::{MAX_BASELINE_DEFINITION_BYTES, parse_baseline_definition, run_offline_baseline};

/// Current baseline-runner contract version.
pub const BASELINE_SCHEMA_VERSION: u32 = 1;
const MAX_BASELINE_TEXT_BYTES: usize = 16 * 1024;
const MAX_RETRIEVED_EXAMPLES: usize = 8;
const MAX_RETRIEVED_EXAMPLE_BYTES: usize = 4 * 1024;
const MAX_RETRIEVED_TOTAL_BYTES: usize = 16 * 1024;

/// Baseline strategy compared with the product path.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineKind {
    /// Return the exact source without inference.
    NoRewrite,
    /// Ask the backend to rewrite from a direct instruction only.
    DirectPrompt,
    /// Add a fixed style description to the direct instruction.
    StyleDescription,
    /// Add an explicit bounded set of retrieved examples.
    RetrievedExamples,
}

/// Exact inference policy shared by every generative baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineInferencePolicy {
    /// Qualified artifact selected for the run.
    pub artifact_id: ArtifactId,
    /// Artifact digest rechecked around every request.
    pub artifact_digest: Digest,
    /// Versioned prompt template.
    pub prompt_template: String,
    /// Digest of the exact prompt template.
    pub prompt_template_digest: Digest,
    /// Structured-output contract passed to the backend.
    pub output: OutputContract,
    /// Qualified source-byte envelope.
    pub source_byte_limit: u64,
    /// Maximum complete serialized prompt bytes accepted by request policy.
    pub input_byte_limit: u64,
    /// Qualified context envelope.
    pub context_token_limit: u32,
    /// Maximum generated tokens requested from the backend.
    pub output_token_limit: u32,
    /// Maximum candidate bytes accepted from the backend.
    pub candidate_byte_limit: u64,
    /// Explicit sampling policy.
    pub sampling: SamplingParameters,
    /// Explicit reasoning-output policy.
    pub reasoning: ReasoningPolicy,
}

/// Versioned baseline definition with explicit style and retrieval inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineDefinition {
    /// Baseline contract version.
    pub schema_version: u32,
    /// Stable machine label for this baseline configuration.
    pub id: String,
    /// Strategy being evaluated.
    pub kind: BaselineKind,
    /// Inference policy for generative strategies.
    #[serde(default)]
    pub inference: Option<BaselineInferencePolicy>,
    /// Explicit fixed style description, if this baseline uses one.
    #[serde(default)]
    pub style_description: Option<String>,
    /// Explicit retrieved examples, if this baseline uses them.
    #[serde(default)]
    pub retrieved_examples: Vec<String>,
}

impl BaselineDefinition {
    fn validate(&self) -> Result<(), BaselineError> {
        if self.schema_version != BASELINE_SCHEMA_VERSION {
            return Err(BaselineError::UnsupportedSchema);
        }
        if !valid_label(&self.id) {
            return Err(BaselineError::InvalidIdentifier);
        }
        match self.kind {
            BaselineKind::NoRewrite => {
                if self.inference.is_some()
                    || self.style_description.is_some()
                    || !self.retrieved_examples.is_empty()
                {
                    return Err(BaselineError::InvalidConfiguration);
                }
            }
            BaselineKind::DirectPrompt => {
                if self.inference.is_none()
                    || self.style_description.is_some()
                    || !self.retrieved_examples.is_empty()
                {
                    return Err(BaselineError::InvalidConfiguration);
                }
            }
            BaselineKind::StyleDescription => {
                if self.inference.is_none()
                    || !valid_optional_text(self.style_description.as_deref())
                    || !self.retrieved_examples.is_empty()
                {
                    return Err(BaselineError::InvalidConfiguration);
                }
            }
            BaselineKind::RetrievedExamples => {
                if self.inference.is_none()
                    || self.style_description.is_some()
                    || !valid_examples(&self.retrieved_examples)
                {
                    return Err(BaselineError::InvalidConfiguration);
                }
            }
        }
        if let Some(policy) = &self.inference {
            validate_inference_policy(policy)?;
        }
        Ok(())
    }
}

/// Aggregate status counts for one baseline run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BaselineStatusCounts {
    /// Cases returning a validated change.
    pub rewritten: usize,
    /// Cases returning the exact source without abstention.
    pub unchanged: usize,
    /// Cases safely retaining the source after policy abstention.
    pub abstained: usize,
    /// Cases failing before a safe transaction outcome.
    pub failed: usize,
}

/// Redacted per-case baseline failure category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineCaseError {
    /// Backend request failed.
    Backend,
    /// Backend response identity or candidate contract was malformed.
    MalformedResponse,
    /// Candidate-check application service failed operationally.
    Application,
}

/// Redacted result for one baseline case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BaselineCaseResult {
    /// Stable evaluation case identifier.
    pub id: String,
    /// Stable evaluation category.
    pub category: String,
    /// Source digest without raw content.
    pub source_digest: Digest,
    /// Returned output digest, if a transaction completed.
    pub output_digest: Option<Digest>,
    /// Transaction status, if a transaction completed.
    pub status: Option<RewriteStatus>,
    /// Stable abstention reason, if present.
    pub reason: Option<ReasonCode>,
    /// Stable operational failure category.
    pub error: Option<BaselineCaseError>,
}

/// Redacted aggregate report for one baseline definition and suite.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BaselineReport {
    /// Baseline report schema version.
    pub schema_version: u32,
    /// Stable baseline configuration identifier.
    pub baseline_id: String,
    /// Baseline strategy.
    pub kind: BaselineKind,
    /// Runtime observed during discovery for a generative baseline.
    pub runtime: Option<RuntimeIdentity>,
    /// Exact artifact selected for a generative baseline.
    pub artifact_id: Option<ArtifactId>,
    /// Total cases attempted.
    pub total: usize,
    /// Aggregate transaction status counts.
    pub statuses: BaselineStatusCounts,
    /// Redacted case results.
    pub cases: Vec<BaselineCaseResult>,
}

impl BaselineReport {
    /// Returns whether every case completed without an operational failure.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.statuses.failed == 0
    }
}

/// Baseline definition or setup failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BaselineError {
    /// Baseline schema is unsupported.
    #[error("unsupported baseline schema")]
    UnsupportedSchema,
    /// Baseline identifier is not a bounded lowercase machine label.
    #[error("invalid baseline identifier")]
    InvalidIdentifier,
    /// Baseline kind, style description, retrieval input, or inference selection is
    /// inconsistent.
    #[error("invalid baseline configuration")]
    InvalidConfiguration,
    /// Generative baseline requires a backend.
    #[error("generative baseline requires an inference backend")]
    MissingBackend,
    /// Backend discovery failed.
    #[error("baseline backend discovery failed")]
    Discovery,
    /// Selected artifact or capability was not present in discovery.
    #[error("baseline artifact is not available for structured generation")]
    ArtifactUnavailable,
    /// Serialized definition exceeds the supported byte limit.
    #[error("baseline definition exceeds the supported byte limit")]
    TooLarge,
    /// Definition JSON is invalid or contains an unknown field.
    #[error("invalid baseline definition")]
    InvalidJson,
}

/// Runs one baseline over a validated evaluation suite.
///
/// Reports contain digests, status, and safe categories only. Raw source, prompt,
/// retrieved examples, and candidates are not serialized.
///
/// # Errors
///
/// Returns [`BaselineError`] for invalid setup or failed runtime discovery. Per-case
/// generation and application failures remain redacted case results.
pub async fn run_baseline(
    definition: &BaselineDefinition,
    suite: &EvaluationSuite,
    backend: Option<&dyn InferenceBackend>,
    cancellation: &CancellationToken,
) -> Result<BaselineReport, BaselineError> {
    definition.validate()?;
    if definition.kind == BaselineKind::NoRewrite {
        return Ok(run_no_rewrite(definition, suite));
    }
    let backend = backend.ok_or(BaselineError::MissingBackend)?;
    let context = OperationContext::new(cancellation, None);
    let discovery = backend
        .discover(context)
        .await
        .map_err(|_error| BaselineError::Discovery)?;
    let policy = definition
        .inference
        .as_ref()
        .ok_or(BaselineError::InvalidConfiguration)?;
    let artifact_available = discovery.inventory.iter().any(|entry| {
        entry.artifact_id == policy.artifact_id && entry.artifact_digest == policy.artifact_digest
    });
    if !artifact_available
        || !discovery
            .capabilities
            .roles
            .contains(&ArtifactRole::Generation)
        || discovery.capabilities.validate().is_err()
        || !discovery.capabilities.admits_output(&policy.output)
    {
        return Err(BaselineError::ArtifactUnavailable);
    }

    let mut statuses = BaselineStatusCounts::default();
    let mut cases = Vec::with_capacity(suite.cases.len());
    for case in &suite.cases {
        let result = run_generative_case(definition, policy, case, backend, cancellation).await;
        update_counts(&mut statuses, &result);
        cases.push(result);
    }
    Ok(BaselineReport {
        schema_version: BASELINE_SCHEMA_VERSION,
        baseline_id: definition.id.clone(),
        kind: definition.kind,
        runtime: Some(discovery.runtime),
        artifact_id: Some(policy.artifact_id.clone()),
        total: cases.len(),
        statuses,
        cases,
    })
}

fn run_no_rewrite(definition: &BaselineDefinition, suite: &EvaluationSuite) -> BaselineReport {
    let cases = suite
        .cases
        .iter()
        .map(|case| {
            let digest = Digest::sha256(case.source.as_bytes());
            BaselineCaseResult {
                id: case.id.clone(),
                category: case.category.clone(),
                source_digest: digest.clone(),
                output_digest: Some(digest),
                status: Some(RewriteStatus::UnchangedNoEligibleContent),
                reason: None,
                error: None,
            }
        })
        .collect::<Vec<_>>();
    BaselineReport {
        schema_version: BASELINE_SCHEMA_VERSION,
        baseline_id: definition.id.clone(),
        kind: definition.kind,
        runtime: None,
        artifact_id: None,
        total: cases.len(),
        statuses: BaselineStatusCounts {
            unchanged: cases.len(),
            ..BaselineStatusCounts::default()
        },
        cases,
    }
}

async fn run_generative_case(
    definition: &BaselineDefinition,
    policy: &BaselineInferencePolicy,
    case: &EvaluationCase,
    backend: &dyn InferenceBackend,
    cancellation: &CancellationToken,
) -> BaselineCaseResult {
    let source_digest = Digest::sha256(case.source.as_bytes());
    let input = render_input(definition, policy, case);
    let request = GenerationRequest {
        schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
        artifact_id: policy.artifact_id.clone(),
        artifact_digest: policy.artifact_digest.clone(),
        input,
        output: policy.output.clone(),
        candidate_count: 1,
        source_byte_count: u64::try_from(case.source.len()).unwrap_or(u64::MAX),
        source_byte_limit: policy.source_byte_limit,
        input_byte_limit: policy.input_byte_limit,
        context_token_limit: policy.context_token_limit,
        output_token_limit: policy.output_token_limit,
        candidate_byte_limit: policy.candidate_byte_limit,
        sampling: policy.sampling,
        reasoning: policy.reasoning,
    };
    if request.validate().is_err() {
        return failed_case(case, source_digest, BaselineCaseError::Application);
    }
    let response = match backend
        .generate(request, OperationContext::new(cancellation, None))
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let category = match error.kind {
                InferenceErrorKind::MalformedResponse => BaselineCaseError::MalformedResponse,
                _ => BaselineCaseError::Backend,
            };
            return failed_case(case, source_digest, category);
        }
    };
    if response.artifact_id != policy.artifact_id
        || response.artifact_digest != policy.artifact_digest
        || response.candidates.len() != 1
        || response.candidates[0].ordinal != 0
        || u64::try_from(response.candidates[0].text.len()).unwrap_or(u64::MAX)
            > policy.candidate_byte_limit
    {
        return failed_case(case, source_digest, BaselineCaseError::MalformedResponse);
    }
    match CandidateCheckService::check(CandidateCheckRequest {
        source: case.source.as_bytes().to_vec(),
        candidate: response.candidates[0].text.clone(),
        protected_terms: case.protected_terms.clone(),
    }) {
        Ok(result) => BaselineCaseResult {
            id: case.id.clone(),
            category: case.category.clone(),
            source_digest,
            output_digest: Some(result.record.output_digest),
            status: Some(result.record.status),
            reason: result.record.reason,
            error: None,
        },
        Err(error) => failed_case(case, source_digest, map_app_error(&error)),
    }
}

fn render_input(
    definition: &BaselineDefinition,
    policy: &BaselineInferencePolicy,
    case: &EvaluationCase,
) -> String {
    let style = definition.style_description.as_deref().unwrap_or("");
    let examples = definition.retrieved_examples.join("\n---\n");
    format!(
        "{}\n<source>\n{}\n</source>\n<style>\n{}\n</style>\n<examples>\n{}\n</examples>",
        policy.prompt_template, case.source, style, examples
    )
}

fn failed_case(
    case: &EvaluationCase,
    source_digest: Digest,
    error: BaselineCaseError,
) -> BaselineCaseResult {
    BaselineCaseResult {
        id: case.id.clone(),
        category: case.category.clone(),
        source_digest,
        output_digest: None,
        status: None,
        reason: None,
        error: Some(error),
    }
}

fn update_counts(counts: &mut BaselineStatusCounts, result: &BaselineCaseResult) {
    match result.status {
        Some(RewriteStatus::Rewritten) => counts.rewritten += 1,
        Some(RewriteStatus::UnchangedNoEligibleContent) => counts.unchanged += 1,
        Some(RewriteStatus::Abstained) => counts.abstained += 1,
        Some(RewriteStatus::Failed) | None => counts.failed += 1,
    }
}

const fn map_app_error(_error: &AppError) -> BaselineCaseError {
    BaselineCaseError::Application
}

fn validate_inference_policy(policy: &BaselineInferencePolicy) -> Result<(), BaselineError> {
    if policy.artifact_id.digest() != &policy.artifact_digest
        || !valid_text(&policy.prompt_template, MAX_BASELINE_TEXT_BYTES)
        || Digest::sha256(policy.prompt_template.as_bytes()) != policy.prompt_template_digest
        || policy.source_byte_limit == 0
        || policy.input_byte_limit == 0
        || policy.context_token_limit == 0
        || policy.output_token_limit == 0
        || policy.candidate_byte_limit == 0
    {
        return Err(BaselineError::InvalidConfiguration);
    }
    Ok(())
}

fn valid_optional_text(value: Option<&str>) -> bool {
    value.is_some_and(|value| valid_text(value, MAX_BASELINE_TEXT_BYTES))
}

fn valid_examples(values: &[String]) -> bool {
    if values.is_empty() || values.len() > MAX_RETRIEVED_EXAMPLES {
        return false;
    }
    let mut total = 0_usize;
    for value in values {
        if !valid_text(value, MAX_RETRIEVED_EXAMPLE_BYTES) {
            return false;
        }
        let Some(next) = total.checked_add(value.len()) else {
            return false;
        };
        total = next;
    }
    total <= MAX_RETRIEVED_TOTAL_BYTES
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

#[cfg(test)]
mod tests;
