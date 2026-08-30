use std::io::Cursor;

use serde_json::json;

use super::*;
use crate::source_build::program_lineage::verify_lineage;

mod fixture;

use fixture::{LINEAGE_PATH, LineageFixture, RECIPE_PATH, TOOL_PATH};

#[test]
fn exact_production_profile_produces_only_an_inert_typed_identity() {
    let fixture = LineageFixture::new();
    let lineage = fixture.verify().expect("verified lineage");
    assert_ne!(lineage.recipe_id().digest(), lineage.lineage_id().digest());
    assert_eq!(fixture.manifest().components().len(), 38);
}

#[test]
fn old_seventeen_member_manifest_never_produces_lineage_wrapper() {
    let mut value = manifest_value();
    value["components"]
        .as_array_mut()
        .expect("components")
        .push(component("modules/second.zip", &["go_module"]));
    value["components"]
        .as_array_mut()
        .expect("components")
        .sort_by(|left, right| {
            left["relative_path"]
                .as_str()
                .cmp(&right["relative_path"].as_str())
        });
    let components = value["components"].as_array().expect("components");
    value["artifact_set_id"] = json!(artifact_set(components).artifact_set_id());
    let verified = verify_runtime_source_build_inputs(
        &canonical(&value),
        RuntimeSourceBuildInputLimits::default(),
        |path| Ok(Cursor::new(fixture_bytes(path.as_str()))),
        || false,
    )
    .expect("legacy byte closure");
    assert_eq!(verified.manifest().components().len(), 17);
    assert!(verified.retained_program_lineage().is_none());
    assert_eq!(
        RuntimeSourceBuildPlan::for_verified_retained_program_closure(
            &verified,
            Digest::sha256(b"unreachable closure"),
        ),
        Err(RuntimeSourceBuildInputError::InvalidProgramLineage),
    );
    let read_only = RuntimeSourceBuildPlan::for_legacy_read_only_verification(verified.manifest());
    assert!(read_only.retained_program_closure_id().is_none());
}

#[test]
fn exact_wrapper_and_complete_closure_identity_are_bound_into_an_inert_plan() {
    let fixture = LineageFixture::new();
    let verified = fixture.verified_inputs().expect("verified inputs");
    let closure_id = Digest::sha256(b"complete opaque closure identity");
    let plan = RuntimeSourceBuildPlan::for_verified_retained_program_closure(
        &verified,
        closure_id.clone(),
    )
    .expect("closure-bound plan");
    assert_eq!(plan.retained_program_closure_id(), Some(&closure_id));
    assert_ne!(
        plan.plan_digest(),
        RuntimeSourceBuildPlan::for_legacy_read_only_verification(verified.manifest())
            .plan_digest()
    );
}

#[test]
fn production_profile_rejects_extras_and_metadata_relabeling() {
    let mut extra = LineageFixture::new();
    extra.add_extra_source_patch().expect("valid manifest");
    assert_eq!(
        extra.verify(),
        Err(RuntimeSourceBuildInputError::ProgramLineageMismatch)
    );

    for (path, field, replacement) in [
        ("scripts/build", "name", json!("relabeled")),
        ("scripts/build", "revision", json!("other")),
        ("scripts/build", "source_locator", json!("urn:retonr:other")),
        (
            "lineage/source/retonr-source.tar",
            "roles",
            json!(["source_patch", "retonr_repository_source"]),
        ),
    ] {
        let mut fixture = LineageFixture::new();
        fixture
            .mutate_component(path, field, replacement)
            .expect("parseable mutation");
        assert_eq!(
            fixture.verify(),
            Err(RuntimeSourceBuildInputError::ProgramLineageMismatch),
            "{path}:{field}"
        );
    }
}

#[test]
fn official_measurements_are_fixed_independently_of_the_record() {
    for field in ["byte_size", "digest"] {
        let mut fixture = LineageFixture::new();
        let replacement = if field == "byte_size" {
            json!(17_472_039_u64)
        } else {
            json!(Digest::sha256(b"substitute cargo"))
        };
        fixture
            .mutate_component(
                "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
                field,
                replacement,
            )
            .expect("parseable mutation");
        assert_eq!(
            fixture.verify(),
            Err(RuntimeSourceBuildInputError::ProgramLineageMismatch),
            "{field}"
        );
    }
}

#[test]
fn recipe_requires_exact_sorted_compact_json_and_one_terminal_lf() {
    let valid = LineageFixture::new().retained(RECIPE_PATH).to_vec();
    for replacement in [
        valid[..valid.len() - 1].to_vec(),
        b"{\"schema_version\":1}\n".to_vec(),
        b"{\"schema_version\":1,\"schema_version\":1}\n".to_vec(),
        [valid.as_slice(), b"\n"].concat(),
    ] {
        let mut fixture = LineageFixture::new();
        fixture.replace_recipe(replacement).expect("valid manifest");
        assert_eq!(
            fixture.verify(),
            Err(RuntimeSourceBuildInputError::InvalidProgramLineage)
        );
    }
}

#[test]
fn every_member_measurement_and_program_semantic_is_joined() {
    for (pointer, replacement, expected) in [
        (
            "/cargo_lock/digest",
            json!(Digest::sha256(b"wrong")),
            RuntimeSourceBuildInputError::ProgramLineageMismatch,
        ),
        (
            "/dependency_source_archive/raw_crate_source_archive/digest",
            json!(Digest::sha256(b"wrong raw crate archive")),
            RuntimeSourceBuildInputError::ProgramLineageMismatch,
        ),
        (
            "/preparation_tools/0/name",
            json!("source_manifest_preparation"),
            RuntimeSourceBuildInputError::InvalidProgramLineage,
        ),
        (
            "/rust_distribution/components/0/target",
            json!("x86_64-unknown-linux-gnu"),
            RuntimeSourceBuildInputError::InvalidProgramLineage,
        ),
        (
            "/build_host/minirootfs_signature_disposition",
            json!("verified"),
            RuntimeSourceBuildInputError::InvalidProgramLineage,
        ),
        (
            "/build_host/libgcc/signature_disposition",
            json!("verified"),
            RuntimeSourceBuildInputError::InvalidProgramLineage,
        ),
        (
            "/build_host/busybox_static/signature_disposition",
            json!("verified"),
            RuntimeSourceBuildInputError::InvalidProgramLineage,
        ),
        (
            "/build_host/busybox_executable/digest",
            json!(Digest::sha256(b"wrong BusyBox executable")),
            RuntimeSourceBuildInputError::ProgramLineageMismatch,
        ),
    ] {
        let mut fixture = LineageFixture::new();
        fixture
            .mutate_lineage(pointer, replacement)
            .expect("valid manifest");
        assert_eq!(fixture.verify(), Err(expected), "{pointer}");
    }
}

#[test]
fn shared_musl_standard_library_is_joined_to_both_semantic_roles() {
    let mut fixture = LineageFixture::new();
    fixture
        .mutate_lineage(
            "/rust_distribution/components/1/payload/digest",
            json!(Digest::sha256(b"other standard library")),
        )
        .expect("valid manifest");
    assert_eq!(
        fixture.verify(),
        Err(RuntimeSourceBuildInputError::ProgramLineageMismatch)
    );
}

#[test]
fn tool_evidence_binds_the_full_named_lineage_subrecord() {
    for pointer in [
        "/retained_program_lineage_binding/lineage_record/digest",
        "/retained_program_lineage_binding/builder/digest",
        "/retained_program_lineage_binding/isolation_helper/digest",
        "/retained_program_lineage_binding/source_archive_preparation/digest",
        "/retained_program_lineage_binding/source_manifest_preparation/digest",
    ] {
        let mut fixture = LineageFixture::new();
        fixture
            .mutate_tool(pointer, json!(Digest::sha256(b"wrong")))
            .expect("valid manifest");
        assert_eq!(
            fixture.verify(),
            Err(RuntimeSourceBuildInputError::ProgramLineageMismatch),
            "{pointer}"
        );
    }
    let mut old_name = LineageFixture::new();
    old_name.rename_tool_binding().expect("valid manifest");
    assert_eq!(
        old_name.verify(),
        Err(RuntimeSourceBuildInputError::InvalidProgramLineage)
    );
}

#[test]
fn archive_roots_lock_and_dirty_workspace_provenance_are_exact() {
    for (pointer, replacement) in [
        ("/dependency_source_archive/archive_root", json!("vendor")),
        (
            "/dependency_source_archive/cargo_lock/digest",
            json!(Digest::sha256(b"other lock")),
        ),
        (
            "/repository_source_archive/workspace_provenance/base_commit",
            json!("0000000000000000000000000000000000000000"),
        ),
        (
            "/repository_source_archive/workspace_provenance/workspace_state",
            json!("clean"),
        ),
    ] {
        let mut fixture = LineageFixture::new();
        fixture
            .mutate_lineage(pointer, replacement)
            .expect("valid manifest");
        assert_eq!(
            fixture.verify(),
            Err(RuntimeSourceBuildInputError::InvalidProgramLineage),
            "{pointer}"
        );
    }
}

#[test]
fn lineage_and_tool_records_are_bounded_unique_key_canonical_schemas() {
    for (path, replacement) in [
        (
            LINEAGE_PATH,
            b"{\"schema_version\":1,\"schema_version\":1}".to_vec(),
        ),
        (TOOL_PATH, b"{ \"schema_version\": 1 }".to_vec()),
    ] {
        let mut fixture = LineageFixture::new();
        fixture
            .replace_retained_record(path, &replacement)
            .expect("valid manifest");
        assert_eq!(
            verify_lineage(&fixture.manifest(), fixture.retained_map()),
            Err(RuntimeSourceBuildInputError::InvalidProgramLineage)
        );
    }
}

#[test]
fn production_profile_excludes_serialized_bootstrap_authority() {
    let fixture = LineageFixture::new();
    let manifest = fixture.manifest();
    assert!(manifest.components().iter().all(|component| {
        component.relative_path().as_str() != "metadata/retained-program-bootstrap-evidence.json"
    }));
    let recipe_bytes = fixture.retained(RECIPE_PATH);
    let body = recipe_bytes.strip_suffix(b"\n").expect("terminal LF");
    let recipe: serde_json::Value = serde_json::from_slice(body).expect("recipe JSON");
    assert_eq!(recipe["procedure_version"], 2);
    assert_eq!(recipe["schema_version"], 2);
    assert_eq!(
        recipe["execution"]["live_result_authority"],
        "opaque_nonserializable_execution_only"
    );
}
