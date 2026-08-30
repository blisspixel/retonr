use rewrite_inference::{
    OutputContract, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters,
};
use rewrite_ollama::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
    OllamaRunningModel,
};
use serde_json::json;

use super::*;

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
    output_json: &str,
    usage: UsageObservation,
) -> StructuredCompletionResponse {
    StructuredCompletionResponse::complete(
        request,
        runtime,
        request.artifact_id.clone(),
        request.artifact_digest.clone(),
        output_json.to_owned(),
        usage,
    )
    .expect("valid structured response fixture")
}

fn response() -> StructuredCompletionResponse {
    response_from(
        &request(),
        runtime(),
        r#"{"candidates":[{"text":"ok"}]}"#,
        usage(),
    )
}

fn preflight() -> OllamaPreflight {
    let running = OllamaRunningModel {
        reference: "fixture:latest".to_owned(),
        inventory_digest: digest("inventory"),
        byte_size: 4_096,
        accelerator_bytes: 512,
        context_tokens: 4_096,
    };
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

fn receipt(response: &StructuredCompletionResponse) -> OllamaSessionExecutionReceipt {
    OllamaSessionExecutionReceipt::for_test(&preflight(), response, 8, 16)
        .expect("retained response receipt fixture")
}

fn artifact() -> RetainedStructuredResponseArtifactV1 {
    let response = response();
    RetainedStructuredResponseArtifactV1::from_retained_response(&response, &receipt(&response))
        .expect("compile response artifact fixture")
}

#[test]
fn canonical_bytes_and_artifact_identity_are_frozen() {
    let artifact = artifact();
    let expected = concat!(
        "{\"schema_version\":1,",
        "\"response_id\":\"79a00c407d0c7ed4becc028d4df0273c3a02b1d7908ccc917abf38c0600f9544\",",
        "\"runtime\":{\"backend\":\"ollama_native\",\"version\":\"0.32.15\",",
        "\"digest\":\"d92c6a81b2ff50096bcda80885427d1f59a25b5f483f7055523504925d16ab23\"},",
        "\"artifact_id\":\"c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c\",",
        "\"artifact_digest\":\"c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c\",",
        "\"structured_request_binding_id\":",
        "\"e92f1b5e4a45cffb360c8d803084174fae8cae78a2194d69215508169fbfdbf1\",",
        "\"output_json\":\"{\\\"candidates\\\":[{\\\"text\\\":\\\"ok\\\"}]}\",",
        "\"usage\":{\"input_tokens\":11,\"output_tokens\":3,",
        "\"generation_micros\":1250},\"finish\":\"complete\"}"
    );

    assert_eq!(artifact.canonical_json_bytes(), expected.as_bytes());
    assert_eq!(artifact.byte_size(), 644);
    assert_eq!(
        artifact.content_artifact_id().digest().as_str(),
        "ef6de706c465094b1f913658430b4a8080cfb883105d06d782c49a01d90a4c64"
    );
    assert_eq!(
        artifact.response_id().digest().as_str(),
        "79a00c407d0c7ed4becc028d4df0273c3a02b1d7908ccc917abf38c0600f9544"
    );
    assert_eq!(artifact.output_json(), r#"{"candidates":[{"text":"ok"}]}"#);
    assert_eq!(
        artifact.structured_request_binding_id(),
        &request().structured_request_binding_id()
    );
}

#[test]
fn exact_artifact_round_trips_and_debug_redacts_content() {
    let artifact = artifact();
    let decoded = RetainedStructuredResponseArtifactV1::from_json_bytes(
        artifact.canonical_json_bytes(),
        &request(),
        artifact.response_id(),
    )
    .expect("decode exact artifact");

    assert_eq!(decoded, artifact);
    assert_eq!(decoded.schema_version(), 1);
    assert_eq!(decoded.usage(), usage());
    assert_eq!(decoded.provider_artifact_id(), &request().artifact_id);
    assert_eq!(
        decoded
            .reconstruct_response(&request())
            .expect("reconstruct exact response"),
        response()
    );
    assert_eq!(
        serde_json::to_vec(&decoded).expect("serialize decoded artifact"),
        decoded.canonical_json_bytes()
    );
    let debug = format!("{decoded:?}");
    assert!(!debug.contains("bounded input"));
    assert!(!debug.contains("candidates"));
    assert!(!debug.contains("ollama_native"));
}

#[test]
fn reconstructed_response_rejects_a_substituted_request() {
    let artifact = artifact();
    let mut wrong_request = request();
    wrong_request.input.push('2');
    assert!(matches!(
        artifact.reconstruct_response(&wrong_request),
        Err(RetainedStructuredResponseArtifactError::RelationshipMismatch)
    ));
}

#[test]
fn constructor_rejects_a_receipt_for_another_response() {
    let exact = response();
    let other = response_from(
        &request(),
        runtime(),
        r#"{"candidates":[{"text":"other"}]}"#,
        usage(),
    );
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_retained_response(&exact, &receipt(&other)),
        Err(RetainedStructuredResponseArtifactError::RelationshipMismatch)
    ));
}

#[test]
fn decoder_rejects_bounds_malformed_unknown_duplicate_and_noncanonical_json() {
    let artifact = artifact();
    let bytes = artifact.canonical_json_bytes();
    assert!(validate_encoded_size(MAX_RETAINED_STRUCTURED_RESPONSE_ARTIFACT_JSON_BYTES).is_ok());
    assert!(matches!(
        validate_encoded_size(MAX_RETAINED_STRUCTURED_RESPONSE_ARTIFACT_JSON_BYTES + 1),
        Err(RetainedStructuredResponseArtifactError::EncodedRecordTooLarge)
    ));

    for invalid in [
        Vec::new(),
        b"{}".to_vec(),
        [bytes, b" trailing"].concat(),
        bytes[..bytes.len() - 1].to_vec(),
    ] {
        assert!(matches!(
            RetainedStructuredResponseArtifactV1::from_json_bytes(
                &invalid,
                &request(),
                artifact.response_id()
            ),
            Err(RetainedStructuredResponseArtifactError::InvalidEncoding)
        ));
    }

    let prefixed = [b" ".as_slice(), bytes].concat();
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            &prefixed,
            &request(),
            artifact.response_id()
        ),
        Err(RetainedStructuredResponseArtifactError::NonCanonicalEncoding)
    ));

    let text = std::str::from_utf8(bytes).expect("artifact is UTF-8");
    let unknown = text.replacen(
        "{\"schema_version\":1,",
        "{\"schema_version\":1,\"unknown\":true,",
        1,
    );
    let duplicate = text.replacen(
        "{\"schema_version\":1,",
        "{\"schema_version\":1,\"schema_version\":1,",
        1,
    );
    for invalid in [unknown, duplicate] {
        assert!(matches!(
            RetainedStructuredResponseArtifactV1::from_json_bytes(
                invalid.as_bytes(),
                &request(),
                artifact.response_id()
            ),
            Err(RetainedStructuredResponseArtifactError::InvalidEncoding)
        ));
    }
}

#[test]
fn future_schema_precedes_relationship_and_canonicality_checks() {
    let artifact = artifact();
    let changed = std::str::from_utf8(artifact.canonical_json_bytes())
        .expect("artifact is UTF-8")
        .replacen("\"schema_version\":1", "\"schema_version\":2", 1);
    let wrong_id = OllamaRetainedSessionResponseId::from_derived_digest(digest("wrong"));
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            changed.as_bytes(),
            &request(),
            &wrong_id
        ),
        Err(RetainedStructuredResponseArtifactError::UnsupportedSchema(
            2
        ))
    ));
}

#[test]
fn decoder_rejects_request_response_and_payload_substitution() {
    let artifact = artifact();
    let mut wrong_request = request();
    wrong_request.input.push('2');
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            artifact.canonical_json_bytes(),
            &wrong_request,
            artifact.response_id()
        ),
        Err(RetainedStructuredResponseArtifactError::RelationshipMismatch)
    ));
    let wrong_id = OllamaRetainedSessionResponseId::from_derived_digest(digest("wrong"));
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            artifact.canonical_json_bytes(),
            &request(),
            &wrong_id
        ),
        Err(RetainedStructuredResponseArtifactError::RelationshipMismatch)
    ));

    let mut value: serde_json::Value =
        serde_json::from_slice(artifact.canonical_json_bytes()).expect("parse artifact fixture");
    value["output_json"] = json!(r#"{"candidates":[{"text":"other"}]}"#);
    let substituted = serde_json::to_vec(&value).expect("serialize substituted artifact");
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            &substituted,
            &request(),
            artifact.response_id()
        ),
        Err(RetainedStructuredResponseArtifactError::RelationshipMismatch)
    ));

    let mut invalid_artifact: serde_json::Value =
        serde_json::from_slice(artifact.canonical_json_bytes()).expect("parse artifact fixture");
    invalid_artifact["artifact_digest"] = json!(digest("substituted artifact"));
    let invalid_artifact =
        serde_json::to_vec(&invalid_artifact).expect("serialize invalid artifact binding");
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            &invalid_artifact,
            &request(),
            artifact.response_id()
        ),
        Err(RetainedStructuredResponseArtifactError::ResponseContract(_))
    ));

    value["finish"] = json!("partial");
    let invalid_finish = serde_json::to_vec(&value).expect("serialize invalid finish");
    assert!(matches!(
        RetainedStructuredResponseArtifactV1::from_json_bytes(
            &invalid_finish,
            &request(),
            artifact.response_id()
        ),
        Err(RetainedStructuredResponseArtifactError::InvalidEncoding)
    ));
}

#[test]
fn every_serialized_field_changes_the_content_artifact_identity() {
    let original = artifact();
    let original_id = serialized_artifact_id(&original);
    let variants = [
        changed(&original, |value| value.schema_version += 1),
        changed(&original, |value| {
            value.response_id =
                OllamaRetainedSessionResponseId::from_derived_digest(digest("other response"));
        }),
        changed(&original, |value| value.runtime.backend.push('2')),
        changed(&original, |value| value.runtime.version.push('2')),
        changed(&original, |value| value.runtime.digest = None),
        changed(&original, |value| {
            value.artifact_id = ArtifactId::from_digest(digest("other artifact id"));
        }),
        changed(&original, |value| {
            value.artifact_digest = digest("other artifact digest");
        }),
        changed(&original, |value| {
            value.structured_request_binding_id =
                StructuredCompletionRequestBindingId::from_derived_digest(digest("other request"));
        }),
        changed(&original, |value| value.output_json.push(' ')),
        changed(&original, |value| value.usage.input_tokens = None),
        changed(&original, |value| value.usage.output_tokens = None),
        changed(&original, |value| value.usage.generation_micros = None),
    ];
    for (index, variant) in variants.iter().enumerate() {
        assert_ne!(
            serialized_artifact_id(variant),
            original_id,
            "serialized field variant {index} did not change the artifact identity"
        );
    }
    let changed_finish = std::str::from_utf8(original.canonical_json_bytes())
        .expect("artifact is UTF-8")
        .replacen("\"finish\":\"complete\"", "\"finish\":\"partial\"", 1);
    assert_ne!(
        ArtifactId::from_digest(Digest::sha256(changed_finish.as_bytes())),
        original_id
    );
}

fn changed(
    original: &RetainedStructuredResponseArtifactV1,
    mutate: impl FnOnce(&mut RetainedStructuredResponseArtifactV1),
) -> RetainedStructuredResponseArtifactV1 {
    let mut value = original.clone();
    mutate(&mut value);
    value
}

fn serialized_artifact_id(value: &RetainedStructuredResponseArtifactV1) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(
        &serde_json::to_vec(value).expect("serialize field-sensitivity fixture"),
    ))
}
