use rewrite_model::{ArtifactId, ModelPackageMemberRole, PackageSourceKind};
use rewrite_ollama_package::OllamaLocalArchiveMemberBinding;
use rewrite_types::{CancellationToken, Digest};

use super::wire::{
    AuthorityWire, ControlEvidenceWire, ControlKindWire, ControlStatusWire, ControlWire,
    FoundationWire, LicenseMemberWire, ReviewDecisionWire, ReviewWire, SourceWire, encode_control,
    parse_control, parse_review,
};
use super::{
    CompiledModelLicenseControl, MODEL_LICENSE_CONTROL_PROCEDURE_ID,
    MODEL_LICENSE_CONTROL_PROCEDURE_VERSION, MODEL_LICENSE_CONTROL_SCHEMA_VERSION,
    MODEL_LICENSE_REVIEW_PROCEDURE_ID, MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
    MODEL_LICENSE_REVIEW_SCHEMA_VERSION, ModelLicenseControlAssessmentDisposition,
    ModelLicenseControlError, ModelLicensePermission, ProductionModelLicenseApprovalPolicy,
    VerifiedApprovedModelLicenseControl, VerifiedModelLicenseControl, control_id,
};
use crate::{ModelPackageFoundationId, VerifiedManagedOllamaModelPackageLease};

const REVIEW_EVIDENCE_DOMAIN: &[u8] = b"retonr:model-license-review-evidence:v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
struct FoundationSnapshot {
    installation_generation: u64,
    foundation_id: ModelPackageFoundationId,
    artifact_set_id: Digest,
    model_package_manifest_id: Digest,
    package_source_id: Digest,
    descriptor_mapping_digest: Digest,
    logical_binding_digest: Digest,
    provenance_manifest: MemberSnapshot,
    source: SourceSnapshot,
    license_members: Vec<MemberSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceSnapshot {
    kind: PackageSourceKind,
    locator: String,
    revision: String,
    provenance_digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MemberSnapshot {
    relative_path: String,
    artifact_id: ArtifactId,
    byte_size: u64,
}

pub(super) fn compile(
    lease: &VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
    reviewer_json: &[u8],
    cancellation: &CancellationToken,
) -> Result<CompiledModelLicenseControl, ModelLicenseControlError> {
    ensure_not_cancelled(cancellation)?;
    let review = parse_review(reviewer_json)?;
    if review.permission != permission {
        return Err(ModelLicenseControlError::PermissionMismatch);
    }
    let snapshot = begin(lease, cancellation)?;
    let (wire, review_evidence_digest) =
        derive_control(&snapshot, permission, review, reviewer_json)?;
    let canonical_bytes = encode_control(&wire)?;
    finish(lease, &snapshot, cancellation)?;
    Ok(CompiledModelLicenseControl {
        control_id: control_id(&canonical_bytes),
        canonical_bytes,
        foundation_id: snapshot.foundation_id,
        permission,
        review_evidence_digest,
        license_member_count: snapshot.license_members.len(),
    })
}

pub(super) fn verify_structural<'lease>(
    control_json: &[u8],
    lease: &'lease VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
    cancellation: &CancellationToken,
) -> Result<VerifiedModelLicenseControl<'lease>, ModelLicenseControlError> {
    ensure_not_cancelled(cancellation)?;
    let observed = parse_control(control_json)?;
    let canonical_reviewer_bytes = super::wire::encode_review(&observed.review)?;
    if observed.permission != permission || observed.review.permission != permission {
        return Err(ModelLicenseControlError::PermissionMismatch);
    }
    let snapshot = begin(lease, cancellation)?;
    let (expected, _review_evidence_digest) = derive_control(
        &snapshot,
        permission,
        observed.review.clone(),
        &canonical_reviewer_bytes,
    )?;
    if observed != expected || encode_control(&expected)? != control_json {
        return Err(ModelLicenseControlError::InvalidBinding);
    }
    finish(lease, &snapshot, cancellation)?;
    Ok(VerifiedModelLicenseControl {
        lease,
        control_id: control_id(control_json),
        foundation_id: snapshot.foundation_id,
        permission,
        installation_generation: snapshot.installation_generation,
        license_member_count: snapshot.license_members.len(),
    })
}

pub(super) fn promote<'lease>(
    verified: VerifiedModelLicenseControl<'lease>,
    approval_policy: &ProductionModelLicenseApprovalPolicy,
    cancellation: &CancellationToken,
) -> Result<VerifiedApprovedModelLicenseControl<'lease>, ModelLicenseControlError> {
    match assess(
        &verified,
        verified.lease,
        verified.permission,
        approval_policy,
        cancellation,
    )? {
        ModelLicenseControlAssessmentDisposition::Approved => {
            Ok(VerifiedApprovedModelLicenseControl { verified })
        }
        ModelLicenseControlAssessmentDisposition::Denied => {
            Err(ModelLicenseControlError::ApprovalPolicyDenied)
        }
    }
}

pub(super) fn assess(
    verified: &VerifiedModelLicenseControl<'_>,
    selected_lease: &VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
    approval_policy: &ProductionModelLicenseApprovalPolicy,
    cancellation: &CancellationToken,
) -> Result<ModelLicenseControlAssessmentDisposition, ModelLicenseControlError> {
    revalidate_structural(verified, selected_lease, permission, cancellation)?;
    if approval_policy.permits(&verified.control_id, permission) {
        Ok(ModelLicenseControlAssessmentDisposition::Approved)
    } else {
        Ok(ModelLicenseControlAssessmentDisposition::Denied)
    }
}

pub(super) fn revalidate_structural(
    authority: &VerifiedModelLicenseControl<'_>,
    selected_lease: &VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
    cancellation: &CancellationToken,
) -> Result<(), ModelLicenseControlError> {
    ensure_not_cancelled(cancellation)?;
    if permission != authority.permission {
        return Err(ModelLicenseControlError::PermissionMismatch);
    }
    if !std::ptr::eq(authority.lease, selected_lease) {
        return Err(ModelLicenseControlError::LeaseMismatch);
    }
    let selected = begin(selected_lease, cancellation)?;
    if selected.installation_generation != authority.installation_generation {
        return Err(ModelLicenseControlError::InstallationGenerationMismatch);
    }
    if selected.foundation_id != authority.foundation_id {
        return Err(ModelLicenseControlError::InvalidBinding);
    }
    Ok(())
}

fn begin(
    lease: &VerifiedManagedOllamaModelPackageLease,
    cancellation: &CancellationToken,
) -> Result<FoundationSnapshot, ModelLicenseControlError> {
    ensure_not_cancelled(cancellation)?;
    lease
        .revalidate(cancellation)
        .map_err(ModelLicenseControlError::Lease)?;
    ensure_not_cancelled(cancellation)?;
    snapshot(lease)
}

fn finish(
    lease: &VerifiedManagedOllamaModelPackageLease,
    before: &FoundationSnapshot,
    cancellation: &CancellationToken,
) -> Result<(), ModelLicenseControlError> {
    ensure_not_cancelled(cancellation)?;
    lease
        .revalidate(cancellation)
        .map_err(ModelLicenseControlError::Lease)?;
    ensure_not_cancelled(cancellation)?;
    if &snapshot(lease)? == before {
        Ok(())
    } else {
        Err(ModelLicenseControlError::InvalidBinding)
    }
}

fn snapshot(
    lease: &VerifiedManagedOllamaModelPackageLease,
) -> Result<FoundationSnapshot, ModelLicenseControlError> {
    let view = lease.private_view();
    let artifact_set = view.artifact_set_manifest();
    let package = view.model_package_manifest();
    let foundation = view.foundation_evidence();
    let source = package.source();
    let installation_generation = view.installation_generation();
    if installation_generation == 0
        || source.kind() != PackageSourceKind::LocalArchive
        || artifact_set.artifact_set_id() != *foundation.artifact_set_id()
        || package.artifact_set_id() != foundation.artifact_set_id()
        || package.model_package_manifest_id() != *foundation.model_package_manifest_id()
        || source.package_source_id() != *foundation.package_source_id()
    {
        return Err(ModelLicenseControlError::InvalidBinding);
    }
    let license_members = foundation
        .license_members()
        .iter()
        .map(member_snapshot)
        .collect::<Vec<_>>();
    validate_license_members(package, &license_members)?;
    Ok(FoundationSnapshot {
        installation_generation,
        foundation_id: lease.foundation_id().clone(),
        artifact_set_id: foundation.artifact_set_id().digest().clone(),
        model_package_manifest_id: foundation.model_package_manifest_id().digest().clone(),
        package_source_id: foundation.package_source_id().digest().clone(),
        descriptor_mapping_digest: foundation.descriptor_mapping_digest().clone(),
        logical_binding_digest: foundation.logical_binding_digest().clone(),
        provenance_manifest: member_snapshot(foundation.provenance_manifest()),
        source: SourceSnapshot {
            kind: source.kind(),
            locator: source.locator().to_owned(),
            revision: source.revision().to_owned(),
            provenance_digest: source.provenance_digest().clone(),
        },
        license_members,
    })
}

fn validate_license_members(
    package: &rewrite_model::ModelPackageManifest,
    retained: &[MemberSnapshot],
) -> Result<(), ModelLicenseControlError> {
    if retained.is_empty()
        || retained
            .windows(2)
            .any(|pair| pair[0].relative_path >= pair[1].relative_path)
    {
        return Err(ModelLicenseControlError::InvalidBinding);
    }
    let declared = package.members().iter().filter(|member| {
        member
            .roles()
            .contains(&ModelPackageMemberRole::LicenseText)
    });
    if !declared
        .map(|member| MemberSnapshot {
            relative_path: member.relative_path().as_str().to_owned(),
            artifact_id: member.artifact_id().clone(),
            byte_size: member.byte_size(),
        })
        .eq(retained.iter().cloned())
    {
        return Err(ModelLicenseControlError::InvalidBinding);
    }
    Ok(())
}

fn derive_control(
    snapshot: &FoundationSnapshot,
    permission: ModelLicensePermission,
    review: ReviewWire,
    canonical_reviewer_bytes: &[u8],
) -> Result<(ControlWire, Digest), ModelLicenseControlError> {
    if review.permission != permission {
        return Err(ModelLicenseControlError::PermissionMismatch);
    }
    if review != expected_review(snapshot, permission) {
        return Err(ModelLicenseControlError::ReviewRequired);
    }
    let review_evidence_digest = review_evidence_digest(canonical_reviewer_bytes);
    let license_member_count = u32::try_from(snapshot.license_members.len())
        .map_err(|_error| ModelLicenseControlError::LimitExceeded)?;
    Ok((
        ControlWire {
            authority: AuthorityWire::None,
            control: ControlKindWire::ModelLicense,
            evidence: ControlEvidenceWire {
                artifact_set_id: snapshot.artifact_set_id.clone(),
                foundation_id: snapshot.foundation_id.digest().clone(),
                license_member_count,
                model_package_manifest_id: snapshot.model_package_manifest_id.clone(),
                package_source_id: snapshot.package_source_id.clone(),
                review_evidence_digest: review_evidence_digest.clone(),
            },
            permission,
            procedure_id: MODEL_LICENSE_CONTROL_PROCEDURE_ID.to_owned(),
            procedure_version: MODEL_LICENSE_CONTROL_PROCEDURE_VERSION,
            review,
            schema_version: MODEL_LICENSE_CONTROL_SCHEMA_VERSION,
            status: ControlStatusWire::Approved,
        },
        review_evidence_digest,
    ))
}

fn expected_review(
    snapshot: &FoundationSnapshot,
    permission: ModelLicensePermission,
) -> ReviewWire {
    ReviewWire {
        decision: ReviewDecisionWire::Approved,
        foundation: FoundationWire {
            artifact_set_id: snapshot.artifact_set_id.clone(),
            descriptor_mapping_digest: snapshot.descriptor_mapping_digest.clone(),
            foundation_id: snapshot.foundation_id.digest().clone(),
            logical_binding_digest: snapshot.logical_binding_digest.clone(),
            model_package_manifest_id: snapshot.model_package_manifest_id.clone(),
            package_source_id: snapshot.package_source_id.clone(),
            provenance_manifest: member_wire(&snapshot.provenance_manifest),
        },
        license_members: snapshot.license_members.iter().map(member_wire).collect(),
        permission,
        procedure_id: MODEL_LICENSE_REVIEW_PROCEDURE_ID.to_owned(),
        procedure_version: MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
        schema_version: MODEL_LICENSE_REVIEW_SCHEMA_VERSION,
        source: SourceWire {
            kind: snapshot.source.kind,
            locator: snapshot.source.locator.clone(),
            provenance_digest: snapshot.source.provenance_digest.clone(),
            revision: snapshot.source.revision.clone(),
        },
    }
}

fn member_snapshot(member: &OllamaLocalArchiveMemberBinding) -> MemberSnapshot {
    MemberSnapshot {
        relative_path: member.relative_path().as_str().to_owned(),
        artifact_id: member.artifact_id().clone(),
        byte_size: member.byte_size(),
    }
}

fn member_wire(member: &MemberSnapshot) -> LicenseMemberWire {
    LicenseMemberWire {
        artifact_id: member.artifact_id.clone(),
        byte_size: member.byte_size,
        relative_path: member.relative_path.clone(),
    }
}

fn review_evidence_digest(bytes: &[u8]) -> Digest {
    let mut material = Vec::with_capacity(REVIEW_EVIDENCE_DOMAIN.len() + bytes.len());
    material.extend_from_slice(REVIEW_EVIDENCE_DOMAIN);
    material.extend_from_slice(bytes);
    Digest::sha256(&material)
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ModelLicenseControlError> {
    if cancellation.is_cancelled() {
        Err(ModelLicenseControlError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(super) fn reviewer_json_for_test(
    lease: &VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
) -> Vec<u8> {
    let snapshot = snapshot(lease).expect("verified fixture snapshot");
    super::wire::encode_review(&expected_review(&snapshot, permission))
        .expect("bounded fixture review")
}
