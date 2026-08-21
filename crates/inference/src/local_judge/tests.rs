use rewrite_types::Digest;
use serde_json::{Value, json};

use super::*;

fn valid_value() -> Value {
    json!({
        "schema_version": 1,
        "case_id": "case_01",
        "choice": "first",
        "rubric_clauses": ["clarity", "meaning"],
        "source_spans": [{"start": 0, "end": 4}, {"start": 4, "end": 8}],
        "first_candidate_spans": [{"start": 2, "end": 7}],
        "second_candidate_spans": [],
    })
}

fn parse_value(value: &Value) -> Result<LocalJudgeAttemptOutput, LocalJudgeAttemptOutputError> {
    parse_local_judge_attempt_output(&serde_json::to_string(value).expect("encode fixture"))
}

#[test]
fn canonical_schema_has_a_golden_digest_and_exact_constraints() {
    let contract = local_judge_attempt_output_contract();
    assert_eq!(
        contract.schema_digest,
        Digest::from_sha256_hex("985c6143582f07b051b7f8b0c2ef4f9eb6d44e0468cbc94011df4a44e8a2c1bd")
            .expect("golden schema digest")
    );
    assert_eq!(
        contract.schema_digest,
        Digest::sha256(contract.schema_json.as_bytes())
    );
    let schema: Value = serde_json::from_str(&contract.schema_json).expect("canonical schema");
    assert_eq!(schema["properties"]["schema_version"]["const"], 1);
    assert_eq!(
        schema["properties"]["choice"]["enum"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
    assert_eq!(schema["properties"]["rubric_clauses"]["maxItems"], 32);
    assert_eq!(schema["$defs"]["spans"]["maxItems"], 32);
    assert_eq!(schema["additionalProperties"], false);
    contract.validate().expect("valid output contract");
}

#[test]
fn parses_valid_output_and_preserves_half_open_spans() {
    let output = parse_value(&valid_value()).expect("valid judge output");
    assert_eq!(
        output.schema_version,
        LOCAL_JUDGE_ATTEMPT_OUTPUT_SCHEMA_VERSION
    );
    assert_eq!(output.case_id, "case_01");
    assert_eq!(output.choice, LocalJudgeChoice::First);
    assert_eq!(output.rubric_clauses, ["clarity", "meaning"]);
    assert_eq!(
        output.source_spans,
        [
            LocalJudgeByteSpan { start: 0, end: 4 },
            LocalJudgeByteSpan { start: 4, end: 8 }
        ]
    );
}

#[test]
fn rejects_size_json_shape_and_version_failures() {
    assert_eq!(
        parse_local_judge_attempt_output(&" ".repeat(MAX_LOCAL_JUDGE_ATTEMPT_OUTPUT_BYTES + 1)),
        Err(LocalJudgeAttemptOutputError::TooLarge)
    );
    for invalid in ["", "{", "[]", "{}", "null", "{} trailing"] {
        assert_eq!(
            parse_local_judge_attempt_output(invalid),
            Err(LocalJudgeAttemptOutputError::InvalidJson),
            "fixture {invalid:?}"
        );
    }
    let mut unknown = valid_value();
    unknown["unexpected"] = json!(true);
    assert_eq!(
        parse_value(&unknown),
        Err(LocalJudgeAttemptOutputError::InvalidJson)
    );
    let mut unknown_span = valid_value();
    unknown_span["source_spans"][0]["text"] = json!("forbidden");
    assert_eq!(
        parse_value(&unknown_span),
        Err(LocalJudgeAttemptOutputError::InvalidJson)
    );
    let mut unsupported = valid_value();
    unsupported["schema_version"] = json!(2);
    assert_eq!(
        parse_value(&unsupported),
        Err(LocalJudgeAttemptOutputError::UnsupportedSchema(2))
    );
    let mut invalid_choice = valid_value();
    invalid_choice["choice"] = json!("candidate_a");
    assert_eq!(
        parse_value(&invalid_choice),
        Err(LocalJudgeAttemptOutputError::InvalidJson)
    );
}

#[test]
fn rejects_noncanonical_case_and_clause_labels() {
    for label in ["", "Case", "case 1", "case.1", "x/y"] {
        let mut value = valid_value();
        value["case_id"] = json!(label);
        assert_eq!(
            parse_value(&value),
            Err(LocalJudgeAttemptOutputError::InvalidCaseId),
            "label {label:?}"
        );
    }
    let mut long_case = valid_value();
    long_case["case_id"] = json!("a".repeat(MAX_LOCAL_JUDGE_LABEL_BYTES + 1));
    assert_eq!(
        parse_value(&long_case),
        Err(LocalJudgeAttemptOutputError::InvalidCaseId)
    );
    for clauses in [
        json!([]),
        json!(["meaning", "clarity"]),
        json!(["meaning", "meaning"]),
        json!(["Meaning"]),
        json!(["bad clause"]),
    ] {
        let mut value = valid_value();
        value["rubric_clauses"] = clauses;
        assert_eq!(
            parse_value(&value),
            Err(LocalJudgeAttemptOutputError::InvalidRubricClauses)
        );
    }
    let mut too_many = valid_value();
    too_many["rubric_clauses"] = json!(
        (0..=MAX_LOCAL_JUDGE_RUBRIC_CLAUSES)
            .map(|index| format!("clause_{index:02}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parse_value(&too_many),
        Err(LocalJudgeAttemptOutputError::InvalidRubricClauses)
    );
}

#[test]
fn rejects_invalid_source_spans_without_applying_input_specific_bounds() {
    assert_span_failures(
        "source_spans",
        LocalJudgeAttemptOutputError::InvalidSourceSpans,
    );
    assert_span_failures(
        "first_candidate_spans",
        LocalJudgeAttemptOutputError::InvalidFirstCandidateSpans,
    );
    assert_span_failures(
        "second_candidate_spans",
        LocalJudgeAttemptOutputError::InvalidSecondCandidateSpans,
    );

    let mut structurally_bounded = valid_value();
    structurally_bounded["source_spans"] = json!([{"start": 0, "end": u32::MAX}]);
    let parsed = parse_value(&structurally_bounded).expect("input ceiling belongs to evaluation");
    assert_eq!(parsed.source_spans[0].end, u32::MAX);
}

fn assert_span_failures(field: &str, expected: LocalJudgeAttemptOutputError) {
    for spans in [
        json!([{"start": 1, "end": 1}]),
        json!([{"start": 2, "end": 1}]),
        json!([{"start": 0, "end": 3}, {"start": 2, "end": 4}]),
        json!([{"start": 3, "end": 4}, {"start": 0, "end": 1}]),
    ] {
        let mut value = valid_value();
        value[field] = spans;
        assert_eq!(parse_value(&value), Err(expected));
    }
    let spans = (0..=MAX_LOCAL_JUDGE_BYTE_SPANS)
        .map(|index| json!({"start": index * 2, "end": index * 2 + 1}))
        .collect::<Vec<_>>();
    let mut value = valid_value();
    value[field] = Value::Array(spans);
    assert_eq!(parse_value(&value), Err(expected));
}
