//! Canonical publication of a ledger rederived from complete durable parents.

use std::{error::Error, fmt};

use rewrite_model::{
    GenerationAttemptLedgerManifestV1, ManagedOllamaCandidateGenerationEvidenceV2Input,
};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};

use super::{
    ArtifactStateStore, WriteDisposition,
    generation_qualification_preregistration::GenerationQualificationPreregistrationReadInput,
};
use crate::{StoreError, StoreResult};

mod repeatability;
pub use repeatability::{
    GenerationRepeatabilityPhaseParentV1, GenerationRepeatabilityPhaseV1Input,
    GenerationRepeatabilityPhaseV1TransactionError,
};

/// Independently retained inputs used to reconstruct every target attempt.
#[derive(Clone, Copy)]
pub struct GenerationAttemptLedgerV1Input<'a> {
    /// Exact preregistration and its independently retained constructor inputs.
    pub preregistration: GenerationQualificationPreregistrationReadInput<'a>,
    /// Managed evidence inputs in target plan order.
    pub managed_evidence_inputs: &'a [ManagedOllamaCandidateGenerationEvidenceV2Input],
    /// Exact manifest to compare against durable parent reconstruction.
    pub manifest: &'a GenerationAttemptLedgerManifestV1,
}

/// Content-free ledger publication failure.
pub enum GenerationAttemptLedgerV1TransactionError<E> {
    /// Durable parent reconstruction or canonical agreement failed.
    Store(StoreError),
    /// The caller's original cancellation or deadline gate rejected publication.
    Gate(E),
}

impl<E> fmt::Debug for GenerationAttemptLedgerV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationAttemptLedgerV1TransactionError::Store",
            Self::Gate(_) => "GenerationAttemptLedgerV1TransactionError::Gate",
        })
    }
}
impl<E> fmt::Display for GenerationAttemptLedgerV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("generation attempt ledger publication failed")
    }
}
impl<E> Error for GenerationAttemptLedgerV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Publishes a canonical ledger after exact durable parent reconstruction.
    ///
    /// All parents and the ledger readback are checked in one immediate
    /// transaction. Returned records and disposition grant no live authority.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for a gate rejection, missing or corrupt
    /// parent, substituted manifest, immutable conflict, or commit failure.
    pub fn transact_generation_attempt_ledger_v1<E>(
        &mut self,
        input: GenerationAttemptLedgerV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationAttemptLedgerV1TransactionError<E>> {
        use GenerationAttemptLedgerV1TransactionError as Error;
        gate().map_err(Error::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        gate().map_err(Error::Gate)?;
        validate_parents(&transaction, input).map_err(Error::Store)?;
        let bytes = serde_json::to_vec(input.manifest)
            .map_err(|_| Error::Store(StoreError::CorruptRecord))?;
        let manifest = input.manifest;
        let changed = transaction
            .execute(
                "INSERT OR IGNORE INTO generation_attempt_ledger_manifests (
             generation_attempt_ledger_manifest_id, generation_system_id,
             generation_qualification_plan_id, generation_suite_manifest_id,
             evidence_item_count, status, canonical_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    manifest.attempt_ledger_manifest_id().digest().as_str(),
                    manifest.generation_system_id().digest().as_str(),
                    manifest
                        .generation_qualification_plan_id()
                        .digest()
                        .as_str(),
                    manifest.suite_manifest_id().digest().as_str(),
                    i64::from(manifest.evidence_item_count()),
                    status(manifest),
                    bytes
                ],
            )
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        if load(&transaction, input).map_err(Error::Store)?.as_ref() != Some(manifest) {
            return Err(Error::Store(StoreError::ImmutableConflict));
        }
        gate().map_err(Error::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        match changed {
            0 => Ok(WriteDisposition::AlreadyPresent),
            1 => Ok(WriteDisposition::Inserted),
            _ => Err(Error::Store(StoreError::CorruptRecord)),
        }
    }

    /// Cold-reads a stored ledger and rederives every durable target parent.
    ///
    /// # Errors
    ///
    /// Returns an error for missing parents, noncanonical or conflicting rows,
    /// or independently retained inputs that do not reproduce the manifest.
    pub fn generation_attempt_ledger_v1(
        &self,
        input: GenerationAttemptLedgerV1Input<'_>,
    ) -> StoreResult<Option<GenerationAttemptLedgerManifestV1>> {
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

fn validate_parents(
    connection: &Connection,
    input: GenerationAttemptLedgerV1Input<'_>,
) -> StoreResult<()> {
    let expected = super::candidate_generation_execution::rederive_attempt_ledger(
        connection,
        input.preregistration,
        input.managed_evidence_inputs,
    )?;
    if &expected.manifest == input.manifest {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn load(
    connection: &Connection,
    input: GenerationAttemptLedgerV1Input<'_>,
) -> StoreResult<Option<GenerationAttemptLedgerManifestV1>> {
    validate_parents(connection, input)?;
    let manifest = input.manifest;
    let expected = serde_json::to_vec(manifest).map_err(|_| StoreError::CorruptRecord)?;
    let row = connection
        .query_row(
            "SELECT generation_attempt_ledger_manifest_id, generation_system_id,
         generation_qualification_plan_id, generation_suite_manifest_id,
         evidence_item_count, status, typeof(canonical_json), length(canonical_json),
         CAST(substr(canonical_json, 1, 16384) AS BLOB)
         FROM generation_attempt_ledger_manifests
         WHERE generation_attempt_ledger_manifest_id = ?1
            OR (generation_qualification_plan_id = ?2 AND generation_system_id = ?3)",
            params![
                manifest.attempt_ledger_manifest_id().digest().as_str(),
                manifest
                    .generation_qualification_plan_id()
                    .digest()
                    .as_str(),
                manifest.generation_system_id().digest().as_str()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Vec<u8>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.0 != manifest.attempt_ledger_manifest_id().digest().as_str()
        || row.1 != manifest.generation_system_id().digest().as_str()
        || row.2
            != manifest
                .generation_qualification_plan_id()
                .digest()
                .as_str()
        || row.3 != manifest.suite_manifest_id().digest().as_str()
        || row.4 != i64::from(manifest.evidence_item_count())
        || row.5 != status(manifest)
        || row.6 != "blob"
        || row.7 != i64::try_from(expected.len()).unwrap_or(i64::MAX)
        || row.8 != expected
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(manifest.clone()))
}

fn status(manifest: &GenerationAttemptLedgerManifestV1) -> &'static str {
    use rewrite_model::GenerationQualificationPhaseStatusV1;
    match manifest.status() {
        GenerationQualificationPhaseStatusV1::Passed => "passed",
        GenerationQualificationPhaseStatusV1::Failed => "failed",
        GenerationQualificationPhaseStatusV1::Skipped => "skipped",
    }
}

#[cfg(test)]
mod tests;
