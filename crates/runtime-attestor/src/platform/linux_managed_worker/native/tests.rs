use std::{fs::File, io::Write as _};

use rewrite_model::{ArtifactId, NativeMappingClass};
use rewrite_types::Digest;

use super::{
    ExecutableObject, NativePolicy, PackageCodeClass, PackageCodeObject, native_digest,
    portable_native_closure_digest, validate_complete_closure,
};
use crate::platform::linux_managed_worker::process::{ObjectIdentity, ObjectKey, object_identity};
use crate::{
    ExpectedExternalNativeComponent, ManagedGenerationWorkerError, ManagedGenerationWorkerProfile,
};

#[test]
fn complete_closure_requires_one_worker_and_every_required_dependency() {
    let (worker_file, worker_identity) = object(b"worker");
    let (dependency_file, dependency_identity) = object(b"dependency");
    let worker_artifact = artifact(b"worker");
    let dependency_artifact = artifact(b"dependency");
    let policy = NativePolicy {
        package: vec![
            PackageCodeObject {
                identity: worker_identity,
                artifact_id: worker_artifact.clone(),
                class: PackageCodeClass::Worker,
                file: worker_file,
            },
            PackageCodeObject {
                identity: dependency_identity,
                artifact_id: dependency_artifact.clone(),
                class: PackageCodeClass::Dependency { required: true },
                file: dependency_file,
            },
        ],
        external: Vec::new(),
        forbidden_artifacts: Vec::new(),
    };
    assert_eq!(
        validate_complete_closure(&[], &policy),
        Err(ManagedGenerationWorkerError::NativeClosureMismatch)
    );
    let worker = executable(
        worker_identity,
        worker_artifact,
        Some(PackageCodeClass::Worker),
    );
    assert_eq!(
        validate_complete_closure(std::slice::from_ref(&worker), &policy),
        Err(ManagedGenerationWorkerError::NativeClosureMismatch)
    );
    let dependency = executable(
        dependency_identity,
        dependency_artifact,
        Some(PackageCodeClass::Dependency { required: true }),
    );
    assert_eq!(
        validate_complete_closure(&[worker, dependency], &policy),
        Ok(())
    );
}

#[test]
fn complete_closure_matches_external_objects_canonically() {
    let (worker_file, worker_identity) = object(b"worker-external-case");
    let (_, external_identity) = object(b"external");
    let worker_artifact = artifact(b"worker-external-case");
    let external_artifact = artifact(b"external");
    let mut policy = NativePolicy {
        package: vec![PackageCodeObject {
            identity: worker_identity,
            artifact_id: worker_artifact.clone(),
            class: PackageCodeClass::Worker,
            file: worker_file,
        }],
        external: vec![ExpectedExternalNativeComponent::new(
            external_artifact.clone(),
            external_identity.bytes,
            NativeMappingClass::ExecutableMapped,
        )],
        forbidden_artifacts: Vec::new(),
    };
    let components = [
        executable(
            worker_identity,
            worker_artifact,
            Some(PackageCodeClass::Worker),
        ),
        executable(external_identity, external_artifact, None),
    ];
    assert_eq!(validate_complete_closure(&components, &policy), Ok(()));
    policy.external[0] = ExpectedExternalNativeComponent::new(
        artifact(b"drift"),
        external_identity.bytes,
        NativeMappingClass::ExecutableMapped,
    );
    assert_eq!(
        validate_complete_closure(&components, &policy),
        Err(ManagedGenerationWorkerError::NativeClosureMismatch)
    );
}

#[test]
fn portable_closure_excludes_device_and_inode_but_binds_normalized_component_facts() {
    let (_, identity) = object(b"portable-worker");
    let base_artifact = artifact(b"portable-worker");
    let base = executable(
        identity,
        base_artifact.clone(),
        Some(PackageCodeClass::Worker),
    );
    let relocated = executable(
        ObjectIdentity {
            key: ObjectKey {
                device: identity.key.device + 1,
                inode: identity.key.inode + 1,
            },
            bytes: identity.bytes,
        },
        base_artifact.clone(),
        Some(PackageCodeClass::Worker),
    );
    let profile = ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu;
    assert_eq!(
        portable_native_closure_digest(profile, std::slice::from_ref(&base)),
        portable_native_closure_digest(profile, std::slice::from_ref(&relocated))
    );
    assert_ne!(
        native_digest(profile, std::slice::from_ref(&base)),
        native_digest(profile, std::slice::from_ref(&relocated))
    );

    let changed_size = executable(
        ObjectIdentity {
            key: identity.key,
            bytes: identity.bytes + 1,
        },
        base_artifact.clone(),
        Some(PackageCodeClass::Worker),
    );
    let changed_artifact = executable(
        identity,
        artifact(b"other portable worker"),
        Some(PackageCodeClass::Worker),
    );
    let changed_class = executable(
        identity,
        base_artifact,
        Some(PackageCodeClass::Dependency { required: true }),
    );
    for changed in [changed_size, changed_artifact, changed_class] {
        assert_ne!(
            portable_native_closure_digest(profile, std::slice::from_ref(&base)),
            portable_native_closure_digest(profile, std::slice::from_ref(&changed))
        );
    }
}

fn object(bytes: &[u8]) -> (File, ObjectIdentity) {
    let mut file = tempfile::tempfile().expect("temporary native object");
    file.write_all(bytes).expect("write native object");
    let identity = object_identity(&file).expect("native object identity");
    (file, identity)
}

fn executable(
    identity: ObjectIdentity,
    artifact_id: ArtifactId,
    package_class: Option<PackageCodeClass>,
) -> ExecutableObject {
    ExecutableObject {
        identity,
        artifact_id,
        package_class,
    }
}

fn artifact(bytes: &[u8]) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(bytes))
}
