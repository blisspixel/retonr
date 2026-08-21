use rewrite_model::RuntimePackageMemberRole;
use rewrite_types::CancellationToken;

use super::{attest_runtime, runtime_fixture};

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
