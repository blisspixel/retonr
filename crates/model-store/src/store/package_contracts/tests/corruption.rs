use rewrite_model::{ModelPackageManifestId, RuntimePackageManifestId};
use rusqlite::params;
use tempfile::tempdir;

use super::{ArtifactStateStore, PackageFixture, StoreError};

#[test]
fn immutable_conflict_is_distinct_from_idempotent_insert() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("conflict.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_runtime_package_manifest(&fixture.runtime_package)
        .expect("store runtime package");
    store
        .connection()
        .execute(
            "UPDATE runtime_package_manifests SET record_json = '{}'
             WHERE runtime_package_manifest_id = ?1",
            [fixture
                .runtime_package
                .runtime_package_manifest_id()
                .digest()
                .as_str()],
        )
        .expect("replace immutable bytes");

    assert!(matches!(
        store.put_runtime_package_manifest(&fixture.runtime_package),
        Err(StoreError::ImmutableConflict)
    ));
}

#[test]
fn reads_reject_wrong_index_dependency_and_noncanonical_json() {
    assert_wrong_index();
    assert_wrong_dependency();
    assert_noncanonical_json();
}

fn assert_wrong_index() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("wrong-index.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_model_package_manifest(&fixture.model_package)
        .expect("store model package");
    let forged = "a".repeat(64);
    store
        .connection()
        .execute(
            "UPDATE model_package_manifests SET model_package_manifest_id = ?1",
            [&forged],
        )
        .expect("forge indexed id");
    let forged_id: ModelPackageManifestId =
        serde_json::from_str(&format!("\"{forged}\"")).expect("parse forged id");
    assert!(matches!(
        store.model_package_manifest(&forged_id),
        Err(StoreError::CorruptRecord)
    ));
}

fn assert_wrong_dependency() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("wrong-dependency.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_runtime_package_manifest(&fixture.runtime_package)
        .expect("store runtime package");
    store
        .connection()
        .execute(
            "UPDATE runtime_package_manifests SET artifact_set_id = ?1
             WHERE runtime_package_manifest_id = ?2",
            params![
                fixture.model_set.artifact_set_id().digest().as_str(),
                fixture
                    .runtime_package
                    .runtime_package_manifest_id()
                    .digest()
                    .as_str()
            ],
        )
        .expect("forge dependency id");
    assert!(matches!(
        store.runtime_package_manifest(&fixture.runtime_package.runtime_package_manifest_id()),
        Err(StoreError::CorruptRecord)
    ));
}

fn assert_noncanonical_json() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("wrong-json.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_model_package_manifest(&fixture.model_package)
        .expect("store model package");
    store
        .connection()
        .execute(
            "UPDATE model_package_manifests SET record_json = record_json || ' '",
            [],
        )
        .expect("make JSON noncanonical");
    assert!(matches!(
        store.model_package_manifest(&fixture.model_package.model_package_manifest_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn native_load_read_rejects_corrupt_recursive_artifact_set() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("recursive-corruption.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_runtime_package_manifest(&fixture.runtime_package)
        .expect("store runtime package");
    store
        .put_native_load_observation(&fixture.native_load)
        .expect("store native observation");
    store
        .connection()
        .execute(
            "UPDATE artifact_set_manifests SET record_json = '{}'
             WHERE artifact_set_id = ?1",
            [fixture.runtime_set.artifact_set_id().digest().as_str()],
        )
        .expect("corrupt recursive dependency");

    assert!(matches!(
        store.native_load_observation(&fixture.native_load.native_load_observation_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn transformed_reads_reject_wrong_or_corrupt_source_relationships() {
    assert_wrong_transformed_source_index();
    assert_corrupt_transformed_source_record();
    assert_untransformed_source_index();
}

fn assert_wrong_transformed_source_index() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("wrong-transformed-source.db");
    let fixture = PackageFixture::new();
    let (runtime_set, runtime) = fixture.transformed_runtime_package();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_artifact_set_manifest(&runtime_set)
        .expect("store transformed runtime byte set");
    fixture.put_source_artifact_set(&store);
    store
        .put_runtime_package_manifest(&runtime)
        .expect("store transformed runtime");
    store
        .connection()
        .execute(
            "UPDATE runtime_package_manifests SET source_artifact_set_id = ?1",
            [fixture.model_set.artifact_set_id().digest().as_str()],
        )
        .expect("forge transformed source index");

    assert!(matches!(
        store.runtime_package_manifest(&runtime.runtime_package_manifest_id()),
        Err(StoreError::CorruptRecord)
    ));
}

fn assert_corrupt_transformed_source_record() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("corrupt-transformed-source.db");
    let fixture = PackageFixture::new();
    let (model_set, model) = fixture.transformed_model_package();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_artifact_set_manifest(&model_set)
        .expect("store transformed model byte set");
    fixture.put_source_artifact_set(&store);
    store
        .put_model_package_manifest(&model)
        .expect("store transformed model");
    store
        .connection()
        .execute(
            "UPDATE artifact_set_manifests SET record_json = '{}'
             WHERE artifact_set_id = ?1",
            [fixture.source_set.artifact_set_id().digest().as_str()],
        )
        .expect("corrupt transformed source record");

    assert!(matches!(
        store.model_package_manifest(&model.model_package_manifest_id()),
        Err(StoreError::CorruptRecord)
    ));
}

fn assert_untransformed_source_index() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("untransformed-source-index.db");
    let fixture = PackageFixture::new();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    fixture.put_artifact_sets(&store);
    store
        .put_model_package_manifest(&fixture.model_package)
        .expect("store untransformed model");
    store
        .connection()
        .execute(
            "UPDATE model_package_manifests SET source_artifact_set_id = ?1",
            [fixture.runtime_set.artifact_set_id().digest().as_str()],
        )
        .expect("forge untransformed source index");

    assert!(matches!(
        store.model_package_manifest(&fixture.model_package.model_package_manifest_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn absent_ids_return_none() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("absent.db");
    let store = ArtifactStateStore::open(&path).expect("open store");
    let id: RuntimePackageManifestId =
        serde_json::from_str(&format!("\"{}\"", "b".repeat(64))).expect("parse absent id");
    assert_eq!(
        store
            .runtime_package_manifest(&id)
            .expect("query absent runtime package"),
        None
    );
}
