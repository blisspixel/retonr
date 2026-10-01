//! Atomic complete Passed phase publication over exact durable parents.

use super::{
    ArtifactStateStore, GenerationAttemptLedgerV1Input, StoreError, StoreResult, WriteDisposition,
};
use rewrite_model::{
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityTerminalStageV1,
};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};
use std::{error::Error, fmt};

mod parents;
pub use parents::GenerationRepeatabilityPhaseParentV1;

/// Exact inert complete closure, reconstructed independently from durable parents.
#[derive(Clone, Copy)]
pub struct GenerationRepeatabilityPhaseV1Input<'a> {
    /// Exact ledger and independently retained preregistration/evidence inputs.
    pub ledger: &'a GenerationAttemptLedgerV1Input<'a>,
    /// Exact parent snapshots retained from each live join in the same order.
    pub expected_parents: &'a [GenerationRepeatabilityPhaseParentV1],
    /// Every Passed result in preregistered repetition order.
    pub ordered_results: &'a [GenerationRepeatabilityResultRecordV1],
    /// Exact complete phase manifest compared with the reconstructed closure.
    pub manifest: &'a GenerationRepeatabilityEvidenceManifestV1,
}

/// Content-free complete repeatability publication failure.
pub enum GenerationRepeatabilityPhaseV1TransactionError<E> {
    /// Storage or exact canonical parent agreement failed.
    Store(StoreError),
    /// The caller's original deadline or cancellation gate rejected publication.
    Gate(E),
}

impl<E> fmt::Debug for GenerationRepeatabilityPhaseV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationRepeatabilityPhaseV1TransactionError::Store",
            Self::Gate(_) => "GenerationRepeatabilityPhaseV1TransactionError::Gate",
        })
    }
}
impl<E> fmt::Display for GenerationRepeatabilityPhaseV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generation repeatability phase publication failed")
    }
}
impl<E> Error for GenerationRepeatabilityPhaseV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Publishes one complete Passed repeatability manifest atomically.
    ///
    /// Prepared scope, ledger and every exact canonical terminal-result parent
    /// are reconstructed inside one immediate transaction. This write grants no
    /// live qualification, activation, or traffic authority.
    ///
    /// # Errors
    ///
    /// Returns a content-free gate, storage, missing-parent, relationship or
    /// immutable conflict error. A failed transaction leaves no phase row.
    pub fn transact_generation_repeatability_phase_v1<E>(
        &mut self,
        input: GenerationRepeatabilityPhaseV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationRepeatabilityPhaseV1TransactionError<E>> {
        use GenerationRepeatabilityPhaseV1TransactionError as Error;
        gate().map_err(Error::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        gate().map_err(Error::Gate)?;
        reconstruct(&transaction, input).map_err(Error::Store)?;
        let manifest = input.manifest;
        let bytes = serde_json::to_vec(manifest)
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        let count = transaction.execute(
            "INSERT OR IGNORE INTO generation_repeatability_evidence_manifests (
             generation_repeatability_evidence_manifest_id, generation_system_id,
             generation_qualification_plan_id, generation_suite_manifest_id,
             evidence_item_count, status, canonical_json) VALUES (?1, ?2, ?3, ?4, ?5, 'passed', ?6)",
            params![manifest.repeatability_evidence_manifest_id().digest().as_str(),
                manifest.generation_system_id().digest().as_str(),
                manifest.generation_qualification_plan_id().digest().as_str(),
                manifest.suite_manifest_id().digest().as_str(), i64::from(manifest.evidence_item_count()), bytes],
        ).map_err(StoreError::from).map_err(Error::Store)?;
        if load(&transaction, input).map_err(Error::Store)?.as_ref() != Some(manifest) {
            return Err(Error::Store(StoreError::ImmutableConflict));
        }
        gate().map_err(Error::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        match count {
            0 => Ok(WriteDisposition::AlreadyPresent),
            1 => Ok(WriteDisposition::Inserted),
            _ => Err(Error::Store(StoreError::CorruptRecord)),
        }
    }

    /// Cold-reads one complete phase after reconstructing exact durable parents.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, reordered, substituted, corrupt, incomplete
    /// or non-Passed parents, noncanonical rows, or an unexpected read mutation.
    pub fn generation_repeatability_phase_v1(
        &self,
        input: GenerationRepeatabilityPhaseV1Input<'_>,
    ) -> StoreResult<Option<GenerationRepeatabilityEvidenceManifestV1>> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let record = load(&transaction, input)?;
        transaction.commit()?;
        if self.connection.total_changes() != before {
            return Err(StoreError::CorruptRecord);
        }
        Ok(record)
    }
}

fn reconstruct(
    connection: &Connection,
    input: GenerationRepeatabilityPhaseV1Input<'_>,
) -> StoreResult<()> {
    if super::load(connection, *input.ledger)?.as_ref() != Some(input.ledger.manifest)
        || input.ledger.manifest.status() != GenerationQualificationPhaseStatusV1::Passed
    {
        return Err(StoreError::MissingRecord);
    }
    let preregistration =
        crate::store::generation_qualification_preregistration::load_preregistration(
            connection,
            input.ledger.preregistration,
        )?
        .ok_or(StoreError::MissingRecord)?;
    let foundation = preregistration.plan_foundation();
    let policy = preregistration.operation_policy();
    let target = foundation
        .generation_systems()
        .iter()
        .find(|system| system.generation_system_id() == policy.target_generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    if input.ordered_results.len() != foundation.repetitions().len()
        || input.expected_parents.len() != input.ordered_results.len()
    {
        return Err(StoreError::CorruptRecord);
    }
    for ((record, repetition), expected_parent) in input
        .ordered_results
        .iter()
        .zip(foundation.repetitions())
        .zip(input.expected_parents)
    {
        if record.repetition_id() != repetition.repetition_id()
            || record.terminal_stage() != GenerationRepeatabilityTerminalStageV1::Passed
            || record.attempt_ledger_manifest_id()
                != input.ledger.manifest.attempt_ledger_manifest_id()
            || record.attempt_ledger_root_digest() != input.ledger.manifest.evidence_root_digest()
            || crate::store::generation_repeatability_terminal_result::load_repeatability_result(
                connection, record,
            )?
            .as_ref()
                != Some(record)
        {
            return Err(StoreError::CorruptRecord);
        }
        parents::confirm(
            connection,
            record,
            expected_parent,
            policy.baseline_generation_system_id(),
        )?;
    }
    let durable_count = connection.query_row(
        "SELECT count(*) FROM generation_repeatability_terminal_result_records WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2",
        params![foundation.plan().qualification_plan_id().digest().as_str(), target.generation_system_id().digest().as_str()],
        |row| row.get::<_, i64>(0),
    )?;
    if durable_count != i64::try_from(input.ordered_results.len()).unwrap_or(i64::MAX) {
        return Err(StoreError::CorruptRecord);
    }
    let expected = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: target,
                qualification_plan: foundation.plan(),
                suite: foundation.suite(),
            },
            phase_policy_digest: policy.repeatability_policy_digest(),
            planned_attempts: foundation.planned_attempts(),
            preregistered_repetitions: foundation.repetitions(),
            results: input.ordered_results,
            status: GenerationQualificationPhaseStatusV1::Passed,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    if &expected != input.manifest {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}

fn load(
    connection: &Connection,
    input: GenerationRepeatabilityPhaseV1Input<'_>,
) -> StoreResult<Option<GenerationRepeatabilityEvidenceManifestV1>> {
    reconstruct(connection, input)?;
    let manifest = input.manifest;
    let expected = serde_json::to_vec(manifest)?;
    let row = connection.query_row(
        "SELECT generation_repeatability_evidence_manifest_id, generation_system_id,
         generation_qualification_plan_id, generation_suite_manifest_id, evidence_item_count, status,
         typeof(canonical_json), length(canonical_json), CAST(substr(canonical_json, 1, 16384) AS BLOB)
         FROM generation_repeatability_evidence_manifests WHERE generation_repeatability_evidence_manifest_id = ?1
           OR (generation_qualification_plan_id = ?2 AND generation_system_id = ?3)",
        params![manifest.repeatability_evidence_manifest_id().digest().as_str(), manifest.generation_qualification_plan_id().digest().as_str(), manifest.generation_system_id().digest().as_str()],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, i64>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, i64>(7)?, row.get::<_, Vec<u8>>(8)?)),
    ).optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.0
        != manifest
            .repeatability_evidence_manifest_id()
            .digest()
            .as_str()
        || row.1 != manifest.generation_system_id().digest().as_str()
        || row.2
            != manifest
                .generation_qualification_plan_id()
                .digest()
                .as_str()
        || row.3 != manifest.suite_manifest_id().digest().as_str()
        || row.4 != i64::from(manifest.evidence_item_count())
        || row.5 != "passed"
        || row.6 != "blob"
        || row.7 != i64::try_from(expected.len()).unwrap_or(i64::MAX)
        || row.8 != expected
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(manifest.clone()))
}
