use rewrite_model::GenerationQualificationOperationPolicyV1;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::wire::{
    ResourcePolicyWire, expected_human, expected_resource, parse_human, parse_resource,
};
use super::{
    GenerationQualificationPhasePolicyError, GenerationQualificationResourcePolicyLimitsV1,
    HUMAN_ADJUDICATION_POLICY_DIGEST_DOMAIN,
    ProductionGenerationQualificationHumanAdjudicationPolicySource,
    ProductionGenerationQualificationResourcePolicySource, RESOURCE_POLICY_DIGEST_DOMAIN,
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    VerifiedGenerationQualificationResourcePolicy,
};

pub(super) fn verify_resource(
    policy_json: &[u8],
    operation_policy: &GenerationQualificationOperationPolicyV1,
    source: &ProductionGenerationQualificationResourcePolicySource,
) -> Result<VerifiedGenerationQualificationResourcePolicy, GenerationQualificationPhasePolicyError>
{
    let authority = verify_resource_structure(policy_json, source)?;
    authority.revalidate_operation_policy(operation_policy)?;
    Ok(authority)
}

pub(super) fn verify_resource_structure(
    policy_json: &[u8],
    source: &ProductionGenerationQualificationResourcePolicySource,
) -> Result<VerifiedGenerationQualificationResourcePolicy, GenerationQualificationPhasePolicyError>
{
    let observed = parse_resource(policy_json)?;
    let limits = resource_limits(&observed);
    validate_nonzero_resource_limits(limits)?;
    if observed != expected_resource(limits) {
        return Err(GenerationQualificationPhasePolicyError::InvalidResourceBinding);
    }
    let policy_digest = policy_digest(RESOURCE_POLICY_DIGEST_DOMAIN, policy_json)?;
    Ok(VerifiedGenerationQualificationResourcePolicy {
        source_disposition: source.disposition(&policy_digest),
        policy_digest,
        limits,
    })
}

pub(super) fn verify_human(
    policy_json: &[u8],
    operation_policy: &GenerationQualificationOperationPolicyV1,
    source: &ProductionGenerationQualificationHumanAdjudicationPolicySource,
) -> Result<
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    GenerationQualificationPhasePolicyError,
> {
    let authority = verify_human_structure(policy_json, source)?;
    authority.revalidate_operation_policy(operation_policy)?;
    Ok(authority)
}

pub(super) fn verify_human_structure(
    policy_json: &[u8],
    source: &ProductionGenerationQualificationHumanAdjudicationPolicySource,
) -> Result<
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    GenerationQualificationPhasePolicyError,
> {
    let observed = parse_human(policy_json)?;
    if observed != expected_human(observed.presentation_seed) {
        return Err(GenerationQualificationPhasePolicyError::InvalidHumanAdjudicationBinding);
    }
    let policy_digest = policy_digest(HUMAN_ADJUDICATION_POLICY_DIGEST_DOMAIN, policy_json)?;
    Ok(VerifiedGenerationQualificationHumanAdjudicationPolicy {
        source_disposition: source.disposition(&policy_digest),
        policy_digest,
        presentation_seed: observed.presentation_seed,
    })
}

pub(super) fn validate_resource_operation_digest(
    policy_digest: &Digest,
    operation_policy: &GenerationQualificationOperationPolicyV1,
) -> Result<(), GenerationQualificationPhasePolicyError> {
    validate_operation_digest(policy_digest, operation_policy.resource_policy_digest())
}

pub(super) fn validate_human_operation_digest(
    policy_digest: &Digest,
    operation_policy: &GenerationQualificationOperationPolicyV1,
) -> Result<(), GenerationQualificationPhasePolicyError> {
    validate_operation_digest(
        policy_digest,
        operation_policy.human_adjudication_policy_digest(),
    )
}

fn validate_operation_digest(
    observed: &Digest,
    expected: &Digest,
) -> Result<(), GenerationQualificationPhasePolicyError> {
    if observed == expected {
        Ok(())
    } else {
        Err(GenerationQualificationPhasePolicyError::OperationPolicyMismatch)
    }
}

fn resource_limits(policy: &ResourcePolicyWire) -> GenerationQualificationResourcePolicyLimitsV1 {
    GenerationQualificationResourcePolicyLimitsV1 {
        maximum_attempt_elapsed_nanoseconds: policy.maximum_attempt_elapsed_nanoseconds,
        maximum_first_response_nanoseconds: policy.maximum_first_response_nanoseconds,
        maximum_cleanup_nanoseconds: policy.maximum_cleanup_nanoseconds,
        maximum_worker_high_water_resident_bytes: policy.maximum_worker_high_water_resident_bytes,
        maximum_installed_footprint_bytes: policy.maximum_installed_footprint_bytes,
    }
}

fn validate_nonzero_resource_limits(
    limits: GenerationQualificationResourcePolicyLimitsV1,
) -> Result<(), GenerationQualificationPhasePolicyError> {
    if [
        limits.maximum_attempt_elapsed_nanoseconds,
        limits.maximum_first_response_nanoseconds,
        limits.maximum_cleanup_nanoseconds,
        limits.maximum_worker_high_water_resident_bytes,
        limits.maximum_installed_footprint_bytes,
    ]
    .contains(&0)
    {
        Err(GenerationQualificationPhasePolicyError::InvalidResourceBinding)
    } else {
        Ok(())
    }
}

fn policy_digest(
    domain: &[u8],
    canonical_bytes: &[u8],
) -> Result<Digest, GenerationQualificationPhasePolicyError> {
    let byte_count = u64::try_from(canonical_bytes.len())
        .map_err(|_| GenerationQualificationPhasePolicyError::LimitExceeded)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(byte_count.to_be_bytes());
    hasher.update(canonical_bytes);
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| GenerationQualificationPhasePolicyError::InvalidEncoding)
}

#[cfg(test)]
pub(super) fn resource_policy_json_for_test(
    limits: GenerationQualificationResourcePolicyLimitsV1,
) -> Vec<u8> {
    super::wire::encode_resource(&expected_resource(limits)).expect("bounded resource policy")
}

#[cfg(test)]
pub(super) fn human_policy_json_for_test(presentation_seed: u64) -> Vec<u8> {
    super::wire::encode_human(&expected_human(presentation_seed)).expect("bounded human policy")
}

#[cfg(test)]
pub(super) fn resource_policy_digest_for_test(bytes: &[u8]) -> Digest {
    policy_digest(RESOURCE_POLICY_DIGEST_DOMAIN, bytes).expect("bounded test resource policy")
}

#[cfg(test)]
pub(super) fn human_policy_digest_for_test(bytes: &[u8]) -> Digest {
    policy_digest(HUMAN_ADJUDICATION_POLICY_DIGEST_DOMAIN, bytes)
        .expect("bounded test human policy")
}
