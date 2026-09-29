//! Store-only writers for schema 13 phase-policy denials.
//!
//! Each denial is one immediate transaction. The resource-attempt result table
//! has no writer in this module. A stored denial grants no qualification,
//! activation, or live-use authority.

use std::{error::Error, fmt};

use rewrite_model::{
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations,
    GenerationResourcePolicyDenialRecordV1, GenerationResourcePolicyDenialRecordV1Relations,
};
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

mod parents;
mod read;
mod record;
mod write;

use record::DenialRecord;

/// Borrowed resource-policy denial and the relations that must revalidate it.
#[derive(Clone, Copy)]
pub struct GenerationResourcePolicyDenialV1Input<'a> {
    /// Exact denial to store.
    pub record: &'a GenerationResourcePolicyDenialRecordV1,
    /// Exact scope and phase-policy digest for that denial.
    pub relations: GenerationResourcePolicyDenialRecordV1Relations<'a>,
}

/// Scope and phase-policy digest used to reload one resource-policy denial.
#[derive(Clone, Copy)]
pub struct GenerationResourcePolicyDenialV1ReadInput<'a> {
    /// Exact scope and phase-policy digest.
    pub relations: GenerationResourcePolicyDenialRecordV1Relations<'a>,
}

/// Borrowed human-adjudication denial and the relations that must revalidate it.
#[derive(Clone, Copy)]
pub struct GenerationHumanAdjudicationPolicyDenialV1Input<'a> {
    /// Exact denial to store.
    pub record: &'a GenerationHumanAdjudicationPolicyDenialRecordV1,
    /// Exact scope and phase-policy digest for that denial.
    pub relations: GenerationHumanAdjudicationPolicyDenialRecordV1Relations<'a>,
}

/// Scope and phase-policy digest used to reload one human-adjudication denial.
#[derive(Clone, Copy)]
pub struct GenerationHumanAdjudicationPolicyDenialV1ReadInput<'a> {
    /// Exact scope and phase-policy digest.
    pub relations: GenerationHumanAdjudicationPolicyDenialRecordV1Relations<'a>,
}

/// Failure of one phase-policy denial transaction.
pub enum PhasePolicyDenialV1TransactionError<E> {
    /// Durable storage or relationship validation failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for PhasePolicyDenialV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "PhasePolicyDenialV1TransactionError::Store",
            Self::Gate(_) => "PhasePolicyDenialV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for PhasePolicyDenialV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "phase policy denial storage failed",
            Self::Gate(_) => "phase policy denial gate rejected the commit",
        })
    }
}

impl<E> Error for PhasePolicyDenialV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Stores one resource-policy denial and cold-revalidates that row.
    ///
    /// The plan foundation is cold-read on the open immediate transaction before
    /// the denial is inserted. Gates run before lock acquisition, after
    /// acquisition, and before commit. The stored row grants no qualification,
    /// activation, or live-use authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing dependency, corrupt
    /// row, immutable conflict, or commit failure. No qualification record is
    /// written.
    pub fn transact_generation_resource_policy_denial_v1<E>(
        &mut self,
        input: GenerationResourcePolicyDenialV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, PhasePolicyDenialV1TransactionError<E>> {
        self.transact(input.record, input.relations, gate)
    }

    /// Cold-reads one resource-policy denial.
    ///
    /// `Ok(None)` means the foundation loaded and no denial row matches that
    /// scope and phase-policy digest. A present row that does not revalidate is
    /// [`StoreError::CorruptRecord`]. The read does not modify durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a foundation is missing, the plan suite or
    /// plan-system pair disagrees, stored bytes disagree, or the read modifies
    /// the database.
    pub fn generation_resource_policy_denial_v1(
        &self,
        input: GenerationResourcePolicyDenialV1ReadInput<'_>,
    ) -> Result<Option<GenerationResourcePolicyDenialRecordV1>, StoreError> {
        self.read_denial(input.relations)
    }

    /// Stores one human-adjudication-policy denial and cold-revalidates that row.
    ///
    /// The transaction shape matches
    /// [`Self::transact_generation_resource_policy_denial_v1`]. The two denial
    /// tables do not deduplicate each other.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing dependency, corrupt
    /// row, immutable conflict, or commit failure. No qualification record is
    /// written.
    pub fn transact_generation_human_adjudication_policy_denial_v1<E>(
        &mut self,
        input: GenerationHumanAdjudicationPolicyDenialV1Input<'_>,
        gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, PhasePolicyDenialV1TransactionError<E>> {
        self.transact(input.record, input.relations, gate)
    }

    /// Cold-reads one human-adjudication-policy denial.
    ///
    /// `Ok(None)` means the foundation loaded and no denial row matches that
    /// scope and phase-policy digest.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a foundation is missing, the plan suite or
    /// plan-system pair disagrees, stored bytes disagree, or the read modifies
    /// the database.
    pub fn generation_human_adjudication_policy_denial_v1(
        &self,
        input: GenerationHumanAdjudicationPolicyDenialV1ReadInput<'_>,
    ) -> Result<Option<GenerationHumanAdjudicationPolicyDenialRecordV1>, StoreError> {
        self.read_denial(input.relations)
    }

    fn transact<R, E>(
        &mut self,
        record: &R,
        relations: R::Relations<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<WriteDisposition, PhasePolicyDenialV1TransactionError<E>>
    where
        R: DenialRecord,
    {
        gate().map_err(PhasePolicyDenialV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(PhasePolicyDenialV1TransactionError::Store)?;
        gate().map_err(PhasePolicyDenialV1TransactionError::Gate)?;
        let disposition = write::write_one(&transaction, record, relations)
            .map_err(PhasePolicyDenialV1TransactionError::Store)?;
        let stored: Option<R> = read::load(&transaction, relations)
            .map_err(PhasePolicyDenialV1TransactionError::Store)?;
        match stored {
            Some(value) if &value == record => {}
            Some(_) => {
                return Err(PhasePolicyDenialV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(PhasePolicyDenialV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(PhasePolicyDenialV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(PhasePolicyDenialV1TransactionError::Store)?;
        Ok(disposition)
    }

    fn read_denial<R: DenialRecord>(
        &self,
        relations: R::Relations<'_>,
    ) -> Result<Option<R>, StoreError> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load(&transaction, relations)?;
        transaction.commit()?;
        if self.connection.total_changes() == before {
            Ok(stored)
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

#[cfg(test)]
#[path = "phase_policy_denial/tests.rs"]
mod tests;
