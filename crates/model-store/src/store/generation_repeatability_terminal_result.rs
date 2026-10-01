//! Store-only writer for one schema 16 repeatability terminal result.
//!
//! The result is one immediate transaction. Parent rows must already exist and
//! agree with the caller record. The attempt-ledger root and terminal evidence
//! digest stay inside canonical JSON. Candidate-generation failure remains on
//! schema 11. This module does not construct the record or write qualification,
//! activation, or live-use authority.

use std::{error::Error, fmt};

use rewrite_model::GenerationRepeatabilityResultRecordV1;
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod write;

pub(crate) fn load_repeatability_result(
    connection: &rusqlite::Connection,
    record: &GenerationRepeatabilityResultRecordV1,
) -> Result<Option<GenerationRepeatabilityResultRecordV1>, StoreError> {
    read::load(connection, record)
}

/// Borrowed repeatability result to store.
#[derive(Clone, Copy)]
pub struct GenerationRepeatabilityTerminalResultV1Input<'a> {
    /// Exact repeatability result to store.
    pub record: &'a GenerationRepeatabilityResultRecordV1,
}

/// Borrowed repeatability result used to confirm one stored row.
#[derive(Clone, Copy)]
pub struct GenerationRepeatabilityTerminalResultV1ReadInput<'a> {
    /// Exact repeatability result whose plan and repetition select the row.
    pub record: &'a GenerationRepeatabilityResultRecordV1,
}

/// Failure of one repeatability-result transaction.
pub enum GenerationRepeatabilityTerminalResultV1TransactionError<E> {
    /// Durable storage or parent agreement failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for GenerationRepeatabilityTerminalResultV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationRepeatabilityTerminalResultV1TransactionError::Store",
            Self::Gate(_) => "GenerationRepeatabilityTerminalResultV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for GenerationRepeatabilityTerminalResultV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation repeatability terminal result storage failed",
            Self::Gate(_) => "generation repeatability terminal result gate rejected the commit",
        })
    }
}

impl<E> Error for GenerationRepeatabilityTerminalResultV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one repeatability result and confirms that row before commit.
    ///
    /// The caller supplies the record. Gates run before lock acquisition, after
    /// acquisition, and before commit. A candidate-generation failure is rejected
    /// and is not written to schema 11.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing parent, corrupt row,
    /// immutable conflict, or commit failure. No qualification record is written.
    pub fn transact_generation_repeatability_terminal_result_v1<E>(
        &mut self,
        input: GenerationRepeatabilityTerminalResultV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationRepeatabilityTerminalResultV1TransactionError<E>> {
        self.transact_repeatability_result(input.record, gate)
    }

    /// Confirms one stored repeatability result against the caller record.
    ///
    /// `Ok(None)` means the plan-repetition slot, ledger slot, and content
    /// identity are all absent. A stored row that disagrees is
    /// [`StoreError::CorruptRecord`]. The read does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a parent is missing, stored rows disagree,
    /// canonical bytes disagree, or the read modifies the database.
    pub fn generation_repeatability_terminal_result_v1(
        &self,
        input: GenerationRepeatabilityTerminalResultV1ReadInput<'_>,
    ) -> Result<Option<GenerationRepeatabilityResultRecordV1>, StoreError> {
        self.read_repeatability_result(input.record)
    }

    fn transact_repeatability_result<E>(
        &mut self,
        record: &GenerationRepeatabilityResultRecordV1,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, GenerationRepeatabilityTerminalResultV1TransactionError<E>> {
        gate().map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Store)?;
        gate().map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, record)
            .map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Store)?;
        let stored = read::load(&transaction, record)
            .map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == record => {}
            Some(_) => {
                return Err(
                    GenerationRepeatabilityTerminalResultV1TransactionError::Store(
                        StoreError::ImmutableConflict,
                    ),
                );
            }
            None => {
                return Err(
                    GenerationRepeatabilityTerminalResultV1TransactionError::Store(
                        StoreError::CorruptRecord,
                    ),
                );
            }
        }
        gate().map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationRepeatabilityTerminalResultV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_repeatability_result(
        &self,
        record: &GenerationRepeatabilityResultRecordV1,
    ) -> Result<Option<GenerationRepeatabilityResultRecordV1>, StoreError> {
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
#[path = "generation_repeatability_terminal_result/tests.rs"]
mod tests;
