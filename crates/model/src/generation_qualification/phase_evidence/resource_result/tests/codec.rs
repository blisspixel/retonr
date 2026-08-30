use super::*;
use rewrite_types::Digest;
use serde_json::{Map, Value};

#[test]
fn decoder_rejects_every_field_substitution() {
    let fixture = fixture();
    let input = input();
    let relations = fixture.relations();
    let result = GenerationResourceAttemptResultRecordV1::new(relations, input.clone())
        .expect("resource result");
    let encoded = serde_json::to_vec(&result).expect("resource JSON");
    let other = Value::String(Digest::sha256(b"substitution").as_str().to_owned());
    let substitutions = [
        ("schema_version", Value::from(2)),
        ("generation_system_id", other.clone()),
        ("generation_qualification_plan_id", other.clone()),
        ("suite_manifest_id", other.clone()),
        ("case_id", other.clone()),
        ("repetition_id", other.clone()),
        ("planned_attempt_id", other.clone()),
        ("attempt_record_id", other.clone()),
        ("candidate_generation_receipt_id", other.clone()),
        ("resource_policy_digest", other),
        (
            "observation_profile",
            Value::String("future_profile".to_owned()),
        ),
        ("prompt_token_count", Value::from(12)),
        ("generated_token_count", Value::from(18)),
        ("total_duration_nanoseconds", Value::from(30_000_001_u64)),
        ("load_duration_nanoseconds", Value::from(1_000_001_u64)),
        (
            "prompt_evaluation_duration_nanoseconds",
            Value::from(2_000_001_u64),
        ),
        (
            "evaluation_duration_nanoseconds",
            Value::from(23_000_998_u64),
        ),
        ("attempt_elapsed_nanoseconds", Value::from(40_000_001_u64)),
        (
            "first_response_elapsed_nanoseconds",
            Value::from(5_000_001_u64),
        ),
        ("cleanup_elapsed_nanoseconds", Value::from(4_000_001_u64)),
        ("worker_high_water_resident_bytes", Value::from(8_193_u64)),
        ("runtime_installed_payload_bytes", Value::from(101_u64)),
        ("model_installed_payload_bytes", Value::from(899_u64)),
        ("installed_footprint_bytes", Value::from(1_001_u64)),
        ("exceeded_limits", serde_json::json!(["attempt_elapsed"])),
    ];
    for (field, replacement) in substitutions {
        let changed = substitute(&encoded, field, replacement);
        let decoded =
            GenerationResourceAttemptResultRecordV1::from_json_bytes(&changed, relations, &input);
        assert!(decoded.is_err(), "field {field} accepted substitution");
    }
}

#[test]
fn decoder_is_bounded_strict_canonical_and_schema_first() {
    let fixture = fixture();
    let input = input();
    let relations = fixture.relations();
    let result = GenerationResourceAttemptResultRecordV1::new(relations, input.clone())
        .expect("resource result");
    let encoded = serde_json::to_vec(&result).expect("resource JSON");
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES + 1],
            relations,
            &input,
        ),
        Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(b"{", relations, &input),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    let duplicate = String::from_utf8(encoded.clone()).expect("UTF-8").replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(
            duplicate.as_bytes(),
            relations,
            &input,
        ),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    let mut unknown: Value = serde_json::from_slice(&encoded).expect("JSON value");
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown JSON"),
            relations,
            &input,
        ),
        Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
    );
    let mut trailing = encoded.clone();
    trailing.push(b' ');
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(&trailing, relations, &input),
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    );
    let reordered = reorder_first_two(&encoded);
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(&reordered, relations, &input),
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    );
    let future = substitute(&encoded, "schema_version", Value::from(2));
    assert_eq!(
        GenerationResourceAttemptResultRecordV1::from_json_bytes(&future, relations, &input),
        Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema)
    );
}

#[test]
fn decoder_rejects_unknown_profile_limit_and_missing_required_observation() {
    let fixture = fixture();
    let input = input();
    let relations = fixture.relations();
    let result = GenerationResourceAttemptResultRecordV1::new(relations, input.clone())
        .expect("resource result");
    let encoded = serde_json::to_vec(&result).expect("resource JSON");
    for changed in [
        substitute(
            &encoded,
            "observation_profile",
            Value::String("managed_other".to_owned()),
        ),
        substitute(
            &encoded,
            "exceeded_limits",
            serde_json::json!(["unknown_limit"]),
        ),
        without_field(&encoded, "prompt_token_count"),
    ] {
        assert_eq!(
            GenerationResourceAttemptResultRecordV1::from_json_bytes(&changed, relations, &input,),
            Err(GenerationQualificationPhaseEvidenceError::InvalidEncoding)
        );
    }
}

fn reorder_first_two(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8(bytes.to_vec()).expect("UTF-8");
    let first = text.find(',').expect("first field");
    let second = text[first + 1..]
        .find(',')
        .map(|index| index + first + 1)
        .expect("second field");
    format!(
        "{{{},{},{}",
        &text[first + 1..second],
        &text[1..first],
        &text[second + 1..]
    )
    .into_bytes()
}

fn without_field(bytes: &[u8], field: &str) -> Vec<u8> {
    let value: Value = serde_json::from_slice(bytes).expect("JSON value");
    let mut object: Map<String, Value> = value.as_object().expect("object").clone();
    object.remove(field).expect("field exists");
    serde_json::to_vec(&Value::Object(object)).expect("JSON")
}
