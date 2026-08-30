use serde_json::Value;

use super::*;

mod adversarial;
use super::test_support::{digest, fixture};

#[test]
fn record_binds_every_exact_relationship_and_policy_fact() {
    let fixture = fixture(false);
    let record = fixture.record();

    assert_eq!(record.schema_version(), 1);
    assert_eq!(
        record.runtime_admission_join_id(),
        &fixture.input().runtime_admission_join_id
    );
    assert_eq!(
        record.managed_generation_path_id(),
        &fixture.input().managed_generation_path_id
    );
    assert_eq!(
        record.frozen_external_component_set_id(),
        &fixture.input().frozen_external_component_set_id
    );
    assert_eq!(
        record.runtime_package_manifest_id(),
        &fixture.runtime_package.runtime_package_manifest_id()
    );
    assert_eq!(
        record.runtime_build_id(),
        &fixture.runtime_build.runtime_build_id()
    );
    assert_eq!(
        record.effective_runtime_state_id(),
        &fixture.runtime_state.effective_runtime_state_id()
    );
    assert_eq!(
        record.model_artifact_set_id(),
        &fixture.model_set.artifact_set_id()
    );
    assert_eq!(
        record.model_package_manifest_id(),
        &fixture.model_package.model_package_manifest_id()
    );
    assert_eq!(record.model_artifact_id(), &fixture.model_artifact_id);
    assert_eq!(
        record.effective_package_evidence_v2_id(),
        &fixture.effective_package.effective_package_evidence_v2_id()
    );

    let input = fixture.input();
    assert_eq!(
        record.static_model_binding_digest(),
        &input.static_model_binding_digest
    );
    assert_eq!(record.strategy_digest(), &input.strategy_digest);
    assert_eq!(record.planner_digest(), &input.planner_digest);
    assert_eq!(record.validator_digest(), &input.validator_digest);
    assert_eq!(record.adapter_digest(), &input.adapter_digest);
    assert_eq!(record.prompt_digest(), &input.prompt_digest);
    assert_eq!(record.output_schema_digest(), &input.output_schema_digest);
    assert_eq!(record.request_policy_digest(), &input.request_policy_digest);
    assert_eq!(record.language_digest(), &input.language_digest);
    assert_eq!(record.mode_digest(), &input.mode_digest);
    assert_eq!(record.format_digest(), &input.format_digest);
    assert_eq!(
        record.operating_system_digest(),
        &input.operating_system_digest
    );
    assert_eq!(record.architecture_digest(), &input.architecture_digest);
    assert_eq!(
        record.execution_class_digest(),
        &input.execution_class_digest
    );
    assert_eq!(
        record.hardware_envelope_digest(),
        &input.hardware_envelope_digest
    );
    record
        .validate_against(fixture.relations())
        .expect("exact relationships revalidate");
}

#[test]
fn canonical_record_round_trips_and_has_stable_identity() {
    let fixture = fixture(false);
    let record = fixture.record();
    let encoded = serde_json::to_vec(&record).expect("encode generation system");
    let decoded = GenerationSystemRecordV1::from_json_bytes(&encoded, fixture.relations())
        .expect("decode generation system");
    assert_eq!(decoded, record);
    assert_eq!(
        record.generation_system_id().digest().as_str(),
        "11a49fbf45e16f0f4dfc881ab102f960ba6cc11caa853f18fea998ff80975298"
    );
}

#[test]
fn policy_and_owner_bindings_are_identity_separated() {
    let fixture = fixture(false);
    let baseline = fixture.record();
    let mut inputs = Vec::new();
    let mut owner = fixture.input();
    owner.runtime_admission_join_id = RuntimeAdmissionJoinId::from_derived_digest(digest("other"));
    inputs.push(owner);
    let mut policy = fixture.input();
    policy.request_policy_digest = digest("other");
    inputs.push(policy);
    let mut selector = fixture.input();
    selector.hardware_envelope_digest = digest("other");
    inputs.push(selector);

    for input in inputs {
        let variant = GenerationSystemRecordV1::new(fixture.relations(), input)
            .expect("independent inert equality binding");
        assert_ne!(
            variant.generation_system_id(),
            baseline.generation_system_id()
        );
    }
}

#[test]
fn every_serialized_identity_field_participates_in_the_system_id() {
    let fixture = fixture(false);
    let record = fixture.record();
    let wire = serde_json::to_value(&record).expect("record value");
    let baseline = independent_system_id(&wire);
    assert_eq!(&baseline, record.generation_system_id().digest());

    for field in SYSTEM_IDENTITY_FIELDS {
        let mut changed = wire.clone();
        changed.as_object_mut().expect("record object").insert(
            (*field).to_owned(),
            Value::String(digest(&format!("changed {field}")).as_str().to_owned()),
        );
        assert_ne!(independent_system_id(&changed), baseline, "field {field}");
    }
}

#[test]
fn debug_exposes_only_schema_and_system_identity() {
    let fixture = fixture(false);
    let record = fixture.record();
    let debug = format!("{record:?}");
    assert!(debug.contains("GenerationSystemRecordV1"));
    assert!(debug.contains(record.generation_system_id().digest().as_str()));
    for hidden in [
        record.runtime_admission_join_id().digest(),
        record.model_artifact_id().digest(),
        record.prompt_digest(),
        record.hardware_envelope_digest(),
    ] {
        assert!(!debug.contains(hidden.as_str()));
    }
}

#[test]
fn shared_content_across_license_and_model_paths_preserves_logical_closure() {
    let fixture = fixture(true);
    let record = fixture.record();
    assert_eq!(record.model_artifact_id(), &fixture.model_artifact_id);
    let matching_paths = fixture
        .model_set
        .members()
        .iter()
        .filter(|member| member.artifact_id() == &fixture.model_artifact_id)
        .count();
    assert_eq!(matching_paths, 2);
}

#[test]
fn reordered_json_and_mutated_cached_identity_are_rejected() {
    let fixture = fixture(false);
    let record = fixture.record();
    let mut encoded = vec![b' '];
    encoded.extend(serde_json::to_vec(&record).expect("encode"));
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(&encoded, fixture.relations()),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );

    let mut mutated = record;
    mutated.id = GenerationSystemId(digest("forged cached ID"));
    assert_eq!(
        mutated.validate_against(fixture.relations()),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
}

fn mutate_json(record: &GenerationSystemRecordV1, field: &str, value: Value) -> Vec<u8> {
    let mut wire = serde_json::to_value(record).expect("record value");
    wire.as_object_mut()
        .expect("record object")
        .insert(field.to_owned(), value);
    serde_json::to_vec(&wire).expect("mutated JSON")
}

const SYSTEM_IDENTITY_FIELDS: &[&str; 25] = &[
    "runtime_admission_join_id",
    "managed_generation_path_id",
    "frozen_external_component_set_id",
    "runtime_package_manifest_id",
    "runtime_build_id",
    "effective_runtime_state_id",
    "model_artifact_set_id",
    "model_package_manifest_id",
    "model_artifact_id",
    "effective_package_evidence_v2_id",
    "static_model_binding_digest",
    "strategy_digest",
    "planner_digest",
    "validator_digest",
    "adapter_digest",
    "prompt_digest",
    "output_schema_digest",
    "request_policy_digest",
    "language_digest",
    "mode_digest",
    "format_digest",
    "operating_system_digest",
    "architecture_digest",
    "execution_class_digest",
    "hardware_envelope_digest",
];

fn independent_system_id(wire: &Value) -> Digest {
    let object = wire.as_object().expect("record object");
    let schema = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .expect("schema version");
    let mut material = b"retonr:generation-system-record:v1\0".to_vec();
    material.extend_from_slice(&schema.to_be_bytes());
    for field in SYSTEM_IDENTITY_FIELDS {
        material.extend_from_slice(
            object
                .get(*field)
                .and_then(Value::as_str)
                .expect("identity field")
                .as_bytes(),
        );
    }
    Digest::sha256(&material)
}
