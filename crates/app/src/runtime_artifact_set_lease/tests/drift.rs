use std::{fs, path::Path};

use rewrite_types::CancellationToken;

use super::{LEASE_LIMITS, MEMBERS, imported_repository, set_root};
use crate::{ArtifactRepositoryError, ArtifactRepositoryErrorKind, ArtifactSetLeaseError};

/// Acquires a lease after applying one managed-tree mutation and returns the error.
fn lease_after(mutate: impl FnOnce(&Path)) -> ArtifactRepositoryError {
    let directory = tempfile::tempdir().expect("temporary directory");
    let (repository, _, artifact_set_id) = imported_repository(directory.path());
    let root = set_root(directory.path(), &artifact_set_id);
    mutate(&root);
    repository
        .lease_set(&artifact_set_id, LEASE_LIMITS, &CancellationToken::new())
        .expect_err("a mutated managed tree must not be leased")
}

#[test]
fn member_content_drift_fails_closed() {
    let error = lease_after(|root| {
        fs::write(root.join(MEMBERS[0].0), b"{\"name\":\"FIXTURE\"}")
            .expect("replace member bytes");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(matches!(
        error,
        ArtifactRepositoryError::SetLease(ArtifactSetLeaseError::StorageConflict)
    ));
}

#[test]
fn member_size_drift_fails_closed() {
    let error = lease_after(|root| {
        fs::write(root.join(MEMBERS[2].0), b"weights and more").expect("grow member");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
}

#[test]
fn an_extra_managed_entry_fails_closed() {
    let error = lease_after(|root| {
        fs::write(root.join("unexpected.bin"), b"extra").expect("write extra entry");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(matches!(
        error,
        ArtifactRepositoryError::SetLease(ArtifactSetLeaseError::StorageConflict)
    ));
}

#[test]
fn a_missing_member_fails_closed() {
    let error = lease_after(|root| {
        fs::remove_file(root.join(MEMBERS[1].0)).expect("remove member");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
}

#[test]
fn registered_state_without_its_set_root_fails_closed() {
    let error = lease_after(|root| {
        fs::remove_dir_all(root).expect("remove the published set root");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::CorruptState);
    assert!(matches!(
        error,
        ArtifactRepositoryError::SetLease(ArtifactSetLeaseError::StateStorageMismatch)
    ));
}

#[cfg(unix)]
#[test]
fn a_managed_member_alias_fails_closed() {
    let error = lease_after(|root| {
        fs::hard_link(
            root.join(MEMBERS[0].0),
            root.join("model").join("alias.bin"),
        )
        .expect("alias a managed member");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
}
