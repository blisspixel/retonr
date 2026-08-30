use rewrite_inference::{
    OutputContract, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest, StructuredCompletionResponse,
    UsageObservation,
};
use rewrite_model::{ArtifactId, RuntimeIdentity};
use rewrite_types::Digest;

use super::{
    OllamaResidentSessionExecutionReceipt, OllamaSessionExecutionReceipt,
    derive_ollama_retained_session_response_id, preflight_binding_digest,
    resident_completion_contract_digest, response_binding_digest,
};
use crate::contract::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
    OllamaRunningModel,
};

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn request() -> StructuredCompletionRequest {
    let artifact_digest = digest("artifact");
    let schema_json = "{\"type\":\"object\"}".to_owned();
    StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(artifact_digest.clone()),
        artifact_digest,
        input: "bounded input".to_owned(),
        output: OutputContract {
            schema_digest: Digest::sha256(schema_json.as_bytes()),
            schema_json,
        },
        source_byte_count: 13,
        source_byte_limit: 1_024,
        input_byte_limit: 2_048,
        context_token_limit: 4_096,
        output_token_limit: 256,
        output_byte_limit: 1_024,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

fn runtime() -> RuntimeIdentity {
    RuntimeIdentity {
        backend: "ollama_native".to_owned(),
        version: "0.32.15".to_owned(),
        digest: Some(digest("runtime")),
    }
}

fn usage() -> UsageObservation {
    UsageObservation {
        input_tokens: Some(11),
        output_tokens: Some(3),
        generation_micros: Some(1_250),
    }
}

fn response_from(
    request: &StructuredCompletionRequest,
    runtime: RuntimeIdentity,
    output: &str,
    usage: UsageObservation,
) -> StructuredCompletionResponse {
    StructuredCompletionResponse::complete(
        request,
        runtime,
        request.artifact_id.clone(),
        request.artifact_digest.clone(),
        output.to_owned(),
        usage,
    )
    .expect("valid response fixture")
}

fn response() -> StructuredCompletionResponse {
    response_from(
        &request(),
        runtime(),
        r#"{"candidates":[{"text":"ok"}]}"#,
        usage(),
    )
}

fn running() -> OllamaRunningModel {
    OllamaRunningModel {
        reference: "fixture:latest".to_owned(),
        inventory_digest: digest("inventory"),
        byte_size: 4_096,
        accelerator_bytes: 512,
        context_tokens: 4_096,
    }
}

fn preflight() -> OllamaPreflight {
    let running = running();
    OllamaPreflight {
        runtime: runtime(),
        inventory: vec![OllamaInventoryEntry {
            reference: running.reference.clone(),
            inventory_digest: running.inventory_digest.clone(),
            byte_size: running.byte_size,
        }],
        bindings: vec![OllamaPreflightBinding {
            reference: running.reference.clone(),
            inventory_digest: running.inventory_digest.clone(),
            details: OllamaModelDetails {
                format: "gguf".to_owned(),
                family: "llama".to_owned(),
                quantization: "Q4_K_M".to_owned(),
                capabilities: vec!["completion".to_owned()],
                license_digest: digest("license"),
                template_digest: digest("template"),
                metadata_digest: digest("metadata"),
            },
        }],
        running: vec![running],
    }
}

fn changed<T: Clone>(original: &T, mutate: impl FnOnce(&mut T)) -> T {
    let mut changed = original.clone();
    mutate(&mut changed);
    changed
}

#[path = "tests/complete_binding.rs"]
mod complete_binding;

#[test]
fn retained_identity_literals_lock_preflight_request_response_and_residency() {
    let preflight = preflight();
    let response = response();
    let execution = OllamaSessionExecutionReceipt::new(&preflight, &response, 8, 16)
        .expect("execution receipt");
    let residency = OllamaResidentSessionExecutionReceipt::new(execution, &running(), 12, 16);

    assert_eq!(
        residency.execution().preflight_digest().as_str(),
        "346f7a8215f31604033d7681fd0ac8615387fc91812e25caa191ffd6f26f2a1e"
    );
    assert_eq!(
        residency.execution().request_digest().as_str(),
        "e92f1b5e4a45cffb360c8d803084174fae8cae78a2194d69215508169fbfdbf1"
    );
    assert_eq!(
        residency.execution().response_digest().as_str(),
        "79a00c407d0c7ed4becc028d4df0273c3a02b1d7908ccc917abf38c0600f9544"
    );
    assert_eq!(
        derive_ollama_retained_session_response_id(&response)
            .digest()
            .as_str(),
        "79a00c407d0c7ed4becc028d4df0273c3a02b1d7908ccc917abf38c0600f9544"
    );
    assert_eq!(
        residency
            .execution()
            .retained_response_id()
            .digest()
            .as_str(),
        "79a00c407d0c7ed4becc028d4df0273c3a02b1d7908ccc917abf38c0600f9544"
    );
    assert_eq!(
        residency.residency_contract_digest().as_str(),
        "4967dc9289c555ec8f3cafa86a5ddec55e9b45b18aad02e111751e3af1b7e895"
    );
    assert_eq!(
        residency.residency_observation_digest().as_str(),
        "46b675b2838360ae42a31727c2c17268b04239f6b777314d572280c1ae4191ea"
    );
    assert_eq!(
        residency.runtime_reference_digest().as_str(),
        "b2025b51064de982f7e02ad4eca54ddcd7ba0a21adda58c3411d13ad4f2647ff"
    );
    assert_eq!(residency.execution().first_response_ordinal(), 8);
    assert_eq!(residency.execution().last_response_ordinal(), 16);
    assert_eq!(residency.first_residency_ordinal(), 12);
    assert_eq!(residency.last_residency_ordinal(), 16);
    assert_eq!(residency.inventory_digest(), &digest("inventory"));
    assert_eq!(residency.byte_size(), 4_096);
    assert_eq!(residency.accelerator_bytes(), 512);
    assert_eq!(residency.context_tokens(), 4_096);
    assert!(residency.runtime_reported_residency_proven());
    assert!(!residency.application_handler_proven());
    assert!(!residency.model_use_proven());
    assert!(!residency.resident_page_identity_proven());
    assert!(!residency.effective_runtime_identity_proven());
    assert!(!residency.qualified());
    let debug = format!("{residency:?}");
    assert!(debug.contains("residency_observation_digest"));
    assert!(!debug.contains("bounded input"));
    assert!(!debug.contains("candidates"));
}

#[test]
fn retained_response_id_wraps_only_the_frozen_response_binding() {
    let exact_response = response();
    let original = OllamaSessionExecutionReceipt::new(&preflight(), &exact_response, 8, 16)
        .expect("execution receipt");
    assert_eq!(
        OllamaSessionExecutionReceipt::for_test(&preflight(), &exact_response, 8, 16)
            .expect("test-support execution receipt"),
        original
    );
    let changed_ordinals = OllamaSessionExecutionReceipt::new(&preflight(), &exact_response, 9, 17)
        .expect("receipt with changed ordinals");
    let changed_preflight = OllamaSessionExecutionReceipt::new(
        &changed(&preflight(), |value| value.runtime.version.push('2')),
        &exact_response,
        8,
        16,
    )
    .expect("receipt with changed preflight");
    let changed_response = OllamaSessionExecutionReceipt::new(
        &preflight(),
        &response_from(
            &request(),
            runtime(),
            r#"{"candidates":[{"text":"other"}]}"#,
            usage(),
        ),
        8,
        16,
    )
    .expect("receipt with changed response");

    assert_eq!(
        original.retained_response_id().digest(),
        original.response_digest()
    );
    assert_eq!(
        changed_ordinals.retained_response_id(),
        original.retained_response_id()
    );
    assert_eq!(
        changed_preflight.retained_response_id(),
        original.retained_response_id()
    );
    assert_ne!(
        changed_response.retained_response_id(),
        original.retained_response_id()
    );
    let debug = format!("{original:?}");
    assert!(!debug.contains("bounded input"));
    assert!(!debug.contains("candidates"));
}

#[test]
fn every_preflight_field_changes_its_binding() {
    let original = preflight();
    let original_digest = preflight_binding_digest(&original).expect("preflight digest");
    let variants = [
        changed(&original, |value| value.runtime.backend.push('2')),
        changed(&original, |value| value.runtime.version.push('1')),
        changed(&original, |value| value.runtime.digest = None),
        changed(&original, |value| {
            value.runtime.digest = Some(digest("other runtime"));
        }),
        changed(&original, |value| value.inventory[0].reference.push('2')),
        changed(&original, |value| {
            value.inventory[0].inventory_digest = digest("other inventory");
        }),
        changed(&original, |value| value.inventory[0].byte_size += 1),
        changed(&original, |value| {
            value.inventory.push(value.inventory[0].clone());
        }),
        changed(&original, |value| value.bindings[0].reference.push('2')),
        changed(&original, |value| {
            value.bindings[0].inventory_digest = digest("other binding inventory");
        }),
        changed(&original, |value| {
            value.bindings[0].details.format.push('2');
        }),
        changed(&original, |value| {
            value.bindings[0].details.family.push('2');
        }),
        changed(&original, |value| {
            value.bindings[0].details.quantization.push('2');
        }),
        changed(&original, |value| {
            value.bindings[0]
                .details
                .capabilities
                .push("other".to_owned());
        }),
        changed(&original, |value| {
            value.bindings[0].details.license_digest = digest("other license");
        }),
        changed(&original, |value| {
            value.bindings[0].details.template_digest = digest("other template");
        }),
        changed(&original, |value| {
            value.bindings[0].details.metadata_digest = digest("other metadata");
        }),
        changed(&original, |value| {
            value.bindings.push(value.bindings[0].clone());
        }),
        changed(&original, |value| value.running[0].reference.push('2')),
        changed(&original, |value| {
            value.running[0].inventory_digest = digest("other running inventory");
        }),
        changed(&original, |value| value.running[0].byte_size += 1),
        changed(&original, |value| value.running[0].accelerator_bytes += 1),
        changed(&original, |value| value.running[0].context_tokens += 1),
        changed(&original, |value| {
            value.running.push(value.running[0].clone());
        }),
    ];

    for (index, variant) in variants.iter().enumerate() {
        assert_ne!(
            preflight_binding_digest(variant).expect("variant preflight digest"),
            original_digest,
            "preflight field variant {index} did not change the binding"
        );
    }
}

#[test]
fn every_structured_response_field_changes_its_binding() {
    let original = response();
    let original_digest = response_binding_digest(&original);
    let base_request = request();
    let mut other_artifact_request = base_request.clone();
    other_artifact_request.artifact_digest = digest("other artifact");
    other_artifact_request.artifact_id =
        ArtifactId::from_digest(other_artifact_request.artifact_digest.clone());
    let mut other_request = base_request.clone();
    other_request.input.push('2');
    let variants = [
        response_from(
            &base_request,
            changed(&runtime(), |value| value.backend.push('2')),
            original.output_json(),
            usage(),
        ),
        response_from(
            &base_request,
            changed(&runtime(), |value| value.version.push('2')),
            original.output_json(),
            usage(),
        ),
        response_from(
            &base_request,
            changed(&runtime(), |value| value.digest = None),
            original.output_json(),
            usage(),
        ),
        response_from(
            &base_request,
            changed(&runtime(), |value| {
                value.digest = Some(digest("other runtime"));
            }),
            original.output_json(),
            usage(),
        ),
        response_from(
            &other_artifact_request,
            runtime(),
            original.output_json(),
            usage(),
        ),
        response_from(&other_request, runtime(), original.output_json(), usage()),
        response_from(
            &base_request,
            runtime(),
            r#"{ "candidates": [{"text":"ok"}] }"#,
            usage(),
        ),
        response_from(
            &base_request,
            runtime(),
            original.output_json(),
            changed(&usage(), |value| value.input_tokens = None),
        ),
        response_from(
            &base_request,
            runtime(),
            original.output_json(),
            changed(&usage(), |value| value.input_tokens = Some(12)),
        ),
        response_from(
            &base_request,
            runtime(),
            original.output_json(),
            changed(&usage(), |value| value.output_tokens = None),
        ),
        response_from(
            &base_request,
            runtime(),
            original.output_json(),
            changed(&usage(), |value| value.output_tokens = Some(4)),
        ),
        response_from(
            &base_request,
            runtime(),
            original.output_json(),
            changed(&usage(), |value| value.generation_micros = None),
        ),
        response_from(
            &base_request,
            runtime(),
            original.output_json(),
            changed(&usage(), |value| value.generation_micros = Some(1_251)),
        ),
    ];

    for (index, variant) in variants.iter().enumerate() {
        assert_ne!(
            response_binding_digest(variant),
            original_digest,
            "response field variant {index} did not change the binding"
        );
    }
}

#[test]
fn every_residency_observation_field_changes_its_binding() {
    let original_execution = OllamaSessionExecutionReceipt::new(&preflight(), &response(), 8, 16)
        .expect("execution receipt");
    let original_running = running();
    let original = OllamaResidentSessionExecutionReceipt::new(
        original_execution.clone(),
        &original_running,
        12,
        16,
    );
    let changed_response = response_from(
        &request(),
        runtime(),
        r#"{"candidates":[{"text":"other"}]}"#,
        usage(),
    );
    let changed_execution =
        OllamaSessionExecutionReceipt::new(&preflight(), &changed_response, 8, 16)
            .expect("changed execution receipt");
    let variants = [
        OllamaResidentSessionExecutionReceipt::new(changed_execution, &original_running, 12, 16),
        OllamaResidentSessionExecutionReceipt::new(
            original_execution.clone(),
            &changed(&original_running, |value| value.reference.push('2')),
            12,
            16,
        ),
        OllamaResidentSessionExecutionReceipt::new(
            original_execution.clone(),
            &changed(&original_running, |value| {
                value.inventory_digest = digest("other inventory");
            }),
            12,
            16,
        ),
        OllamaResidentSessionExecutionReceipt::new(
            original_execution.clone(),
            &changed(&original_running, |value| value.byte_size += 1),
            12,
            16,
        ),
        OllamaResidentSessionExecutionReceipt::new(
            original_execution.clone(),
            &changed(&original_running, |value| value.accelerator_bytes += 1),
            12,
            16,
        ),
        OllamaResidentSessionExecutionReceipt::new(
            original_execution.clone(),
            &changed(&original_running, |value| value.context_tokens += 1),
            12,
            16,
        ),
        OllamaResidentSessionExecutionReceipt::new(
            original_execution.clone(),
            &original_running,
            13,
            16,
        ),
        OllamaResidentSessionExecutionReceipt::new(original_execution, &original_running, 12, 17),
    ];

    assert_eq!(
        original.residency_contract_digest(),
        &resident_completion_contract_digest()
    );
    for (index, variant) in variants.iter().enumerate() {
        assert_ne!(
            variant.residency_observation_digest(),
            original.residency_observation_digest(),
            "residency field variant {index} did not change the binding"
        );
    }
}
