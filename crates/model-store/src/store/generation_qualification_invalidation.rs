//! Store-only writer for one schema 18 generation-qualification invalidation.
//!
//! The invalidation is one immediate transaction. The named qualification
//! record must already exist. This module does not construct the invalidation,
//! change the qualification row, or write selection, activation, or live-use
//! authority.

use std::{error::Error, fmt};

use rewrite_model::GenerationQualificationInvalidationV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

/// Borrowed invalidation to store.
#[derive(Clone, Copy)]
pub struct GenerationQualificationInvalidationV1Input<'a> {
    /// Exact invalidation to store.
    pub invalidation: &'a GenerationQualificationInvalidationV1,
}

/// Borrowed invalidation used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct GenerationQualificationInvalidationV1ReadInput<'a> {
    /// Exact invalidation whose qualification identity selects the row.
    pub invalidation: &'a GenerationQualificationInvalidationV1,
}

/// Failure of one invalidation transaction.
pub enum GenerationQualificationInvalidationV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for GenerationQualificationInvalidationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationQualificationInvalidationV1TransactionError::Store",
            Self::Gate(_) => "GenerationQualificationInvalidationV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for GenerationQualificationInvalidationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation qualification invalidation storage failed",
            Self::Gate(_) => "generation qualification invalidation gate rejected the commit",
        })
    }
}

impl<E> Error for GenerationQualificationInvalidationV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one invalidation and confirms that row before commit.
    ///
    /// The caller supplies the invalidation. Gates run before lock acquisition,
    /// after acquisition, and before commit. A stored row does not change the
    /// qualification record and grants no qualification, activation, or
    /// live-use authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing qualification,
    /// corrupt row, immutable conflict, or commit failure. No selection or
    /// activation record is written.
    pub fn transact_generation_qualification_invalidation_v1<E>(
        &mut self,
        input: GenerationQualificationInvalidationV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationInvalidationV1TransactionError<E>> {
        self.transact_qualification_invalidation(input.invalidation, gate)
    }

    /// Confirms one stored invalidation against the caller record.
    ///
    /// `Ok(None)` means the qualification slot and content identity are absent.
    /// A stored row that disagrees is [`StoreError::CorruptRecord`]. The read
    /// does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the qualification is missing, stored rows
    /// disagree, canonical bytes disagree, or the read modifies the database.
    pub fn generation_qualification_invalidation_v1(
        &self,
        input: GenerationQualificationInvalidationV1ReadInput<'_>,
    ) -> Result<Option<GenerationQualificationInvalidationV1>, StoreError> {
        self.read_qualification_invalidation(input.invalidation)
    }

    fn transact_qualification_invalidation<E>(
        &mut self,
        invalidation: &GenerationQualificationInvalidationV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationQualificationInvalidationV1TransactionError<E>> {
        gate().map_err(GenerationQualificationInvalidationV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationQualificationInvalidationV1TransactionError::Store)?;
        gate().map_err(GenerationQualificationInvalidationV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, invalidation)
            .map_err(GenerationQualificationInvalidationV1TransactionError::Store)?;
        let stored = read::load(&transaction, invalidation)
            .map_err(GenerationQualificationInvalidationV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == invalidation => {}
            Some(_) => {
                return Err(
                    GenerationQualificationInvalidationV1TransactionError::Store(
                        StoreError::ImmutableConflict,
                    ),
                );
            }
            None => {
                return Err(
                    GenerationQualificationInvalidationV1TransactionError::Store(
                        StoreError::CorruptRecord,
                    ),
                );
            }
        }
        gate().map_err(GenerationQualificationInvalidationV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationQualificationInvalidationV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_qualification_invalidation(
        &self,
        invalidation: &GenerationQualificationInvalidationV1,
    ) -> Result<Option<GenerationQualificationInvalidationV1>, StoreError> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load(&transaction, invalidation)?;
        transaction.commit()?;
        if self.connection.total_changes() == before {
            Ok(stored)
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

#[cfg(test)]
#[path = "generation_qualification_invalidation/tests.rs"]
mod tests;
