use std::fs;

use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::support::{import_limits, write_installed_fixture};
#[cfg(unix)]
use crate::OllamaModelImportError;
#[cfg(unix)]
use crate::installed_ollama_import::PinnedInstalledOllamaModel;
use crate::{ArtifactRepository, ArtifactRepositoryErrorKind};

#[test]
fn missing_short_and_digest_drifting_blobs_fail_before_repository_mutation() {
    for mode in ["missing", "short", "digest"] {
        let fixture_root = tempdir().expect("temporary fixture root");
        let fixture = write_installed_fixture(fixture_root.path());
        let blob = fixture.blob_paths.first().expect("fixture blob");
        match mode {
            "missing" => fs::remove_file(blob).expect("remove selected blob"),
            "short" => {
                let mut bytes = fs::read(blob).expect("read selected blob");
                bytes.pop();
                fs::write(blob, bytes).expect("shorten selected blob");
            }
            "digest" => {
                let mut bytes = fs::read(blob).expect("read selected blob");
                *bytes.last_mut().expect("blob is nonempty") ^= 1;
                fs::write(blob, bytes).expect("change selected blob");
            }
            _ => unreachable!(),
        }
        let data = fixture_root.path().join("data");
        let repository = ArtifactRepository::new(&data).expect("repository");
        let error = repository
            .import_installed_ollama_model(
                &fixture.selection,
                import_limits(),
                &CancellationToken::new(),
            )
            .expect_err("drifting package cannot import");
        assert!(
            matches!(
                error.kind(),
                ArtifactRepositoryErrorKind::Operational
                    | ArtifactRepositoryErrorKind::Conflict
                    | ArtifactRepositoryErrorKind::ConcurrentModification
            ),
            "{mode}: {error:?}"
        );
        assert!(!data.exists(), "{mode}");
    }
}

#[test]
fn hard_linked_source_blob_is_rejected() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let blob = fixture.blob_paths.first().expect("fixture blob");
    fs::hard_link(blob, blob.with_extension("alias")).expect("create source hard link");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("multiply linked source cannot import");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(!data.exists());
}

#[cfg(unix)]
#[test]
fn source_name_replacement_after_pinning_is_detected() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let pinned = PinnedInstalledOllamaModel::open_and_reconstruct(
        &fixture.selection,
        &import_limits().reconstruction,
        &CancellationToken::new(),
    )
    .expect("pin exact source");
    let original = fs::read(&fixture.manifest_path).expect("read manifest");
    let displaced = fixture.manifest_path.with_extension("displaced");
    fs::rename(&fixture.manifest_path, &displaced).expect("replace manifest name");
    fs::write(&fixture.manifest_path, original).expect("write replacement manifest");
    assert!(matches!(
        pinned.recheck(),
        Err(OllamaModelImportError::SourceChanged)
    ));
}

#[cfg(unix)]
#[test]
fn source_symlink_substitution_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let blob = fixture.blob_paths.first().expect("fixture blob");
    let target = blob.with_extension("target");
    fs::rename(blob, &target).expect("move source blob");
    symlink(&target, blob).expect("create source symlink");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("indirect blob cannot import");
    assert!(matches!(
        error.kind(),
        ArtifactRepositoryErrorKind::Conflict | ArtifactRepositoryErrorKind::Operational
    ));
    assert!(!data.exists());
}

#[cfg(windows)]
#[test]
fn source_reparse_substitution_is_rejected() {
    use std::os::windows::fs::symlink_file;

    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let blob = fixture.blob_paths.first().expect("fixture blob");
    let target = blob.with_extension("target");
    fs::rename(blob, &target).expect("move source blob");
    if let Err(error) = symlink_file(&target, blob) {
        if crate::symlink_test_support::skip_unavailable_link("Ollama blob reparse", &error) {
            return;
        }
        panic!("create source reparse fixture: {error}");
    }
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("indirect blob cannot import");
    assert!(matches!(
        error.kind(),
        ArtifactRepositoryErrorKind::Conflict | ArtifactRepositoryErrorKind::Operational
    ));
    assert!(!data.exists());
}

#[test]
fn conflicting_managed_bytes_and_corrupt_store_fail_closed() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let first = repository
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("initial import");
    let set_root = data.join("artifact-storage/sets").join(format!(
        "set-v1-{}",
        first.artifact_set_key.artifact_set_id().digest().as_str()
    ));
    let model = set_root.join("model/model.gguf");
    let mut changed = fs::read(&model).expect("read managed model");
    *changed.last_mut().expect("model is nonempty") ^= 1;
    fs::write(&model, changed).expect("corrupt managed model");
    let conflict = repository
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("managed conflict cannot be reconfirmed");
    assert_eq!(conflict.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/.set-staging"))
            .expect("read staging")
            .count(),
        0
    );

    let connection = rusqlite::Connection::open(data.join("artifact-state.sqlite3"))
        .expect("open state for corruption fixture");
    connection
        .execute("DROP TABLE model_package_manifests", [])
        .expect("remove required schema table");
    drop(connection);
    let state_error = repository
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("corrupt store cannot import");
    assert!(matches!(
        state_error.kind(),
        ArtifactRepositoryErrorKind::CorruptState | ArtifactRepositoryErrorKind::IncompatibleState
    ));
}
