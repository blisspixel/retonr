//! Pure semantic resource closure and limit classification derivation.

use super::{
    CancellationToken, DerivedResourcePhase, GenerationQualificationPhaseScopeV1,
    GenerationQualificationResourcePhaseDerivationError,
    GenerationQualificationResourcePolicyLimitsV1, GenerationResourceAttemptResultRecordV1,
    GenerationResourceEvidenceManifestV1, GenerationResourceEvidenceManifestV1Relations,
    MAX_GENERATION_QUALIFICATION_PHASE_ITEMS, ResourceEvidenceView, ResourceManifestDerivation,
    VerifiedCompletePassedRepeatabilityJoins, VerifiedGenerationQualificationResourcePolicy,
    check_derivation_active, exceeded_limits, phase_status, resource_facts,
};

pub(super) fn derive_phase(
    authority: &VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    results: &[GenerationResourceAttemptResultRecordV1],
    cancellation: &CancellationToken,
) -> Result<DerivedResourcePhase, GenerationQualificationResourcePhaseDerivationError> {
    derive_phase_with_policy(
        authority,
        resource_policy.policy_digest(),
        resource_policy.limits(),
        results,
        cancellation,
    )
}

pub(super) fn derive_phase_with_policy(
    authority: &VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    policy_digest: &rewrite_types::Digest,
    limits: GenerationQualificationResourcePolicyLimitsV1,
    results: &[GenerationResourceAttemptResultRecordV1],
    cancellation: &CancellationToken,
) -> Result<DerivedResourcePhase, GenerationQualificationResourcePhaseDerivationError> {
    check_derivation_active(cancellation)?;
    let expected_count = authority
        .preregistered_repetitions()
        .len()
        .checked_mul(authority.suite().case_ids().len())
        .filter(|count| *count > 0 && *count <= MAX_GENERATION_QUALIFICATION_PHASE_ITEMS)
        .ok_or(GenerationQualificationResourcePhaseDerivationError::InvalidCount)?;
    if results.len() != expected_count {
        return Err(GenerationQualificationResourcePhaseDerivationError::InvalidCount);
    }

    let target_id = authority.target_generation_system().generation_system_id();
    let mut cursor = 0;
    let mut evidence = Vec::with_capacity(results.len());
    for repetition in authority.preregistered_repetitions() {
        for case_id in authority.suite().case_ids() {
            check_derivation_active(cancellation)?;
            let matching = authority
                .planned_attempts()
                .iter()
                .filter(|attempt| {
                    attempt.generation_system_id() == target_id
                        && attempt.repetition_id() == repetition.repetition_id()
                        && attempt.case_id() == case_id
                })
                .collect::<Vec<_>>();
            let [planned_attempt] = matching.as_slice() else {
                return Err(GenerationQualificationResourcePhaseDerivationError::Relationship);
            };
            let result = results
                .get(cursor)
                .ok_or(GenerationQualificationResourcePhaseDerivationError::InvalidCount)?;
            let relationships_match = result.generation_system_id() == target_id
                && result.generation_qualification_plan_id()
                    == authority.qualification_plan().qualification_plan_id()
                && result.suite_manifest_id() == authority.suite().suite_manifest_id()
                && result.repetition_id() == repetition.repetition_id()
                && result.case_id() == case_id
                && result.planned_attempt_id() == planned_attempt.planned_attempt_id()
                && result.resource_policy_digest() == policy_digest;
            evidence.push(ResourceEvidenceView {
                relationships_match,
                facts: resource_facts(result),
                observed_exceeded: result.exceeded_limits().to_vec(),
                digest: result.resource_attempt_result_id().digest().clone(),
            });
            cursor += 1;
        }
    }
    if cursor != results.len() {
        return Err(GenerationQualificationResourcePhaseDerivationError::InvalidCount);
    }
    let manifest = derive_resource_manifest(
        ResourceManifestDerivation {
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: authority.target_generation_system(),
                qualification_plan: authority.qualification_plan(),
                suite: authority.suite(),
            },
            policy_digest,
            limits,
            expected_count,
            evidence: &evidence,
        },
        cancellation,
    )?;
    Ok(DerivedResourcePhase {
        results: results.to_vec(),
        manifest,
    })
}

pub(super) fn derive_resource_manifest(
    derivation: ResourceManifestDerivation<'_>,
    cancellation: &CancellationToken,
) -> Result<GenerationResourceEvidenceManifestV1, GenerationQualificationResourcePhaseDerivationError>
{
    check_derivation_active(cancellation)?;
    if derivation.expected_count == 0
        || derivation.expected_count > MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
        || derivation.evidence.len() != derivation.expected_count
    {
        return Err(GenerationQualificationResourcePhaseDerivationError::InvalidCount);
    }
    let mut any_exceeded = false;
    for evidence in derivation.evidence {
        check_derivation_active(cancellation)?;
        if !evidence.relationships_match {
            return Err(GenerationQualificationResourcePhaseDerivationError::Relationship);
        }
        let expected_exceeded = exceeded_limits(evidence.facts, derivation.limits);
        if evidence.observed_exceeded != expected_exceeded {
            return Err(GenerationQualificationResourcePhaseDerivationError::LimitClassification);
        }
        any_exceeded |= !expected_exceeded.is_empty();
    }
    check_derivation_active(cancellation)?;
    let digests = derivation
        .evidence
        .iter()
        .map(|evidence| evidence.digest.clone())
        .collect::<Vec<_>>();
    GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
        scope: derivation.scope,
        phase_policy_digest: derivation.policy_digest,
        evidence_record_digests: &digests,
        status: phase_status(any_exceeded),
    })
    .map_err(GenerationQualificationResourcePhaseDerivationError::Manifest)
}
