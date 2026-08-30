use rewrite_inference::{
    InferenceError, OperationContext, ReasoningPolicy, StructuredCompletionRequest,
    StructuredCompletionResponse, UsageObservation, candidate_output_contract,
    local_judge_attempt_output_contract, parse_local_judge_attempt_output,
};

use super::super::{
    OllamaObservedPreflightError, OllamaResponseObservation, SingleConnectionTransport,
    resource_observation::{
        GenerateProviderResourceValues, OllamaGenerateResourceObservation,
        OllamaRetainedSessionSubject,
    },
    transport::ResponseHeadCheckpoint,
};
use crate::{
    OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE, OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION,
    OllamaModelBinding, OllamaPreflight, OllamaRunningModel,
    response::{
        compatibility_error, malformed_error, parse_candidates, parse_ollama_inventory,
        policy_error, single_candidate_output_policy, validate_generate_response,
    },
    wire::{GenerateOptions, GenerateRequest, GenerateResponse},
};

#[derive(Clone, Copy)]
pub(super) enum StructuredOutputProfile {
    Candidate,
    LocalJudgeAttempt,
}

pub(super) fn validate_completion_request<'a>(
    request: &StructuredCompletionRequest,
    bindings: &'a [OllamaModelBinding],
    completion_input_bytes: u32,
) -> Result<(&'a OllamaModelBinding, StructuredOutputProfile), InferenceError> {
    request
        .validate()
        .map_err(|_error| policy_error("invalid_structured_completion_request"))?;
    if u64::try_from(request.input.len()).unwrap_or(u64::MAX) > u64::from(completion_input_bytes) {
        return Err(policy_error("retained_session_input_too_large"));
    }
    let profile = if request.output == candidate_output_contract() {
        StructuredOutputProfile::Candidate
    } else if request.output == local_judge_attempt_output_contract() {
        StructuredOutputProfile::LocalJudgeAttempt
    } else {
        return Err(compatibility_error("unsupported_output_contract"));
    };
    if request.sampling.temperature != 0.0 || request.reasoning != ReasoningPolicy::Disabled {
        return Err(policy_error("nondeterministic_judge_request"));
    }
    let binding = bindings
        .iter()
        .find(|binding| binding.artifact_id() == &request.artifact_id)
        .ok_or_else(|| policy_error("artifact_not_bound"))?;
    if binding.artifact_digest() != &request.artifact_digest {
        return Err(policy_error("artifact_digest_mismatch"));
    }
    Ok((binding, profile))
}

pub(super) async fn run_completion<F, E>(
    transport: &mut SingleConnectionTransport,
    observer: &mut F,
    preflight: &OllamaPreflight,
    binding: &OllamaModelBinding,
    profile: StructuredOutputProfile,
    request: &StructuredCompletionRequest,
    context: OperationContext<'_>,
) -> Result<StructuredCompletionResponse, OllamaObservedPreflightError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    run_completion_inner(
        transport, observer, preflight, binding, profile, request, context, false, None,
    )
    .await
    .map(|outcome| outcome.response)
}

pub(super) async fn run_completion_with_residency<F, E>(
    transport: &mut SingleConnectionTransport,
    observer: &mut F,
    preflight: &OllamaPreflight,
    binding: &OllamaModelBinding,
    profile: StructuredOutputProfile,
    request: &StructuredCompletionRequest,
    context: OperationContext<'_>,
) -> Result<(StructuredCompletionResponse, OllamaRunningModel), OllamaObservedPreflightError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    if preflight.runtime.version != OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("resident_completion_runtime_unreviewed"),
        ));
    }
    if !preflight.running.is_empty() {
        return Err(OllamaObservedPreflightError::Preflight(policy_error(
            "resident_completion_requires_idle_preflight",
        )));
    }
    run_completion_inner(
        transport, observer, preflight, binding, profile, request, context, true, None,
    )
    .await
    .and_then(|outcome| {
        outcome
            .residency
            .map(|residency| (outcome.response, residency))
            .ok_or_else(|| {
                OllamaObservedPreflightError::Preflight(malformed_error(
                    "resident_completion_evidence_missing",
                ))
            })
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the retained exchange keeps its exact session subject explicit"
)]
pub(super) async fn run_completion_with_residency_and_resource_observation<F, E>(
    transport: &mut SingleConnectionTransport,
    observer: &mut F,
    preflight: &OllamaPreflight,
    binding: &OllamaModelBinding,
    profile: StructuredOutputProfile,
    request: &StructuredCompletionRequest,
    context: OperationContext<'_>,
    session_subject: &OllamaRetainedSessionSubject,
) -> Result<
    (
        StructuredCompletionResponse,
        OllamaRunningModel,
        OllamaGenerateResourceObservation,
    ),
    OllamaObservedPreflightError<E>,
>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    if preflight.runtime.version != OLLAMA_RESIDENT_COMPLETION_RUNTIME_VERSION {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("resident_completion_runtime_unreviewed"),
        ));
    }
    if !preflight.running.is_empty() {
        return Err(OllamaObservedPreflightError::Preflight(policy_error(
            "resident_completion_requires_idle_preflight",
        )));
    }
    run_completion_inner(
        transport,
        observer,
        preflight,
        binding,
        profile,
        request,
        context,
        true,
        Some(session_subject),
    )
    .await
    .and_then(|outcome| {
        let residency = outcome.residency.ok_or_else(|| {
            OllamaObservedPreflightError::Preflight(malformed_error(
                "resident_completion_evidence_missing",
            ))
        })?;
        let resource_observation = outcome.resource_observation.ok_or_else(|| {
            OllamaObservedPreflightError::Preflight(malformed_error(
                "resource_observation_unavailable",
            ))
        })?;
        Ok((outcome.response, residency, resource_observation))
    })
}

struct CompletionOutcome {
    response: StructuredCompletionResponse,
    residency: Option<OllamaRunningModel>,
    resource_observation: Option<OllamaGenerateResourceObservation>,
}

struct GenerateResourceWireParts {
    checkpoint: ResponseHeadCheckpoint,
    body: Vec<u8>,
}

type GenerateResourceParts = (ResponseHeadCheckpoint, GenerateProviderResourceValues);

#[expect(
    clippy::too_many_arguments,
    reason = "the internal exchange keeps every retained-session input explicit"
)]
async fn run_completion_inner<F, E>(
    transport: &mut SingleConnectionTransport,
    observer: &mut F,
    preflight: &OllamaPreflight,
    binding: &OllamaModelBinding,
    profile: StructuredOutputProfile,
    request: &StructuredCompletionRequest,
    context: OperationContext<'_>,
    require_residency: bool,
    resource_session_subject: Option<&OllamaRetainedSessionSubject>,
) -> Result<CompletionOutcome, OllamaObservedPreflightError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    crate::response::check_context(context).map_err(OllamaObservedPreflightError::Preflight)?;
    let runtime_before = transport.runtime_identity(context, observer).await?;
    require_runtime(preflight, &runtime_before)?;
    let tags_before = transport.tags(context, observer).await?;
    require_inventory(preflight, &tags_before)?;
    let details_before = transport
        .show_details(binding.reference(), context, observer)
        .await?;
    require_details(preflight, binding.reference(), &details_before)?;
    let wire_request = build_wire_request(binding, request, require_residency)?;
    let (generated, resource_parts) = generate(
        transport,
        observer,
        &wire_request,
        context,
        resource_session_subject.is_some(),
    )
    .await?;
    validate_generate_response(&generated, binding)
        .map_err(OllamaObservedPreflightError::Preflight)?;
    validate_structured_output(profile, request, &generated.response)?;
    let resource_parts = require_resource_values(resource_parts)?;
    let residency_after_generation = if require_residency {
        let running = transport.running_models(context, observer).await?;
        Some(require_exact_residency(binding, request, &running)?)
    } else {
        None
    };
    let runtime_after = transport.runtime_identity(context, observer).await?;
    require_runtime(preflight, &runtime_after)?;
    let tags_after = transport.tags(context, observer).await?;
    require_inventory(preflight, &tags_after)?;
    let details_after = transport
        .show_details(binding.reference(), context, observer)
        .await?;
    require_details(preflight, binding.reference(), &details_after)?;
    if runtime_before != runtime_after || details_before != details_after {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("runtime_changed_during_generation"),
        ));
    }
    if let Some(initial) = residency_after_generation.as_ref() {
        let running = transport.running_models(context, observer).await?;
        let final_residency = require_exact_residency(binding, request, &running)?;
        if initial != &final_residency {
            return Err(OllamaObservedPreflightError::Preflight(
                compatibility_error("model_residency_changed_after_generation"),
            ));
        }
    }
    transport
        .ensure_open(context)
        .await
        .map_err(OllamaObservedPreflightError::Preflight)?;
    let response = StructuredCompletionResponse::complete(
        request,
        runtime_after,
        binding.artifact_id().clone(),
        binding.artifact_digest().clone(),
        generated.response,
        UsageObservation {
            input_tokens: generated.prompt_eval_count,
            output_tokens: generated.eval_count,
            generation_micros: generated
                .eval_duration
                .and_then(|value| value.checked_div(1_000)),
        },
    )
    .map_err(|_error| {
        OllamaObservedPreflightError::Preflight(malformed_error("invalid_structured_output"))
    })?;
    let resource_observation = resource_parts.zip(resource_session_subject).map(
        |((checkpoint, values), session_subject)| {
            OllamaGenerateResourceObservation::new(session_subject, &response, checkpoint, values)
        },
    );
    Ok(CompletionOutcome {
        response,
        residency: residency_after_generation,
        resource_observation,
    })
}

fn build_wire_request<'a, E>(
    binding: &'a OllamaModelBinding,
    request: &'a StructuredCompletionRequest,
    require_residency: bool,
) -> Result<GenerateRequest<'a>, OllamaObservedPreflightError<E>> {
    let schema = serde_json::from_str(&request.output.schema_json).map_err(|_error| {
        OllamaObservedPreflightError::Preflight(policy_error("invalid_output_schema_json"))
    })?;
    Ok(GenerateRequest {
        model: binding.reference(),
        prompt: &request.input,
        stream: false,
        format: schema,
        think: false,
        raw: false,
        keep_alive: require_residency.then_some(OLLAMA_RESIDENT_COMPLETION_KEEP_ALIVE),
        options: GenerateOptions {
            temperature: 0.0,
            top_p: request.sampling.top_p,
            seed: request.sampling.seed,
            num_ctx: request.context_token_limit,
            num_predict: request.output_token_limit,
            num_gpu: require_residency.then_some(0),
            stop: Vec::new(),
        },
    })
}

fn require_resource_values<E>(
    wire_parts: Option<GenerateResourceWireParts>,
) -> Result<Option<GenerateResourceParts>, OllamaObservedPreflightError<E>> {
    wire_parts
        .map(|parts| {
            GenerateProviderResourceValues::from_body(&parts.body)
                .map(|values| (parts.checkpoint, values))
        })
        .transpose()
        .map_err(|_error| {
            OllamaObservedPreflightError::Preflight(malformed_error(
                "resource_observation_unavailable",
            ))
        })
}

async fn generate<F, E>(
    transport: &mut SingleConnectionTransport,
    observer: &mut F,
    request: &GenerateRequest<'_>,
    context: OperationContext<'_>,
    require_resource_observation: bool,
) -> Result<(GenerateResponse, Option<GenerateResourceWireParts>), OllamaObservedPreflightError<E>>
where
    F: FnMut(OllamaResponseObservation) -> Result<(), E>,
{
    if !require_resource_observation {
        return transport
            .generate(request, context, observer)
            .await
            .map(|generated| (generated, None));
    }
    let (generated, checkpoint, body) = transport
        .generate_with_response_head(request, context, observer)
        .await?;
    Ok((
        generated,
        Some(GenerateResourceWireParts { checkpoint, body }),
    ))
}

fn require_exact_residency<E>(
    binding: &OllamaModelBinding,
    request: &StructuredCompletionRequest,
    running: &[OllamaRunningModel],
) -> Result<OllamaRunningModel, OllamaObservedPreflightError<E>> {
    let [resident] = running else {
        let code = if running.is_empty() {
            "model_residency_not_observed"
        } else {
            "model_residency_ambiguous"
        };
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error(code),
        ));
    };
    if resident.reference != binding.reference()
        || resident.inventory_digest != *binding.inventory_digest()
        || resident.accelerator_bytes > resident.byte_size
        || resident.context_tokens != request.context_token_limit
    {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("model_residency_mismatch"),
        ));
    }
    Ok(resident.clone())
}

fn require_runtime<E>(
    preflight: &OllamaPreflight,
    runtime: &rewrite_model::RuntimeIdentity,
) -> Result<(), OllamaObservedPreflightError<E>> {
    if runtime != &preflight.runtime {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("runtime_changed_after_preflight"),
        ));
    }
    Ok(())
}

fn require_inventory<E>(
    preflight: &OllamaPreflight,
    tags: &crate::wire::TagsResponse,
) -> Result<(), OllamaObservedPreflightError<E>> {
    let mut inventory =
        parse_ollama_inventory(tags).map_err(OllamaObservedPreflightError::Preflight)?;
    inventory.sort_unstable_by(|left, right| left.reference.cmp(&right.reference));
    if inventory != preflight.inventory {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("inventory_changed_after_preflight"),
        ));
    }
    Ok(())
}

fn require_details<E>(
    preflight: &OllamaPreflight,
    reference: &str,
    details: &crate::OllamaModelDetails,
) -> Result<(), OllamaObservedPreflightError<E>> {
    let expected = preflight
        .bindings
        .iter()
        .find(|binding| binding.reference == reference)
        .ok_or_else(|| {
            OllamaObservedPreflightError::Preflight(policy_error("binding_not_preflighted"))
        })?;
    if details != &expected.details {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("model_details_changed_after_preflight"),
        ));
    }
    if !details
        .capabilities
        .iter()
        .any(|capability| capability == "completion")
    {
        return Err(OllamaObservedPreflightError::Preflight(
            compatibility_error("completion_not_supported"),
        ));
    }
    Ok(())
}

fn validate_structured_output<E>(
    profile: StructuredOutputProfile,
    request: &StructuredCompletionRequest,
    output: &str,
) -> Result<(), OllamaObservedPreflightError<E>> {
    match profile {
        StructuredOutputProfile::Candidate => {
            let policy = single_candidate_output_policy(request.output_byte_limit)
                .map_err(OllamaObservedPreflightError::Preflight)?;
            parse_candidates(output.as_bytes(), policy)
                .map(|_candidates| ())
                .map_err(OllamaObservedPreflightError::Preflight)
        }
        StructuredOutputProfile::LocalJudgeAttempt => parse_local_judge_attempt_output(output)
            .map(|_output| ())
            .map_err(|_error| {
                OllamaObservedPreflightError::Preflight(malformed_error(
                    "invalid_local_judge_attempt_output",
                ))
            }),
    }
}
