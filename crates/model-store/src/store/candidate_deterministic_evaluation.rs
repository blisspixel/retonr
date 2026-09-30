//! Store-only writer for one schema 15 deterministic evaluation record.
//!
//! The evaluation is one immediate transaction. Both receipt-set rows must
//! already exist and agree with the caller record. Report bytes, digests,
//! counts, coverage, and status stay inside canonical JSON.
//! This module does not construct the record or write qualification, activation,
//! or live-use authority.

use std::{error::Error, fmt};

use rewrite_model::CandidateDeterministicEvaluationRecordV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

/// Borrowed deterministic evaluation to store.
#[derive(Clone, Copy)]
pub struct CandidateDeterministicEvaluationV1Input<'a> {
    /// Exact evaluation record to store.
    pub record: &'a CandidateDeterministicEvaluationRecordV1,
}

/// Borrowed deterministic evaluation used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct CandidateDeterministicEvaluationV1ReadInput<'a> {
    /// Exact evaluation record whose content identity selects the row.
    pub record: &'a CandidateDeterministicEvaluationRecordV1,
}

/// Failure of one deterministic-evaluation transaction.
pub enum CandidateDeterministicEvaluationV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for CandidateDeterministicEvaluationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "CandidateDeterministicEvaluationV1TransactionError::Store",
            Self::Gate(_) => "CandidateDeterministicEvaluationV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for CandidateDeterministicEvaluationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "candidate deterministic evaluation storage failed",
            Self::Gate(_) => "candidate deterministic evaluation gate rejected the commit",
        })
    }
}

impl<E> Error for CandidateDeterministicEvaluationV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one deterministic evaluation and confirms that row before commit.
    ///
    /// The caller supplies the record. Gates run before lock acquisition, after
    /// acquisition, and before commit. Parent receipt sets are read on the open
    /// immediate transaction. Report bytes and aggregate facts stay inside
    /// canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing parent, corrupt row,
    /// immutable conflict, or commit failure. No qualification record is written.
    pub fn transact_candidate_deterministic_evaluation_v1<E>(
        &mut self,
        input: CandidateDeterministicEvaluationV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, CandidateDeterministicEvaluationV1TransactionError<E>> {
        self.transact_evaluation(input.record, gate)
    }

    /// Confirms one stored deterministic evaluation against the caller record.
    ///
    /// `Ok(None)` means both cited receipt sets loaded and no row has this
    /// content identity. A present row that disagrees is
    /// [`StoreError::CorruptRecord`]. Report bytes are not reconstructed from
    /// parent rows. The read does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a parent is missing, stored rows disagree,
    /// canonical bytes disagree, or the read modifies the database.
    pub fn candidate_deterministic_evaluation_v1(
        &self,
        input: CandidateDeterministicEvaluationV1ReadInput<'_>,
    ) -> Result<Option<CandidateDeterministicEvaluationRecordV1>, StoreError> {
        self.read_evaluation(input.record)
    }

    fn transact_evaluation<E>(
        &mut self,
        record: &CandidateDeterministicEvaluationRecordV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, CandidateDeterministicEvaluationV1TransactionError<E>> {
        gate().map_err(CandidateDeterministicEvaluationV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(CandidateDeterministicEvaluationV1TransactionError::Store)?;
        gate().map_err(CandidateDeterministicEvaluationV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, record)
            .map_err(CandidateDeterministicEvaluationV1TransactionError::Store)?;
        let stored = read::load(&transaction, record)
            .map_err(CandidateDeterministicEvaluationV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == record => {}
            Some(_) => {
                return Err(CandidateDeterministicEvaluationV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(CandidateDeterministicEvaluationV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(CandidateDeterministicEvaluationV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(CandidateDeterministicEvaluationV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_evaluation(
        &self,
        record: &CandidateDeterministicEvaluationRecordV1,
    ) -> Result<Option<CandidateDeterministicEvaluationRecordV1>, StoreError> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load(&transaction, record)?;
        transaction.commit()?;
        if self.connection.total_changes() == before {
            Ok(stored)
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

#[cfg(test)]
#[path = "candidate_deterministic_evaluation/tests.rs"]
mod tests;
