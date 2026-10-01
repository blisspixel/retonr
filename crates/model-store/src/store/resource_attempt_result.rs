//! Store-only writer for one schema 13 resource-attempt result.
//!
//! The result is one immediate transaction. Parent rows must already exist and
//! agree with the caller record. This module does not construct that record,
//! write a policy denial, or write a repeatability row. A stored result grants
//! no qualification, activation, or live-use authority.

use std::{error::Error, fmt};

use rewrite_model::GenerationResourceAttemptResultRecordV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

/// Borrowed resource-attempt result to store.
#[derive(Clone, Copy)]
pub struct GenerationResourceAttemptResultV1Input<'a> {
    /// Exact result to store.
    pub record: &'a GenerationResourceAttemptResultRecordV1,
}

/// Borrowed resource-attempt result used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct GenerationResourceAttemptResultV1ReadInput<'a> {
    /// Exact result whose plan and planned attempt select the row.
    pub record: &'a GenerationResourceAttemptResultRecordV1,
}

/// Failure of one resource-attempt result transaction.
pub enum GenerationResourceAttemptResultV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for GenerationResourceAttemptResultV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationResourceAttemptResultV1TransactionError::Store",
            Self::Gate(_) => "GenerationResourceAttemptResultV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for GenerationResourceAttemptResultV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "resource attempt result storage failed",
            Self::Gate(_) => "resource attempt result gate rejected the commit",
        })
    }
}

impl<E> Error for GenerationResourceAttemptResultV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one resource-attempt result and confirms that row before commit.
    ///
    /// The caller supplies the record. Gates run before lock acquisition, after
    /// acquisition, and before commit. Parent rows are read on the open immediate
    /// transaction. Counts and durations stay inside canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing parent, corrupt row,
    /// immutable conflict, or commit failure. No qualification record is written.
    pub fn transact_generation_resource_attempt_result_v1<E>(
        &mut self,
        input: GenerationResourceAttemptResultV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationResourceAttemptResultV1TransactionError<E>> {
        self.transact_result(input.record, gate)
    }

    /// Confirms one stored resource-attempt result against the caller record.
    ///
    /// `Ok(None)` means the cited parents loaded and no row occupies that plan
    /// and planned attempt. A present row that disagrees is
    /// [`StoreError::CorruptRecord`]. Observations are not reconstructed from
    /// parent rows. The read does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a parent is missing, stored rows disagree,
    /// canonical bytes disagree, or the read modifies the database.
    pub fn generation_resource_attempt_result_v1(
        &self,
        input: GenerationResourceAttemptResultV1ReadInput<'_>,
    ) -> Result<Option<GenerationResourceAttemptResultRecordV1>, StoreError> {
        self.read_result(input.record)
    }

    fn transact_result<E>(
        &mut self,
        record: &GenerationResourceAttemptResultRecordV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationResourceAttemptResultV1TransactionError<E>> {
        gate().map_err(GenerationResourceAttemptResultV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationResourceAttemptResultV1TransactionError::Store)?;
        gate().map_err(GenerationResourceAttemptResultV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, record)
            .map_err(GenerationResourceAttemptResultV1TransactionError::Store)?;
        let stored = read::load(&transaction, record)
            .map_err(GenerationResourceAttemptResultV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == record => {}
            Some(_) => {
                return Err(GenerationResourceAttemptResultV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(GenerationResourceAttemptResultV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(GenerationResourceAttemptResultV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationResourceAttemptResultV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_result(
        &self,
        record: &GenerationResourceAttemptResultRecordV1,
    ) -> Result<Option<GenerationResourceAttemptResultRecordV1>, StoreError> {
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
#[path = "resource_attempt_result/tests.rs"]
mod tests;

pub(crate) fn write_resource_result(
    connection: &rusqlite::Connection,
    record: &GenerationResourceAttemptResultRecordV1,
) -> crate::StoreResult<WriteDisposition> {
    write::write_one(connection, record)
}

pub(crate) fn load_resource_result(
    connection: &rusqlite::Connection,
    record: &GenerationResourceAttemptResultRecordV1,
) -> crate::StoreResult<Option<GenerationResourceAttemptResultRecordV1>> {
    read::load(connection, record)
}
