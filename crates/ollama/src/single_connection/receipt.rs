use std::fmt;

use rewrite_inference::{InferenceError, StructuredCompletionFinish, StructuredCompletionResponse};
use rewrite_model::OllamaRetainedSessionResponseId;
use rewrite_types::Digest;

use crate::response::malformed_error;
use crate::{OllamaPreflight, OllamaRunningModel};

/// Exact Ollama version whose resident-completion behavior was reviewed.
pub const OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION: &str = "0.32.15";
/// Exact Ollama source revision reviewed for resident-completion behavior.
pub const OLLAMA_RESIDENT_COMPLETION_SOURCE_REVISION: &str =
    "b7871fc0d1d82fe109536efa3e0e8e411c766c75";
/// Explicit model retention requested by the resident-completion profile.
pub const OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE: &str = "5m";

const RESIDENT_SESSION_EXECUTION_RECEIPT_COMPLETE_BINDING_DOMAIN: &[u8] =
    b"ollama/retained-session/resident-execution-receipt/complete-binding/v1\0";

/// Derives the inert typed identity of one exact structured response.
///
/// The identity binds every field of [`StructuredCompletionResponse`] using the
/// unchanged retained-session response V1 encoding. It proves no transport,
/// process, model-use, handler-placement, or qualification fact.
#[must_use]
pub fn derive_ollama_retained_session_response_id(
    response: &StructuredCompletionResponse,
) -> OllamaRetainedSessionResponseId {
    OllamaRetainedSessionResponseId::from_derived_digest(response_binding_digest(response))
}

/// Content-free binding evidence for one retained-stream completion.
///
/// The request and response digests are equality bindings, not anonymization.
/// Predictable prompts or outputs may be recoverable by dictionary attack. This
/// type deliberately has no serialization implementation.
#[derive(Clone, Eq, PartialEq)]
pub struct OllamaSessionExecutionReceipt {
    preflight_digest: Digest,
    request_digest: Digest,
    response_digest: Digest,
    first_response_ordinal: usize,
    last_response_ordinal: usize,
}

impl OllamaSessionExecutionReceipt {
    pub(super) fn new(
        preflight: &OllamaPreflight,
        response: &StructuredCompletionResponse,
        first_response_ordinal: usize,
        last_response_ordinal: usize,
    ) -> Result<Self, InferenceError> {
        let response_id = derive_ollama_retained_session_response_id(response);
        Ok(Self {
            preflight_digest: preflight_binding_digest(preflight)?,
            request_digest: response.request_binding_digest().clone(),
            response_digest: response_id.digest().clone(),
            first_response_ordinal,
            last_response_ordinal,
        })
    }

    /// Creates deterministic receipt evidence for downstream contract tests.
    ///
    /// This helper is unavailable unless the explicit `test-support` feature is
    /// enabled. It applies the same complete derivation as a retained session.
    ///
    /// # Errors
    ///
    /// Returns [`InferenceError`] if the supplied preflight cannot be encoded.
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(
        preflight: &OllamaPreflight,
        response: &StructuredCompletionResponse,
        first_response_ordinal: usize,
        last_response_ordinal: usize,
    ) -> Result<Self, InferenceError> {
        Self::new(
            preflight,
            response,
            first_response_ordinal,
            last_response_ordinal,
        )
    }

    /// Returns the digest of the exact successful preflight evidence.
    #[must_use]
    pub const fn preflight_digest(&self) -> &Digest {
        &self.preflight_digest
    }

    /// Returns the digest binding the complete structured request.
    #[must_use]
    pub const fn request_digest(&self) -> &Digest {
        &self.request_digest
    }

    /// Returns the digest binding the complete structured response.
    #[must_use]
    pub const fn response_digest(&self) -> &Digest {
        &self.response_digest
    }

    /// Returns the typed identity of the exact retained structured response.
    ///
    /// This wraps the unchanged V1 response binding. It deliberately excludes
    /// preflight facts and retained-transport ordinals, which are bound by the
    /// surrounding execution receipt instead.
    #[must_use]
    pub fn retained_response_id(&self) -> OllamaRetainedSessionResponseId {
        OllamaRetainedSessionResponseId::from_derived_digest(self.response_digest.clone())
    }

    /// Returns the first response ordinal consumed by this completion.
    #[must_use]
    pub const fn first_response_ordinal(&self) -> usize {
        self.first_response_ordinal
    }

    /// Returns the final response ordinal consumed by this completion.
    #[must_use]
    pub const fn last_response_ordinal(&self) -> usize {
        self.last_response_ordinal
    }
}

impl fmt::Debug for OllamaSessionExecutionReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OllamaSessionExecutionReceipt")
            .field("preflight_digest", &self.preflight_digest)
            .field("request_digest", &self.request_digest)
            .field("response_digest", &self.response_digest)
            .field("first_response_ordinal", &self.first_response_ordinal)
            .field("last_response_ordinal", &self.last_response_ordinal)
            .finish()
    }
}

/// Content-free binding evidence for one retained-stream completion with
/// two exact post-generation Ollama residency reports.
///
/// The receipt establishes only what the reviewed Ollama API reported on the
/// retained stream. It does not prove application-handler execution, model use,
/// resident-page identity, effective runtime identity, or qualification. Its
/// digests are equality bindings, not anonymization. This type deliberately has
/// no serialization implementation.
#[derive(Clone, Eq, PartialEq)]
pub struct OllamaResidentSessionExecutionReceipt {
    execution: OllamaSessionExecutionReceipt,
    residency_contract_digest: Digest,
    residency_observation_digest: Digest,
    runtime_reference_digest: Digest,
    inventory_digest: Digest,
    byte_size: u64,
    accelerator_bytes: u64,
    context_tokens: u32,
    first_residency_ordinal: usize,
    last_residency_ordinal: usize,
}

impl OllamaResidentSessionExecutionReceipt {
    pub(super) fn new(
        execution: OllamaSessionExecutionReceipt,
        running: &OllamaRunningModel,
        first_residency_ordinal: usize,
        last_residency_ordinal: usize,
    ) -> Self {
        let residency_contract_digest = resident_completion_contract_digest();
        let runtime_reference_digest = Digest::sha256(running.reference.as_bytes());
        let mut material = Vec::with_capacity(384);
        push_bytes(&mut material, residency_contract_digest.as_str().as_bytes());
        push_bytes(
            &mut material,
            execution.response_digest().as_str().as_bytes(),
        );
        push_bytes(&mut material, running.reference.as_bytes());
        push_bytes(&mut material, running.inventory_digest.as_str().as_bytes());
        material.extend_from_slice(&running.byte_size.to_be_bytes());
        material.extend_from_slice(&running.accelerator_bytes.to_be_bytes());
        material.extend_from_slice(&running.context_tokens.to_be_bytes());
        material.extend_from_slice(&(first_residency_ordinal as u64).to_be_bytes());
        material.extend_from_slice(&(last_residency_ordinal as u64).to_be_bytes());
        let residency_observation_digest = domain_digest(
            b"ollama/retained-session/post-generation-residency/v1",
            &material,
        );
        Self {
            execution,
            residency_contract_digest,
            residency_observation_digest,
            runtime_reference_digest,
            inventory_digest: running.inventory_digest.clone(),
            byte_size: running.byte_size,
            accelerator_bytes: running.accelerator_bytes,
            context_tokens: running.context_tokens,
            first_residency_ordinal,
            last_residency_ordinal,
        }
    }

    /// Constructs inert resident execution evidence for downstream offline tests.
    ///
    /// This helper is absent unless the `test-support` feature is enabled and
    /// does not construct retained transport or execution authority.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn for_test(
        execution: OllamaSessionExecutionReceipt,
        running: &OllamaRunningModel,
        first_residency_ordinal: usize,
        last_residency_ordinal: usize,
    ) -> Self {
        Self::new(
            execution,
            running,
            first_residency_ordinal,
            last_residency_ordinal,
        )
    }

    /// Returns the underlying preflight, request, response, and ordinal binding.
    #[must_use]
    pub const fn execution(&self) -> &OllamaSessionExecutionReceipt {
        &self.execution
    }

    /// Returns the digest of the reviewed source-scoped residency contract.
    #[must_use]
    pub const fn residency_contract_digest(&self) -> &Digest {
        &self.residency_contract_digest
    }

    /// Returns the digest binding both equal post-generation residency reports.
    #[must_use]
    pub const fn residency_observation_digest(&self) -> &Digest {
        &self.residency_observation_digest
    }

    /// Returns the digest of the exact runtime-local model reference.
    #[must_use]
    pub const fn runtime_reference_digest(&self) -> &Digest {
        &self.runtime_reference_digest
    }

    /// Returns the exact runtime-reported model inventory digest.
    #[must_use]
    pub const fn inventory_digest(&self) -> &Digest {
        &self.inventory_digest
    }

    /// Returns the runtime-reported total loaded-model memory bytes.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the runtime-reported accelerator-resident byte count.
    #[must_use]
    pub const fn accelerator_bytes(&self) -> u64 {
        self.accelerator_bytes
    }

    /// Returns the runtime-reported effective context length.
    #[must_use]
    pub const fn context_tokens(&self) -> u32 {
        self.context_tokens
    }

    /// Returns the first post-generation residency response ordinal.
    #[must_use]
    pub const fn first_residency_ordinal(&self) -> usize {
        self.first_residency_ordinal
    }

    /// Returns the confirming post-generation residency response ordinal.
    #[must_use]
    pub const fn last_residency_ordinal(&self) -> usize {
        self.last_residency_ordinal
    }

    /// Returns the owner-derived binding over every execution and residency field.
    ///
    /// This digest is an equality binding for exact receipt comparison. It does
    /// not prove that execution occurred, that the selected model was used, or
    /// that any runtime, response, or model is qualified.
    #[must_use]
    pub fn complete_binding_digest(&self) -> Digest {
        let execution = &self.execution;
        let mut material = Vec::with_capacity(640);
        material.extend_from_slice(RESIDENT_SESSION_EXECUTION_RECEIPT_COMPLETE_BINDING_DOMAIN);
        for digest in [
            &execution.preflight_digest,
            &execution.request_digest,
            &execution.response_digest,
        ] {
            material.extend_from_slice(digest.as_str().as_bytes());
        }
        material.extend_from_slice(&(execution.first_response_ordinal as u64).to_be_bytes());
        material.extend_from_slice(&(execution.last_response_ordinal as u64).to_be_bytes());
        for digest in [
            &self.residency_contract_digest,
            &self.residency_observation_digest,
            &self.runtime_reference_digest,
            &self.inventory_digest,
        ] {
            material.extend_from_slice(digest.as_str().as_bytes());
        }
        material.extend_from_slice(&self.byte_size.to_be_bytes());
        material.extend_from_slice(&self.accelerator_bytes.to_be_bytes());
        material.extend_from_slice(&self.context_tokens.to_be_bytes());
        material.extend_from_slice(&(self.first_residency_ordinal as u64).to_be_bytes());
        material.extend_from_slice(&(self.last_residency_ordinal as u64).to_be_bytes());
        Digest::sha256(&material)
    }

    /// Returns true because both admitted Ollama residency reports were equal.
    #[must_use]
    pub const fn runtime_reported_residency_proven(&self) -> bool {
        true
    }

    /// Always false because API residency does not identify a request handler.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        false
    }

    /// Always false because API residency and a response do not prove weight use.
    #[must_use]
    pub const fn model_use_proven(&self) -> bool {
        false
    }

    /// Always false because `/api/ps` does not attest resident memory pages.
    #[must_use]
    pub const fn resident_page_identity_proven(&self) -> bool {
        false
    }

    /// Always false because this receipt is not an effective runtime identity.
    #[must_use]
    pub const fn effective_runtime_identity_proven(&self) -> bool {
        false
    }

    /// Always false because this receipt cannot qualify a model or runtime.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        false
    }
}

impl fmt::Debug for OllamaResidentSessionExecutionReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OllamaResidentSessionExecutionReceipt")
            .field("execution", &self.execution)
            .field("residency_contract_digest", &self.residency_contract_digest)
            .field(
                "residency_observation_digest",
                &self.residency_observation_digest,
            )
            .field("runtime_reference_digest", &self.runtime_reference_digest)
            .field("inventory_digest", &self.inventory_digest)
            .field("byte_size", &self.byte_size)
            .field("accelerator_bytes", &self.accelerator_bytes)
            .field("context_tokens", &self.context_tokens)
            .field("first_residency_ordinal", &self.first_residency_ordinal)
            .field("last_residency_ordinal", &self.last_residency_ordinal)
            .finish()
    }
}

fn resident_completion_contract_digest() -> Digest {
    let mut material = Vec::with_capacity(256);
    for value in [
        OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION,
        OLLAMA_RESIDENT_COMPLETION_SOURCE_REVISION,
        OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE,
        "version,tags,show,generate,ps,version,tags,show,ps",
        "handler=false;model_use=false;page_identity=false;effective_identity=false;qualified=false",
    ] {
        push_bytes(&mut material, value.as_bytes());
    }
    domain_digest(b"ollama/retained-session/residency-contract/v1", &material)
}

fn preflight_binding_digest(preflight: &OllamaPreflight) -> Result<Digest, InferenceError> {
    let encoded = serde_json::to_vec(preflight)
        .map_err(|_error| malformed_error("preflight_receipt_encoding_failed"))?;
    Ok(domain_digest(
        b"ollama/retained-session/preflight/v1",
        &encoded,
    ))
}

fn response_binding_digest(response: &StructuredCompletionResponse) -> Digest {
    let mut encoded = Vec::new();
    match response.finish() {
        StructuredCompletionFinish::Complete => {}
    }
    push_bytes(&mut encoded, response.runtime().backend.as_bytes());
    push_bytes(&mut encoded, response.runtime().version.as_bytes());
    match &response.runtime().digest {
        Some(digest) => {
            encoded.push(1);
            push_bytes(&mut encoded, digest.as_str().as_bytes());
        }
        None => encoded.push(0),
    }
    push_bytes(
        &mut encoded,
        response.artifact_id().digest().as_str().as_bytes(),
    );
    push_bytes(&mut encoded, response.artifact_digest().as_str().as_bytes());
    push_bytes(
        &mut encoded,
        response.request_binding_digest().as_str().as_bytes(),
    );
    push_bytes(&mut encoded, response.output_json().as_bytes());
    let usage = response.usage();
    push_optional_u64(&mut encoded, usage.input_tokens);
    push_optional_u64(&mut encoded, usage.output_tokens);
    push_optional_u64(&mut encoded, usage.generation_micros);
    domain_digest(b"ollama/retained-session/response/v1", &encoded)
}

fn domain_digest(domain: &[u8], value: &[u8]) -> Digest {
    let mut material = Vec::with_capacity(domain.len() + value.len() + 16);
    push_bytes(&mut material, domain);
    push_bytes(&mut material, value);
    Digest::sha256(&material)
}

fn push_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_be_bytes());
    target.extend_from_slice(value);
}

fn push_optional_u64(target: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            target.push(1);
            target.extend_from_slice(&value.to_be_bytes());
        }
        None => target.push(0),
    }
}

#[cfg(test)]
#[path = "receipt/tests.rs"]
mod tests;
