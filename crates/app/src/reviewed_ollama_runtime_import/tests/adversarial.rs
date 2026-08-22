use std::fs;

use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::support::{import_limits, write_runtime_fixture};
#[cfg(unix)]
use crate::reviewed_ollama_runtime_import::PinnedReviewedOllamaRuntime;
use crate::{ArtifactRepository, ArtifactRepositoryErrorKind};

#[test]
fn extra_tree_file_fails_closed_before_package_persistence() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    fs::create_dir_all(fixture.member_root.join("tmp")).expect("create extra parent");
    fs::write(fixture.member_root.join("tmp/extra.bin"), b"extra").expect("write extra");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("extra tree files cannot import");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    if data.join("artifact-state.sqlite3").exists() {
        let store = rewrite_model_store::ArtifactStateStore::open_existing_read_only(
            &data.join("artifact-state.sqlite3"),
        )
        .expect("read repository state");
        assert!(
            store
                .artifact_inventory(1)
                .expect("legacy inventory")
                .is_empty()
        );
    }
}

#[test]
fn missing_short_and_digest_drifting_members_fail_closed() {
    for mode in ["missing", "short", "digest"] {
        let fixture_root = tempdir().expect("temporary fixture root");
        let fixture = write_runtime_fixture(fixture_root.path());
        let member = fixture.member_paths.first().expect("fixture member");
        match mode {
            "missing" => fs::remove_file(member).expect("remove selected member"),
            "short" => {
                let mut bytes = fs::read(member).expect("read selected member");
                bytes.pop();
                fs::write(member, bytes).expect("shorten selected member");
            }
            "digest" => {
                let mut bytes = fs::read(member).expect("read selected member");
                *bytes.last_mut().expect("member is nonempty") ^= 1;
                fs::write(member, bytes).expect("change selected member");
            }
            _ => unreachable!(),
        }
        let data = fixture_root.path().join("data");
        let error = ArtifactRepository::new(&data)
            .expect("repository")
            .import_reviewed_ollama_runtime(
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
fn cancelled_import_does_not_create_repository_state() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(&fixture.selection, import_limits(), &cancellation)
        .expect_err("cancelled import cannot complete");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Cancelled);
    assert!(!data.exists());
}

#[test]
fn extra_empty_directory_fails_closed() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    fs::create_dir(fixture.member_root.join("tmp")).expect("create extra directory");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("extra directories cannot import");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
}

#[test]
fn hard_linked_layout_file_is_rejected() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    fs::hard_link(
        &fixture.layout_path,
        fixture.layout_path.with_extension("alias"),
    )
    .expect("create layout hard link");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("multiply linked layout cannot import");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(!data.exists());
}

#[cfg(unix)]
#[test]
fn source_name_replacement_after_pinning_is_detected() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let pinned = PinnedReviewedOllamaRuntime::open_and_reconstruct(
        &fixture.selection,
        &import_limits().reconstruction,
        &CancellationToken::new(),
    )
    .expect("pin exact source");
    let original = fs::read(&fixture.layout_path).expect("read layout");
    let displaced = fixture.layout_path.with_extension("displaced");
    fs::rename(&fixture.layout_path, &displaced).expect("replace layout name");
    fs::write(&fixture.layout_path, original).expect("write replacement layout");
    assert!(matches!(
        pinned.recheck(),
        Err(crate::OllamaRuntimeImportError::SourceChanged)
    ));
}

#[cfg(unix)]
#[test]
fn source_symlink_substitution_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let member = fixture.member_paths.first().expect("fixture member");
    let target = member.with_extension("target");
    fs::rename(member, &target).expect("move source member");
    symlink(&target, member).expect("create source symlink");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("indirect member cannot import");
    assert!(
        matches!(
            error.kind(),
            ArtifactRepositoryErrorKind::Conflict
                | ArtifactRepositoryErrorKind::Operational
                | ArtifactRepositoryErrorKind::ConcurrentModification
        ),
        "{error:?}"
    );
    assert!(!data.exists());
}

#[cfg(windows)]
#[test]
fn source_reparse_substitution_is_rejected() {
    use std::os::windows::fs::symlink_file;

    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let member = fixture.member_paths.first().expect("fixture member");
    let target = member.with_extension("target");
    fs::rename(member, &target).expect("move source member");
    if let Err(error) = symlink_file(&target, member) {
        if crate::symlink_test_support::skip_unavailable_link("runtime member reparse", &error) {
            return;
        }
        panic!("create source reparse fixture: {error}");
    }
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("indirect member cannot import");
    assert!(
        matches!(
            error.kind(),
            ArtifactRepositoryErrorKind::Conflict
                | ArtifactRepositoryErrorKind::Operational
                | ArtifactRepositoryErrorKind::ConcurrentModification
        ),
        "{error:?}"
    );
    assert!(!data.exists());
}

#[test]
fn conflicting_managed_bytes_fail_closed() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let first = repository
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("initial import");
    let set_root = data.join("artifact-storage/sets").join(format!(
        "set-v1-{}",
        first.artifact_set_key.artifact_set_id().digest().as_str()
    ));
    let entrypoint = set_root.join("bin/ollama");
    let mut changed = fs::read(&entrypoint).expect("read managed entrypoint");
    *changed.last_mut().expect("entrypoint is nonempty") ^= 1;
    fs::write(&entrypoint, changed).expect("corrupt managed entrypoint");
    let conflict = repository
        .import_reviewed_ollama_runtime(
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
}

#[test]
fn hard_linked_source_member_is_rejected() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let member = fixture.member_paths.first().expect("fixture member");
    fs::hard_link(member, member.with_extension("alias")).expect("create source hard link");
    let data = fixture_root.path().join("data");
    let error = ArtifactRepository::new(&data)
        .expect("repository")
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("multiply linked source cannot import");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(!data.exists());
}
