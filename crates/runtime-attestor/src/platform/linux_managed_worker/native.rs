use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    time::Instant,
};

use rewrite_model::{ArtifactId, RuntimePackageLoadPolicy, RuntimePackageMemberRole};
use rewrite_types::{CancellationToken, Digest};
use rustix::fd::OwnedFd;

use super::{
    super::linux::ensure_pidfd_alive,
    process::{ObjectIdentity, object_identity},
};
use crate::managed_worker::ensure_worker_active;
use crate::{
    ExpectedExternalNativeComponent, ManagedGenerationWorkerError, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerProfile,
};

mod file_hash;
mod maps;
mod model_mapping;
use file_hash::hash_file;
use maps::{accelerator_path, open_map_file, read_mappings};
pub(super) use model_mapping::{
    ModelMappingBaseline, establish_model_mapping, reobserve_model_mapping,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PackageCodeClass {
    Worker,
    Dependency { required: bool },
    ForbiddenServerCode,
}

struct PackageCodeObject {
    identity: ObjectIdentity,
    artifact_id: ArtifactId,
    class: PackageCodeClass,
    file: File,
}

pub(super) struct NativePolicy {
    package: Vec<PackageCodeObject>,
    external: Vec<ExpectedExternalNativeComponent>,
    forbidden_artifacts: Vec<ArtifactId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableObject {
    identity: ObjectIdentity,
    artifact_id: ArtifactId,
    package_class: Option<PackageCodeClass>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeSnapshot {
    components: Vec<ExecutableObject>,
    pub(super) component_count: usize,
    pub(super) portable_closure_digest: Digest,
    pub(super) digest: Digest,
}

pub(super) fn build_native_policy(
    request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
    _profile: ManagedGenerationWorkerProfile,
    expected_worker: &File,
    worker_artifact_id: &ArtifactId,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<NativePolicy, ManagedGenerationWorkerError> {
    let declared = request.package.members().iter().filter(|member| {
        member.roles().iter().any(|role| {
            matches!(
                role,
                RuntimePackageMemberRole::Entrypoint
                    | RuntimePackageMemberRole::NativeDependency
                    | RuntimePackageMemberRole::HelperExecutable
                    | RuntimePackageMemberRole::WorkerExecutable
            )
        })
    });
    let mut remaining_hash_bytes = limits.native_load.maximum_aggregate_hash_bytes;
    let mut package = Vec::new();
    let mut identities = BTreeSet::new();
    for (member, retained) in declared.zip(request.retained_package_code) {
        ensure_worker_active(cancellation, started, limits)?;
        let file = retained
            .file()
            .try_clone()
            .map_err(|_error| ManagedGenerationWorkerError::InvalidNativeLoadRequest)?;
        let before = object_identity(&file)?;
        let artifact_id = hash_file(
            &file,
            member.byte_size(),
            &mut remaining_hash_bytes,
            limits,
            cancellation,
            started,
        )?;
        if before != object_identity(&file)?
            || artifact_id != *member.artifact_id()
            || !identities.insert(before.key)
        {
            return Err(ManagedGenerationWorkerError::InvalidNativeLoadRequest);
        }
        let class = if member.roles() == [RuntimePackageMemberRole::WorkerExecutable] {
            if member.artifact_id() != worker_artifact_id
                || before != object_identity(expected_worker)?
            {
                return Err(ManagedGenerationWorkerError::ExecutableMismatch);
            }
            PackageCodeClass::Worker
        } else if member
            .roles()
            .contains(&RuntimePackageMemberRole::NativeDependency)
        {
            PackageCodeClass::Dependency {
                required: member.load_policy() == RuntimePackageLoadPolicy::RequiredAtReady,
            }
        } else {
            PackageCodeClass::ForbiddenServerCode
        };
        package.push(PackageCodeObject {
            identity: before,
            artifact_id,
            class,
            file,
        });
    }
    Ok(NativePolicy {
        package,
        external: request.expected_external_components.to_vec(),
        forbidden_artifacts: request
            .package
            .members()
            .iter()
            .filter(|member| {
                member
                    .roles()
                    .contains(&RuntimePackageMemberRole::UtilityExecutable)
            })
            .map(|member| member.artifact_id().clone())
            .collect(),
    })
}

pub(super) fn native_snapshot(
    pid: u32,
    pidfd: &OwnedFd,
    profile: ManagedGenerationWorkerProfile,
    policy: &NativePolicy,
    limits: ManagedGenerationWorkerLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<NativeSnapshot, ManagedGenerationWorkerError> {
    ensure_alive(pidfd)?;
    validate_retained_package(policy)?;
    let mappings = read_mappings(pid, limits, cancellation, started)?;
    let mut unique = BTreeMap::new();
    for mapping in mappings.iter().filter(|mapping| mapping.executable) {
        if unique.len() >= limits.native_load.maximum_components {
            return Err(ManagedGenerationWorkerError::ResourceLimit);
        }
        unique.entry(mapping.key).or_insert(mapping);
    }
    let mut remaining_hash_bytes = limits.native_load.maximum_aggregate_hash_bytes;
    let mut components = Vec::new();
    for mapping in unique.values() {
        ensure_worker_active(cancellation, started, limits)?;
        if accelerator_path(&mapping.path) {
            return Err(ManagedGenerationWorkerError::AcceleratorLibraryMapped);
        }
        let file = open_map_file(pid, mapping)?;
        let identity = object_identity(&file)?;
        if identity.key != mapping.key {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        let artifact_id = hash_file(
            &file,
            identity.bytes,
            &mut remaining_hash_bytes,
            limits,
            cancellation,
            started,
        )?;
        let package = policy
            .package
            .iter()
            .find(|object| object.identity == identity);
        let package_class = package.map(|object| object.class);
        match package_class {
            Some(PackageCodeClass::ForbiddenServerCode)
                if !profile.admits_server_code_mapping() =>
            {
                return Err(ManagedGenerationWorkerError::ForbiddenServerCodeMapped);
            }
            Some(_) => {
                if package.is_none_or(|object| object.artifact_id != artifact_id) {
                    return Err(ManagedGenerationWorkerError::NativeClosureMismatch);
                }
            }
            None => {
                if policy
                    .forbidden_artifacts
                    .iter()
                    .any(|forbidden| forbidden == &artifact_id)
                {
                    return Err(ManagedGenerationWorkerError::ForbiddenServerCodeMapped);
                }
                if !policy.external.iter().any(|expected| {
                    expected.artifact_id() == &artifact_id && expected.byte_size() == identity.bytes
                }) {
                    return Err(ManagedGenerationWorkerError::NativeClosureMismatch);
                }
            }
        }
        components.push(ExecutableObject {
            identity,
            artifact_id,
            package_class,
        });
    }
    components.sort_by_key(|component| {
        (
            component.identity.key,
            component.artifact_id.digest().as_str().to_owned(),
        )
    });
    validate_complete_closure(&components, policy)?;
    let portable_closure_digest = portable_native_closure_digest(profile, &components);
    let digest = native_digest(profile, &components);
    ensure_alive(pidfd)?;
    Ok(NativeSnapshot {
        component_count: components.len(),
        components,
        portable_closure_digest,
        digest,
    })
}

fn validate_complete_closure(
    components: &[ExecutableObject],
    policy: &NativePolicy,
) -> Result<(), ManagedGenerationWorkerError> {
    let workers = components
        .iter()
        .filter(|component| component.package_class == Some(PackageCodeClass::Worker))
        .count();
    if workers != 1 {
        return Err(ManagedGenerationWorkerError::NativeClosureMismatch);
    }
    for required in policy.package.iter().filter(|object| {
        matches!(
            object.class,
            PackageCodeClass::Dependency { required: true }
        )
    }) {
        if !components
            .iter()
            .any(|component| component.identity == required.identity)
        {
            return Err(ManagedGenerationWorkerError::NativeClosureMismatch);
        }
    }
    let mut observed_external = components
        .iter()
        .filter(|component| component.package_class.is_none())
        .map(|component| {
            (
                component.artifact_id.digest().as_str(),
                component.identity.bytes,
            )
        })
        .collect::<Vec<_>>();
    observed_external.sort_unstable();
    let mut expected_external = policy
        .external
        .iter()
        .map(|component| {
            (
                component.artifact_id().digest().as_str(),
                component.byte_size(),
            )
        })
        .collect::<Vec<_>>();
    expected_external.sort_unstable();
    if observed_external != expected_external {
        return Err(ManagedGenerationWorkerError::NativeClosureMismatch);
    }
    Ok(())
}

fn validate_retained_package(policy: &NativePolicy) -> Result<(), ManagedGenerationWorkerError> {
    for object in &policy.package {
        if object_identity(&object.file)? != object.identity {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
    }
    Ok(())
}

fn native_digest(
    profile: ManagedGenerationWorkerProfile,
    components: &[ExecutableObject],
) -> Digest {
    let mut material = format!("managed-worker-native-closure-v1\0{profile:?}").into_bytes();
    for component in components {
        material.extend_from_slice(
            format!(
                "\0{}\0{}\0{}\0{}",
                component.identity.key.device,
                component.identity.key.inode,
                component.identity.bytes,
                component.artifact_id.digest().as_str()
            )
            .as_bytes(),
        );
    }
    Digest::sha256(&material)
}

fn portable_native_closure_digest(
    profile: ManagedGenerationWorkerProfile,
    components: &[ExecutableObject],
) -> Digest {
    let mut canonical = components
        .iter()
        .map(|component| {
            let (origin, package_class, required) = package_class_code(component.package_class);
            (
                component.artifact_id.digest().as_str().to_owned(),
                component.identity.bytes,
                origin,
                package_class,
                required,
            )
        })
        .collect::<Vec<_>>();
    canonical.sort_unstable();

    let mut material = Vec::with_capacity(128 + canonical.len() * 96);
    push_portable_field(
        &mut material,
        b"retonr:managed-worker-portable-native-closure:v1",
    );
    push_portable_field(&mut material, profile.command_contract_id().as_bytes());
    material.extend_from_slice(&(canonical.len() as u64).to_be_bytes());
    for (artifact, byte_size, origin, package_class, required) in canonical {
        push_portable_field(&mut material, artifact.as_bytes());
        material.extend_from_slice(&byte_size.to_be_bytes());
        material.push(origin);
        material.push(package_class);
        material.push(required);
    }
    Digest::sha256(&material)
}

const fn package_class_code(package_class: Option<PackageCodeClass>) -> (u8, u8, u8) {
    match package_class {
        Some(PackageCodeClass::Worker) => (0, 0, 0),
        Some(PackageCodeClass::Dependency { required }) => (0, 1, required as u8),
        Some(PackageCodeClass::ForbiddenServerCode) => (0, 2, 0),
        None => (1, 3, 0),
    }
}

fn push_portable_field(material: &mut Vec<u8>, value: &[u8]) {
    material.extend_from_slice(&(value.len() as u64).to_be_bytes());
    material.extend_from_slice(value);
}

fn ensure_alive(pidfd: &OwnedFd) -> Result<(), ManagedGenerationWorkerError> {
    ensure_pidfd_alive(pidfd).map_err(|_error| ManagedGenerationWorkerError::WorkerChanged)
}

#[cfg(test)]
mod tests;
