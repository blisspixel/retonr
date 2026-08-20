use std::{collections::BTreeSet, sync::Arc};

use reqwest::Client;
use rewrite_inference::{
    BackendDiscovery, BackendId, GenerationRequest, GenerationResponse, InferenceBackend,
    InferenceCapabilities, InferenceError, OperationContext, PortFuture,
    StructuredCompletionRequest, StructuredCompletionResponse, UsageObservation,
    candidate_output_contract,
};
use rewrite_model::{ArtifactRole, RuntimeIdentity};
use rewrite_types::Digest;
use serde::de::DeserializeOwned;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{
    OllamaEndpoint,
    contract::{
        BACKEND_ID, MAX_METADATA_BYTES, MAX_PREFLIGHT_TARGETS, MAX_VERSION_BYTES, OllamaLimits,
        OllamaModelBinding, OllamaModelDetails, OllamaPreflightTarget, OllamaRunningModel,
    },
    response::{
        await_context, check_context, compatibility_error, confirm_binding_in_tags,
        decode_response, malformed_error, map_transport_error, parse_candidates, parse_inventory,
        parse_running_models, permanent_error, policy_error, valid_text,
        validate_generate_response,
    },
    wire::{
        CandidateEnvelope, GenerateOptions, GenerateRequest as WireGenerateRequest,
        GenerateResponse, PsResponse, ShowRequest, ShowResponse, TagsResponse, VersionResponse,
    },
};

mod preflight;

/// Bounded loopback-only implementation of the backend-neutral inference port.
#[derive(Clone, Debug)]
pub struct OllamaBackend {
    endpoint: OllamaEndpoint,
    client: Client,
    limits: OllamaLimits,
    bindings: Vec<OllamaModelBinding>,
    preflight_targets: Vec<OllamaPreflightTarget>,
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
        Self::from_parts(endpoint, bindings, Vec::new(), limits)
    }

    /// Creates a read-only adapter that carries no Retonr artifact identity.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid limits, an empty or oversized target set, duplicate target
    /// references or digests, or client setup.
    pub fn new_preflight(
        endpoint: OllamaEndpoint,
        targets: Vec<OllamaPreflightTarget>,
        limits: OllamaLimits,
    ) -> Result<Self, InferenceError> {
        let limits = limits.validate()?;
        if targets.is_empty() || targets.len() > MAX_PREFLIGHT_TARGETS {
            return Err(policy_error("invalid_preflight_targets"));
        }
        let mut references = BTreeSet::new();
        let mut digests = BTreeSet::new();
        for target in &targets {
            if !references.insert(target.reference.as_str())
                || !digests.insert(target.inventory_digest.as_str())
            {
                return Err(policy_error("duplicate_preflight_target"));
            }
        }
        Self::from_parts(endpoint, Vec::new(), targets, limits)
    }

    fn from_parts(
        endpoint: OllamaEndpoint,
        bindings: Vec<OllamaModelBinding>,
        preflight_targets: Vec<OllamaPreflightTarget>,
        limits: OllamaLimits,
    ) -> Result<Self, InferenceError> {
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
            preflight_targets,
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
        self.show_details(binding.reference(), context).await
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
            let details = self.show_details(binding.reference(), context).await?;
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
                admitted_output_contract_digests: vec![candidate_output_contract().schema_digest],
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
        let details = self.show_details(binding.reference(), context).await?;
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

    async fn complete_structured_inner(
        &self,
        request: StructuredCompletionRequest,
        context: OperationContext<'_>,
    ) -> Result<StructuredCompletionResponse, InferenceError> {
        let _permit = self.operation_permit(context).await?;
        check_context(context)?;
        request
            .validate()
            .map_err(|_error| policy_error("invalid_structured_completion_request"))?;
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
        let details = self.show_details(binding.reference(), context).await?;
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
        let usage = usage_observation(&response);
        StructuredCompletionResponse::complete(
            &request,
            runtime_after,
            binding.artifact_id.clone(),
            binding.artifact_digest.clone(),
            response.response,
            usage,
        )
        .map_err(|_error| malformed_error("invalid_structured_output"))
    }

    async fn tags(&self, context: OperationContext<'_>) -> Result<TagsResponse, InferenceError> {
        self.get_json("api/tags", self.limits.discovery_body_bytes, context)
            .await
    }

    async fn running_models(
        &self,
        context: OperationContext<'_>,
    ) -> Result<Vec<OllamaRunningModel>, InferenceError> {
        let response: PsResponse = self
            .get_json("api/ps", self.limits.discovery_body_bytes, context)
            .await?;
        parse_running_models(&response)
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
        reference: &str,
        context: OperationContext<'_>,
    ) -> Result<OllamaModelDetails, InferenceError> {
        let request = ShowRequest {
            model: reference,
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
        if !response.remote_model.is_empty()
            || !response.remote_host.is_empty()
            || !valid_text(&response.details.format, MAX_METADATA_BYTES)
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
        let mut capabilities = response.capabilities;
        capabilities.sort_unstable();
        Ok(OllamaModelDetails {
            format: response.details.format,
            family: response.details.family,
            quantization: response.details.quantization_level,
            capabilities,
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

    fn complete_structured<'a>(
        &'a self,
        request: StructuredCompletionRequest,
        context: OperationContext<'a>,
    ) -> PortFuture<'a, Result<StructuredCompletionResponse, InferenceError>> {
        Box::pin(self.complete_structured_inner(request, context))
    }
}

fn usage_observation(response: &GenerateResponse) -> UsageObservation {
    UsageObservation {
        input_tokens: response.prompt_eval_count,
        output_tokens: response.eval_count,
        generation_micros: response
            .eval_duration
            .and_then(|value| value.checked_div(1_000)),
    }
}
