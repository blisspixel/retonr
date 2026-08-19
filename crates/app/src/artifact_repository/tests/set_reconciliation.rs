use std::fs;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::super::ArtifactRepository;
use crate::{
    ArtifactReconciliationDisposition, ArtifactSetImportLimits, ArtifactSetReconciliationLimits,
    OfflineArtifactSetImportRequest,
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

const fn import_limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 8,
        maximum_member_bytes: 1024,
        maximum_total_bytes: 4096,
        maximum_tree_entries: 16,
        maximum_storage_entries: 8,
        maximum_staging_entries: 8,
    }
}

const fn reconcile_limits() -> ArtifactSetReconciliationLimits {
    ArtifactSetReconciliationLimits {
        maximum_members: 8,
        maximum_member_bytes: 1024,
        maximum_total_bytes: 4096,
        maximum_tree_entries: 16,
        maximum_storage_entries: 8,
    }
}

#[test]
fn repository_reconciles_an_imported_set_without_copying() {
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
                source_root: source,
                manifest: manifest.clone(),
            },
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("import set");

    let result = repository
        .reconcile_set(
            manifest.clone(),
            reconcile_limits(),
            &CancellationToken::new(),
        )
        .expect("reconcile existing set");
    assert_eq!(
        result.disposition,
        ArtifactReconciliationDisposition::AlreadyRegistered
    );
    assert_eq!(result.key.artifact_set_id(), &manifest.artifact_set_id());
    assert_eq!(result.key.installation_generation(), 1);
}

#[test]
fn repository_set_reconcile_requires_an_initialized_repository() {
    let directory = tempdir().expect("temporary directory");
    let repository = ArtifactRepository::new(directory.path().join("missing")).expect("repository");
    let (manifest, _) = fixture();
    let error = repository
        .reconcile_set(manifest, reconcile_limits(), &CancellationToken::new())
        .expect_err("missing repository");
    assert_eq!(
        error.kind(),
        super::super::ArtifactRepositoryErrorKind::NotInitialized
    );
}
