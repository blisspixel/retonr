use std::{collections::BTreeSet, sync::Arc};

use reqwest::Client;
use rewrite_inference::{
    BackendDiscovery, BackendId, GenerationRequest, GenerationResponse, InferenceBackend,
    InferenceCapabilities, InferenceError, OperationContext, PortFuture, UsageObservation,
};
use rewrite_model::{ArtifactRole, RuntimeIdentity};
use rewrite_types::Digest;
use serde::de::DeserializeOwned;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{
    OllamaEndpoint,
    contract::{
        BACKEND_ID, MAX_METADATA_BYTES, MAX_VERSION_BYTES, OllamaLimits, OllamaModelBinding,
        OllamaModelDetails, candidate_output_contract,
    },
    response::{
        await_context, check_context, compatibility_error, confirm_binding_in_tags,
        decode_response, malformed_error, map_transport_error, parse_candidates, parse_inventory,
        permanent_error, policy_error, valid_text, validate_generate_response,
    },
    wire::{
        CandidateEnvelope, GenerateOptions, GenerateRequest as WireGenerateRequest,
        GenerateResponse, ShowRequest, ShowResponse, TagsResponse, VersionResponse,
    },
};

/// Bounded loopback-only implementation of the backend-neutral inference port.
#[derive(Clone, Debug)]
pub struct OllamaBackend {
    endpoint: OllamaEndpoint,
    client: Client,
    limits: OllamaLimits,
    bindings: Vec<OllamaModelBinding>,
    permits: Arc<Semaphore>,
}

impl OllamaBackend {
    /// Creates an adapter with explicit model bindings and fail-closed transport.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid limits, duplicate bindings, or client setup.
    pub fn new(
        endpoint: OllamaEndpoint,
        bindings: Vec<OllamaModelBinding>,
        limits: OllamaLimits,
    ) -> Result<Self, InferenceError> {
        let limits = limits.validate()?;
        let mut references = BTreeSet::new();
        let mut artifacts = BTreeSet::new();
        for binding in &bindings {
            if !references.insert(binding.reference.as_str())
                || !artifacts.insert(binding.artifact_digest.as_str())
            {
                return Err(policy_error("duplicate_model_binding"));
            }
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .referer(false)
            .connect_timeout(limits.connect_timeout)
            .read_timeout(limits.read_timeout)
            .timeout(limits.request_timeout)
            .http1_only()
            .pool_max_idle_per_host(limits.max_concurrency)
            .build()
            .map_err(|_error| permanent_error("client_build_failed"))?;
        Ok(Self {
            endpoint,
            client,
            limits,
            bindings,
            permits: Arc::new(Semaphore::new(limits.max_concurrency)),
        })
    }

    /// Inspects a configured model without returning raw license or template text.
    ///
    /// # Errors
    ///
    /// Returns an error when the reference is unbound or the response is invalid.
    pub async fn inspect_model(
        &self,
        reference: &str,
        context: OperationContext<'_>,
    ) -> Result<OllamaModelDetails, InferenceError> {
        let _permit = self.operation_permit(context).await?;
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.reference == reference)
            .ok_or_else(|| policy_error("unbound_model_reference"))?;
        self.confirm_binding(binding, context).await?;
        self.show_details(binding, context).await
    }

    async fn discover_inner(
        &self,
        context: OperationContext<'_>,
    ) -> Result<BackendDiscovery, InferenceError> {
        let _permit = self.operation_permit(context).await?;
        check_context(context)?;
        let runtime = self.runtime_identity(context).await?;
        let tags = self.tags(context).await?;
        let inventory = parse_inventory(&tags)?;
        for binding in &self.bindings {
            confirm_binding_in_tags(binding, &tags)?;
            let details = self.show_details(binding, context).await?;
            if !details
                .capabilities
                .iter()
                .any(|capability| capability == "completion")
            {
                return Err(compatibility_error("completion_not_supported"));
            }
        }
        Ok(BackendDiscovery {
            backend_id: BackendId::new(BACKEND_ID)
                .map_err(|_error| permanent_error("invalid_backend_id"))?,
            runtime,
            capabilities: InferenceCapabilities {
                roles: vec![ArtifactRole::Generation],
                structured_output: true,
                seed: true,
                disable_reasoning: true,
            },
            inventory,
        })
    }

    async fn generate_inner(
        &self,
        request: GenerationRequest,
        context: OperationContext<'_>,
    ) -> Result<GenerationResponse, InferenceError> {
        let _permit = self.operation_permit(context).await?;
        check_context(context)?;
        request
            .validate()
            .map_err(|_error| policy_error("invalid_generation_request"))?;
        if request.output != candidate_output_contract() {
            return Err(compatibility_error("unsupported_output_contract"));
        }
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.artifact_id == request.artifact_id)
            .ok_or_else(|| policy_error("artifact_not_bound"))?;
        if binding.artifact_digest != request.artifact_digest {
            return Err(policy_error("artifact_digest_mismatch"));
        }
        self.confirm_binding(binding, context).await?;
        let details = self.show_details(binding, context).await?;
        if !details
            .capabilities
            .iter()
            .any(|capability| capability == "completion")
        {
            return Err(compatibility_error("completion_not_supported"));
        }
        let runtime_before = self.runtime_identity(context).await?;
        let schema = serde_json::from_str(&request.output.schema_json)
            .map_err(|_error| policy_error("invalid_output_schema_json"))?;
        let wire_request = WireGenerateRequest {
            model: binding.reference(),
            prompt: &request.input,
            stream: false,
            format: schema,
            think: false,
            raw: false,
            options: GenerateOptions {
                temperature: request.sampling.temperature,
                top_p: request.sampling.top_p,
                seed: request.sampling.seed,
                num_ctx: request.context_token_limit,
                num_predict: request.output_token_limit,
                stop: Vec::new(),
            },
        };
        let response: GenerateResponse = self
            .send_json(
                "api/generate",
                &wire_request,
                self.limits.generation_body_bytes,
                context,
            )
            .await?;
        let runtime_after = self.runtime_identity(context).await?;
        if runtime_before != runtime_after {
            return Err(compatibility_error("runtime_changed_during_generation"));
        }
        self.confirm_binding(binding, context).await?;
        validate_generate_response(&response, binding)?;
        let envelope: CandidateEnvelope = serde_json::from_str(&response.response)
            .map_err(|_error| malformed_error("invalid_candidate_envelope"))?;
        let candidates = parse_candidates(envelope, &request)?;
        Ok(GenerationResponse {
            runtime: runtime_after,
            artifact_id: binding.artifact_id.clone(),
            artifact_digest: binding.artifact_digest.clone(),
            candidates,
            usage: UsageObservation {
                input_tokens: response.prompt_eval_count,
                output_tokens: response.eval_count,
                generation_micros: response
                    .eval_duration
                    .and_then(|value| value.checked_div(1_000)),
            },
        })
    }

    async fn runtime_identity(
        &self,
        context: OperationContext<'_>,
    ) -> Result<RuntimeIdentity, InferenceError> {
        let response: VersionResponse = self
            .get_json("api/version", self.limits.discovery_body_bytes, context)
            .await?;
        if !valid_text(&response.version, MAX_VERSION_BYTES) {
            return Err(malformed_error("invalid_runtime_version"));
        }
        Ok(RuntimeIdentity {
            backend: BACKEND_ID.to_owned(),
            version: response.version,
            digest: None,
        })
    }

    async fn tags(&self, context: OperationContext<'_>) -> Result<TagsResponse, InferenceError> {
        self.get_json("api/tags", self.limits.discovery_body_bytes, context)
            .await
    }

    async fn confirm_binding(
        &self,
        binding: &OllamaModelBinding,
        context: OperationContext<'_>,
    ) -> Result<(), InferenceError> {
        let tags = self.tags(context).await?;
        parse_inventory(&tags)?;
        confirm_binding_in_tags(binding, &tags)
    }

    async fn show_details(
        &self,
        binding: &OllamaModelBinding,
        context: OperationContext<'_>,
    ) -> Result<OllamaModelDetails, InferenceError> {
        let request = ShowRequest {
            model: binding.reference(),
            verbose: true,
        };
        let response: ShowResponse = self
            .send_json(
                "api/show",
                &request,
                self.limits.discovery_body_bytes,
                context,
            )
            .await?;
        let unique_capabilities = response.capabilities.iter().collect::<BTreeSet<_>>();
        if !valid_text(&response.details.format, MAX_METADATA_BYTES)
            || !valid_text(&response.details.family, MAX_METADATA_BYTES)
            || response.details.quantization_level.len() > MAX_METADATA_BYTES
            || response
                .details
                .quantization_level
                .chars()
                .any(char::is_control)
            || response.capabilities.len() > 64
            || unique_capabilities.len() != response.capabilities.len()
            || response
                .capabilities
                .iter()
                .any(|value| !valid_text(value, MAX_METADATA_BYTES))
        {
            return Err(malformed_error("invalid_model_metadata"));
        }
        let metadata = serde_json::to_vec(&response.model_info)
            .map_err(|_error| malformed_error("invalid_model_info"))?;
        Ok(OllamaModelDetails {
            format: response.details.format,
            family: response.details.family,
            quantization: response.details.quantization_level,
            capabilities: response.capabilities,
            license_digest: Digest::sha256(response.license.as_bytes()),
            template_digest: Digest::sha256(response.template.as_bytes()),
            metadata_digest: Digest::sha256(&metadata),
        })
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body_limit: usize,
        context: OperationContext<'_>,
    ) -> Result<T, InferenceError> {
        let url = self
            .endpoint
            .join(path)
            .map_err(|_error| policy_error("invalid_endpoint_path"))?;
        let response = await_context(context, self.client.get(url).send())
            .await?
            .map_err(|error| map_transport_error(&error))?;
        decode_response(response, body_limit, context).await
    }

    async fn send_json<B: serde::Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        body_limit: usize,
        context: OperationContext<'_>,
    ) -> Result<T, InferenceError> {
        let url = self
            .endpoint
            .join(path)
            .map_err(|_error| policy_error("invalid_endpoint_path"))?;
        let response = await_context(context, self.client.post(url).json(body).send())
            .await?
            .map_err(|error| map_transport_error(&error))?;
        decode_response(response, body_limit, context).await
    }

    async fn operation_permit(
        &self,
        context: OperationContext<'_>,
    ) -> Result<SemaphorePermit<'_>, InferenceError> {
        await_context(context, self.permits.acquire())
            .await?
            .map_err(|_error| permanent_error("concurrency_gate_closed"))
    }
}

impl InferenceBackend for OllamaBackend {
    fn discover<'a>(
        &'a self,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<BackendDiscovery, InferenceError>> {
        Box::pin(self.discover_inner(context))
    }

    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<GenerationResponse, InferenceError>> {
        Box::pin(self.generate_inner(request, context))
    }
}
