use std::io::{Read as _, Seek as _, SeekFrom};

use rewrite_model::RuntimePackageMemberRole;
use rewrite_types::CancellationToken;

use super::{attest_model, attest_runtime, model_fixture, runtime_fixture};

#[test]
fn equal_runtime_installations_have_distinct_live_identity_tokens() {
    let (_first_directory, first_repository, _first_set, first_package) = runtime_fixture();
    let (_second_directory, second_repository, _second_set, second_package) = runtime_fixture();
    let first = attest_runtime(&first_repository, &first_package);
    let second = attest_runtime(&second_repository, &second_package);
    assert_eq!(first.evidence(), second.evidence());
    assert_eq!(
        first.installation_key().installation_generation(),
        second.installation_key().installation_generation()
    );

    let first_token = first.identity_token();
    let cloned_token = first_token.clone();
    assert!(first.binds_identity_token(&first_token));
    assert!(first.binds_identity_token(&cloned_token));
    assert!(!second.binds_identity_token(&first_token));
}

#[test]
fn model_evidence_pins_and_revalidates_the_exact_set() {
    let (_directory, repository, set, package) = model_fixture();
    let lease = attest_model(&repository, &package);
    assert_eq!(lease.evidence().artifact_set_id(), &set.artifact_set_id());
    assert_eq!(
        lease.evidence().model_package_manifest_id(),
        &package.model_package_manifest_id()
    );
    assert_eq!(lease.evidence().member_count(), 6);
    assert_eq!(lease.evidence().byte_size(), set.total_byte_size());
    lease
        .revalidate(&CancellationToken::new())
        .expect("stable model package revalidates");
}

#[test]
fn native_observation_members_are_exact_and_canonical() {
    let (_directory, repository, _set, package) = runtime_fixture();
    let mut lease = attest_runtime(&repository, &package);
    let retained = lease
        .clone_members_for_native_observation(&CancellationToken::new())
        .expect("clone retained native members");
    let expected = package
        .members()
        .iter()
        .filter(|member| {
            member.roles().iter().any(|role| {
                matches!(
                    role,
                    RuntimePackageMemberRole::Entrypoint
                        | RuntimePackageMemberRole::NativeDependency
                        | RuntimePackageMemberRole::HelperExecutable
                        | RuntimePackageMemberRole::WorkerExecutable
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), expected.len());
    for (retained, expected) in retained.iter().zip(expected) {
        assert_eq!(retained.relative_path(), expected.relative_path());
        assert_eq!(retained.artifact_id(), expected.artifact_id());
        assert_eq!(retained.byte_size(), expected.byte_size());
    }
}

#[test]
fn exact_six_member_model_package_is_retained_for_isolation() {
    let (_directory, repository, set, package) = model_fixture();
    let lease = attest_model(&repository, &package);
    let mut source = lease
        .clone_members_for_isolation(&CancellationToken::new())
        .expect("clone exact model members for isolation");

    assert_eq!(source.artifact_set_id(), &set.artifact_set_id());
    assert_eq!(
        source.model_package_manifest_id(),
        &package.model_package_manifest_id()
    );
    assert_eq!(
        source.installation_generation(),
        lease.installation_key().installation_generation()
    );
    assert_eq!(source.member_count(), 6);
    assert_eq!(source.byte_size(), set.total_byte_size());
    assert_eq!(
        source.binding_digest(),
        &super::super::isolation_source_binding_digest(
            &set.artifact_set_id(),
            &package.model_package_manifest_id(),
            lease.installation_key().installation_generation(),
        )
    );
    assert!(!format!("{source:?}").contains("model/model.gguf"));
    source
        .revalidate(&CancellationToken::new())
        .expect("cloned isolation objects revalidate");

    for (retained, expected) in source.members.iter_mut().zip(package.members()) {
        assert_eq!(retained.relative_path, *expected.relative_path());
        assert_eq!(retained.artifact_id, *expected.artifact_id());
        assert_eq!(retained.byte_size, expected.byte_size());
        assert_eq!(retained.roles, expected.roles());
        retained
            .file
            .seek(SeekFrom::Start(0))
            .expect("rewind retained model member");
        let mut bytes = Vec::new();
        retained
            .file
            .read_to_end(&mut bytes)
            .expect("read retained model member");
        assert_eq!(
            rewrite_types::Digest::sha256(&bytes),
            *expected.artifact_id().digest()
        );
    }
}

#[test]
fn cancelled_isolation_clone_fails_before_returning_a_capability() {
    let (_directory, repository, _set, package) = model_fixture();
    let lease = attest_model(&repository, &package);
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = lease
        .clone_members_for_isolation(&cancellation)
        .expect_err("cancelled isolation clone must fail");
    assert!(matches!(error, super::PackageAttestationError::Cancelled));
}

#[test]
fn retained_model_revalidation_is_concurrent_and_cursor_independent() {
    let (_directory, repository, _set, package) = model_fixture();
    let lease = attest_model(&repository, &package);
    let mut source = lease
        .clone_members_for_isolation(&CancellationToken::new())
        .expect("clone exact model members for isolation");
    let initial_offsets = source
        .members
        .iter_mut()
        .map(|member| member.file.stream_position().expect("member offset"))
        .collect::<Vec<_>>();

    std::thread::scope(|scope| {
        let workers = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    for _ in 0..32 {
                        source
                            .revalidate(&CancellationToken::new())
                            .expect("concurrent retained revalidation");
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("revalidation worker");
        }
    });

    let final_offsets = source
        .members
        .iter_mut()
        .map(|member| member.file.stream_position().expect("member offset"))
        .collect::<Vec<_>>();
    assert_eq!(final_offsets, initial_offsets);
}
