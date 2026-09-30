//! Store-only writer for one schema 19 generation-qualification selection.
//!
//! The selection is one immediate transaction. The named qualification record
//! must already exist. This module does not construct the selection, change
//! the qualification row, or write invalidation, activation, or live-use
//! authority.

use std::{error::Error, fmt};

use rewrite_model::GenerationQualificationSelectionV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

/// Borrowed selection to store.
#[derive(Clone, Copy)]
pub struct GenerationQualificationSelectionV1Input<'a> {
    /// Exact selection to store.
    pub selection: &'a GenerationQualificationSelectionV1,
}

/// Borrowed selection used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct GenerationQualificationSelectionV1ReadInput<'a> {
    /// Exact selection whose qualification identity selects the row.
    pub selection: &'a GenerationQualificationSelectionV1,
}

/// Failure of one selection transaction.
pub enum GenerationQualificationSelectionV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for GenerationQualificationSelectionV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationQualificationSelectionV1TransactionError::Store",
            Self::Gate(_) => "GenerationQualificationSelectionV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for GenerationQualificationSelectionV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation qualification selection storage failed",
            Self::Gate(_) => "generation qualification selection gate rejected the commit",
        })
    }
}

impl<E> Error for GenerationQualificationSelectionV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one selection and confirms that row before commit.
    ///
    /// The caller supplies the selection. Gates run before lock acquisition,
    /// after acquisition, and before commit. A stored row does not change the
    /// qualification record and grants no qualification, activation, or
    /// live-use authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing qualification,
    /// corrupt row, immutable conflict, or commit failure. No invalidation or
    /// activation record is written.
    pub fn transact_generation_qualification_selection_v1<E>(
        &mut self,
        input: GenerationQualificationSelectionV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationSelectionV1TransactionError<E>> {
        self.transact_qualification_selection(input.selection, gate)
    }

    /// Confirms one stored selection against the caller record.
    ///
    /// `Ok(None)` means the qualification slot and content identity are absent.
    /// A stored row that disagrees is [`StoreError::CorruptRecord`]. The read
    /// does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the qualification is missing, stored rows
    /// disagree, canonical bytes disagree, or the read modifies the database.
    pub fn generation_qualification_selection_v1(
        &self,
        input: GenerationQualificationSelectionV1ReadInput<'_>,
    ) -> Result<Option<GenerationQualificationSelectionV1>, StoreError> {
        self.read_qualification_selection(input.selection)
    }

    fn transact_qualification_selection<E>(
        &mut self,
        selection: &GenerationQualificationSelectionV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationSelectionV1TransactionError<E>> {
        gate().map_err(GenerationQualificationSelectionV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationQualificationSelectionV1TransactionError::Store)?;
        gate().map_err(GenerationQualificationSelectionV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, selection)
            .map_err(GenerationQualificationSelectionV1TransactionError::Store)?;
        let stored = read::load(&transaction, selection)
            .map_err(GenerationQualificationSelectionV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == selection => {}
            Some(_) => {
                return Err(GenerationQualificationSelectionV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(GenerationQualificationSelectionV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(GenerationQualificationSelectionV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationQualificationSelectionV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_qualification_selection(
        &self,
        selection: &GenerationQualificationSelectionV1,
    ) -> Result<Option<GenerationQualificationSelectionV1>, StoreError> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load(&transaction, selection)?;
        transaction.commit()?;
        if self.connection.total_changes() == before {
            Ok(stored)
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

#[cfg(test)]
#[path = "generation_qualification_selection/tests.rs"]
mod tests;
