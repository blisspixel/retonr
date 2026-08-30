use rewrite_types::Digest;

use super::wire::{encode_control, expected_control, expected_review, parse_control, parse_review};
use super::{
    CompiledGenerationSystemPolicyControl, GenerationSystemPolicyBindingsV1,
    GenerationSystemPolicyError, GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose,
    ProductionGenerationSystemPolicyApproval, VerifiedGenerationSystemPolicy, control_id,
};

const REVIEW_EVIDENCE_DOMAIN: &[u8] = b"retonr:generation-system-policy-review:v1\0";

pub(super) fn compile(
    bindings: &GenerationSystemPolicyBindingsV1,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
    reviewer_json: &[u8],
) -> Result<CompiledGenerationSystemPolicyControl, GenerationSystemPolicyError> {
    let review = parse_review(reviewer_json)?;
    validate_permission_and_purpose(review.permission, review.purpose, permission, purpose)?;
    if review != expected_review(bindings, permission, purpose) {
        return Err(GenerationSystemPolicyError::ReviewRequired);
    }
    let review_evidence_digest = review_evidence_digest(reviewer_json);
    let control = expected_control(review, review_evidence_digest.clone(), permission, purpose);
    let canonical_bytes = encode_control(&control)?;
    Ok(CompiledGenerationSystemPolicyControl {
        control_id: control_id(&canonical_bytes),
        canonical_bytes,
        permission,
        purpose,
        review_evidence_digest,
        bindings: bindings.clone(),
    })
}

pub(super) fn verify(
    control_json: &[u8],
    expected_bindings: &GenerationSystemPolicyBindingsV1,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
    approval: &ProductionGenerationSystemPolicyApproval,
) -> Result<VerifiedGenerationSystemPolicy, GenerationSystemPolicyError> {
    let observed = parse_control(control_json)?;
    validate_permission_and_purpose(observed.permission, observed.purpose, permission, purpose)?;
    validate_permission_and_purpose(
        observed.review.permission,
        observed.review.purpose,
        permission,
        purpose,
    )?;
    let review = expected_review(expected_bindings, permission, purpose);
    let canonical_review = super::wire::encode_review(&review)?;
    let expected = expected_control(
        review,
        review_evidence_digest(&canonical_review),
        permission,
        purpose,
    );
    if observed != expected || encode_control(&expected)? != control_json {
        return Err(GenerationSystemPolicyError::InvalidBinding);
    }
    let id = control_id(control_json);
    if !approval.permits(&id, permission, purpose) {
        return Err(GenerationSystemPolicyError::ApprovalPolicyDenied);
    }
    Ok(VerifiedGenerationSystemPolicy {
        control_id: id,
        permission,
        purpose,
        bindings: expected_bindings.clone(),
    })
}

fn validate_permission_and_purpose(
    observed_permission: GenerationSystemPolicyPermission,
    observed_purpose: GenerationSystemPolicyPurpose,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
) -> Result<(), GenerationSystemPolicyError> {
    if observed_permission != permission {
        return Err(GenerationSystemPolicyError::PermissionMismatch);
    }
    if observed_purpose != purpose {
        return Err(GenerationSystemPolicyError::PurposeMismatch);
    }
    Ok(())
}

fn review_evidence_digest(bytes: &[u8]) -> Digest {
    let mut material = Vec::with_capacity(REVIEW_EVIDENCE_DOMAIN.len() + bytes.len());
    material.extend_from_slice(REVIEW_EVIDENCE_DOMAIN);
    material.extend_from_slice(bytes);
    Digest::sha256(&material)
}

#[cfg(test)]
pub(super) fn reviewer_json_for_test(
    bindings: &GenerationSystemPolicyBindingsV1,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
) -> Vec<u8> {
    super::wire::encode_review(&expected_review(bindings, permission, purpose))
        .expect("bounded exact policy review")
}
