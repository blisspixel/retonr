//! In-process fake-backend conformance with no network or process start.

use serde::Deserialize;

use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::Digest;

use crate::{
    BackendDiscovery, BackendId, GenerationCandidate, GenerationRequest, GenerationResponse,
    InferenceBackend, InferenceCapabilities, InferenceError, InferenceErrorKind, InventoryEntry,
    OperationContext, PortFuture, StructuredCompletionRequest, StructuredCompletionResponse,
    UsageObservation, candidate_output_contract,
};

/// Backend identifier used by retained fake-backend conformance.
pub const CONFORMANCE_BACKEND_ID: &str = "fake";

/// In-process identity generator bound to one recovered artifact.
///
/// Discovery admits only the candidate contract. Structured claim completion is
/// refused. The backend never starts a process or opens a network path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceInferenceBackend {
    discovery: BackendDiscovery,
}

#[derive(Deserialize)]
struct ConformanceEnvelope {
    masked_source: String,
}

impl ConformanceInferenceBackend {
    /// Binds one recovered generation artifact to the in-process fake backend.
    ///
    /// # Errors
    ///
    /// Returns a compatibility error unless the runtime backend is
    /// [`CONFORMANCE_BACKEND_ID`] and the identifier is a valid backend label.
    pub fn bind(
        artifact_id: ArtifactId,
        artifact_digest: Digest,
        runtime: RuntimeIdentity,
        byte_size: Option<u64>,
    ) -> Result<Self, InferenceError> {
        if runtime.backend != CONFORMANCE_BACKEND_ID {
            return Err(InferenceError::new(
                InferenceErrorKind::Compatibility,
                "conformance_backend_required",
            ));
        }
        if artifact_id.digest() != &artifact_digest {
            return Err(InferenceError::new(
                InferenceErrorKind::Policy,
                "conformance_artifact_mismatch",
            ));
        }
        let backend_id = BackendId::new(CONFORMANCE_BACKEND_ID).map_err(|_error| {
            InferenceError::new(InferenceErrorKind::Permanent, "conformance_backend_id")
        })?;
        let contract = candidate_output_contract();
        Ok(Self {
            discovery: BackendDiscovery {
                backend_id,
                runtime,
                capabilities: InferenceCapabilities {
                    roles: vec![ArtifactRole::Generation],
                    admitted_output_contract_digests: vec![contract.schema_digest],
                    seed: true,
                    disable_reasoning: true,
                },
                inventory: vec![InventoryEntry {
                    reference: "conformance:recovered".to_owned(),
                    artifact_id,
                    artifact_digest,
                    byte_size,
                }],
            },
        })
    }
}

impl InferenceBackend for ConformanceInferenceBackend {
    fn discover<'a>(
        &'a self,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<BackendDiscovery, InferenceError>> {
        let result = preflight(context).map(|()| self.discovery.clone());
        Box::pin(async move { result })
    }

    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<GenerationResponse, InferenceError>> {
        let result = (|| {
            preflight(context)?;
            request.validate().map_err(|_error| {
                InferenceError::new(InferenceErrorKind::Policy, "invalid_generation_request")
            })?;
            let bound = self.discovery.inventory.first().ok_or_else(|| {
                InferenceError::new(InferenceErrorKind::Permanent, "empty_inventory")
            })?;
            if request.artifact_id != bound.artifact_id
                || request.artifact_digest != bound.artifact_digest
            {
                return Err(InferenceError::new(
                    InferenceErrorKind::Policy,
                    "conformance_artifact_mismatch",
                ));
            }
            if !self.discovery.capabilities.admits_output(&request.output) {
                return Err(InferenceError::new(
                    InferenceErrorKind::Compatibility,
                    "conformance_candidate_contract_only",
                ));
            }
            let masked_source = extract_masked_source(&request.input)?;
            if u64::try_from(masked_source.len()).unwrap_or(u64::MAX) > request.candidate_byte_limit
            {
                return Err(InferenceError::new(
                    InferenceErrorKind::Policy,
                    "conformance_candidate_too_large",
                ));
            }
            let candidates = (0..request.candidate_count)
                .map(|ordinal| GenerationCandidate {
                    ordinal,
                    text: masked_source.clone(),
                })
                .collect();
            Ok(GenerationResponse {
                runtime: self.discovery.runtime.clone(),
                artifact_id: bound.artifact_id.clone(),
                artifact_digest: bound.artifact_digest.clone(),
                candidates,
                usage: UsageObservation {
                    input_tokens: None,
                    output_tokens: None,
                    generation_micros: None,
                },
            })
        })();
        Box::pin(async move { result })
    }

    fn complete_structured<'a>(
        &'a self,
        _request: StructuredCompletionRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<StructuredCompletionResponse, InferenceError>> {
        let result = preflight(context).and_then(|()| {
            Err(InferenceError::new(
                InferenceErrorKind::Compatibility,
                "conformance_candidate_contract_only",
            ))
        });
        Box::pin(async move { result })
    }
}

fn preflight(context: OperationContext<'_>) -> Result<(), InferenceError> {
    if context.is_cancelled() {
        Err(InferenceError::new(
            InferenceErrorKind::Cancelled,
            "cancelled_before_conformance",
        ))
    } else if context.is_expired() {
        Err(InferenceError::new(
            InferenceErrorKind::Deadline,
            "deadline_before_conformance",
        ))
    } else {
        Ok(())
    }
}

fn extract_masked_source(input: &str) -> Result<String, InferenceError> {
    let json = input.rsplit_once('\n').map_or(input, |(_, suffix)| suffix);
    let envelope = serde_json::from_str::<ConformanceEnvelope>(json).map_err(|_error| {
        InferenceError::new(InferenceErrorKind::Policy, "conformance_input_unusable")
    })?;
    if envelope.masked_source.is_empty() {
        return Err(InferenceError::new(
            InferenceErrorKind::Policy,
            "conformance_input_unusable",
        ));
    }
    Ok(envelope.masked_source)
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
        time::Instant,
    };

    use rewrite_types::CancellationToken;

    use super::{CONFORMANCE_BACKEND_ID, ConformanceInferenceBackend};
    use crate::{
        GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, InferenceBackend, InferenceErrorKind,
        OperationContext, ReasoningPolicy, SamplingParameters, candidate_output_contract,
        claim_output_contract,
    };

    fn block_ready<T>(future: impl Future<Output = T>) -> T {
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("conformance backend must complete immediately"),
        }
    }

    fn backend() -> ConformanceInferenceBackend {
        let digest = rewrite_types::Digest::sha256(b"artifact");
        ConformanceInferenceBackend::bind(
            rewrite_model::ArtifactId::from_digest(digest.clone()),
            digest,
            rewrite_model::RuntimeIdentity {
                backend: CONFORMANCE_BACKEND_ID.to_owned(),
                version: "1.0.0".to_owned(),
                digest: None,
            },
            Some(8),
        )
        .expect("bind conformance")
    }

    fn request(backend: &ConformanceInferenceBackend, input: &str) -> GenerationRequest {
        let entry = &backend.discovery.inventory[0];
        GenerationRequest {
            schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
            artifact_id: entry.artifact_id.clone(),
            artifact_digest: entry.artifact_digest.clone(),
            input: input.to_owned(),
            output: candidate_output_contract(),
            candidate_count: 1,
            source_byte_count: 7,
            source_byte_limit: 1_024,
            input_byte_limit: 4_096,
            context_token_limit: 2_048,
            output_token_limit: 256,
            candidate_byte_limit: 1_024,
            sampling: SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                seed: Some(1),
            },
            reasoning: ReasoningPolicy::Disabled,
        }
    }

    #[test]
    fn bind_rejects_a_non_fake_runtime() {
        let digest = rewrite_types::Digest::sha256(b"artifact");
        let error = ConformanceInferenceBackend::bind(
            rewrite_model::ArtifactId::from_digest(digest.clone()),
            digest,
            rewrite_model::RuntimeIdentity {
                backend: "ollama".to_owned(),
                version: "1.0.0".to_owned(),
                digest: None,
            },
            None,
        )
        .expect_err("non-fake runtime");
        assert_eq!(error.kind, InferenceErrorKind::Compatibility);
    }

    #[test]
    fn generate_returns_the_masked_source_without_network_work() {
        let backend = backend();
        let token = CancellationToken::new();
        let response = block_ready(backend.generate(
            request(
                &backend,
                "template\n{\"masked_source\":\"Version {{PROTECTED_NUMBER_0001}} works.\"}",
            ),
            OperationContext::new(&token, None),
        ))
        .expect("identity generate");
        assert_eq!(response.runtime.backend, CONFORMANCE_BACKEND_ID);
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(
            response.candidates[0].text,
            "Version {{PROTECTED_NUMBER_0001}} works."
        );
    }

    #[test]
    fn structured_completion_is_refused() {
        let backend = backend();
        let token = CancellationToken::new();
        let claim = claim_output_contract();
        let entry = &backend.discovery.inventory[0];
        let error = block_ready(backend.complete_structured(
            crate::StructuredCompletionRequest {
                schema_version: crate::STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
                artifact_id: entry.artifact_id.clone(),
                artifact_digest: entry.artifact_digest.clone(),
                input: "{}".to_owned(),
                output: claim,
                source_byte_count: 2,
                source_byte_limit: 1_024,
                input_byte_limit: 1_024,
                context_token_limit: 128,
                output_token_limit: 128,
                output_byte_limit: 1_024,
                sampling: SamplingParameters {
                    temperature: 0.0,
                    top_p: 1.0,
                    seed: Some(1),
                },
                reasoning: ReasoningPolicy::Disabled,
            },
            OperationContext::new(&token, None),
        ))
        .expect_err("claim contract refused");
        assert_eq!(error.kind, InferenceErrorKind::Compatibility);
        assert_eq!(error.code, "conformance_candidate_contract_only");
    }

    #[test]
    fn cancellation_prevents_generation() {
        let backend = backend();
        let token = CancellationToken::new();
        token.cancel();
        let error = block_ready(backend.generate(
            request(&backend, "template\n{\"masked_source\":\"Hello\"}"),
            OperationContext::new(&token, Some(Instant::now())),
        ))
        .expect_err("cancelled");
        assert_eq!(error.kind, InferenceErrorKind::Cancelled);
    }
}
