use std::fs;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::super::ArtifactRepository;
use crate::{
    ArtifactRemovalDisposition, ArtifactSetImportLimits, ArtifactSetRemovalLimits,
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

const fn removal_limits() -> ArtifactSetRemovalLimits {
    ArtifactSetRemovalLimits {
        maximum_members: 8,
        maximum_member_bytes: 1024,
        maximum_total_bytes: 4096,
        maximum_tree_entries: 16,
        maximum_storage_entries: 8,
    }
}

#[test]
fn repository_removes_an_imported_set_without_touching_single_file_state() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("repository");
    let repository = ArtifactRepository::new(&data).expect("open repository");
    let (manifest, files) = fixture();
    let source = directory.path().join("source");
    fs::create_dir_all(source.join("model")).expect("source tree");
    for (path, bytes) in &files {
        fs::write(source.join(path), bytes).expect("write member");
    }
    let imported = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest,
            },
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("import set");

    let removed = repository
        .remove_set(&imported.key, removal_limits(), &CancellationToken::new())
        .expect("remove set");
    assert_eq!(removed.disposition, ArtifactRemovalDisposition::Removed);
    assert_eq!(removed.key, imported.key);

    let pending = repository
        .pending_operations(8, &CancellationToken::new())
        .expect("inspect pending operations");
    assert!(pending.artifact_removals.is_empty());
    assert!(pending.artifact_set_removals.is_empty());

    let already = repository
        .remove_set(&imported.key, removal_limits(), &CancellationToken::new())
        .expect("repeat completed removal");
    assert_eq!(
        already.disposition,
        ArtifactRemovalDisposition::AlreadyRemoved
    );
}
