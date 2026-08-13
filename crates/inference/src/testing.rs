//! Deterministic inference fake for application and conformance tests.

use std::sync::Mutex;

use crate::{
    BackendDiscovery, GenerationRequest, GenerationResponse, InferenceBackend, InferenceError,
    InferenceErrorKind, OperationContext, PortFuture, StructuredCompletionRequest,
    StructuredCompletionResponse,
};

/// One scripted generation outcome consumed by the fake backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeGenerationStep {
    /// Return a complete response.
    Response(GenerationResponse),
    /// Return a redacted backend error.
    Error(InferenceError),
}

/// One scripted structured-completion outcome consumed by the fake backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeStructuredCompletionStep {
    /// Return a complete response.
    Response(StructuredCompletionResponse),
    /// Return a redacted backend error.
    Error(InferenceError),
}

/// Deterministic, thread-safe backend with retained request observations.
#[derive(Debug)]
pub struct FakeInferenceBackend {
    discovery: BackendDiscovery,
    steps: Mutex<std::collections::VecDeque<FakeGenerationStep>>,
    requests: Mutex<Vec<GenerationRequest>>,
    structured_steps: Mutex<std::collections::VecDeque<FakeStructuredCompletionStep>>,
    structured_requests: Mutex<Vec<StructuredCompletionRequest>>,
}

impl FakeInferenceBackend {
    /// Creates a fake with fixed discovery and ordered generation outcomes.
    #[must_use]
    pub fn new(
        discovery: BackendDiscovery,
        steps: impl IntoIterator<Item = FakeGenerationStep>,
    ) -> Self {
        Self {
            discovery,
            steps: Mutex::new(steps.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
            structured_steps: Mutex::new(std::collections::VecDeque::new()),
            structured_requests: Mutex::new(Vec::new()),
        }
    }

    /// Adds ordered structured-completion outcomes to this fake.
    #[must_use]
    pub fn with_structured_steps(
        mut self,
        steps: impl IntoIterator<Item = FakeStructuredCompletionStep>,
    ) -> Self {
        self.structured_steps = Mutex::new(steps.into_iter().collect());
        self
    }

    /// Returns the retained generation requests without raw logging side effects.
    ///
    /// # Errors
    ///
    /// Returns a permanent fake-state error if another thread poisoned the request
    /// lock during a panic.
    pub fn requests(&self) -> Result<Vec<GenerationRequest>, InferenceError> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .map_err(|_error| InferenceError::new(InferenceErrorKind::Permanent, "fake_poisoned"))
    }

    /// Returns retained structured requests without raw logging side effects.
    ///
    /// # Errors
    ///
    /// Returns a permanent fake-state error if another thread poisoned the lock.
    pub fn structured_requests(&self) -> Result<Vec<StructuredCompletionRequest>, InferenceError> {
        self.structured_requests
            .lock()
            .map(|requests| requests.clone())
            .map_err(|_error| InferenceError::new(InferenceErrorKind::Permanent, "fake_poisoned"))
    }
}

impl InferenceBackend for FakeInferenceBackend {
    fn discover<'a>(
        &'a self,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<BackendDiscovery, InferenceError>> {
        let result = if context.is_cancelled() {
            Err(InferenceError::new(
                InferenceErrorKind::Cancelled,
                "cancelled_before_discovery",
            ))
        } else if context.is_expired() {
            Err(InferenceError::new(
                InferenceErrorKind::Deadline,
                "deadline_before_discovery",
            ))
        } else {
            Ok(self.discovery.clone())
        };
        Box::pin(async move { result })
    }

    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<GenerationResponse, InferenceError>> {
        let result = if context.is_cancelled() {
            Err(InferenceError::new(
                InferenceErrorKind::Cancelled,
                "cancelled_before_generation",
            ))
        } else if context.is_expired() {
            Err(InferenceError::new(
                InferenceErrorKind::Deadline,
                "deadline_before_generation",
            ))
        } else if request.validate().is_err() {
            Err(InferenceError::new(
                InferenceErrorKind::Policy,
                "invalid_generation_request",
            ))
        } else {
            let record_result = self.requests.lock().map(|mut requests| {
                requests.push(request);
            });
            if record_result.is_err() {
                Err(InferenceError::new(
                    InferenceErrorKind::Permanent,
                    "fake_poisoned",
                ))
            } else {
                self.steps
                    .lock()
                    .map_err(|_error| {
                        InferenceError::new(InferenceErrorKind::Permanent, "fake_poisoned")
                    })
                    .and_then(|mut steps| {
                        steps.pop_front().map_or_else(
                            || {
                                Err(InferenceError::new(
                                    InferenceErrorKind::Permanent,
                                    "fake_script_exhausted",
                                ))
                            },
                            |step| match step {
                                FakeGenerationStep::Response(response) => Ok(response),
                                FakeGenerationStep::Error(error) => Err(error),
                            },
                        )
                    })
            }
        };
        Box::pin(async move { result })
    }

    fn complete_structured<'a>(
        &'a self,
        request: StructuredCompletionRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<StructuredCompletionResponse, InferenceError>> {
        let result = if context.is_cancelled() {
            Err(InferenceError::new(
                InferenceErrorKind::Cancelled,
                "cancelled_before_structured_completion",
            ))
        } else if context.is_expired() {
            Err(InferenceError::new(
                InferenceErrorKind::Deadline,
                "deadline_before_structured_completion",
            ))
        } else if request.validate().is_err() {
            Err(InferenceError::new(
                InferenceErrorKind::Policy,
                "invalid_structured_completion_request",
            ))
        } else if self
            .structured_requests
            .lock()
            .map(|mut requests| requests.push(request))
            .is_err()
        {
            Err(InferenceError::new(
                InferenceErrorKind::Permanent,
                "fake_poisoned",
            ))
        } else {
            self.structured_steps
                .lock()
                .map_err(|_error| {
                    InferenceError::new(InferenceErrorKind::Permanent, "fake_poisoned")
                })
                .and_then(|mut steps| {
                    steps.pop_front().map_or_else(
                        || {
                            Err(InferenceError::new(
                                InferenceErrorKind::Permanent,
                                "fake_script_exhausted",
                            ))
                        },
                        |step| match step {
                            FakeStructuredCompletionStep::Response(response) => Ok(response),
                            FakeStructuredCompletionStep::Error(error) => Err(error),
                        },
                    )
                })
        };
        Box::pin(async move { result })
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Poll, Waker};

    use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
    use rewrite_types::{CancellationToken, Digest};

    use super::{FakeGenerationStep, FakeInferenceBackend};
    use crate::{
        BackendDiscovery, BackendId, GENERATION_REQUEST_SCHEMA_VERSION, GenerationCandidate,
        GenerationRequest, GenerationResponse, InferenceBackend, InferenceCapabilities,
        OperationContext, OutputContract, ReasoningPolicy,
        STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, SamplingParameters,
        StructuredCompletionFinish, StructuredCompletionRequest, StructuredCompletionResponse,
        UsageObservation,
    };

    fn poll_ready<T>(mut future: crate::PortFuture<'_, T>) -> T {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("fake future must complete immediately"),
        }
    }

    fn fixtures() -> (BackendDiscovery, GenerationRequest, GenerationResponse) {
        let digest = Digest::sha256(b"artifact");
        let artifact_id = ArtifactId::from_digest(digest.clone());
        let schema_json = "{\"type\":\"object\"}".to_owned();
        let schema_digest = Digest::sha256(schema_json.as_bytes());
        let runtime = RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1".to_owned(),
            digest: None,
        };
        let discovery = BackendDiscovery {
            backend_id: BackendId::new("fake").expect("valid ID"),
            runtime: runtime.clone(),
            capabilities: InferenceCapabilities {
                roles: vec![ArtifactRole::Generation],
                admitted_output_contract_digests: vec![schema_digest.clone()],
                seed: true,
                disable_reasoning: true,
            },
            inventory: Vec::new(),
        };
        let request = GenerationRequest {
            schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
            artifact_id: artifact_id.clone(),
            artifact_digest: digest.clone(),
            input: "input".to_owned(),
            output: OutputContract {
                schema_digest,
                schema_json,
            },
            candidate_count: 1,
            source_byte_count: 5,
            source_byte_limit: 128,
            input_byte_limit: 256,
            context_token_limit: 256,
            output_token_limit: 64,
            candidate_byte_limit: 128,
            sampling: SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                seed: Some(1),
            },
            reasoning: ReasoningPolicy::Disabled,
        };
        let response = GenerationResponse {
            runtime,
            artifact_id,
            artifact_digest: digest,
            candidates: vec![GenerationCandidate {
                ordinal: 0,
                text: "candidate".to_owned(),
            }],
            usage: UsageObservation {
                input_tokens: Some(1),
                output_tokens: Some(1),
                generation_micros: Some(10),
            },
        };
        (discovery, request, response)
    }

    #[test]
    fn fake_records_request_and_consumes_script() {
        let (discovery, request, response) = fixtures();
        let fake =
            FakeInferenceBackend::new(discovery, [FakeGenerationStep::Response(response.clone())]);
        let token = CancellationToken::new();
        let actual =
            poll_ready(fake.generate(request.clone(), OperationContext::new(&token, None)))
                .expect("scripted response");
        assert_eq!(actual, response);
        assert_eq!(
            fake.requests().expect("request observations"),
            vec![request]
        );
    }

    #[test]
    fn fake_honors_cancellation_before_consuming_script() {
        let (discovery, request, response) = fixtures();
        let fake = FakeInferenceBackend::new(discovery, [FakeGenerationStep::Response(response)]);
        let token = CancellationToken::new();
        token.cancel();
        let error = poll_ready(fake.generate(request, OperationContext::new(&token, None)))
            .expect_err("cancelled operation fails");
        assert_eq!(error.kind, crate::InferenceErrorKind::Cancelled);
        assert!(fake.requests().expect("request observations").is_empty());
    }

    #[test]
    fn fake_records_structured_request_and_honors_pre_cancellation() {
        let (discovery, generation, response) = fixtures();
        let request = StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: generation.artifact_id.clone(),
            artifact_digest: generation.artifact_digest.clone(),
            input: generation.input.clone(),
            output: generation.output.clone(),
            source_byte_count: generation.source_byte_count,
            source_byte_limit: generation.source_byte_limit,
            input_byte_limit: generation.input_byte_limit,
            context_token_limit: generation.context_token_limit,
            output_token_limit: generation.output_token_limit,
            output_byte_limit: generation.candidate_byte_limit,
            sampling: generation.sampling,
            reasoning: generation.reasoning,
        };
        let structured = StructuredCompletionResponse::complete(
            &request,
            response.runtime,
            response.artifact_id,
            response.artifact_digest,
            "{}".to_owned(),
            response.usage,
        )
        .expect("valid structured response");
        let fake = FakeInferenceBackend::new(discovery, [])
            .with_structured_steps([super::FakeStructuredCompletionStep::Response(structured)]);
        let token = CancellationToken::new();
        let actual = poll_ready(
            fake.complete_structured(request.clone(), OperationContext::new(&token, None)),
        )
        .expect("scripted structured response");
        assert_eq!(actual.finish(), StructuredCompletionFinish::Complete);
        assert_eq!(
            fake.structured_requests().expect("structured observations"),
            vec![request.clone()]
        );

        token.cancel();
        let error =
            poll_ready(fake.complete_structured(request, OperationContext::new(&token, None)))
                .expect_err("pre-cancelled structured request fails");
        assert_eq!(error.kind, crate::InferenceErrorKind::Cancelled);
        assert_eq!(
            fake.structured_requests()
                .expect("structured observations")
                .len(),
            1
        );
    }
}
