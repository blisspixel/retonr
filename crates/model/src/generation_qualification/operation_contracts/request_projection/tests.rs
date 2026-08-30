use rewrite_types::Digest;
use serde_json::Value;

use super::super::operation_policy::test_support;
use super::*;
use crate::StructuredCompletionRequestBindingId;

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn relations(
    fixture: &test_support::Fixture,
) -> GenerationQualificationRequestProjectionV1Relations<'_> {
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy: &fixture.policy,
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        planned_attempts: &fixture.attempts,
    }
}

pub(super) fn entry_inputs(
    fixture: &test_support::Fixture,
) -> Vec<GenerationQualificationRequestProjectionEntryV1Input> {
    fixture
        .attempts
        .iter()
        .enumerate()
        .map(
            |(index, attempt)| GenerationQualificationRequestProjectionEntryV1Input {
                structured_completion_request_binding_id:
                    StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                        "structured request {index}"
                    ))),
                complete_input_byte_count: attempt.source_byte_count() + 100,
                context_token_limit: fixture.policy.limits().maximum_context_tokens(),
                output_token_limit: fixture.policy.limits().maximum_output_tokens(),
            },
        )
        .collect()
}

pub(super) fn make_projection(
    fixture: &test_support::Fixture,
    inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
) -> GenerationQualificationRequestProjectionV1 {
    GenerationQualificationRequestProjectionV1::new(relations(fixture), inputs)
        .expect("request projection")
}

#[test]
fn projection_round_trips_exposes_complete_plan_order_and_has_stable_identity() {
    let fixture = test_support::fixture();
    let inputs = entry_inputs(&fixture);
    let projection = make_projection(&fixture, &inputs);

    assert_eq!(projection.schema_version(), 1);
    assert_eq!(
        projection.operation_policy_id(),
        fixture.policy.operation_policy_id()
    );
    assert_eq!(
        projection.target_generation_system_id(),
        fixture.systems[0].generation_system_id()
    );
    assert_eq!(
        projection.baseline_generation_system_id(),
        fixture.systems[1].generation_system_id()
    );
    assert_eq!(
        projection.generation_qualification_plan_id(),
        fixture.plan.qualification_plan_id()
    );
    assert_eq!(
        projection.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );
    assert_eq!(
        projection.entry_count(),
        u64::try_from(fixture.attempts.len()).expect("entry count")
    );
    assert_eq!(projection.entries().len(), fixture.attempts.len());
    for ((entry, attempt), input) in projection
        .entries()
        .iter()
        .zip(&fixture.attempts)
        .zip(&inputs)
    {
        assert_eq!(entry.planned_attempt_id(), attempt.planned_attempt_id());
        assert_eq!(entry.generation_system_id(), attempt.generation_system_id());
        assert_eq!(
            entry.generation_request_binding_id(),
            attempt.generation_request_binding_id()
        );
        assert_eq!(
            entry.structured_completion_request_binding_id(),
            &input.structured_completion_request_binding_id
        );
        assert_eq!(entry.source_byte_count(), attempt.source_byte_count());
        assert_eq!(
            entry.complete_input_byte_count(),
            input.complete_input_byte_count
        );
        assert_eq!(
            entry.source_byte_limit(),
            fixture.policy.limits().maximum_source_bytes()
        );
        assert_eq!(
            entry.complete_input_byte_limit(),
            fixture.policy.limits().maximum_complete_input_bytes()
        );
        assert_eq!(entry.context_token_limit(), input.context_token_limit);
        assert_eq!(entry.output_token_limit(), input.output_token_limit);
        assert_eq!(entry.candidate_count(), 1);
        assert_eq!(entry.candidate_byte_limit(), test_support::CANDIDATE_BYTES);
        assert_eq!(
            entry.aggregate_candidate_byte_limit(),
            test_support::CANDIDATE_BYTES
        );
        assert_eq!(
            entry.output_byte_limit(),
            test_support::OUTPUT_ENVELOPE_BYTES
        );
    }

    let bytes = serde_json::to_vec(&projection).expect("projection JSON");
    assert_eq!(
        GenerationQualificationRequestProjectionV1::from_json_bytes(
            &bytes,
            relations(&fixture),
            &inputs,
        )
        .expect("projection decode"),
        projection
    );
    projection
        .validate_against(relations(&fixture), &inputs)
        .expect("projection revalidation");
    assert_eq!(
        projection.request_projection_id().digest().as_str(),
        "36cc14f7b738a8d3a25d2f4561dd267b328d20f537540978978d305fbdfed40d"
    );
    assert_ne!(
        projection.request_projection_id().digest(),
        fixture.policy.operation_policy_id().digest()
    );
}

#[test]
fn decoder_is_strict_bounded_and_checks_the_declared_count() {
    let fixture = test_support::fixture();
    let inputs = entry_inputs(&fixture);
    let projection = make_projection(&fixture, &inputs);
    let bytes = serde_json::to_vec(&projection).expect("projection JSON");

    assert_eq!(
        GenerationQualificationRequestProjectionV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES + 1],
            relations(&fixture),
            &inputs,
        ),
        Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        decode(b"{", &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );

    let mut unknown = value(&projection);
    unknown["authority"] = Value::Bool(true);
    assert_eq!(
        decode_value(&unknown, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let duplicate = String::from_utf8(bytes.clone())
        .expect("UTF-8 JSON")
        .replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
            1,
        );
    assert_eq!(
        decode(duplicate.as_bytes(), &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let mut missing = value(&projection);
    missing
        .as_object_mut()
        .expect("object")
        .remove("suite_manifest_id");
    assert_eq!(
        decode_value(&missing, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::InvalidEncoding)
    );
    let reordered = reorder_first_two_fields(&bytes);
    assert_eq!(
        decode(&reordered, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut trailing = bytes;
    trailing.push(b' ');
    assert_eq!(
        decode(&trailing, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    );
    let mut unsupported = value(&projection);
    unsupported["schema_version"] = Value::from(2);
    assert_eq!(
        decode_value(&unsupported, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::UnsupportedSchema)
    );
    for declared_count in [
        0_u64,
        projection.entry_count() - 1,
        projection.entry_count() + 1,
    ] {
        let mut changed = value(&projection);
        changed["entry_count"] = Value::from(declared_count);
        assert_eq!(
            decode_value(&changed, &fixture, &inputs),
            Err(GenerationQualificationOperationContractError::InvalidCount)
        );
    }
}

#[test]
fn every_top_level_and_entry_field_substitution_is_rejected() {
    let fixture = test_support::fixture();
    let inputs = entry_inputs(&fixture);
    let projection = make_projection(&fixture, &inputs);
    let alternate = Value::String(digest("projection substitution").as_str().to_owned());

    for field in [
        "operation_policy_id",
        "target_generation_system_id",
        "baseline_generation_system_id",
        "generation_qualification_plan_id",
        "suite_manifest_id",
    ] {
        let mut changed = value(&projection);
        changed[field] = alternate.clone();
        assert_eq!(
            decode_value(&changed, &fixture, &inputs),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted substituted {field}"
        );
    }

    let id_fields = [
        "planned_attempt_id",
        "generation_system_id",
        "generation_request_binding_id",
        "structured_completion_request_binding_id",
    ];
    for field in id_fields {
        let mut changed = value(&projection);
        changed["entries"][0][field] = alternate.clone();
        assert_eq!(
            decode_value(&changed, &fixture, &inputs),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted substituted entry {field}"
        );
    }
    for field in [
        "source_byte_count",
        "complete_input_byte_count",
        "source_byte_limit",
        "complete_input_byte_limit",
        "context_token_limit",
        "output_token_limit",
        "candidate_count",
        "candidate_byte_limit",
        "aggregate_candidate_byte_limit",
        "output_byte_limit",
    ] {
        let mut changed = value(&projection);
        let current = changed["entries"][0][field]
            .as_u64()
            .expect("numeric entry field");
        changed["entries"][0][field] = Value::from(current + 1);
        assert_eq!(
            decode_value(&changed, &fixture, &inputs),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch),
            "accepted substituted entry {field}"
        );
    }
}

#[test]
fn entry_order_membership_and_count_are_exact() {
    let fixture = test_support::fixture();
    let inputs = entry_inputs(&fixture);
    let projection = make_projection(&fixture, &inputs);

    let mut reordered = value(&projection);
    reordered["entries"]
        .as_array_mut()
        .expect("entries")
        .swap(0, 1);
    assert_eq!(
        decode_value(&reordered, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
    let mut duplicate = value(&projection);
    let first = duplicate["entries"][0].clone();
    duplicate["entries"][1] = first;
    assert_eq!(
        decode_value(&duplicate, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
    let mut missing = value(&projection);
    missing["entries"].as_array_mut().expect("entries").pop();
    assert_eq!(
        decode_value(&missing, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::InvalidCount)
    );
    let mut extra = value(&projection);
    let first = extra["entries"][0].clone();
    extra["entries"]
        .as_array_mut()
        .expect("entries")
        .push(first);
    assert_eq!(
        decode_value(&extra, &fixture, &inputs),
        Err(GenerationQualificationOperationContractError::InvalidCount)
    );
}

#[test]
fn independently_derived_request_facts_are_exact() {
    let fixture = test_support::fixture();
    let inputs = entry_inputs(&fixture);
    let projection = make_projection(&fixture, &inputs);
    let mut fewer_inputs = inputs.clone();
    fewer_inputs.pop();
    assert_eq!(
        GenerationQualificationRequestProjectionV1::new(relations(&fixture), &fewer_inputs),
        Err(GenerationQualificationOperationContractError::InvalidCount)
    );
    let mut extra_inputs = inputs.clone();
    extra_inputs.push(inputs[0].clone());
    assert_eq!(
        GenerationQualificationRequestProjectionV1::new(relations(&fixture), &extra_inputs),
        Err(GenerationQualificationOperationContractError::InvalidCount)
    );
    let mut reordered_inputs = inputs.clone();
    reordered_inputs.swap(0, 1);
    assert_eq!(
        projection.validate_against(relations(&fixture), &reordered_inputs),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );

    for changed_input in [
        request_input(
            &inputs[0],
            None,
            Some(fixture.policy.limits().maximum_complete_input_bytes() + 1),
            None,
            None,
        ),
        request_input(
            &inputs[0],
            None,
            None,
            Some(inputs[0].context_token_limit - 1),
            None,
        ),
        request_input(
            &inputs[0],
            None,
            None,
            None,
            Some(inputs[0].output_token_limit - 1),
        ),
    ] {
        let mut changed = inputs.clone();
        changed[0] = changed_input;
        assert_eq!(
            GenerationQualificationRequestProjectionV1::new(relations(&fixture), &changed),
            Err(GenerationQualificationOperationContractError::RelationshipMismatch)
        );
    }

    let masked_input = request_input(
        &inputs[0],
        None,
        Some(fixture.attempts[0].source_byte_count() - 1),
        None,
        None,
    );
    let masked_inputs = [masked_input]
        .into_iter()
        .chain(inputs.iter().skip(1).cloned())
        .collect::<Vec<_>>();
    GenerationQualificationRequestProjectionV1::new(relations(&fixture), &masked_inputs)
        .expect("a masked complete input may be shorter than the original source");

    let changed_binding = request_input(
        &inputs[0],
        Some(StructuredCompletionRequestBindingId::from_derived_digest(
            digest("changed trusted structured request"),
        )),
        None,
        None,
        None,
    );
    let mut changed_inputs = inputs.clone();
    changed_inputs[0] = changed_binding;
    let changed_projection = make_projection(&fixture, &changed_inputs);
    assert_ne!(
        projection.request_projection_id(),
        changed_projection.request_projection_id()
    );
    assert_eq!(
        projection.validate_against(relations(&fixture), &changed_inputs),
        Err(GenerationQualificationOperationContractError::RelationshipMismatch)
    );
}

#[test]
fn debug_and_errors_are_content_free() {
    let fixture = test_support::fixture();
    let inputs = entry_inputs(&fixture);
    let projection = make_projection(&fixture, &inputs);
    let rendered = format!("{projection:?}");
    assert!(
        !rendered.contains(
            inputs[0]
                .structured_completion_request_binding_id
                .digest()
                .as_str()
        )
    );
    assert!(
        !rendered.contains(
            fixture.attempts[0]
                .generation_request_binding_id()
                .digest()
                .as_str()
        )
    );
    assert_eq!(
        GenerationQualificationOperationContractError::RelationshipMismatch.to_string(),
        "generation qualification operation relationship does not match"
    );
}

fn request_input(
    base: &GenerationQualificationRequestProjectionEntryV1Input,
    structured: Option<StructuredCompletionRequestBindingId>,
    complete_input_byte_count: Option<u64>,
    context_token_limit: Option<u32>,
    output_token_limit: Option<u32>,
) -> GenerationQualificationRequestProjectionEntryV1Input {
    GenerationQualificationRequestProjectionEntryV1Input {
        structured_completion_request_binding_id: structured
            .unwrap_or_else(|| base.structured_completion_request_binding_id.clone()),
        complete_input_byte_count: complete_input_byte_count
            .unwrap_or(base.complete_input_byte_count),
        context_token_limit: context_token_limit.unwrap_or(base.context_token_limit),
        output_token_limit: output_token_limit.unwrap_or(base.output_token_limit),
    }
}

fn value(projection: &GenerationQualificationRequestProjectionV1) -> Value {
    serde_json::to_value(projection).expect("projection value")
}

fn decode(
    bytes: &[u8],
    fixture: &test_support::Fixture,
    inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
) -> Result<GenerationQualificationRequestProjectionV1, GenerationQualificationOperationContractError>
{
    GenerationQualificationRequestProjectionV1::from_json_bytes(bytes, relations(fixture), inputs)
}

fn decode_value(
    value: &Value,
    fixture: &test_support::Fixture,
    inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
) -> Result<GenerationQualificationRequestProjectionV1, GenerationQualificationOperationContractError>
{
    decode(
        &serde_json::to_vec(value).expect("projection JSON"),
        fixture,
        inputs,
    )
}

fn reorder_first_two_fields(bytes: &[u8]) -> Vec<u8> {
    let value = String::from_utf8(bytes.to_vec()).expect("UTF-8 JSON");
    let first_comma = value.find(',').expect("first comma");
    let second_comma = value[first_comma + 1..]
        .find(',')
        .map(|index| index + first_comma + 1)
        .expect("second comma");
    format!(
        "{{{},{},{}",
        &value[first_comma + 1..second_comma],
        &value[1..first_comma],
        &value[second_comma + 1..]
    )
    .into_bytes()
}
