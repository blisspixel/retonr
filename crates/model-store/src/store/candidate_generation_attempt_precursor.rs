//! Atomic pretraffic checkpoint for one planned candidate-generation attempt.

use std::{error::Error, fmt};

use rewrite_model::{
    CandidateGenerationAttemptPrecursorV1, GenerationQualificationPlanId, GenerationSystemRecordV1,
    MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES, PlannedCandidateAttemptId,
};
use rusqlite::{Connection, OptionalExtension as _, TransactionBehavior, params};

use super::generation_qualification_plan_foundation::GenerationQualificationPlanFoundationV1;
use super::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationReadInput, StoredGenerationQualificationPreregistration,
    load_preregistration,
};
use super::generation_system_foundation::read::read_bounded_blob;
use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

/// Exact inert input required to checkpoint one attempt before traffic.
#[derive(Clone, Copy)]
pub struct CandidateGenerationAttemptPrecursorCheckpointV1Input<'a> {
    /// Complete prelaunch precursor.
    pub precursor: &'a CandidateGenerationAttemptPrecursorV1,
    /// Independently retained preregistration relationships and inputs.
    pub preregistration: GenerationQualificationPreregistrationReadInput<'a>,
}

/// Typed records borrowed only while the checkpoint transaction is staged.
pub struct CandidateGenerationAttemptPrecursorCheckpointV1Readback<'a> {
    preregistration: &'a StoredGenerationQualificationPreregistration,
    precursor: &'a CandidateGenerationAttemptPrecursorV1,
}

impl CandidateGenerationAttemptPrecursorCheckpointV1Readback<'_> {
    /// Returns the recursively revalidated preregistration closure.
    #[must_use]
    pub const fn preregistration(&self) -> &StoredGenerationQualificationPreregistration {
        self.preregistration
    }

    /// Returns the cold-decoded inert precursor checkpoint.
    #[must_use]
    pub const fn precursor(&self) -> &CandidateGenerationAttemptPrecursorV1 {
        self.precursor
    }
}

/// Outcome of one pretraffic checkpoint write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateGenerationAttemptPrecursorCheckpointV1WriteDisposition {
    /// Whether the exact checkpoint was newly inserted or already present.
    pub precursor: WriteDisposition,
}

/// Failure of one atomic pretraffic checkpoint transaction.
pub enum CandidateGenerationAttemptPrecursorCheckpointV1TransactionError<E> {
    /// Durable storage or typed validation failed.
    Store(StoreError),
    /// The caller's cancellation or deadline gate rejected the operation.
    Gate(E),
    /// The caller rejected the typed staged readback.
    Validation(E),
}

impl<E> fmt::Debug for CandidateGenerationAttemptPrecursorCheckpointV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => {
                "CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store"
            }
            Self::Gate(_) => {
                "CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Gate"
            }
            Self::Validation(_) => {
                "CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Validation"
            }
        })
    }
}

impl<E> fmt::Display for CandidateGenerationAttemptPrecursorCheckpointV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "candidate generation precursor checkpoint storage failed",
            Self::Gate(_) => "candidate generation precursor checkpoint gate rejected the commit",
            Self::Validation(_) => {
                "candidate generation precursor checkpoint readback validation failed"
            }
        })
    }
}

impl<E> Error for CandidateGenerationAttemptPrecursorCheckpointV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically checkpoints and cold-revalidates one precursor before traffic.
    ///
    /// The exact schema 9 plan foundation and preregistration are cold-read under
    /// the same immediate write transaction. An exact replay is reported but does
    /// not by itself authorize another execution.
    ///
    /// The gate runs before lock acquisition, after acquisition, and immediately
    /// before commit.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate or callback, missing or corrupt
    /// preregistration, selection mismatch, immutable conflict, or commit failure.
    pub fn transact_candidate_generation_attempt_precursor_checkpoint_v1<T, E>(
        &mut self,
        input: CandidateGenerationAttemptPrecursorCheckpointV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
        validate: impl for<'readback> FnOnce(
            CandidateGenerationAttemptPrecursorCheckpointV1Readback<'readback>,
        ) -> Result<T, E>,
    ) -> Result<
        (
            T,
            CandidateGenerationAttemptPrecursorCheckpointV1WriteDisposition,
        ),
        CandidateGenerationAttemptPrecursorCheckpointV1TransactionError<E>,
    > {
        gate().map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Gate)?;
        let encoded = serde_json::to_vec(input.precursor)
            .map_err(StoreError::from)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        if encoded.is_empty()
            || encoded.len() > MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES
        {
            return Err(
                CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store(
                    StoreError::RecordTooLarge,
                ),
            );
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        gate().map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Gate)?;
        let preregistration = load_preregistration(&transaction, input.preregistration)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?
            .ok_or(StoreError::MissingRecord)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        let (attempt, system) = validate_selection(&preregistration, input.precursor)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        input
            .precursor
            .validate_against(preregistration.plan_foundation().plan(), attempt, system)
            .map_err(StoreError::InvalidCandidateGenerationAttemptPrecursor)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        let disposition = insert_or_verify(&transaction, input.precursor, &encoded)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        let stored = load_precursor(
            &transaction,
            preregistration.plan_foundation(),
            input.precursor.qualification_plan_id(),
            input.precursor.planned_attempt_id(),
        )
        .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?
        .ok_or(StoreError::CorruptRecord)
        .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        if stored != *input.precursor {
            return Err(
                CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ),
            );
        }
        let output = validate(CandidateGenerationAttemptPrecursorCheckpointV1Readback {
            preregistration: &preregistration,
            precursor: &stored,
        })
        .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Validation)?;
        gate().map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(CandidateGenerationAttemptPrecursorCheckpointV1TransactionError::Store)?;
        Ok((
            output,
            CandidateGenerationAttemptPrecursorCheckpointV1WriteDisposition {
                precursor: disposition,
            },
        ))
    }

    /// Reads and recursively revalidates one inert precursor checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for a missing plan foundation dependency, corrupt or
    /// oversized bytes, substituted index, or relationship mismatch.
    pub fn candidate_generation_attempt_precursor_checkpoint_v1(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
    ) -> StoreResult<Option<CandidateGenerationAttemptPrecursorV1>> {
        let transaction = self.connection.unchecked_transaction()?;
        let foundation = super::generation_qualification_plan_foundation::read::load_foundation(
            &transaction,
            plan_id,
        )?
        .ok_or(StoreError::MissingRecord)?;
        let value = load_precursor(&transaction, &foundation, plan_id, attempt_id)?;
        transaction.commit()?;
        Ok(value)
    }
}

fn validate_selection<'a>(
    preregistration: &'a StoredGenerationQualificationPreregistration,
    precursor: &CandidateGenerationAttemptPrecursorV1,
) -> StoreResult<(
    &'a rewrite_model::PlannedCandidateAttemptV1,
    &'a GenerationSystemRecordV1,
)> {
    let foundation = preregistration.plan_foundation();
    if precursor.qualification_plan_id() != foundation.plan().qualification_plan_id()
        || preregistration
            .request_projection()
            .generation_qualification_plan_id()
            != precursor.qualification_plan_id()
    {
        return Err(StoreError::InvalidCandidateGenerationAttemptPrecursor(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ));
    }
    let attempt = foundation
        .planned_attempts()
        .iter()
        .find(|attempt| attempt.planned_attempt_id() == precursor.planned_attempt_id())
        .ok_or(StoreError::InvalidCandidateGenerationAttemptPrecursor(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ))?;
    let system = foundation
        .generation_systems()
        .iter()
        .find(|system| system.generation_system_id() == attempt.generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let projection = preregistration
        .request_projection()
        .entries()
        .get(usize::try_from(attempt.attempt_ordinal()).map_err(|_| StoreError::CorruptRecord)?)
        .ok_or(StoreError::CorruptRecord)?;
    if projection.planned_attempt_id() != attempt.planned_attempt_id()
        || projection.generation_system_id() != system.generation_system_id()
        || projection.structured_completion_request_binding_id()
            != precursor.structured_request_binding_id()
    {
        return Err(StoreError::InvalidCandidateGenerationAttemptPrecursor(
            rewrite_model::GenerationQualificationContractError::PlannedAttemptMismatch,
        ));
    }
    Ok((attempt, system))
}

fn insert_or_verify(
    connection: &Connection,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    encoded: &[u8],
) -> StoreResult<WriteDisposition> {
    if let Some(stored) = load_indexed_row(connection, precursor.planned_attempt_id())? {
        return if stored.precursor_id == precursor.precursor_id().digest().as_str()
            && stored.plan_id == precursor.qualification_plan_id().digest().as_str()
            && stored.attempt_id == precursor.planned_attempt_id().digest().as_str()
            && stored.structured_request_binding_id
                == precursor.structured_request_binding_id().digest().as_str()
            && stored.canonical_json == encoded
        {
            Ok(WriteDisposition::AlreadyPresent)
        } else {
            Err(StoreError::ImmutableConflict)
        };
    }
    connection.execute(
        "INSERT INTO candidate_generation_attempt_precursors
             (candidate_generation_attempt_precursor_id,
              generation_qualification_plan_id,
              planned_candidate_attempt_id,
              structured_request_binding_id,
              canonical_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            precursor.precursor_id().digest().as_str(),
            precursor.qualification_plan_id().digest().as_str(),
            precursor.planned_attempt_id().digest().as_str(),
            precursor.structured_request_binding_id().digest().as_str(),
            encoded,
        ],
    )?;
    Ok(WriteDisposition::Inserted)
}

struct PrecursorRow {
    precursor_id: String,
    plan_id: String,
    attempt_id: String,
    structured_request_binding_id: String,
    canonical_json: Vec<u8>,
}

fn load_indexed_row(
    connection: &Connection,
    attempt_id: &PlannedCandidateAttemptId,
) -> StoreResult<Option<PrecursorRow>> {
    let row = connection
        .query_row(
            "SELECT candidate_generation_attempt_precursor_id,
                    generation_qualification_plan_id, planned_candidate_attempt_id,
                    structured_request_binding_id, typeof(canonical_json),
                    length(canonical_json)
             FROM candidate_generation_attempt_precursors
             WHERE planned_candidate_attempt_id = ?1",
            [attempt_id.digest().as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?;
    let Some((precursor_id, plan_id, stored_attempt_id, binding_id, kind, length)) = row else {
        return Ok(None);
    };
    for digest in [&precursor_id, &plan_id, &stored_attempt_id, &binding_id] {
        require_digest(digest)?;
    }
    let canonical_json = read_bounded_blob(
        connection,
        "candidate_generation_attempt_precursors",
        "candidate_generation_attempt_precursor_id",
        &precursor_id,
        &kind,
        length,
        MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES,
    )?;
    Ok(Some(PrecursorRow {
        precursor_id,
        plan_id,
        attempt_id: stored_attempt_id,
        structured_request_binding_id: binding_id,
        canonical_json,
    }))
}

pub(super) fn load_precursor(
    connection: &Connection,
    foundation: &GenerationQualificationPlanFoundationV1,
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
) -> StoreResult<Option<CandidateGenerationAttemptPrecursorV1>> {
    let Some(row) = load_indexed_row(connection, attempt_id)? else {
        return Ok(None);
    };
    if row.plan_id != plan_id.digest().as_str() || row.attempt_id != attempt_id.digest().as_str() {
        return Err(StoreError::CorruptRecord);
    }
    let attempt = foundation
        .planned_attempts()
        .iter()
        .find(|attempt| attempt.planned_attempt_id() == attempt_id)
        .ok_or(StoreError::CorruptRecord)?;
    let system = foundation
        .generation_systems()
        .iter()
        .find(|system| system.generation_system_id() == attempt.generation_system_id())
        .ok_or(StoreError::CorruptRecord)?;
    let precursor = CandidateGenerationAttemptPrecursorV1::from_json_bytes(
        &row.canonical_json,
        foundation.plan(),
        attempt,
        system,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    if precursor.precursor_id().digest().as_str() != row.precursor_id
        || precursor.structured_request_binding_id().digest().as_str()
            != row.structured_request_binding_id
        || serde_json::to_vec(&precursor)? != row.canonical_json
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(precursor))
}

fn require_digest(value: &str) -> StoreResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

#[cfg(test)]
#[path = "candidate_generation_attempt_precursor/tests.rs"]
mod tests;
