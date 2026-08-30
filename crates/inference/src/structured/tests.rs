use rewrite_model::ArtifactId;
use rewrite_types::Digest;

use crate::{
    OutputContract, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest,
};

fn request() -> StructuredCompletionRequest {
    let artifact_digest = Digest::sha256(b"artifact");
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

fn changed(
    original: &StructuredCompletionRequest,
    mutate: impl FnOnce(&mut StructuredCompletionRequest),
) -> StructuredCompletionRequest {
    let mut changed = original.clone();
    mutate(&mut changed);
    changed
}

#[test]
fn structured_request_binding_has_a_literal_vector_and_json_contract() {
    let request = request();
    assert_eq!(
        request.binding_digest().as_str(),
        "e92f1b5e4a45cffb360c8d803084174fae8cae78a2194d69215508169fbfdbf1"
    );
    assert_eq!(
        request.structured_request_binding_id().digest(),
        &request.binding_digest()
    );
    assert_eq!(
        serde_json::to_string(&request).expect("request serializes"),
        concat!(
            "{\"schema_version\":1,",
            "\"artifact_id\":\"c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c\",",
            "\"artifact_digest\":\"c7c5c1d70c5dec4416ab6158afd0b223ef40c29b1dc1f97ed9428b94d4cadb1c\",",
            "\"input\":\"bounded input\",",
            "\"output\":{\"schema_digest\":\"a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0\",",
            "\"schema_json\":\"{\\\"type\\\":\\\"object\\\"}\"},",
            "\"source_byte_count\":13,\"source_byte_limit\":1024,",
            "\"input_byte_limit\":2048,\"context_token_limit\":4096,",
            "\"output_token_limit\":256,\"output_byte_limit\":1024,",
            "\"sampling\":{\"temperature\":0.0,\"top_p\":1.0,\"seed\":7},",
            "\"reasoning\":\"disabled\"}"
        )
    );
}

#[test]
fn every_structured_request_field_changes_the_binding() {
    let original = request();
    let original_digest = original.binding_digest();
    let variants = [
        changed(&original, |value| value.schema_version = 2),
        changed(&original, |value| {
            value.artifact_id = ArtifactId::from_digest(Digest::sha256(b"other artifact id"));
        }),
        changed(&original, |value| {
            value.artifact_digest = Digest::sha256(b"other artifact digest");
        }),
        changed(&original, |value| value.input.push('!')),
        changed(&original, |value| {
            value.output.schema_digest = Digest::sha256(b"other schema digest");
        }),
        changed(&original, |value| value.output.schema_json.push(' ')),
        changed(&original, |value| value.source_byte_count += 1),
        changed(&original, |value| value.source_byte_limit += 1),
        changed(&original, |value| value.input_byte_limit += 1),
        changed(&original, |value| value.context_token_limit += 1),
        changed(&original, |value| value.output_token_limit += 1),
        changed(&original, |value| value.output_byte_limit += 1),
        changed(&original, |value| value.sampling.temperature = 0.5),
        changed(&original, |value| value.sampling.top_p = 0.5),
        changed(&original, |value| value.sampling.seed = None),
        changed(&original, |value| {
            value.reasoning = ReasoningPolicy::Discard;
        }),
    ];

    for (index, variant) in variants.iter().enumerate() {
        assert_ne!(
            variant.binding_digest(),
            original_digest,
            "field variant {index} did not change the binding"
        );
    }
}

#[test]
fn structured_request_allows_masked_input_shorter_than_original_source() {
    let mut value = request();
    value.source_byte_count = u64::try_from(value.input.len()).expect("input byte count") + 1;
    value
        .validate()
        .expect("masked input may be shorter than the original source");

    value.source_byte_count = value.source_byte_limit + 1;
    assert_eq!(value.validate(), Err(crate::ContractError::InvalidLimits));
}
