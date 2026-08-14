use std::fs;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use rusqlite::Connection;
use tempfile::tempdir;

use super::super::{ArtifactRepository, ArtifactRepositoryError, ArtifactRepositoryErrorKind};
use crate::{
    ArtifactSetImportDisposition, ArtifactSetImportLimits, OfflineArtifactSetImportRequest,
};

fn fixture() -> (ArtifactSetManifest, Vec<(&'static str, &'static [u8])>) {
    let files: Vec<(&str, &[u8])> = vec![("config.json", b"{}"), ("model/weights.bin", b"set")];
    let members = files
        .iter()
        .map(|(path, bytes)| {
            ArtifactSetMember::new(
                ArtifactId::from_digest(Digest::sha256(bytes)),
                u64::try_from(bytes.len()).expect("fixture size"),
                ArtifactSetRelativePath::new(*path).expect("fixture path"),
            )
        })
        .collect();
    (
        ArtifactSetManifest::new(members).expect("fixture manifest"),
        files,
    )
}

fn sibling_file_manifest(bytes: &[u8]) -> (rewrite_model::ArtifactManifest, rewrite_types::Digest) {
    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId as FileArtifactId, ArtifactManifest,
        ArtifactRole, ArtifactSource, DeclaredCapabilities, LicenseRecord,
    };

    let digest = Digest::sha256(bytes);
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: FileArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/sibling".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: digest.clone(),
        byte_size: u64::try_from(bytes.len()).expect("fixture size"),
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: Some("transformer".to_owned()),
        quantization: Some("q4".to_owned()),
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "weights".to_owned(),
            identifier: "Apache-2.0".to_owned(),
            text_digest: Digest::sha256(b"license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: Some(8_192),
        },
    };
    (manifest, digest)
}

const fn limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 8,
        maximum_member_bytes: 1024,
        maximum_total_bytes: 4096,
        maximum_tree_entries: 16,
        maximum_storage_entries: 8,
        maximum_staging_entries: 8,
    }
}

#[test]
fn repository_imports_set_and_creates_no_legacy_member_authority() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    fs::create_dir(&source).expect("source root");
    fs::create_dir(source.join("model")).expect("source nested root");
    let (manifest, files) = fixture();
    for (path, bytes) in &files {
        fs::write(source.join(path), bytes).expect("source member");
    }
    let data = directory.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let result = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: manifest.clone(),
            },
            limits(),
            &CancellationToken::new(),
        )
        .expect("repository set import");

    assert_eq!(result.disposition, ArtifactSetImportDisposition::Imported);
    assert_eq!(result.key.artifact_set_id(), &manifest.artifact_set_id());
    assert_eq!(result.key.installation_generation(), 1);
    let store = ArtifactStateStore::open_existing_read_only(&data.join("artifact-state.sqlite3"))
        .expect("read repository state");
    assert!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("set installation")
            .is_some()
    );
    assert!(
        store
            .artifact_inventory(manifest.members().len())
            .expect("legacy artifact inventory")
            .is_empty()
    );
}

#[test]
fn invalid_set_limits_fail_before_repository_initialization() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let (manifest, _) = fixture();
    let mut invalid = limits();
    invalid.maximum_total_bytes = 0;
    let error = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: directory.path().join("absent"),
                manifest,
            },
            invalid,
            &CancellationToken::new(),
        )
        .expect_err("invalid limits must fail");

    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::InvalidInput);
    assert!(matches!(error, ArtifactRepositoryError::SetImport(_)));
    assert!(!data.exists());
}

#[test]
fn set_import_refuses_legacy_schema_without_implicit_migration() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    fs::create_dir(&source).expect("source root");
    fs::create_dir(source.join("model")).expect("source nested root");
    let (manifest, files) = fixture();
    for (path, bytes) in &files {
        fs::write(source.join(path), bytes).expect("source member");
    }
    let data = directory.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let request = OfflineArtifactSetImportRequest {
        source_root: source,
        manifest: manifest.clone(),
    };
    let first = repository
        .import_set(&request, limits(), &CancellationToken::new())
        .expect("initialize current repository");
    let managed_member = data
        .join("artifact-storage/sets")
        .join(format!(
            "set-v1-{}",
            first.key.artifact_set_id().digest().as_str()
        ))
        .join("config.json");
    let state_path = data.join("artifact-state.sqlite3");
    Connection::open(&state_path)
        .expect("open state fixture")
        .execute_batch("DROP TABLE installed_artifact_sets; PRAGMA user_version = 3;")
        .expect("restore exact schema three");

    let error = repository
        .import_set(&request, limits(), &CancellationToken::new())
        .expect_err("ordinary import must not migrate state");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::IncompatibleState);
    assert_eq!(
        Connection::open(&state_path)
            .expect("reopen legacy state")
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .expect("read legacy version"),
        3
    );
    assert_eq!(
        fs::read(managed_member).expect("read preserved managed member"),
        b"{}"
    );
}

#[cfg(windows)]
#[test]
fn repository_never_adopts_wrong_case_managed_storage_root() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    fs::create_dir(&source).expect("source root");
    fs::create_dir(source.join("model")).expect("source nested root");
    let (manifest, files) = fixture();
    for (path, bytes) in &files {
        fs::write(source.join(path), bytes).expect("source member");
    }
    let data = directory.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: source.clone(),
                manifest: manifest.clone(),
            },
            limits(),
            &CancellationToken::new(),
        )
        .expect("initialize repository");
    let canonical = data.join("artifact-storage");
    let intermediate = data.join("storage-case-change");
    let alias = data.join("ARTIFACT-STORAGE");
    fs::rename(&canonical, &intermediate).expect("begin managed-root case change");
    fs::rename(&intermediate, &alias).expect("finish managed-root case change");
    fs::write(alias.join("sentinel"), b"external").expect("alias sentinel");
    fs::write(source.join("config.json"), b"changed").expect("change source config");
    let changed_manifest = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(
            ArtifactId::from_digest(Digest::sha256(b"changed")),
            7,
            ArtifactSetRelativePath::new("config.json").expect("changed config path"),
        ),
        ArtifactSetMember::new(
            ArtifactId::from_digest(Digest::sha256(b"set")),
            3,
            ArtifactSetRelativePath::new("model/weights.bin").expect("changed weights path"),
        ),
    ])
    .expect("changed manifest");
    let error = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: changed_manifest.clone(),
            },
            limits(),
            &CancellationToken::new(),
        )
        .expect_err("wrong-case managed root must not be adopted");

    assert_eq!(
        error.kind(),
        ArtifactRepositoryErrorKind::ConcurrentModification
    );
    assert_eq!(
        fs::read(alias.join("sentinel")).expect("read alias sentinel"),
        b"external"
    );
    let store = ArtifactStateStore::open_existing_read_only(&data.join("artifact-state.sqlite3"))
        .expect("read repository state");
    assert!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("original set installation")
            .is_some()
    );
    assert!(
        store
            .artifact_set_installation(&changed_manifest.artifact_set_id())
            .expect("changed set installation")
            .is_none()
    );
}

#[test]
fn set_import_coexists_with_single_file_import_and_inventory() {
    use crate::{
        ArtifactImportLimits, ArtifactInventoryLimits, OfflineArtifactImportRequest,
        RegisteredArtifactBytes,
    };

    let directory = tempdir().expect("temporary directory");
    let set_source = directory.path().join("set-source");
    fs::create_dir(&set_source).expect("set source root");
    fs::create_dir(set_source.join("model")).expect("set nested root");
    let (set_manifest, files) = fixture();
    for (path, bytes) in &files {
        fs::write(set_source.join(path), bytes).expect("set source member");
    }
    let file_bytes = b"single-file sibling";
    let file_source = directory.path().join("sibling.gguf");
    fs::write(&file_source, file_bytes).expect("single-file source");
    let (file_manifest, file_digest) = sibling_file_manifest(file_bytes);
    let data = directory.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");

    let set_result = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: set_source,
                manifest: set_manifest.clone(),
            },
            limits(),
            &CancellationToken::new(),
        )
        .expect("set import");
    let file_result = repository
        .import(
            &OfflineArtifactImportRequest {
                source: file_source,
                manifest: file_manifest,
            },
            ArtifactImportLimits {
                maximum_artifact_bytes: 1024,
                maximum_storage_entries: 16,
            },
            &CancellationToken::new(),
        )
        .expect("single-file import after set import");
    let inventory = repository
        .inventory(
            ArtifactInventoryLimits {
                maximum_state_entries: 16,
                maximum_storage_entries: 16,
                maximum_artifact_bytes: 1024,
                maximum_total_verification_bytes: 4096,
            },
            &CancellationToken::new(),
        )
        .expect("inventory after mixed imports");

    assert_eq!(
        set_result.disposition,
        ArtifactSetImportDisposition::Imported
    );
    assert_eq!(
        file_result.disposition,
        crate::ArtifactRepositoryImportDisposition::Imported
    );
    assert_eq!(inventory.registered.len(), 1);
    assert_eq!(
        inventory.registered[0].bytes,
        RegisteredArtifactBytes::Verified
    );
    assert_eq!(
        inventory.registered[0].installation.artifact_id().digest(),
        &file_digest
    );
    assert!(inventory.verified_orphans.is_empty());
    assert_eq!(
        fs::read(
            data.join("artifact-storage/artifacts")
                .join(file_digest.as_str())
        )
        .expect("read single-file bytes"),
        file_bytes
    );
    assert_eq!(
        fs::read(
            data.join("artifact-storage/sets")
                .join(format!(
                    "set-v1-{}",
                    set_result.key.artifact_set_id().digest().as_str()
                ))
                .join("config.json")
        )
        .expect("read set member"),
        b"{}"
    );
    let store = ArtifactStateStore::open_existing_read_only(&data.join("artifact-state.sqlite3"))
        .expect("read repository state");
    assert!(
        store
            .artifact_set_installation(&set_manifest.artifact_set_id())
            .expect("set installation after sibling import")
            .is_some()
    );
}
