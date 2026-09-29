//! Atomic terminal persistence for one candidate-generation attempt.

use std::{error::Error, fmt};

use rewrite_model::{
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationAttemptRecordV1,
    CandidateGenerationCleanupRecordV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationReceiptV1,
    GenerationAttemptLedgerManifestV1, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input, PlannedCandidateAttemptId,
};
use rusqlite::TransactionBehavior;

use super::candidate_generation_evidence_storage::CandidateGenerationEvidenceBundleStorageV1;
use super::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationReadInput, StoredGenerationQualificationPreregistration,
};
use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

mod codec;
mod read;
mod write;

pub(crate) use read::{RederivedAttemptLedger, rederive_attempt_ledger};

/// Exact inert records for one terminal candidate-generation outcome.
#[derive(Clone, Copy)]
pub enum CandidateGenerationExecutionV1Input<'a> {
    /// A terminal failed attempt with an optional previously checkpointed precursor.
    Failed {
        /// Exact schema 9 preregistration closure.
        preregistration: GenerationQualificationPreregistrationReadInput<'a>,
        /// Precursor when execution passed precursor compilation.
        precursor: Option<&'a CandidateGenerationAttemptPrecursorV1>,
        /// Exact failed terminal record.
        attempt: &'a CandidateGenerationAttemptRecordV1,
    },
    /// A cleanup, publication, readback, and receipt complete attempt.
    Completed {
        /// Exact schema 9 preregistration closure.
        preregistration: GenerationQualificationPreregistrationReadInput<'a>,
        /// Previously checkpointed precursor.
        precursor: &'a CandidateGenerationAttemptPrecursorV1,
        /// Managed execution evidence.
        managed_evidence: &'a ManagedOllamaCandidateGenerationEvidenceV2,
        /// Independently retained upper-layer managed-evidence facts.
        managed_evidence_input: &'a ManagedOllamaCandidateGenerationEvidenceV2Input,
        /// Successful cleanup record.
        cleanup: &'a CandidateGenerationCleanupRecordV1,
        /// Published bundle manifest.
        bundle: &'a CandidateGenerationEvidenceBundleManifestV1,
        /// Root-bound local bundle reference.
        storage: &'a CandidateGenerationEvidenceBundleStorageV1,
        /// Fresh bundle readback.
        readback: &'a CandidateGenerationEvidenceBundleReadbackV1,
        /// Cleanup-and-readback-gated receipt.
        receipt: &'a CandidateGenerationReceiptV1,
        /// Exact completed terminal record.
        attempt: &'a CandidateGenerationAttemptRecordV1,
    },
}

impl<'a> CandidateGenerationExecutionV1Input<'a> {
    fn preregistration(self) -> GenerationQualificationPreregistrationReadInput<'a> {
        match self {
            Self::Failed {
                preregistration, ..
            }
            | Self::Completed {
                preregistration, ..
            } => preregistration,
        }
    }

    fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        match self {
            Self::Failed { attempt, .. } | Self::Completed { attempt, .. } => {
                codec::attempt_planned_id(attempt)
            }
        }
    }
}

/// Independently retained inputs required to cold-read one expected outcome.
#[derive(Clone, Copy)]
pub enum CandidateGenerationExecutionV1ReadInput<'a> {
    /// Expect one failed terminal record.
    Failed {
        /// Exact schema 9 preregistration closure.
        preregistration: GenerationQualificationPreregistrationReadInput<'a>,
        /// Exact planned attempt to read.
        planned_attempt_id: &'a PlannedCandidateAttemptId,
    },
    /// Expect one completed terminal closure.
    Completed {
        /// Exact schema 9 preregistration closure.
        preregistration: GenerationQualificationPreregistrationReadInput<'a>,
        /// Exact planned attempt to read.
        planned_attempt_id: &'a PlannedCandidateAttemptId,
        /// Independently retained upper-layer managed-evidence facts.
        managed_evidence_input: &'a ManagedOllamaCandidateGenerationEvidenceV2Input,
    },
}

impl<'a> CandidateGenerationExecutionV1ReadInput<'a> {
    pub(super) fn preregistration(self) -> GenerationQualificationPreregistrationReadInput<'a> {
        match self {
            Self::Failed {
                preregistration, ..
            }
            | Self::Completed {
                preregistration, ..
            } => preregistration,
        }
    }

    pub(super) const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        match self {
            Self::Failed {
                planned_attempt_id, ..
            }
            | Self::Completed {
                planned_attempt_id, ..
            } => planned_attempt_id,
        }
    }
}

/// Owned inert records from one recursively revalidated terminal outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "cold read returns the complete exact inert terminal closure"
)]
pub enum StoredCandidateGenerationExecutionV1 {
    /// Failed attempt closure.
    Failed {
        /// Exact preregistration closure.
        preregistration: StoredGenerationQualificationPreregistration,
        /// Exact precursor when present.
        precursor: Option<CandidateGenerationAttemptPrecursorV1>,
        /// Exact failed attempt record.
        attempt: CandidateGenerationAttemptRecordV1,
    },
    /// Completed attempt closure.
    Completed {
        /// Exact preregistration closure.
        preregistration: StoredGenerationQualificationPreregistration,
        /// Exact precursor.
        precursor: CandidateGenerationAttemptPrecursorV1,
        /// Exact managed evidence.
        managed_evidence: ManagedOllamaCandidateGenerationEvidenceV2,
        /// Exact successful cleanup record.
        cleanup: CandidateGenerationCleanupRecordV1,
        /// Exact bundle manifest.
        bundle: CandidateGenerationEvidenceBundleManifestV1,
        /// Exact root-bound local storage registration.
        storage: CandidateGenerationEvidenceBundleStorageV1,
        /// Exact verified readback record.
        readback: CandidateGenerationEvidenceBundleReadbackV1,
        /// Exact receipt.
        receipt: CandidateGenerationReceiptV1,
        /// Exact completed attempt record.
        attempt: CandidateGenerationAttemptRecordV1,
    },
}

/// Typed records borrowed only while the terminal transaction is staged.
pub struct CandidateGenerationExecutionV1Readback<'a> {
    execution: &'a StoredCandidateGenerationExecutionV1,
}

impl CandidateGenerationExecutionV1Readback<'_> {
    /// Returns the complete inert terminal closure.
    #[must_use]
    pub const fn execution(&self) -> &StoredCandidateGenerationExecutionV1 {
        self.execution
    }
}

/// Per-record outcome of one atomic terminal write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateGenerationExecutionV1WriteDisposition {
    /// Existing precursor participation, when applicable.
    pub precursor: Option<WriteDisposition>,
    /// Managed evidence write, completed outcomes only.
    pub managed_evidence: Option<WriteDisposition>,
    /// Cleanup write, completed outcomes only.
    pub cleanup: Option<WriteDisposition>,
    /// Bundle-manifest write, completed outcomes only.
    pub bundle: Option<WriteDisposition>,
    /// Storage-registration write, completed outcomes only.
    pub storage: Option<WriteDisposition>,
    /// Readback write, completed outcomes only.
    pub readback: Option<WriteDisposition>,
    /// Receipt write, completed outcomes only.
    pub receipt: Option<WriteDisposition>,
    /// Terminal attempt-record write.
    pub attempt: WriteDisposition,
}

/// Failure of one atomic terminal candidate transaction.
pub enum CandidateGenerationExecutionV1TransactionError<E> {
    /// Durable storage or typed relationship validation failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
    /// The caller rejected the staged typed readback.
    Validation(E),
}

impl<E> fmt::Debug for CandidateGenerationExecutionV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "CandidateGenerationExecutionV1TransactionError::Store",
            Self::Gate(_) => "CandidateGenerationExecutionV1TransactionError::Gate",
            Self::Validation(_) => "CandidateGenerationExecutionV1TransactionError::Validation",
        })
    }
}

impl<E> fmt::Display for CandidateGenerationExecutionV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "candidate generation terminal storage failed",
            Self::Gate(_) => "candidate generation terminal gate rejected the commit",
            Self::Validation(_) => "candidate generation terminal readback validation failed",
        })
    }
}

impl<E> Error for CandidateGenerationExecutionV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically stores and cold-revalidates one terminal candidate outcome.
    ///
    /// The exact preregistration is cold-read under the same immediate write lock.
    /// Gates run before lock acquisition, after acquisition, and before commit.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate or callback, missing or corrupt
    /// dependencies, immutable conflict, invalid relationship, or commit failure.
    pub fn transact_candidate_generation_execution_v1<T, E>(
        &mut self,
        input: CandidateGenerationExecutionV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
        validate: impl for<'readback> FnOnce(
            CandidateGenerationExecutionV1Readback<'readback>,
        ) -> Result<T, E>,
    ) -> Result<
        (T, CandidateGenerationExecutionV1WriteDisposition),
        CandidateGenerationExecutionV1TransactionError<E>,
    > {
        gate().map_err(CandidateGenerationExecutionV1TransactionError::Gate)?;
        let encoded =
            codec::encode(input).map_err(CandidateGenerationExecutionV1TransactionError::Store)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(CandidateGenerationExecutionV1TransactionError::Store)?;
        gate().map_err(CandidateGenerationExecutionV1TransactionError::Gate)?;
        codec::validate_input(&transaction, input)
            .map_err(CandidateGenerationExecutionV1TransactionError::Store)?;
        let disposition = write::write_execution(&transaction, input, &encoded)
            .map_err(CandidateGenerationExecutionV1TransactionError::Store)?;
        let stored = read::load_execution(&transaction, read::read_input(input))
            .map_err(CandidateGenerationExecutionV1TransactionError::Store)?
            .ok_or(StoreError::CorruptRecord)
            .map_err(CandidateGenerationExecutionV1TransactionError::Store)?;
        if !codec::matches_input(&stored, input) {
            return Err(CandidateGenerationExecutionV1TransactionError::Store(
                StoreError::ImmutableConflict,
            ));
        }
        let output = validate(CandidateGenerationExecutionV1Readback { execution: &stored })
            .map_err(CandidateGenerationExecutionV1TransactionError::Validation)?;
        gate().map_err(CandidateGenerationExecutionV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(CandidateGenerationExecutionV1TransactionError::Store)?;
        Ok((output, disposition))
    }

    /// Cold-reads one complete expected terminal outcome.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for missing dependencies, an outcome mismatch,
    /// oversized storage, corruption, or any substituted relationship.
    pub fn candidate_generation_execution_v1(
        &self,
        input: CandidateGenerationExecutionV1ReadInput<'_>,
    ) -> StoreResult<Option<StoredCandidateGenerationExecutionV1>> {
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load_execution(&transaction, input)?;
        transaction.commit()?;
        Ok(stored)
    }

    /// Rebuilds the target attempt-ledger manifest from durable attempt rows.
    ///
    /// The read does not insert, update, or delete. A completed attempt requires
    /// the same independently retained managed-evidence facts as its cold read.
    /// The manifest comes from the model constructor and grants no live authority.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when preregistration is missing, a stored attempt is
    /// outside the target prefix, a completed attempt lacks its managed-evidence
    /// input, or the derived manifest is inconsistent.
    pub fn rederive_attempt_ledger_manifest_v1(
        &self,
        preregistration: GenerationQualificationPreregistrationReadInput<'_>,
        managed_evidence_inputs: &[ManagedOllamaCandidateGenerationEvidenceV2Input],
    ) -> StoreResult<GenerationAttemptLedgerManifestV1> {
        let transaction = self.connection.unchecked_transaction()?;
        let rederived =
            read::rederive_attempt_ledger(&transaction, preregistration, managed_evidence_inputs)?;
        transaction.commit()?;
        Ok(rederived.manifest)
    }
}

#[cfg(test)]
#[path = "candidate_generation_execution/tests.rs"]
pub(crate) mod tests;
