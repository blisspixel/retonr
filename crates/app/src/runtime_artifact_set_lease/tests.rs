use std::{fs, path::Path};

use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    ArtifactRepository, ArtifactRepositoryErrorKind, ArtifactSetImportLimits,
    ArtifactSetLeaseError, OfflineArtifactSetImportRequest, RuntimeArtifactSetLeaseLimits,
};

pub(super) const LEASE_LIMITS: RuntimeArtifactSetLeaseLimits = RuntimeArtifactSetLeaseLimits {
    maximum_members: 16,
    maximum_member_bytes: 1024,
    maximum_total_bytes: 4096,
    maximum_tree_entries: 32,
    maximum_storage_entries: 16,
};

const IMPORT_LIMITS: ArtifactSetImportLimits = ArtifactSetImportLimits {
    maximum_members: 16,
    maximum_member_bytes: 1024,
    maximum_total_bytes: 4096,
    maximum_tree_entries: 32,
    maximum_storage_entries: 16,
    maximum_staging_entries: 16,
};

pub(super) const MEMBERS: [(&str, &[u8]); 3] = [
    ("config.json", b"{\"name\":\"fixture\"}"),
    ("model/tokenizer.json", b"tokenizer"),
    ("model/weights.bin", b"weights"),
];

pub(super) fn manifest() -> ArtifactSetManifest {
    let members = MEMBERS
        .iter()
        .map(|(path, bytes)| {
            ArtifactSetMember::new(
                ArtifactId::from_digest(Digest::sha256(bytes)),
                u64::try_from(bytes.len()).expect("member size"),
                ArtifactSetRelativePath::new((*path).to_owned()).expect("relative path"),
            )
        })
        .collect::<Vec<_>>();
    ArtifactSetManifest::new(members).expect("canonical fixture manifest")
}

pub(super) fn write_source(root: &Path) {
    for (path, bytes) in MEMBERS {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("create source directory");
        }
        fs::write(target, bytes).expect("write source member");
    }
}

/// Creates an initialized repository holding one imported artifact set.
pub(super) fn imported_repository(
    directory: &Path,
) -> (ArtifactRepository, ArtifactSetManifest, ArtifactSetId) {
    let source = directory.join("source");
    fs::create_dir(&source).expect("create source root");
    write_source(&source);
    let registered = manifest();
    let repository = ArtifactRepository::new(directory.join("data")).expect("repository");
    let result = repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: source,
                manifest: manifest(),
            },
            IMPORT_LIMITS,
            &CancellationToken::new(),
        )
        .expect("import artifact set");
    let artifact_set_id = result.key.artifact_set_id().clone();
    (repository, registered, artifact_set_id)
}

pub(super) fn set_root(directory: &Path, artifact_set_id: &ArtifactSetId) -> std::path::PathBuf {
    directory
        .join("data")
        .join("artifact-storage")
        .join("sets")
        .join(format!("set-v1-{}", artifact_set_id.digest().as_str()))
}

fn import_again(
    repository: &ArtifactRepository,
    directory: &Path,
) -> crate::ArtifactRepositoryError {
    repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: directory.join("source"),
                manifest: manifest(),
            },
            IMPORT_LIMITS,
            &CancellationToken::new(),
        )
        .expect_err("an exclusive import must not run beside a live lease")
}

#[test]
fn a_live_set_lease_excludes_exclusive_repository_operations() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, manifest, artifact_set_id) = imported_repository(directory.path());

    let lease = repository
        .lease_set(&artifact_set_id, LEASE_LIMITS, &CancellationToken::new())
        .expect("acquire artifact-set lease");
    assert_eq!(lease.key().artifact_set_id(), &artifact_set_id);
    assert_eq!(lease.manifest(), &manifest);
    assert_eq!(lease.key().installation_generation(), 1);

    let blocked = import_again(&repository, directory.path());
    assert_eq!(blocked.kind(), ArtifactRepositoryErrorKind::InUse);
    assert!(matches!(
        repository.migrate(
            crate::ArtifactRepositoryMigrationLimits {
                maximum_state_bytes: 1 << 20,
                maximum_repository_entries: 64,
            },
            &CancellationToken::new(),
        ),
        Err(error) if error.kind() == ArtifactRepositoryErrorKind::InUse
    ));

    drop(lease);
    repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root: directory.path().join("source"),
                manifest: manifest.clone(),
            },
            IMPORT_LIMITS,
            &CancellationToken::new(),
        )
        .expect("import succeeds after the lease is released");
}

#[test]
fn a_live_set_lease_admits_read_only_inspection() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, _, artifact_set_id) = imported_repository(directory.path());
    let lease = repository
        .lease_set(&artifact_set_id, LEASE_LIMITS, &CancellationToken::new())
        .expect("acquire artifact-set lease");

    repository
        .inventory(
            crate::ArtifactInventoryLimits {
                maximum_state_entries: 64,
                maximum_storage_entries: 64,
                maximum_artifact_bytes: 1024,
                maximum_total_verification_bytes: 4096,
            },
            &CancellationToken::new(),
        )
        .expect("read-only inventory runs beside a live lease");
    repository
        .pending_operations(64, &CancellationToken::new())
        .expect("read-only inspection runs beside a live lease");

    let second = repository
        .lease_set(&artifact_set_id, LEASE_LIMITS, &CancellationToken::new())
        .expect("shared leases coexist");
    drop(second);
    drop(lease);
}

#[test]
fn an_unregistered_artifact_set_is_not_leasable() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, _, _) = imported_repository(directory.path());
    let unknown = ArtifactSetId::from_digest(Digest::sha256(b"absent"));
    let error = repository
        .lease_set(&unknown, LEASE_LIMITS, &CancellationToken::new())
        .expect_err("an unregistered set has no lease");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::NotFound);
    assert!(matches!(
        error,
        crate::ArtifactRepositoryError::SetLease(ArtifactSetLeaseError::ArtifactSetNotInstalled)
    ));
}

#[test]
fn cancellation_before_acquisition_yields_a_typed_cancellation() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, _, artifact_set_id) = imported_repository(directory.path());
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = repository
        .lease_set(&artifact_set_id, LEASE_LIMITS, &cancellation)
        .expect_err("cancellation is observed before any lease is granted");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Cancelled);
}

#[test]
fn invalid_and_undersized_ceilings_are_rejected() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, _, artifact_set_id) = imported_repository(directory.path());
    let cases = [
        RuntimeArtifactSetLeaseLimits {
            maximum_members: 0,
            ..LEASE_LIMITS
        },
        RuntimeArtifactSetLeaseLimits {
            maximum_member_bytes: 0,
            ..LEASE_LIMITS
        },
        RuntimeArtifactSetLeaseLimits {
            maximum_total_bytes: 0,
            ..LEASE_LIMITS
        },
        RuntimeArtifactSetLeaseLimits {
            maximum_tree_entries: 0,
            ..LEASE_LIMITS
        },
        RuntimeArtifactSetLeaseLimits {
            maximum_storage_entries: 0,
            ..LEASE_LIMITS
        },
    ];
    for limits in cases {
        let error = repository
            .lease_set(&artifact_set_id, limits, &CancellationToken::new())
            .expect_err("a zero ceiling is invalid");
        assert_eq!(error.kind(), ArtifactRepositoryErrorKind::InvalidInput);
    }

    let error = repository
        .lease_set(
            &artifact_set_id,
            RuntimeArtifactSetLeaseLimits {
                maximum_members: 2,
                ..LEASE_LIMITS
            },
            &CancellationToken::new(),
        )
        .expect_err("a manifest above the member ceiling is refused");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::ResourceLimit);
    assert!(matches!(
        error,
        crate::ArtifactRepositoryError::SetLease(ArtifactSetLeaseError::TooManyMembers {
            actual: 3,
            maximum: 2
        })
    ));
}

#[cfg(unix)]
#[test]
fn read_only_members_can_be_leased() {
    use std::os::unix::fs::PermissionsExt as _;

    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, _, artifact_set_id) = imported_repository(directory.path());
    let root = set_root(directory.path(), &artifact_set_id);
    for (path, _) in MEMBERS {
        let member = root.join(path);
        fs::set_permissions(&member, fs::Permissions::from_mode(0o400))
            .expect("make member read-only");
    }
    repository
        .lease_set(&artifact_set_id, LEASE_LIMITS, &CancellationToken::new())
        .expect("read-only managed members lease");
}

#[cfg(test)]
mod drift;
