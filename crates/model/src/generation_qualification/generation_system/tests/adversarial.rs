use super::super::test_support::{effective_package, other_runtime_package, runtime_state};
use super::*;
use crate::{RuntimeBuildIdentityInput, RuntimeBuildMode};

#[test]
fn decoder_rejects_bounds_malformed_unknown_duplicate_and_noncanonical_json() {
    let fixture = fixture(false);
    let record = fixture.record();
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_SYSTEM_JSON_BYTES + 1],
            fixture.relations(),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    for bytes in [b"{".as_slice(), b"null".as_slice(), b"[]".as_slice()] {
        assert_eq!(
            GenerationSystemRecordV1::from_json_bytes(bytes, fixture.relations()),
            Err(GenerationQualificationContractError::InvalidEncoding)
        );
    }

    let mut unknown = serde_json::to_value(&record).expect("record value");
    unknown
        .as_object_mut()
        .expect("object")
        .insert("authority".to_owned(), Value::Bool(true));
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(
            &serde_json::to_vec(&unknown).expect("unknown field JSON"),
            fixture.relations(),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let encoded = serde_json::to_string(&record).expect("record JSON");
    let duplicate = encoded.replacen(
        "{\"schema_version\":1,",
        "{\"schema_version\":1,\"schema_version\":1,",
        1,
    );
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(duplicate.as_bytes(), fixture.relations()),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let trailing = format!("{encoded} true");
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(trailing.as_bytes(), fixture.relations()),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let unsupported = mutate_json(&record, "schema_version", Value::from(2));
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(&unsupported, fixture.relations()),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    let mut future: Value = serde_json::from_slice(&unsupported).expect("future record value");
    future.as_object_mut().expect("object").insert(
        "runtime_package_manifest_id".to_owned(),
        Value::String(digest("future relationship").as_str().to_owned()),
    );
    assert_eq!(
        GenerationSystemRecordV1::from_json_bytes(
            &serde_json::to_vec(&future).expect("future record JSON"),
            fixture.relations(),
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
}

#[test]
fn decoder_classifies_each_substituted_relationship() {
    let fixture = fixture(false);
    let record = fixture.record();
    let cases = [
        (
            "runtime_package_manifest_id",
            Value::String(digest("wrong runtime package").as_str().to_owned()),
            GenerationQualificationContractError::RuntimePackageMismatch,
        ),
        (
            "runtime_build_id",
            Value::String(digest("wrong build").as_str().to_owned()),
            GenerationQualificationContractError::RuntimeBuildMismatch,
        ),
        (
            "effective_runtime_state_id",
            Value::String(digest("wrong state").as_str().to_owned()),
            GenerationQualificationContractError::RuntimeBuildMismatch,
        ),
        (
            "model_artifact_set_id",
            Value::String(digest("wrong set").as_str().to_owned()),
            GenerationQualificationContractError::ModelPackageMismatch,
        ),
        (
            "model_package_manifest_id",
            Value::String(digest("wrong model package").as_str().to_owned()),
            GenerationQualificationContractError::ModelPackageMismatch,
        ),
        (
            "effective_package_evidence_v2_id",
            Value::String(digest("wrong evidence").as_str().to_owned()),
            GenerationQualificationContractError::EffectivePackageMismatch,
        ),
    ];
    for (field, value, expected) in cases {
        assert_eq!(
            GenerationSystemRecordV1::from_json_bytes(
                &mutate_json(&record, field, value),
                fixture.relations(),
            ),
            Err(expected),
            "field {field}"
        );
    }
}

#[test]
fn constructor_rejects_swapped_runtime_package_build_and_state() {
    let fixture = fixture(false);
    let other_package = other_runtime_package();
    let mut relations = fixture.relations();
    relations.runtime_package_manifest = &other_package;
    assert_eq!(
        GenerationSystemRecordV1::new(relations, fixture.input()),
        Err(GenerationQualificationContractError::RuntimePackageMismatch)
    );

    let changed_build = RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode: RuntimeBuildMode::ManagedProcess,
        runtime_family: "ollama".to_owned(),
        reported_version: "0.32.15".to_owned(),
        build_revision: Some("changed-build".to_owned()),
        target: fixture.runtime_build.target(),
        package_manifest_digest: fixture
            .runtime_package
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        entrypoint_digest: fixture.runtime_build.entrypoint_digest().clone(),
        packaged_dependencies_digest: fixture.runtime_build.packaged_dependencies_digest().clone(),
        build_configuration_digest: fixture.runtime_build.build_configuration_digest().clone(),
    })
    .expect("changed build");
    relations = fixture.relations();
    relations.runtime_build = &changed_build;
    assert_eq!(
        GenerationSystemRecordV1::new(relations, fixture.input()),
        Err(GenerationQualificationContractError::RuntimeBuildMismatch)
    );

    let changed_state = runtime_state(&fixture.runtime_build, "changed");
    let changed_evidence = effective_package(
        &fixture.model_set,
        &fixture.runtime_build,
        &changed_state,
        false,
    );
    relations = fixture.relations();
    relations.effective_package_evidence_v2 = &changed_evidence;
    assert_eq!(
        GenerationSystemRecordV1::new(relations, fixture.input()),
        Err(GenerationQualificationContractError::EffectivePackageMismatch)
    );
}

#[test]
fn constructor_rejects_model_set_package_artifact_and_closure_substitution() {
    let system = fixture(false);
    let other = fixture(true);
    let mut relations = system.relations();
    relations.model_artifact_set = &other.model_set;
    assert_eq!(
        GenerationSystemRecordV1::new(relations, system.input()),
        Err(GenerationQualificationContractError::ModelPackageMismatch)
    );

    relations = system.relations();
    relations.model_package_manifest = &other.model_package;
    assert_eq!(
        GenerationSystemRecordV1::new(relations, system.input()),
        Err(GenerationQualificationContractError::ModelPackageMismatch)
    );

    let mut wrong_artifact = system.input();
    wrong_artifact.model_artifact_id = ArtifactId::from_digest(digest("not a model member"));
    assert_eq!(
        GenerationSystemRecordV1::new(system.relations(), wrong_artifact),
        Err(GenerationQualificationContractError::ModelArtifactMismatch)
    );

    let excluded = effective_package(
        &system.model_set,
        &system.runtime_build,
        &system.runtime_state,
        true,
    );
    relations = system.relations();
    relations.effective_package_evidence_v2 = &excluded;
    assert_eq!(
        GenerationSystemRecordV1::new(relations, system.input()),
        Err(GenerationQualificationContractError::ModelArtifactMismatch)
    );
}
