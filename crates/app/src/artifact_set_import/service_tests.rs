use std::fs;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_model_store::{ArtifactStateStore, WriteDisposition};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::{
    ArtifactSetImportDisposition, ArtifactSetImportError, ArtifactSetImportLimits,
    ArtifactSetImportStage, OfflineArtifactSetImportRequest, OfflineArtifactSetImportService,
};

#[path = "service_tests/adversarial.rs"]
mod adversarial;
#[path = "service_tests/source_and_layout.rs"]
mod source_and_layout;

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size"),
        ArtifactSetRelativePath::new(path).expect("fixture path"),
    )
}

fn manifest() -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        member("config.json", b"{}"),
        member("model/empty.bin", b""),
        member("model/weights.bin", b"weights"),
    ])
    .expect("fixture manifest")
}

fn request(source_root: &std::path::Path) -> OfflineArtifactSetImportRequest {
    OfflineArtifactSetImportRequest {
        source_root: source_root.to_path_buf(),
        manifest: manifest(),
    }
}

fn storage_key(manifest: &ArtifactSetManifest) -> String {
    format!("set-v1-{}", manifest.artifact_set_id().digest().as_str())
}

fn assert_no_state(store: &ArtifactStateStore, manifest: &ArtifactSetManifest) {
    assert!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("read artifact-set installation")
            .is_none()
    );
}

fn write_source(root: &std::path::Path) {
    fs::create_dir(root).expect("source root");
    fs::create_dir(root.join("model")).expect("nested source directory");
    fs::write(root.join("config.json"), b"{}").expect("config source");
    fs::write(root.join("model/empty.bin"), b"").expect("empty source");
    fs::write(root.join("model/weights.bin"), b"weights").expect("weights source");
}

const fn limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 16,
        maximum_member_bytes: 1024,
        maximum_total_bytes: 4096,
        maximum_tree_entries: 32,
        maximum_storage_entries: 16,
        maximum_staging_entries: 16,
    }
}

#[test]
fn publishes_nested_exact_tree_and_repeats_idempotently() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let manifest = manifest();
    let request = OfflineArtifactSetImportRequest {
        source_root: source,
        manifest: manifest.clone(),
    };
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service = OfflineArtifactSetImportService::open(&storage, &mut store, limits())
        .expect("set import service");
    let mut progress = Vec::new();
    let first = service
        .import(&request, &CancellationToken::new(), |event| {
            progress.push(event);
        })
        .expect("first set import");

    assert_eq!(first.disposition, ArtifactSetImportDisposition::Imported);
    assert_eq!(first.state.manifest, WriteDisposition::Inserted);
    assert_eq!(first.state.installed, WriteDisposition::Inserted);
    assert_eq!(first.state.installation.epoch.get(), 1);
    let final_root = storage.join("sets").join(first.installed.storage_key());
    assert_eq!(
        fs::read(final_root.join("config.json")).expect("read managed config"),
        b"{}"
    );
    assert_eq!(
        fs::read(final_root.join("model/empty.bin")).expect("read managed empty member"),
        b""
    );
    assert_eq!(
        fs::read(final_root.join("model/weights.bin")).expect("read managed weights"),
        b"weights"
    );
    assert_eq!(
        progress.iter().map(|event| event.stage).collect::<Vec<_>>(),
        vec![
            ArtifactSetImportStage::InspectingSource,
            ArtifactSetImportStage::StagingAndVerifying,
            ArtifactSetImportStage::StagingAndVerifying,
            ArtifactSetImportStage::StagingAndVerifying,
            ArtifactSetImportStage::StagingAndVerifying,
            ArtifactSetImportStage::PublishingTree,
            ArtifactSetImportStage::Finalizing,
        ]
    );

    let repeated = service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("repeat exact import");
    assert_eq!(
        repeated.disposition,
        ArtifactSetImportDisposition::AlreadyPresent
    );
    assert_eq!(repeated.installed, first.installed);
    assert_eq!(repeated.state.installed, WriteDisposition::AlreadyPresent);
}

#[test]
fn registers_an_exact_published_orphan_without_recopying() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = OfflineArtifactSetImportRequest {
        source_root: source,
        manifest: manifest(),
    };
    {
        let mut first_store =
            ArtifactStateStore::open(&directory.path().join("first.sqlite3")).expect("first store");
        OfflineArtifactSetImportService::open(&storage, &mut first_store, limits())
            .expect("first service")
            .import(&request, &CancellationToken::new(), |_| {})
            .expect("publish fixture");
    }
    let mut fresh_store = ArtifactStateStore::open(&directory.path().join("fresh.sqlite3"))
        .expect("fresh state store");
    let result = OfflineArtifactSetImportService::open(&storage, &mut fresh_store, limits())
        .expect("fresh service")
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("register exact orphan");

    assert_eq!(
        result.disposition,
        ArtifactSetImportDisposition::RegisteredExisting
    );
    assert_eq!(result.state.installed, WriteDisposition::Inserted);
    assert_eq!(
        fs::read(
            storage
                .join("sets")
                .join(result.installed.storage_key())
                .join("model/weights.bin")
        )
        .expect("read unrecreated orphan member"),
        b"weights"
    );
    assert!(
        fs::read_dir(storage.join(".set-staging"))
            .expect("staging directory")
            .next()
            .is_none(),
        "orphan registration must not restage the published tree"
    );
}

#[test]
fn installed_state_without_its_final_root_fails_closed() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = OfflineArtifactSetImportRequest {
        source_root: source,
        manifest: manifest(),
    };
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let first = service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("initial import");
    fs::remove_dir_all(storage.join("sets").join(first.installed.storage_key()))
        .expect("remove final fixture tree");

    assert!(matches!(
        service.import(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::StateStorageMismatch)
    ));
}

#[test]
fn rejects_extra_empty_directory_and_member_content_drift() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    write_source(&source);
    fs::create_dir(source.join("extra")).expect("extra empty directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service = OfflineArtifactSetImportService::open(
        directory.path().join("storage"),
        &mut store,
        limits(),
    )
    .expect("service");
    let request = OfflineArtifactSetImportRequest {
        source_root: source.clone(),
        manifest: manifest(),
    };
    assert!(matches!(
        service.import(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::SourceTreeMismatch)
    ));

    fs::remove_dir(source.join("extra")).expect("remove extra fixture");
    fs::write(source.join("model/weights.bin"), b"changed").expect("drift source bytes");
    assert!(matches!(
        service.import(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::DigestMismatch)
    ));
}

#[test]
fn final_callback_cancellation_cleans_the_owned_stage_before_publication() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = OfflineArtifactSetImportRequest {
        source_root: source,
        manifest: manifest(),
    };
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let cancellation = CancellationToken::new();
    let callback_token = cancellation.clone();
    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactSetImportStage::Finalizing {
                callback_token.cancel();
            }
        })
        .expect_err("final prepublication cancellation must stop import");

    assert!(
        matches!(error, ArtifactSetImportError::Cancelled),
        "unexpected cancellation result: {error:?}"
    );
    assert!(
        fs::read_dir(storage.join(".set-staging"))
            .expect("staging directory")
            .next()
            .is_none()
    );
    assert!(
        fs::read_dir(storage.join("sets"))
            .expect("sets directory")
            .next()
            .is_none()
    );
    assert!(
        store
            .artifact_set_installation(&request.manifest.artifact_set_id())
            .expect("read state")
            .is_none()
    );
}

#[test]
fn source_root_must_be_a_direct_directory() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.bin");
    fs::write(&source, b"not-a-directory").expect("write file source");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service = OfflineArtifactSetImportService::open(
        directory.path().join("storage"),
        &mut store,
        limits(),
    )
    .expect("service");

    assert!(matches!(
        service.import(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: manifest(),
            },
            &CancellationToken::new(),
            |_| {},
        ),
        Err(ArtifactSetImportError::SourceNotDirectory)
    ));
}

#[cfg(windows)]
#[test]
fn source_root_junction_is_rejected() {
    use std::os::windows::fs::symlink_dir;

    let directory = tempdir().expect("temporary directory");
    let real_source = directory.path().join("real-source");
    let linked = directory.path().join("linked-source");
    write_source(&real_source);
    match symlink_dir(&real_source, &linked) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("create source-root junction: {error}"),
    }
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service = OfflineArtifactSetImportService::open(
        directory.path().join("storage"),
        &mut store,
        limits(),
    )
    .expect("service");

    assert!(matches!(
        service.import(
            &OfflineArtifactSetImportRequest {
                source_root: linked,
                manifest: manifest(),
            },
            &CancellationToken::new(),
            |_| {},
        ),
        Err(ArtifactSetImportError::IndirectSource)
    ));
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_links_inside_the_source_tree() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    write_source(&source);
    fs::remove_file(source.join("config.json")).expect("remove direct source");
    symlink(source.join("model/weights.bin"), source.join("config.json")).expect("source symlink");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service = OfflineArtifactSetImportService::open(
        directory.path().join("storage"),
        &mut store,
        limits(),
    )
    .expect("service");

    assert!(matches!(
        service.import(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: manifest(),
            },
            &CancellationToken::new(),
            |_| {},
        ),
        Err(ArtifactSetImportError::UnsafeSourceTree)
    ));
}
