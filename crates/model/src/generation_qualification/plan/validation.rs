use std::collections::HashSet;

use super::{
    GenerationQualificationPlanLimitsV1, MAX_GENERATION_SYSTEMS_PER_PLAN,
    MAX_PLANNED_GENERATION_ATTEMPTS,
};
use crate::generation_qualification::{
    GenerationQualificationContractError, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    GenerationSystemId, GenerationSystemRecordV1, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1,
};

#[derive(Clone, Copy)]
pub(super) struct PlanBuildContext<'a> {
    pub(super) suite: &'a GenerationSuiteManifestV1,
    pub(super) repetitions: &'a [GenerationRepetitionRecordV1],
    pub(super) generation_systems: &'a [GenerationSystemRecordV1],
    pub(super) planned_attempts: &'a [PlannedCandidateAttemptV1],
    pub(super) wire_ids: Option<(&'a [GenerationSystemId], &'a [PlannedCandidateAttemptId])>,
}

pub(super) fn validate_records(
    suite: &GenerationSuiteManifestV1,
    repetitions: &[GenerationRepetitionRecordV1],
    systems: &[GenerationSystemRecordV1],
    attempts: &[PlannedCandidateAttemptV1],
    limits: GenerationQualificationPlanLimitsV1,
) -> Result<(), GenerationQualificationContractError> {
    validate_repetitions(suite, repetitions)?;
    if systems.is_empty() || systems.len() > MAX_GENERATION_SYSTEMS_PER_PLAN {
        return Err(GenerationQualificationContractError::InvalidCollectionSize);
    }
    if systems.windows(2).any(|pair| {
        pair[0].generation_system_id().digest().as_str()
            >= pair[1].generation_system_id().digest().as_str()
    }) {
        return Err(GenerationQualificationContractError::NonCanonicalSetOrder);
    }
    let local_maximum = usize::try_from(limits.maximum_predeclared_attempts())
        .map_err(|_| GenerationQualificationContractError::InvalidLimits)?;
    if attempts.is_empty()
        || attempts.len() > MAX_PLANNED_GENERATION_ATTEMPTS
        || attempts.len() > local_maximum
    {
        return Err(GenerationQualificationContractError::InvalidCollectionSize);
    }
    validate_attempt_closure(suite, repetitions, systems, attempts, limits)
}

fn validate_attempt_closure(
    suite: &GenerationSuiteManifestV1,
    repetitions: &[GenerationRepetitionRecordV1],
    systems: &[GenerationSystemRecordV1],
    attempts: &[PlannedCandidateAttemptV1],
    limits: GenerationQualificationPlanLimitsV1,
) -> Result<(), GenerationQualificationContractError> {
    let mut unique = HashSet::with_capacity(attempts.len());
    let available = systems
        .iter()
        .map(GenerationSystemRecordV1::generation_system_id)
        .collect::<HashSet<_>>();
    let mut used = HashSet::with_capacity(systems.len());
    let repetition_ids = repetitions
        .iter()
        .map(GenerationRepetitionRecordV1::repetition_id)
        .collect::<HashSet<_>>();
    let mut used_repetitions = HashSet::with_capacity(repetitions.len());
    for (ordinal, attempt) in attempts.iter().enumerate() {
        if !unique.insert(attempt.planned_attempt_id()) {
            return Err(GenerationQualificationContractError::DuplicateEntry);
        }
        validate_attempt(suite, &available, &repetition_ids, attempt, ordinal, limits)?;
        used.insert(attempt.generation_system_id());
        used_repetitions.insert(attempt.repetition_id());
    }
    if used != available {
        return Err(GenerationQualificationContractError::GenerationSystemMismatch);
    }
    if used_repetitions != repetition_ids {
        return Err(GenerationQualificationContractError::RepetitionMismatch);
    }
    Ok(())
}

fn validate_attempt(
    suite: &GenerationSuiteManifestV1,
    available_systems: &HashSet<&GenerationSystemId>,
    available_repetitions: &HashSet<&crate::generation_qualification::GenerationRepetitionId>,
    attempt: &PlannedCandidateAttemptV1,
    ordinal: usize,
    limits: GenerationQualificationPlanLimitsV1,
) -> Result<(), GenerationQualificationContractError> {
    if usize::try_from(attempt.attempt_ordinal()) != Ok(ordinal) {
        return Err(GenerationQualificationContractError::AttemptOrdinalMismatch);
    }
    if attempt.suite_manifest_id() != suite.suite_manifest_id() {
        return Err(GenerationQualificationContractError::SuiteMismatch);
    }
    if !available_repetitions.contains(attempt.repetition_id()) {
        return Err(GenerationQualificationContractError::RepetitionMismatch);
    }
    if !available_systems.contains(attempt.generation_system_id()) {
        return Err(GenerationQualificationContractError::GenerationSystemMismatch);
    }
    let output = attempt.output_ceilings();
    if output.candidate_count() > limits.maximum_candidates_per_completion()
        || attempt.source_byte_count() > limits.maximum_retained_input_bytes()
        || output.maximum_candidate_bytes() > limits.maximum_evidence_bundle_bytes()
        || output.maximum_aggregate_candidate_bytes() > limits.maximum_evidence_bundle_bytes()
        || output.maximum_envelope_bytes() > limits.maximum_evidence_bundle_bytes()
    {
        return Err(GenerationQualificationContractError::InvalidLimits);
    }
    Ok(())
}

fn validate_repetitions(
    suite: &GenerationSuiteManifestV1,
    repetitions: &[GenerationRepetitionRecordV1],
) -> Result<(), GenerationQualificationContractError> {
    if repetitions.is_empty() || repetitions.len() > MAX_PLANNED_GENERATION_ATTEMPTS {
        return Err(GenerationQualificationContractError::InvalidCollectionSize);
    }
    for (ordinal, repetition) in repetitions.iter().enumerate() {
        if repetition.suite_manifest_id() != suite.suite_manifest_id()
            || usize::try_from(repetition.repetition_ordinal()) != Ok(ordinal)
        {
            return Err(GenerationQualificationContractError::RepetitionMismatch);
        }
    }
    Ok(())
}
