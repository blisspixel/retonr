//! Atomic inert resource cohort publication over the complete durable phase.

use super::{ArtifactStateStore, WriteDisposition};
use crate::{GenerationRepeatabilityPhaseV1Input, StoreError, StoreResult};
use rewrite_model::{
    GenerationResourceAttemptResultRecordV1, GenerationResourceEvidenceManifestV1,
};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};
use std::{error::Error, fmt};

mod closure;

/// Exact records projected from one retained complete live resource phase.
#[derive(Clone, Copy)]
pub struct GenerationResourcePhaseV1Input<'a> {
    /// Independently retained complete repeatability parent closure.
    pub repeatability: &'a GenerationRepeatabilityPhaseV1Input<'a>,
    /// Target observations in repetition and semantic suite order.
    pub ordered_results: &'a [GenerationResourceAttemptResultRecordV1],
    /// Exact manifest to compare with the derived ordered result closure.
    pub manifest: &'a GenerationResourceEvidenceManifestV1,
}

/// Inert resource cohort recovered after full canonical parent checks.
#[derive(Debug, Eq, PartialEq)]
pub struct StoredGenerationResourcePhaseV1 {
    /// Exact ordered result records.
    pub ordered_results: Vec<GenerationResourceAttemptResultRecordV1>,
    /// Derived Passed or Failed manifest.
    pub manifest: GenerationResourceEvidenceManifestV1,
}

/// Immutable row dispositions for one atomic cohort publication.
#[derive(Debug, Eq, PartialEq)]
pub struct GenerationResourcePhaseV1Disposition {
    /// Each result disposition in semantic order.
    pub ordered_results: Vec<WriteDisposition>,
    /// Manifest disposition.
    pub manifest: WriteDisposition,
}

/// Content-free failure retaining a typed caller gate.
pub enum GenerationResourcePhaseV1TransactionError<E> {
    /// Durable parent or canonical row agreement failed.
    Store(StoreError),
    /// The original deadline or cancellation gate rejected publication.
    Gate(E),
}
impl<E> fmt::Debug for GenerationResourcePhaseV1TransactionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Store(_) => "GenerationResourcePhaseV1TransactionError::Store",
            Self::Gate(_) => "GenerationResourcePhaseV1TransactionError::Gate",
        })
    }
}
impl<E> fmt::Display for GenerationResourcePhaseV1TransactionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("generation resource phase publication failed")
    }
}
impl<E> Error for GenerationResourcePhaseV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically writes all resource results and the derived resource manifest.
    ///
    /// This preserves existing exact rows and grants no live authority. Complete
    /// canonical repeatability and execution parents must already be durable.
    ///
    /// # Errors
    /// Returns a gate, parent, canonical corruption or immutable conflict error.
    pub fn transact_generation_resource_phase_v1<E>(
        &mut self,
        input: GenerationResourcePhaseV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<GenerationResourcePhaseV1Disposition, GenerationResourcePhaseV1TransactionError<E>>
    {
        use GenerationResourcePhaseV1TransactionError as Error;
        gate().map_err(Error::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        gate().map_err(Error::Gate)?;
        closure::validate(&transaction, input).map_err(Error::Store)?;
        let ordered_results = input
            .ordered_results
            .iter()
            .map(|record| {
                super::resource_attempt_result::write_resource_result(&transaction, record)
            })
            .collect::<StoreResult<Vec<_>>>()
            .map_err(Error::Store)?;
        let m = input.manifest;
        let bytes = serde_json::to_vec(m)
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        let count = transaction.execute(
            "INSERT OR IGNORE INTO generation_resource_evidence_manifests (generation_resource_evidence_manifest_id, generation_system_id, generation_qualification_plan_id, generation_suite_manifest_id, evidence_item_count, status, canonical_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![m.resource_evidence_manifest_id().digest().as_str(), m.generation_system_id().digest().as_str(), m.generation_qualification_plan_id().digest().as_str(), m.suite_manifest_id().digest().as_str(), i64::from(m.evidence_item_count()), status(m), bytes],
        ).map_err(StoreError::from).map_err(Error::Store)?;
        if load(&transaction, input).map_err(Error::Store)?.is_none() {
            return Err(Error::Store(StoreError::ImmutableConflict));
        }
        gate().map_err(Error::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(Error::Store)?;
        Ok(GenerationResourcePhaseV1Disposition {
            ordered_results,
            manifest: if count == 1 {
                WriteDisposition::Inserted
            } else {
                WriteDisposition::AlreadyPresent
            },
        })
    }

    /// Cold-reads an inert phase within one full parent snapshot.
    ///
    /// # Errors
    /// Rejects partial, reordered, substituted or noncanonical parent/result rows.
    pub fn generation_resource_phase_v1(
        &self,
        input: GenerationResourcePhaseV1Input<'_>,
    ) -> StoreResult<Option<StoredGenerationResourcePhaseV1>> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let result = load(&transaction, input)?;
        transaction.commit()?;
        if self.connection.total_changes() != before {
            return Err(StoreError::CorruptRecord);
        }
        Ok(result)
    }
}

fn status(manifest: &GenerationResourceEvidenceManifestV1) -> &'static str {
    use rewrite_model::GenerationQualificationPhaseStatusV1 as Status;
    match manifest.status() {
        Status::Passed => "passed",
        Status::Failed => "failed",
        Status::Skipped => "skipped",
    }
}

fn load(
    connection: &Connection,
    input: GenerationResourcePhaseV1Input<'_>,
) -> StoreResult<Option<StoredGenerationResourcePhaseV1>> {
    closure::validate(connection, input)?;
    let m = input.manifest;
    let expected = serde_json::to_vec(m)?;
    let row = connection.query_row(
        "SELECT generation_resource_evidence_manifest_id, generation_system_id, generation_qualification_plan_id, generation_suite_manifest_id, evidence_item_count, status, typeof(canonical_json), length(canonical_json), CAST(substr(canonical_json, 1, ?4) AS BLOB) FROM generation_resource_evidence_manifests WHERE generation_resource_evidence_manifest_id = ?1 OR (generation_qualification_plan_id = ?2 AND generation_system_id = ?3)",
        params![m.resource_evidence_manifest_id().digest().as_str(), m.generation_qualification_plan_id().digest().as_str(), m.generation_system_id().digest().as_str(), i64::try_from(rewrite_model::MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES).unwrap_or(i64::MAX)],
        |r| Ok((r.get::<_, String>(0)?,r.get::<_, String>(1)?,r.get::<_, String>(2)?,r.get::<_, String>(3)?,r.get::<_, i64>(4)?,r.get::<_, String>(5)?,r.get::<_, String>(6)?,r.get::<_, i64>(7)?,r.get::<_, Vec<u8>>(8)?)),
    ).optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.0 != m.resource_evidence_manifest_id().digest().as_str()
        || row.1 != m.generation_system_id().digest().as_str()
        || row.2 != m.generation_qualification_plan_id().digest().as_str()
        || row.3 != m.suite_manifest_id().digest().as_str()
        || row.4 != i64::from(m.evidence_item_count())
        || row.5 != status(m)
        || row.6 != "blob"
        || row.7 != i64::try_from(expected.len()).unwrap_or(i64::MAX)
        || row.8 != expected
    {
        return Err(StoreError::CorruptRecord);
    }
    for record in input.ordered_results {
        if super::resource_attempt_result::load_resource_result(connection, record)?.as_ref()
            != Some(record)
        {
            return Err(StoreError::MissingRecord);
        }
    }
    let count = connection.query_row("SELECT count(*) FROM generation_resource_attempt_result_records WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2", params![m.generation_qualification_plan_id().digest().as_str(),m.generation_system_id().digest().as_str()], |r|r.get::<_,i64>(0))?;
    if count != i64::try_from(input.ordered_results.len()).unwrap_or(i64::MAX) {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(StoredGenerationResourcePhaseV1 {
        ordered_results: input.ordered_results.to_vec(),
        manifest: m.clone(),
    }))
}
