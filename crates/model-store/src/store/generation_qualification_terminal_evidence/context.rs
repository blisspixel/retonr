use rewrite_model::{
    GenerationAttemptLedgerManifestV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPlatformEvidenceV1Relations, GenerationSystemRecordV1,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
};
use rusqlite::Connection;

use crate::store::candidate_generation_execution::{
    RederivedAttemptLedger, rederive_attempt_ledger,
};
use crate::store::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationReadInput, StoredGenerationQualificationPreregistration,
    load_preregistration,
};
use crate::{StoreError, StoreResult};

pub(super) struct Anchor<'a> {
    pub(super) preregistration: StoredGenerationQualificationPreregistration,
    pub(super) ledger: RederivedAttemptLedger,
    pub(super) target_index: usize,
    pub(super) system_relations: rewrite_model::GenerationSystemRecordV1Relations<'a>,
}

pub(super) fn load_anchor<'a>(
    connection: &Connection,
    preregistration: GenerationQualificationPreregistrationReadInput<'a>,
    managed_evidence_inputs: &'a [ManagedOllamaCandidateGenerationEvidenceV2Input],
) -> StoreResult<Anchor<'a>> {
    let stored =
        load_preregistration(connection, preregistration)?.ok_or(StoreError::MissingRecord)?;
    let ledger = rederive_attempt_ledger(connection, preregistration, managed_evidence_inputs)?;
    let target_id = stored
        .operation_policy()
        .target_generation_system_id()
        .clone();
    let target_index = stored
        .plan_foundation()
        .generation_systems()
        .iter()
        .position(|system| system.generation_system_id() == &target_id)
        .ok_or(StoreError::CorruptRecord)?;
    let supplied = preregistration.operation_policy_relations.target_system;
    let stored_target = stored
        .plan_foundation()
        .generation_systems()
        .get(target_index)
        .ok_or(StoreError::CorruptRecord)?;
    if supplied.generation_system != stored_target {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Anchor {
        preregistration: stored,
        ledger,
        target_index,
        system_relations: supplied.relations,
    })
}

pub(super) fn scope<'a>(
    anchor: &'a Anchor<'_>,
) -> StoreResult<GenerationQualificationPhaseScopeV1<'a>> {
    let foundation = anchor.preregistration.plan_foundation();
    Ok(GenerationQualificationPhaseScopeV1 {
        generation_system: target_system(anchor)?,
        qualification_plan: foundation.plan(),
        suite: foundation.suite(),
    })
}

pub(super) fn platform_relations<'a>(
    anchor: &'a Anchor<'_>,
) -> StoreResult<GenerationQualificationPlatformEvidenceV1Relations<'a>> {
    Ok(GenerationQualificationPlatformEvidenceV1Relations {
        operation_policy: anchor.preregistration.operation_policy(),
        request_projection: anchor.preregistration.request_projection(),
        target_generation_system: target_system(anchor)?,
        target_generation_system_relations: anchor.system_relations,
    })
}

pub(super) fn ledger_relations<'a>(
    anchor: &'a Anchor<'_>,
) -> StoreResult<GenerationAttemptLedgerManifestV1Relations<'a>> {
    let foundation = anchor.preregistration.plan_foundation();
    Ok(GenerationAttemptLedgerManifestV1Relations {
        scope: scope(anchor)?,
        phase_policy_digest: anchor
            .preregistration
            .operation_policy()
            .attempt_ledger_policy_digest(),
        planned_attempts: foundation.planned_attempts(),
        attempt_records: &anchor.ledger.attempt_records,
        status: anchor.ledger.manifest.status(),
    })
}

fn target_system<'a>(anchor: &'a Anchor<'_>) -> StoreResult<&'a GenerationSystemRecordV1> {
    anchor
        .preregistration
        .plan_foundation()
        .generation_systems()
        .get(anchor.target_index)
        .ok_or(StoreError::CorruptRecord)
}
