use rewrite_model::ModelPackageManifest;
use rewrite_types::CancellationToken;

use crate::RuntimeArtifactSetLease;

use super::{
    ModelPackageLeaseLimits, PackageAttestationError, RetainedModelMember, ensure_not_cancelled,
    recheck_retained_model_identities, require_stable_handle, revalidate_model_member,
    validate_declared_model_limits,
};

/// Opens and retains every exact model-package member without hashing its bytes.
///
/// This helper exists only for a specialized verifier that immediately consumes
/// every retained stream in one complete semantic reconstruction before it can
/// construct or return any lease. The ordinary generic attestation path remains
/// fully hashing.
pub(in crate::package_attestation) fn retain_model_package_for_full_verification(
    artifact_set: &RuntimeArtifactSetLease,
    package: &ModelPackageManifest,
    limits: ModelPackageLeaseLimits,
    cancellation: &CancellationToken,
) -> Result<Vec<RetainedModelMember>, PackageAttestationError> {
    limits.validate()?;
    ensure_not_cancelled(cancellation)?;
    package
        .validate_against(artifact_set.manifest())
        .map_err(PackageAttestationError::ModelRelationship)?;
    validate_declared_model_limits(package.members(), limits)?;
    artifact_set
        .revalidate_tree_identity(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;

    let mut retained = Vec::with_capacity(package.members().len());
    for member in package.members() {
        ensure_not_cancelled(cancellation)?;
        let opened = artifact_set
            .open_member(member.relative_path())
            .map_err(PackageAttestationError::from_set_lease)?;
        if opened.byte_size != member.byte_size() || !opened.fingerprint.has_single_link() {
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
    recheck_retained_model_identities(artifact_set, &retained)?;
    ensure_not_cancelled(cancellation)?;
    Ok(retained)
}

pub(in crate::package_attestation) fn recheck_retained_model_package_identities(
    artifact_set: &RuntimeArtifactSetLease,
    retained: &[RetainedModelMember],
    cancellation: &CancellationToken,
) -> Result<(), PackageAttestationError> {
    ensure_not_cancelled(cancellation)?;
    artifact_set
        .revalidate_tree_identity(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    recheck_retained_model_identities(artifact_set, retained)?;
    ensure_not_cancelled(cancellation)
}

/// Rehashes each retained model member exactly once under metadata-only tree checks.
///
/// This is reserved for the specialized foundation lease, whose initial full
/// reconstruction already established the exact semantic content boundary.
pub(in crate::package_attestation) fn revalidate_verified_model_members_once(
    artifact_set: &RuntimeArtifactSetLease,
    retained: &[RetainedModelMember],
    cancellation: &CancellationToken,
) -> Result<(), PackageAttestationError> {
    artifact_set
        .revalidate_tree_identity(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    for member in retained {
        revalidate_model_member(member, cancellation)?;
        artifact_set
            .recheck_member(&member.relative_path, &member.fingerprint)
            .map_err(|_| PackageAttestationError::MemberIdentityChanged)?;
    }
    artifact_set
        .revalidate_tree_identity(cancellation)
        .map_err(PackageAttestationError::from_set_lease)?;
    recheck_retained_model_identities(artifact_set, retained)?;
    ensure_not_cancelled(cancellation)
}
