use tempfile::tempdir;

use super::super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

#[path = "tests/corruption.rs"]
mod corruption;
#[path = "tests/support.rs"]
mod support;

use support::PackageFixture;

#[test]
fn all_three_contracts_round_trip_idempotently_without_granting_authority() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("packages.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);

    assert_eq!(
        store
            .put_runtime_package_manifest(&fixture.runtime_package)
            .expect("store runtime package"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .put_model_package_manifest(&fixture.model_package)
            .expect("store model package"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .put_native_load_observation(&fixture.native_load)
            .expect("store native load"),
        WriteDisposition::Inserted
    );

    assert_eq!(
        store
            .put_runtime_package_manifest(&fixture.runtime_package)
            .expect("repeat runtime package"),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(
        store
            .put_model_package_manifest(&fixture.model_package)
            .expect("repeat model package"),
        WriteDisposition::AlreadyPresent
    );
    assert_eq!(
        store
            .put_native_load_observation(&fixture.native_load)
            .expect("repeat native load"),
        WriteDisposition::AlreadyPresent
    );

    assert_eq!(
        store
            .runtime_package_manifest(&fixture.runtime_package.runtime_package_manifest_id())
            .expect("load runtime package"),
        Some(fixture.runtime_package.clone())
    );
    assert_eq!(
        store
            .model_package_manifest(&fixture.model_package.model_package_manifest_id())
            .expect("load model package"),
        Some(fixture.model_package.clone())
    );
    assert_eq!(
        store
            .native_load_observation(&fixture.native_load.native_load_observation_id())
            .expect("load native observation"),
        Some(fixture.native_load.clone())
    );

    drop(store);
    let read_only = ArtifactStateStore::open_existing_read_only(&path).expect("open read only");
    assert_eq!(
        read_only
            .native_load_observation(&fixture.native_load.native_load_observation_id())
            .expect("recover recursively through a read transaction"),
        Some(fixture.native_load.clone())
    );

    for table in ["runtime_package_manifests", "model_package_manifests"] {
        let source_id: Option<String> = read_only
            .connection()
            .query_row(
                &format!("SELECT source_artifact_set_id FROM {table}"),
                [],
                |row| row.get(0),
            )
            .expect("load untransformed source relationship");
        assert_eq!(source_id, None, "{table} must store NULL for untransformed");
    }

    for table in [
        "installed_artifact_sets",
        "qualification_v2_records",
        "active_bindings",
        "activation_decisions",
    ] {
        let count: i64 = read_only
            .connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count authority rows");
        assert_eq!(count, 0, "{table} must remain empty");
    }
}

#[test]
fn transformed_packages_round_trip_with_exact_source_relationships() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("transformed.db");
    let fixture = PackageFixture::new();
    let (runtime_set, runtime) = fixture.transformed_runtime_package();
    let (model_set, model) = fixture.transformed_model_package();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    store
        .put_artifact_set_manifest(&runtime_set)
        .expect("store transformed runtime byte set");
    store
        .put_artifact_set_manifest(&model_set)
        .expect("store transformed model byte set");
    fixture.put_source_artifact_set(&store);

    assert_eq!(
        store
            .put_runtime_package_manifest(&runtime)
            .expect("store transformed runtime"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .put_model_package_manifest(&model)
            .expect("store transformed model"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store
            .runtime_package_manifest(&runtime.runtime_package_manifest_id())
            .expect("load transformed runtime"),
        Some(runtime)
    );
    assert_eq!(
        store
            .model_package_manifest(&model.model_package_manifest_id())
            .expect("load transformed model"),
        Some(model)
    );

    let expected = fixture.source_set.artifact_set_id();
    for table in ["runtime_package_manifests", "model_package_manifests"] {
        let source_id: String = store
            .connection()
            .query_row(
                &format!("SELECT source_artifact_set_id FROM {table}"),
                [],
                |row| row.get(0),
            )
            .expect("load transformed source relationship");
        assert_eq!(source_id, expected.digest().as_str());
    }
}

#[test]
fn transformed_puts_require_the_exact_source_artifact_set() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("missing-transformed-source.db");
    let fixture = PackageFixture::new();
    let (runtime_set, runtime) = fixture.transformed_runtime_package();
    let (model_set, model) = fixture.transformed_model_package();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    store
        .put_artifact_set_manifest(&runtime_set)
        .expect("store transformed runtime byte set");
    store
        .put_artifact_set_manifest(&model_set)
        .expect("store transformed model byte set");

    assert!(matches!(
        store.put_runtime_package_manifest(&runtime),
        Err(StoreError::MissingRecord)
    ));
    assert!(matches!(
        store.put_model_package_manifest(&model),
        Err(StoreError::MissingRecord)
    ));
    for table in ["runtime_package_manifests", "model_package_manifests"] {
        let count: i64 = store
            .connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count rejected transformed packages");
        assert_eq!(count, 0);
    }
}

#[test]
fn puts_reject_missing_dependencies_without_partial_writes() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("missing.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");

    assert!(matches!(
        store.put_runtime_package_manifest(&fixture.runtime_package),
        Err(StoreError::MissingRecord)
    ));
    assert!(matches!(
        store.put_model_package_manifest(&fixture.model_package),
        Err(StoreError::MissingRecord)
    ));
    assert!(matches!(
        store.put_native_load_observation(&fixture.native_load),
        Err(StoreError::MissingRecord)
    ));
    for table in [
        "runtime_package_manifests",
        "model_package_manifests",
        "native_load_observations",
    ] {
        let count: i64 = store
            .connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count package rows");
        assert_eq!(count, 0);
    }
}
