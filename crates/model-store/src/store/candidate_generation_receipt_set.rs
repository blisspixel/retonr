//! Store-only writer for one schema 14 candidate-generation receipt set.
//!
//! The receipt set is one immediate transaction. Parent rows must already exist
//! and agree with the caller record. Entries stay inside canonical JSON.
//! This module does not construct the record or write qualification, activation,
//! or live-use authority.

use std::{error::Error, fmt};

use rewrite_model::CandidateGenerationReceiptSetV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

/// Borrowed receipt set to store.
#[derive(Clone, Copy)]
pub struct CandidateGenerationReceiptSetV1Input<'a> {
    /// Exact receipt set to store.
    pub record: &'a CandidateGenerationReceiptSetV1,
}

/// Borrowed receipt set used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct CandidateGenerationReceiptSetV1ReadInput<'a> {
    /// Exact receipt set whose plan, repetition, and system select the row.
    pub record: &'a CandidateGenerationReceiptSetV1,
}

/// Failure of one receipt-set transaction.
pub enum CandidateGenerationReceiptSetV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for CandidateGenerationReceiptSetV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "CandidateGenerationReceiptSetV1TransactionError::Store",
            Self::Gate(_) => "CandidateGenerationReceiptSetV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for CandidateGenerationReceiptSetV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "candidate generation receipt set storage failed",
            Self::Gate(_) => "candidate generation receipt set gate rejected the commit",
        })
    }
}

impl<E> Error for CandidateGenerationReceiptSetV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one receipt set and confirms that row before commit.
    ///
    /// The caller supplies the record. Gates run before lock acquisition, after
    /// acquisition, and before commit. Parent rows are read on the open immediate
    /// transaction. Entries stay inside canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing parent, corrupt row,
    /// immutable conflict, or commit failure. No qualification record is written.
    pub fn transact_candidate_generation_receipt_set_v1<E>(
        &mut self,
        input: CandidateGenerationReceiptSetV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, CandidateGenerationReceiptSetV1TransactionError<E>> {
        self.transact_receipt_set(input.record, gate)
    }

    /// Confirms one stored receipt set against the caller record.
    ///
    /// `Ok(None)` means the cited parents loaded and no row occupies that plan,
    /// repetition, and system. A present row that disagrees is
    /// [`StoreError::CorruptRecord`]. Entries are not reconstructed from parent
    /// rows. The read does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a parent is missing, stored rows disagree,
    /// canonical bytes disagree, or the read modifies the database.
    pub fn candidate_generation_receipt_set_v1(
        &self,
        input: CandidateGenerationReceiptSetV1ReadInput<'_>,
    ) -> Result<Option<CandidateGenerationReceiptSetV1>, StoreError> {
        self.read_receipt_set(input.record)
    }

    fn transact_receipt_set<E>(
        &mut self,
        record: &CandidateGenerationReceiptSetV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, CandidateGenerationReceiptSetV1TransactionError<E>> {
        gate().map_err(CandidateGenerationReceiptSetV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(CandidateGenerationReceiptSetV1TransactionError::Store)?;
        gate().map_err(CandidateGenerationReceiptSetV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, record)
            .map_err(CandidateGenerationReceiptSetV1TransactionError::Store)?;
        let stored = read::load(&transaction, record)
            .map_err(CandidateGenerationReceiptSetV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == record => {}
            Some(_) => {
                return Err(CandidateGenerationReceiptSetV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(CandidateGenerationReceiptSetV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(CandidateGenerationReceiptSetV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(CandidateGenerationReceiptSetV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_receipt_set(
        &self,
        record: &CandidateGenerationReceiptSetV1,
    ) -> Result<Option<CandidateGenerationReceiptSetV1>, StoreError> {
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
#[path = "candidate_generation_receipt_set/tests.rs"]
mod tests;
