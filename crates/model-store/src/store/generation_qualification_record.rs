//! Store-only writer for one schema 17 generation-qualification record.
//!
//! The record is one immediate transaction. Parent rows must already exist and
//! agree with the caller record. Phase facts stay inside the operation receipt
//! and canonical JSON. This module does not construct the record or write
//! invalidation, selection, activation, or live-use authority.

use std::{error::Error, fmt};

use rewrite_model::GenerationQualificationRecordV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

/// Borrowed qualification record to store.
#[derive(Clone, Copy)]
pub struct GenerationQualificationRecordV1Input<'a> {
    /// Exact qualification record to store.
    pub record: &'a GenerationQualificationRecordV1,
}

/// Borrowed qualification record used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct GenerationQualificationRecordV1ReadInput<'a> {
    /// Exact qualification record whose operation receipt selects the row.
    pub record: &'a GenerationQualificationRecordV1,
}

/// Failure of one qualification-record transaction.
pub enum GenerationQualificationRecordV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for GenerationQualificationRecordV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationQualificationRecordV1TransactionError::Store",
            Self::Gate(_) => "GenerationQualificationRecordV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for GenerationQualificationRecordV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation qualification record storage failed",
            Self::Gate(_) => "generation qualification record gate rejected the commit",
        })
    }
}

impl<E> Error for GenerationQualificationRecordV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one qualification record and confirms that row before commit.
    ///
    /// The caller supplies the record. Gates run before lock acquisition, after
    /// acquisition, and before commit. A stored row grants no qualification,
    /// activation, or live-use authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing parent, corrupt row,
    /// immutable conflict, or commit failure. No activation record is written.
    pub fn transact_generation_qualification_record_v1<E>(
        &mut self,
        input: GenerationQualificationRecordV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationRecordV1TransactionError<E>> {
        self.transact_qualification_record(input.record, gate)
    }

    /// Confirms one stored qualification record against the caller record.
    ///
    /// `Ok(None)` means the receipt slot and content identity are absent. A
    /// stored row that disagrees is [`StoreError::CorruptRecord`]. The read
    /// does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a parent is missing, stored rows disagree,
    /// canonical bytes disagree, or the read modifies the database.
    pub fn generation_qualification_record_v1(
        &self,
        input: GenerationQualificationRecordV1ReadInput<'_>,
    ) -> Result<Option<GenerationQualificationRecordV1>, StoreError> {
        self.read_qualification_record(input.record)
    }

    fn transact_qualification_record<E>(
        &mut self,
        record: &GenerationQualificationRecordV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationRecordV1TransactionError<E>> {
        gate().map_err(GenerationQualificationRecordV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationQualificationRecordV1TransactionError::Store)?;
        gate().map_err(GenerationQualificationRecordV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, record)
            .map_err(GenerationQualificationRecordV1TransactionError::Store)?;
        let stored = read::load(&transaction, record)
            .map_err(GenerationQualificationRecordV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == record => {}
            Some(_) => {
                return Err(GenerationQualificationRecordV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(GenerationQualificationRecordV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(GenerationQualificationRecordV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationQualificationRecordV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_qualification_record(
        &self,
        record: &GenerationQualificationRecordV1,
    ) -> Result<Option<GenerationQualificationRecordV1>, StoreError> {
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
#[path = "generation_qualification_record/tests.rs"]
pub(crate) mod tests;
