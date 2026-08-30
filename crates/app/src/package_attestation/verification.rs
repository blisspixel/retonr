use std::{
    fs::File,
    io::{Read as _, Seek as _, SeekFrom},
};

use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, ModelPackageManifest, ModelPackageMemberRole,
    RuntimePackageManifest,
};
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::{
    RuntimeArtifactSetLease,
    artifact_storage::{MetadataFingerprint, fingerprint_std_file},
};

use super::{
    ModelPackageLeaseLimits, PackageAttestationError, RuntimePackageLeaseLimits,
    ensure_not_cancelled, is_packaged_code,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[cfg(target_os = "linux")]
mod executable_snapshot;
mod specialized_model;

pub(super) use specialized_model::{
    recheck_retained_model_package_identities, retain_model_package_for_full_verification,
    revalidate_verified_model_members_once,
};

pub(super) struct RetainedCodeMember {
    file: File,
    fingerprint: MetadataFingerprint,
    artifact_id: ArtifactId,
    relative_path: ArtifactSetRelativePath,
    pub(super) entrypoint: bool,
    pub(super) byte_size: u64,
}

pub(super) struct RetainedModelMember {
    pub(super) file: File,
    pub(super) fingerprint: MetadataFingerprint,
    pub(super) artifact_id: ArtifactId,
    pub(super) relative_path: ArtifactSetRelativePath,
    pub(super) byte_size: u64,
    pub(super) roles: Vec<ModelPackageMemberRole>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VerificationStage {
    AfterInitialSetRevalidation,
    BeforeMemberOpen(usize),
    AfterMemberHash(usize),
    BeforeFinalSetRevalidation,
}

pub(super) struct VerificationObserver<'a> {
    callback: Option<&'a mut dyn FnMut(VerificationStage)>,
}

impl VerificationObserver<'_> {
    pub(super) const fn none() -> Self {
        Self { callback: None }
    }

    #[cfg(test)]
    pub(super) fn new(callback: &mut dyn FnMut(VerificationStage)) -> VerificationObserver<'_> {
        VerificationObserver {
            callback: Some(callback),
        }
    }

    fn notify(&mut self, stage: VerificationStage) {
        if let Some(callback) = self.callback.as_deref_mut() {
            callback(stage);
        }
    }
}

pub(super) fn attest_runtime_package(
    artifact_set: &RuntimeArtifactSetLease,
    package: &RuntimePackageManifest,
    limits: RuntimePackageLeaseLimits,
    cancellation: &CancellationToken,
    observer: &mut VerificationObserver<'_>,
) -> Result<Vec<RetainedCodeMember>, PackageAttestationError> {
    limits.validate()?;
    ensure_not_cancelled(cancellation)?;
    package
        .validate_against(artifact_set.manifest())
        .map_err(PackageAttestationError::RuntimeRelationship)?;
    let code_members = package
        .members()
        .iter()
        .filter(|member| is_packaged_code(member.roles()))
        .collect::<Vec<_>>();
    validate_declared_limits(&code_members, limits)?;

    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    observer.notify(VerificationStage::AfterInitialSetRevalidation);

    let mut retained = Vec::with_capacity(code_members.len());
    for (index, member) in code_members.into_iter().enumerate() {
        ensure_not_cancelled(cancellation)?;
        observer.notify(VerificationStage::BeforeMemberOpen(index));
        let mut opened = artifact_set
            .open_member(member.relative_path())
            .map_err(PackageAttestationError::from_set_lease)?;
        if opened.byte_size != member.byte_size() || !opened.fingerprint.has_single_link() {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        let member_digest = hash_exact(&mut opened.file, member.byte_size(), cancellation)?;
        observer.notify(VerificationStage::AfterMemberHash(index));
        if member_digest != *member.artifact_id().digest() {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        require_stable_handle(&opened.file, &opened.fingerprint)?;
        artifact_set
            .recheck_member(member.relative_path(), &opened.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
        retained.push(RetainedCodeMember {
            file: opened.file,
            fingerprint: opened.fingerprint,
            artifact_id: member.artifact_id().clone(),
            relative_path: member.relative_path().clone(),
            entrypoint: member
                .roles()
                .contains(&rewrite_model::RuntimePackageMemberRole::Entrypoint),
            byte_size: member.byte_size(),
        });
    }

    observer.notify(VerificationStage::BeforeFinalSetRevalidation);
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    recheck_retained_identities(artifact_set, &retained)?;
    Ok(retained)
}

pub(super) fn attest_model_package(
    artifact_set: &RuntimeArtifactSetLease,
    package: &ModelPackageManifest,
    limits: ModelPackageLeaseLimits,
    cancellation: &CancellationToken,
    observer: &mut VerificationObserver<'_>,
) -> Result<Vec<RetainedModelMember>, PackageAttestationError> {
    limits.validate()?;
    ensure_not_cancelled(cancellation)?;
    package
        .validate_against(artifact_set.manifest())
        .map_err(PackageAttestationError::ModelRelationship)?;
    validate_declared_model_limits(package.members(), limits)?;
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    observer.notify(VerificationStage::AfterInitialSetRevalidation);

    let mut retained = Vec::with_capacity(package.members().len());
    for (index, member) in package.members().iter().enumerate() {
        ensure_not_cancelled(cancellation)?;
        observer.notify(VerificationStage::BeforeMemberOpen(index));
        let mut opened = artifact_set
            .open_member(member.relative_path())
            .map_err(PackageAttestationError::from_set_lease)?;
        if opened.byte_size != member.byte_size() || !opened.fingerprint.has_single_link() {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        let member_digest = hash_exact(&mut opened.file, member.byte_size(), cancellation)?;
        observer.notify(VerificationStage::AfterMemberHash(index));
        if member_digest != *member.artifact_id().digest() {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        require_stable_handle(&opened.file, &opened.fingerprint)?;
        artifact_set
            .recheck_member(member.relative_path(), &opened.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
        retained.push(RetainedModelMember {
            file: opened.file,
            fingerprint: opened.fingerprint,
            artifact_id: member.artifact_id().clone(),
            relative_path: member.relative_path().clone(),
            byte_size: member.byte_size(),
            roles: member.roles().to_vec(),
        });
    }

    observer.notify(VerificationStage::BeforeFinalSetRevalidation);
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    recheck_retained_model_identities(artifact_set, &retained)?;
    Ok(retained)
}

pub(super) fn clone_retained_entrypoint(
    retained: &[RetainedCodeMember],
    cancellation: &CancellationToken,
) -> Result<File, PackageAttestationError> {
    ensure_not_cancelled(cancellation)?;
    let mut entrypoints = retained.iter().filter(|member| member.entrypoint);
    let entrypoint = entrypoints
        .next()
        .ok_or(PackageAttestationError::MemberIdentityChanged)?;
    if entrypoints.next().is_some() {
        return Err(PackageAttestationError::MemberIdentityChanged);
    }
    require_stable_handle(&entrypoint.file, &entrypoint.fingerprint)?;
    #[cfg(target_os = "linux")]
    let cloned = executable_snapshot::snapshot(
        &entrypoint.file,
        &entrypoint.fingerprint,
        &entrypoint.artifact_id,
        entrypoint.byte_size,
        cancellation,
    )?;
    #[cfg(not(target_os = "linux"))]
    let cloned = entrypoint
        .file
        .try_clone()
        .map_err(PackageAttestationError::MemberIo)?;
    #[cfg(not(target_os = "linux"))]
    require_stable_handle(&cloned, &entrypoint.fingerprint)?;
    Ok(cloned)
}

pub(super) fn clone_retained_native_members(
    retained: &[RetainedCodeMember],
) -> Result<Vec<rewrite_runtime_attestor::RetainedNativePackageMember>, PackageAttestationError> {
    let mut cloned = Vec::with_capacity(retained.len());
    for member in retained {
        require_stable_handle(&member.file, &member.fingerprint)?;
        let file = member
            .file
            .try_clone()
            .map_err(PackageAttestationError::MemberIo)?;
        require_stable_handle(&file, &member.fingerprint)?;
        cloned.push(
            rewrite_runtime_attestor::RetainedNativePackageMember::new(
                member.relative_path.clone(),
                member.artifact_id.clone(),
                member.byte_size,
                file,
            )
            .map_err(|_error| PackageAttestationError::MemberIdentityChanged)?,
        );
    }
    Ok(cloned)
}

pub(super) fn clone_retained_model_members(
    retained: &[RetainedModelMember],
) -> Result<Vec<RetainedModelMember>, PackageAttestationError> {
    let mut cloned = Vec::with_capacity(retained.len());
    for member in retained {
        require_stable_handle(&member.file, &member.fingerprint)?;
        let file = member
            .file
            .try_clone()
            .map_err(PackageAttestationError::MemberIo)?;
        require_stable_handle(&file, &member.fingerprint)?;
        let fingerprint = fingerprint_std_file(&file).map_err(map_fingerprint_error)?;
        cloned.push(RetainedModelMember {
            file,
            fingerprint,
            artifact_id: member.artifact_id.clone(),
            relative_path: member.relative_path.clone(),
            byte_size: member.byte_size,
            roles: member.roles.clone(),
        });
    }
    Ok(cloned)
}

pub(super) fn revalidate_retained_package(
    artifact_set: &RuntimeArtifactSetLease,
    retained: &mut [RetainedCodeMember],
    limits: RuntimePackageLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<(), PackageAttestationError> {
    limits.validate()?;
    ensure_not_cancelled(cancellation)?;
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    for member in retained.iter_mut() {
        ensure_not_cancelled(cancellation)?;
        require_stable_handle(&member.file, &member.fingerprint)?;
        let observed = hash_exact(&mut member.file, member.byte_size, cancellation)?;
        if observed != *member.artifact_id.digest() {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        require_stable_handle(&member.file, &member.fingerprint)?;
        artifact_set
            .recheck_member(&member.relative_path, &member.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
    }
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)
}

pub(super) fn revalidate_retained_model_members(
    artifact_set: &RuntimeArtifactSetLease,
    retained: &[RetainedModelMember],
    cancellation: &CancellationToken,
) -> Result<(), PackageAttestationError> {
    ensure_not_cancelled(cancellation)?;
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    for member in retained {
        revalidate_model_member(member, cancellation)?;
        artifact_set
            .recheck_member(&member.relative_path, &member.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
    }
    artifact_set
        .revalidate(cancellation)
        .map_err(PackageAttestationError::from_set_lease)
}

pub(super) fn revalidate_detached_model_members(
    retained: &[RetainedModelMember],
    cancellation: &CancellationToken,
) -> Result<(), PackageAttestationError> {
    ensure_not_cancelled(cancellation)?;
    for member in retained {
        revalidate_model_member(member, cancellation)?;
    }
    Ok(())
}

fn validate_declared_limits(
    members: &[&rewrite_model::RuntimePackageMember],
    limits: RuntimePackageLeaseLimits,
) -> Result<(), PackageAttestationError> {
    if members.len() > limits.maximum_code_members {
        return Err(PackageAttestationError::TooManyCodeMembers {
            actual: members.len(),
            maximum: limits.maximum_code_members,
        });
    }
    let mut total = 0u64;
    for member in members {
        if member.byte_size() > limits.maximum_code_member_bytes {
            return Err(PackageAttestationError::CodeMemberTooLarge {
                actual: member.byte_size(),
                maximum: limits.maximum_code_member_bytes,
            });
        }
        total = total
            .checked_add(member.byte_size())
            .ok_or(PackageAttestationError::InvalidLimits)?;
    }
    if total > limits.maximum_code_bytes {
        Err(PackageAttestationError::CodeBytesTooLarge {
            actual: total,
            maximum: limits.maximum_code_bytes,
        })
    } else {
        Ok(())
    }
}

fn validate_declared_model_limits(
    members: &[rewrite_model::ModelPackageMember],
    limits: ModelPackageLeaseLimits,
) -> Result<(), PackageAttestationError> {
    if members.len() > limits.maximum_members {
        return Err(PackageAttestationError::TooManyModelMembers {
            actual: members.len(),
            maximum: limits.maximum_members,
        });
    }
    let mut total = 0u64;
    for member in members {
        if member.byte_size() > limits.maximum_member_bytes {
            return Err(PackageAttestationError::ModelMemberTooLarge {
                actual: member.byte_size(),
                maximum: limits.maximum_member_bytes,
            });
        }
        total = total
            .checked_add(member.byte_size())
            .ok_or(PackageAttestationError::InvalidModelLimits)?;
    }
    if total > limits.maximum_bytes {
        Err(PackageAttestationError::ModelBytesTooLarge {
            actual: total,
            maximum: limits.maximum_bytes,
        })
    } else {
        Ok(())
    }
}

fn recheck_retained_identities(
    artifact_set: &RuntimeArtifactSetLease,
    retained: &[RetainedCodeMember],
) -> Result<(), PackageAttestationError> {
    for member in retained {
        require_stable_handle(&member.file, &member.fingerprint)?;
        artifact_set
            .recheck_member(&member.relative_path, &member.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
    }
    Ok(())
}

fn recheck_retained_model_identities(
    artifact_set: &RuntimeArtifactSetLease,
    retained: &[RetainedModelMember],
) -> Result<(), PackageAttestationError> {
    for member in retained {
        require_stable_handle(&member.file, &member.fingerprint)?;
        artifact_set
            .recheck_member(&member.relative_path, &member.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
    }
    Ok(())
}

fn revalidate_model_member(
    member: &RetainedModelMember,
    cancellation: &CancellationToken,
) -> Result<(), PackageAttestationError> {
    ensure_not_cancelled(cancellation)?;
    require_stable_handle(&member.file, &member.fingerprint)?;
    let observed = hash_exact_positioned(&member.file, member.byte_size, cancellation)?;
    if observed != *member.artifact_id.digest() {
        return Err(PackageAttestationError::MemberBytesConflict);
    }
    require_stable_handle(&member.file, &member.fingerprint)
}

fn require_stable_handle(
    file: &File,
    expected: &MetadataFingerprint,
) -> Result<(), PackageAttestationError> {
    let observed = fingerprint_std_file(file).map_err(|error| match error {
        crate::ArtifactInventoryError::StorageIo(error) => PackageAttestationError::MemberIo(error),
        _ => PackageAttestationError::MemberIdentityChanged,
    })?;
    if &observed == expected && observed.has_single_link() {
        Ok(())
    } else {
        Err(PackageAttestationError::MemberIdentityChanged)
    }
}

fn map_fingerprint_error(error: crate::ArtifactInventoryError) -> PackageAttestationError {
    match error {
        crate::ArtifactInventoryError::StorageIo(error) => PackageAttestationError::MemberIo(error),
        _ => PackageAttestationError::MemberIdentityChanged,
    }
}

fn hash_exact(
    file: &mut File,
    expected_size: u64,
    cancellation: &CancellationToken,
) -> Result<Digest, PackageAttestationError> {
    file.seek(SeekFrom::Start(0))
        .map_err(PackageAttestationError::MemberIo)?;
    hash_exact_with(expected_size, cancellation, |buffer, _offset| {
        file.read(buffer)
    })
}

fn hash_exact_positioned(
    file: &File,
    expected_size: u64,
    cancellation: &CancellationToken,
) -> Result<Digest, PackageAttestationError> {
    hash_exact_with(expected_size, cancellation, |buffer, offset| {
        read_positioned(file, buffer, offset)
    })
}

fn hash_exact_with(
    expected_size: u64,
    cancellation: &CancellationToken,
    mut read_next: impl FnMut(&mut [u8], u64) -> std::io::Result<usize>,
) -> Result<Digest, PackageAttestationError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; HASH_BUFFER_BYTES];
    let mut observed = 0u64;
    while observed < expected_size {
        ensure_not_cancelled(cancellation)?;
        let maximum = usize::try_from(expected_size - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let count = read_next(&mut buffer[..maximum], observed)
            .map_err(PackageAttestationError::MemberIo)?;
        if count == 0 {
            return Err(PackageAttestationError::MemberBytesConflict);
        }
        observed = observed
            .checked_add(
                u64::try_from(count).map_err(|_| PackageAttestationError::MemberBytesConflict)?,
            )
            .ok_or(PackageAttestationError::MemberBytesConflict)?;
        hasher.update(&buffer[..count]);
    }
    ensure_not_cancelled(cancellation)?;
    let mut trailing = [0u8; 1];
    if read_next(&mut trailing, observed).map_err(PackageAttestationError::MemberIo)? != 0 {
        return Err(PackageAttestationError::MemberBytesConflict);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| PackageAttestationError::MemberBytesConflict)
}

#[cfg(unix)]
fn read_positioned(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt as _;

    file.read_at(buffer, offset)
}

#[cfg(windows)]
fn read_positioned(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt as _;

    file.seek_read(buffer, offset)
}
