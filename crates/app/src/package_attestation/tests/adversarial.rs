use std::{fs, path::Path};

use rewrite_types::CancellationToken;

use crate::{
    ArtifactRepositoryError, ArtifactRepositoryErrorKind, ArtifactSetLeaseError,
    PackageAttestationError, PackageAttestationService,
};

use super::{
    PACKAGE_LIMITS, RUNTIME_FILES, SET_LIMITS, VerificationObserver, VerificationStage, lease_set,
    runtime_fixture, set_root,
};

fn lease_after_mutation(mutate: impl FnOnce(&Path)) -> ArtifactRepositoryError {
    let (directory, repository, set, _package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    mutate(&root);
    repository
        .lease_set(
            &set.artifact_set_id(),
            SET_LIMITS,
            &CancellationToken::new(),
        )
        .expect_err("mutated managed set must not be leased")
}

#[test]
fn missing_and_extra_members_fail_before_package_evidence() {
    let missing = lease_after_mutation(|root| {
        fs::remove_file(root.join(RUNTIME_FILES[2].0)).expect("remove managed member");
    });
    assert_eq!(missing.kind(), ArtifactRepositoryErrorKind::Conflict);

    let extra = lease_after_mutation(|root| {
        fs::write(root.join("unexpected.bin"), b"extra").expect("write extra member");
    });
    assert_eq!(extra.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(matches!(
        extra,
        ArtifactRepositoryError::SetLease(ArtifactSetLeaseError::StorageConflict)
    ));
}

#[test]
fn size_and_digest_drift_fail_before_package_evidence() {
    let size = lease_after_mutation(|root| {
        fs::write(root.join(RUNTIME_FILES[2].0), b"larger-build-configuration")
            .expect("change managed member size");
    });
    assert_eq!(size.kind(), ArtifactRepositoryErrorKind::Conflict);

    let digest = lease_after_mutation(|root| {
        fs::write(root.join(RUNTIME_FILES[0].0), b"helper-v2")
            .expect("change same-size managed bytes");
    });
    assert_eq!(digest.kind(), ArtifactRepositoryErrorKind::Conflict);
}

#[test]
fn hard_link_alias_fails_before_package_evidence() {
    let error = lease_after_mutation(|root| {
        fs::hard_link(root.join(RUNTIME_FILES[0].0), root.join("helper-alias"))
            .expect("create managed member hard-link alias");
    });
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
}

#[test]
fn symlink_or_reparse_member_fails_before_package_evidence() {
    let (directory, repository, set, _package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let member = root.join(RUNTIME_FILES[0].0);
    let target = directory.path().join("replacement-helper");
    fs::write(&target, RUNTIME_FILES[0].1).expect("write symlink target");
    fs::remove_file(&member).expect("remove direct managed member");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&target, &member);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&target, &member);
    if let Err(error) = linked {
        if crate::symlink_test_support::skip_unavailable_link("package member reparse", &error) {
            return;
        }
        panic!("create package member link: {error}");
    }
    let error = repository
        .lease_set(
            &set.artifact_set_id(),
            SET_LIMITS,
            &CancellationToken::new(),
        )
        .expect_err("indirect managed member must not be leased");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
}

#[test]
fn replacement_before_member_hash_is_rejected() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let mut callback = |stage| {
        if stage == VerificationStage::BeforeMemberOpen(0) {
            fs::write(root.join(RUNTIME_FILES[0].0), b"helper-v2").expect("replace before hash");
        }
    };
    let mut observer = VerificationObserver::new(&mut callback);
    let error = PackageAttestationService::attest_runtime_with_observer(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
        &mut observer,
    )
    .expect_err("replacement before hash must fail");
    assert!(matches!(
        error,
        PackageAttestationError::MemberBytesConflict
            | PackageAttestationError::MemberIdentityChanged
            | PackageAttestationError::ArtifactSet(_)
    ));
}

#[cfg(unix)]
#[test]
fn replacement_after_member_hash_is_rejected() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let replacement = directory.path().join("replacement-after-hash");
    fs::write(&replacement, b"helper-v2").expect("write replacement");
    let mut callback = |stage| {
        if stage == VerificationStage::AfterMemberHash(0) {
            fs::rename(&replacement, root.join(RUNTIME_FILES[0].0))
                .expect("replace named member after hash");
        }
    };
    let mut observer = VerificationObserver::new(&mut callback);
    let error = PackageAttestationService::attest_runtime_with_observer(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
        &mut observer,
    )
    .expect_err("replacement after hash must fail");
    assert!(matches!(
        error,
        PackageAttestationError::MemberIdentityChanged | PackageAttestationError::ArtifactSet(_)
    ));
}

#[cfg(windows)]
#[test]
fn retained_windows_handle_denies_replacement_after_hash() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let replacement = directory.path().join("replacement-after-hash");
    fs::write(&replacement, b"helper-v2").expect("write replacement");
    let mut replacement_result = None;
    let lease = {
        let mut callback = |stage| {
            if stage == VerificationStage::AfterMemberHash(0) {
                replacement_result = Some(fs::rename(&replacement, root.join(RUNTIME_FILES[0].0)));
            }
        };
        let mut observer = VerificationObserver::new(&mut callback);
        PackageAttestationService::attest_runtime_with_observer(
            lease_set(&repository, &set.artifact_set_id()),
            &package,
            PACKAGE_LIMITS,
            &CancellationToken::new(),
            &mut observer,
        )
        .expect("the operating system denied replacement")
    };
    assert!(
        replacement_result
            .expect("replacement was attempted")
            .is_err()
    );
    drop(lease);
}

#[test]
fn extra_member_during_handle_verification_is_rejected() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let mut callback = |stage| {
        if stage == VerificationStage::BeforeFinalSetRevalidation {
            fs::write(root.join("late-extra.bin"), b"extra").expect("write late extra");
        }
    };
    let mut observer = VerificationObserver::new(&mut callback);
    let error = PackageAttestationService::attest_runtime_with_observer(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
        &mut observer,
    )
    .expect_err("late extra member must fail");
    assert!(matches!(error, PackageAttestationError::ArtifactSet(_)));
}

#[cfg(unix)]
#[test]
fn retained_handle_revalidation_rejects_lifetime_drift() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let mut lease = PackageAttestationService::attest_runtime(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
    )
    .expect("attest stable runtime package");
    fs::write(root.join(RUNTIME_FILES[0].0), b"helper-v2").expect("drift retained member");
    let error = lease
        .revalidate(&CancellationToken::new())
        .expect_err("retained member drift must fail");
    assert!(matches!(
        error,
        PackageAttestationError::MemberBytesConflict
            | PackageAttestationError::MemberIdentityChanged
            | PackageAttestationError::ArtifactSet(_)
    ));
}

#[cfg(windows)]
#[test]
fn retained_windows_handle_denies_lifetime_byte_drift() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let lease = PackageAttestationService::attest_runtime(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
    )
    .expect("attest stable runtime package");
    assert!(fs::write(root.join(RUNTIME_FILES[0].0), b"helper-v2").is_err());
    drop(lease);
}

#[test]
fn non_code_member_drift_is_rejected_on_explicit_revalidation() {
    let (directory, repository, set, package) = runtime_fixture();
    let root = set_root(directory.path(), &set.artifact_set_id());
    let mut lease = PackageAttestationService::attest_runtime(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
    )
    .expect("attest stable runtime package");
    fs::write(root.join(RUNTIME_FILES[2].0), b"{\"build\":2}").expect("drift non-code member");
    let error = lease
        .revalidate(&CancellationToken::new())
        .expect_err("whole package drift must fail");
    assert!(matches!(error, PackageAttestationError::ArtifactSet(_)));
}
