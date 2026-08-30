use std::{
    error::Error,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

use rewrite_inference::StructuredCompletionResponse;
use rewrite_model::OllamaRetainedSessionResponseId;
use rewrite_types::Digest;

use super::{
    derive_ollama_retained_session_response_id, receipt::OllamaResidentSessionExecutionReceipt,
    transport::ResponseHeadCheckpoint,
};

const GENERATION_RESPONSE_OFFSET: usize = 3;

mod subject;

pub(super) use subject::OllamaRetainedSessionSubject;
pub use subject::OllamaRetainedSessionSubjectToken;
use subject::RetainedSessionSubjectMarker;

/// Content-free failure while validating an Ollama resource observation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OllamaGenerateResourceObservationError {
    kind: ErrorKind,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ErrorKind {
    MissingProviderValue,
    BindingMismatch,
    ReversedMonotonicTime,
}

impl OllamaGenerateResourceObservationError {
    const fn missing_provider_value() -> Self {
        Self {
            kind: ErrorKind::MissingProviderValue,
        }
    }

    const fn binding_mismatch() -> Self {
        Self {
            kind: ErrorKind::BindingMismatch,
        }
    }

    const fn reversed_monotonic_time() -> Self {
        Self {
            kind: ErrorKind::ReversedMonotonicTime,
        }
    }
}

impl fmt::Debug for OllamaGenerateResourceObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OllamaGenerateResourceObservationError")
    }
}

impl fmt::Display for OllamaGenerateResourceObservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Ollama resource observation failed")
    }
}

impl Error for OllamaGenerateResourceObservationError {}

pub(super) struct GenerateProviderResourceValues {
    total_duration_nanoseconds: u64,
    load_duration_nanoseconds: u64,
    prompt_token_count: u64,
    prompt_evaluation_duration_nanoseconds: u64,
    generated_token_count: u64,
    evaluation_duration_nanoseconds: u64,
}

#[derive(serde::Deserialize)]
struct GenerateResourceTelemetry {
    total_duration: u64,
    load_duration: u64,
    prompt_eval_count: u64,
    prompt_eval_duration: u64,
    eval_count: u64,
    eval_duration: u64,
}

impl GenerateProviderResourceValues {
    pub(super) fn from_body(body: &[u8]) -> Result<Self, OllamaGenerateResourceObservationError> {
        let response: GenerateResourceTelemetry = serde_json::from_slice(body)
            .map_err(|_error| OllamaGenerateResourceObservationError::missing_provider_value())?;
        Ok(Self {
            total_duration_nanoseconds: response.total_duration,
            load_duration_nanoseconds: response.load_duration,
            prompt_token_count: response.prompt_eval_count,
            prompt_evaluation_duration_nanoseconds: response.prompt_eval_duration,
            generated_token_count: response.eval_count,
            evaluation_duration_nanoseconds: response.eval_duration,
        })
    }
}

/// Exact provider-reported resource values for one retained completion.
///
/// The observation is available only from the explicit retained resident
/// resource-evidence path. It binds the exact structured request identity,
/// final response identity, generation response ordinal, response-head
/// checkpoint, and six provider values. Provider values are observations, not
/// independently attested resource use or qualification authority.
///
/// This type cannot be cloned or serialized and its debug representation is
/// content-redacted.
///
/// ```compile_fail
/// fn clone_observation(value: rewrite_ollama::OllamaGenerateResourceObservation) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn serialize_observation(value: &rewrite_ollama::OllamaGenerateResourceObservation) {
///     let _json = serde_json::to_string(value).unwrap();
/// }
/// ```
pub struct OllamaGenerateResourceObservation {
    session_subject: Arc<RetainedSessionSubjectMarker>,
    request_binding_digest: Digest,
    response_id: OllamaRetainedSessionResponseId,
    response_ordinal: usize,
    response_head: Instant,
    values: GenerateProviderResourceValues,
}

impl OllamaGenerateResourceObservation {
    pub(super) fn new(
        session_subject: &OllamaRetainedSessionSubject,
        response: &StructuredCompletionResponse,
        checkpoint: ResponseHeadCheckpoint,
        values: GenerateProviderResourceValues,
    ) -> Self {
        Self {
            session_subject: Arc::clone(&session_subject.marker),
            request_binding_digest: response.request_binding_digest().clone(),
            response_id: derive_ollama_retained_session_response_id(response),
            response_ordinal: checkpoint.ordinal,
            response_head: checkpoint.at,
            values,
        }
    }

    /// Returns the exact structured request binding used by the session.
    #[must_use]
    pub const fn request_binding_digest(&self) -> &Digest {
        &self.request_binding_digest
    }

    /// Returns the identity of the exact final structured response.
    #[must_use]
    pub const fn response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.response_id
    }

    /// Returns the one-based ordinal assigned to the generate response head.
    #[must_use]
    pub const fn response_ordinal(&self) -> usize {
        self.response_ordinal
    }

    /// Verifies the exact final response and reviewed generate ordinal binding.
    ///
    /// # Errors
    ///
    /// Returns a content-free error when either identity or the ordinal differs
    /// from the retained completion that produced this observation.
    pub fn verify_completion_binding(
        &self,
        response: &StructuredCompletionResponse,
        response_ordinal: usize,
    ) -> Result<(), OllamaGenerateResourceObservationError> {
        if self.request_binding_digest != *response.request_binding_digest()
            || self.response_id != derive_ollama_retained_session_response_id(response)
            || self.response_ordinal != response_ordinal
        {
            return Err(OllamaGenerateResourceObservationError::binding_mismatch());
        }
        Ok(())
    }

    /// Returns elapsed time from a caller-owned start to the response head.
    ///
    /// # Errors
    ///
    /// Returns a content-free error when `start` is after the retained response
    /// head checkpoint.
    pub fn response_head_elapsed_since(
        &self,
        start: Instant,
    ) -> Result<Duration, OllamaGenerateResourceObservationError> {
        checked_elapsed(self.response_head, start)
    }

    /// Returns Ollama's exact total duration in nanoseconds.
    #[must_use]
    pub const fn total_duration_nanoseconds(&self) -> u64 {
        self.values.total_duration_nanoseconds
    }

    /// Returns Ollama's exact model-load duration in nanoseconds.
    #[must_use]
    pub const fn load_duration_nanoseconds(&self) -> u64 {
        self.values.load_duration_nanoseconds
    }

    /// Returns Ollama's exact evaluated prompt token count.
    #[must_use]
    pub const fn prompt_token_count(&self) -> u64 {
        self.values.prompt_token_count
    }

    /// Returns Ollama's exact prompt-evaluation duration in nanoseconds.
    #[must_use]
    pub const fn prompt_evaluation_duration_nanoseconds(&self) -> u64 {
        self.values.prompt_evaluation_duration_nanoseconds
    }

    /// Returns Ollama's exact generated token count.
    #[must_use]
    pub const fn generated_token_count(&self) -> u64 {
        self.values.generated_token_count
    }

    /// Returns Ollama's exact generation duration in nanoseconds.
    #[must_use]
    pub const fn evaluation_duration_nanoseconds(&self) -> u64 {
        self.values.evaluation_duration_nanoseconds
    }
}

impl fmt::Debug for OllamaGenerateResourceObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OllamaGenerateResourceObservation")
            .field("content", &"redacted")
            .finish()
    }
}

/// One inseparable retained completion with exact provider resource evidence.
///
/// This value owns the structured response, resident execution receipt, and
/// resource observation produced by one retained session. Callers can inspect
/// them but cannot extract, replace, clone, or serialize any constituent. This
/// prevents valid portable values from being recombined across two live
/// retained sessions before the trusted consumer joins the evidence.
///
/// ```compile_fail
/// fn clone_completion(value: rewrite_ollama::OllamaResidentResourceObservedCompletion) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// fn serialize_completion(value: &rewrite_ollama::OllamaResidentResourceObservedCompletion) {
///     let _json = serde_json::to_string(value).unwrap();
/// }
/// ```
///
/// ```compile_fail
/// fn recombine_sessions(
///     first: rewrite_ollama::OllamaResidentResourceObservedCompletion,
///     second: rewrite_ollama::OllamaResidentResourceObservedCompletion,
/// ) {
///     let rewrite_ollama::OllamaResidentResourceObservedCompletion {
///         response, receipt, ..
///     } = first;
///     let _mixed = rewrite_ollama::OllamaResidentResourceObservedCompletion {
///         response,
///         receipt,
///         observation: second.observation,
///     };
/// }
/// ```
pub struct OllamaResidentResourceObservedCompletion {
    response: StructuredCompletionResponse,
    receipt: OllamaResidentSessionExecutionReceipt,
    observation: OllamaGenerateResourceObservation,
    session_subject: Arc<RetainedSessionSubjectMarker>,
}

impl OllamaResidentResourceObservedCompletion {
    pub(super) fn new(
        response: StructuredCompletionResponse,
        receipt: OllamaResidentSessionExecutionReceipt,
        observation: OllamaGenerateResourceObservation,
    ) -> Result<Self, OllamaGenerateResourceObservationError> {
        let session_subject = Arc::clone(&observation.session_subject);
        let completion = Self {
            response,
            receipt,
            observation,
            session_subject,
        };
        completion.verify_completion_binding()?;
        Ok(completion)
    }

    /// Constructs one inert owned completion for downstream contract tests.
    ///
    /// This helper is absent unless the explicit `test-support` feature is
    /// enabled. It creates no connection, execution authority, resource
    /// authority, or qualification authority.
    ///
    /// # Errors
    ///
    /// Returns a content-free error when the response, receipt, or reviewed
    /// generate ordinal do not bind exactly.
    #[cfg(feature = "test-support")]
    #[expect(
        clippy::too_many_arguments,
        reason = "the test seam keeps all exact provider values explicit"
    )]
    pub fn for_test(
        response: StructuredCompletionResponse,
        receipt: OllamaResidentSessionExecutionReceipt,
        response_ordinal: usize,
        response_head: Instant,
        total_duration_nanoseconds: u64,
        load_duration_nanoseconds: u64,
        prompt_token_count: u64,
        prompt_evaluation_duration_nanoseconds: u64,
        generated_token_count: u64,
        evaluation_duration_nanoseconds: u64,
    ) -> Result<Self, OllamaGenerateResourceObservationError> {
        let observation = OllamaGenerateResourceObservation::new(
            &OllamaRetainedSessionSubject::new(),
            &response,
            ResponseHeadCheckpoint {
                at: response_head,
                ordinal: response_ordinal,
            },
            GenerateProviderResourceValues {
                total_duration_nanoseconds,
                load_duration_nanoseconds,
                prompt_token_count,
                prompt_evaluation_duration_nanoseconds,
                generated_token_count,
                evaluation_duration_nanoseconds,
            },
        );
        Self::new(response, receipt, observation)
    }

    /// Returns the exact structured completion response owned by this evidence.
    #[must_use]
    pub const fn response(&self) -> &StructuredCompletionResponse {
        &self.response
    }

    /// Returns the exact resident execution receipt owned by this evidence.
    #[must_use]
    pub const fn resident_execution_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        &self.receipt
    }

    /// Returns a cloneable token for this exact ephemeral retained session.
    ///
    /// The token carries no execution, resource, model-use, or qualification
    /// authority. It exists only for an in-process trusted consumer to retain
    /// and later compare with this inseparable completion.
    #[must_use]
    pub fn retained_session_subject_token(&self) -> OllamaRetainedSessionSubjectToken {
        OllamaRetainedSessionSubjectToken {
            marker: Arc::clone(&self.session_subject),
        }
    }

    /// Returns whether `token` identifies this exact retained session subject.
    ///
    /// This process-local pointer identity check proves no execution, resource
    /// accuracy, model use, runtime state, persistence, or qualification fact.
    #[must_use]
    pub fn binds_retained_session_subject(
        &self,
        token: &OllamaRetainedSessionSubjectToken,
    ) -> bool {
        Arc::ptr_eq(&self.session_subject, &token.marker)
    }

    /// Verifies all internal request, response, and reviewed ordinal bindings.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if any constituent does not bind the exact
    /// owned completion or if the generate response is not at the reviewed
    /// offset within the resident execution.
    pub fn verify_completion_binding(&self) -> Result<(), OllamaGenerateResourceObservationError> {
        let execution = self.receipt.execution();
        let generation_ordinal = execution
            .first_response_ordinal()
            .checked_add(GENERATION_RESPONSE_OFFSET)
            .ok_or_else(OllamaGenerateResourceObservationError::binding_mismatch)?;
        if generation_ordinal > execution.last_response_ordinal()
            || !Arc::ptr_eq(&self.session_subject, &self.observation.session_subject)
            || execution.request_digest() != self.response.request_binding_digest()
            || execution.response_digest()
                != derive_ollama_retained_session_response_id(&self.response).digest()
        {
            return Err(OllamaGenerateResourceObservationError::binding_mismatch());
        }
        self.observation
            .verify_completion_binding(&self.response, generation_ordinal)
    }

    /// Returns elapsed time from a caller-owned start to the response head.
    ///
    /// # Errors
    ///
    /// Returns a content-free error when `start` is after the response head.
    pub fn response_head_elapsed_since(
        &self,
        start: Instant,
    ) -> Result<Duration, OllamaGenerateResourceObservationError> {
        self.observation.response_head_elapsed_since(start)
    }

    /// Returns Ollama's exact total duration in nanoseconds.
    #[must_use]
    pub const fn total_duration_nanoseconds(&self) -> u64 {
        self.observation.total_duration_nanoseconds()
    }

    /// Returns Ollama's exact model-load duration in nanoseconds.
    #[must_use]
    pub const fn load_duration_nanoseconds(&self) -> u64 {
        self.observation.load_duration_nanoseconds()
    }

    /// Returns Ollama's exact evaluated prompt token count.
    #[must_use]
    pub const fn prompt_token_count(&self) -> u64 {
        self.observation.prompt_token_count()
    }

    /// Returns Ollama's exact prompt-evaluation duration in nanoseconds.
    #[must_use]
    pub const fn prompt_evaluation_duration_nanoseconds(&self) -> u64 {
        self.observation.prompt_evaluation_duration_nanoseconds()
    }

    /// Returns Ollama's exact generated token count.
    #[must_use]
    pub const fn generated_token_count(&self) -> u64 {
        self.observation.generated_token_count()
    }

    /// Returns Ollama's exact generation duration in nanoseconds.
    #[must_use]
    pub const fn evaluation_duration_nanoseconds(&self) -> u64 {
        self.observation.evaluation_duration_nanoseconds()
    }

    #[cfg(test)]
    pub(in crate::single_connection) fn shares_retained_session_subject(
        &self,
        other: &Self,
    ) -> bool {
        self.binds_retained_session_subject(&other.retained_session_subject_token())
    }
}

impl fmt::Debug for OllamaResidentResourceObservedCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OllamaResidentResourceObservedCompletion")
            .field("content", &"redacted")
            .finish()
    }
}

fn checked_elapsed(
    response_head: Instant,
    start: Instant,
) -> Result<Duration, OllamaGenerateResourceObservationError> {
    response_head
        .checked_duration_since(start)
        .ok_or_else(OllamaGenerateResourceObservationError::reversed_monotonic_time)
}

#[cfg(test)]
#[path = "resource_observation/tests.rs"]
mod tests;
