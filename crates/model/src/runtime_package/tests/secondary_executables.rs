use super::*;

#[test]
fn secondary_executables_have_explicit_non_entrypoint_policies() {
    let mut runtime_members = members();
    runtime_members.push(RuntimePackageMember::new(
        artifact("worker"),
        15,
        path("lib/worker"),
        vec![RuntimePackageMemberRole::WorkerExecutable],
        RuntimePackageLoadPolicy::BackendConditional,
    ));
    runtime_members.push(RuntimePackageMember::new(
        artifact("utility"),
        16,
        path("lib/worker-tool"),
        vec![RuntimePackageMemberRole::UtilityExecutable],
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
    ));
    let artifact_set = artifact_set_for(&runtime_members);
    let package = RuntimePackageManifest::new(
        &artifact_set,
        "fixture-runtime",
        "1.2.3",
        None,
        target(),
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"same"),
        },
        runtime_members.clone(),
    )
    .expect("worker package");
    assert_ne!(
        package.packaged_dependencies_digest(),
        manifest().packaged_dependencies_digest()
    );
    let encoded = serde_json::to_vec(&package).expect("serialize worker package");
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(&encoded, &artifact_set)
            .expect("decode worker package"),
        package
    );

    for invalid_policy in [
        RuntimePackageLoadPolicy::RequiredAtReady,
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
    ] {
        let mut invalid = runtime_members.clone();
        invalid[5] = RuntimePackageMember::new(
            artifact("worker"),
            15,
            path("lib/worker"),
            vec![RuntimePackageMemberRole::WorkerExecutable],
            invalid_policy,
        );
        assert_eq!(
            RuntimePackageManifest::new(
                &artifact_set,
                "fixture-runtime",
                "1.2.3",
                None,
                target(),
                source(),
                PackageTransformation::Untransformed {
                    evidence_digest: Digest::sha256(b"same"),
                },
                invalid,
            ),
            Err(RuntimePackageManifestError::InvalidLoadPolicy)
        );
    }

    let mut invalid = runtime_members;
    invalid[6] = RuntimePackageMember::new(
        artifact("utility"),
        16,
        path("lib/worker-tool"),
        vec![RuntimePackageMemberRole::UtilityExecutable],
        RuntimePackageLoadPolicy::BackendConditional,
    );
    assert_eq!(
        RuntimePackageManifest::new(
            &artifact_set,
            "fixture-runtime",
            "1.2.3",
            None,
            target(),
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            invalid,
        ),
        Err(RuntimePackageManifestError::InvalidLoadPolicy)
    );
}
