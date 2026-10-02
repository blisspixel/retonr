//! Atomic schema 11 terminal-evidence cohort for one preregistered operation.
//!
//! The stored cohort is inert. It grants no live, qualification, or activation
//! authority, and this module does not write a qualification record.

use std::{error::Error, fmt};

use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationLicenseEvidenceV1Input,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationPhaseInterruptionRecordV1Input,
    GenerationQualificationPlatformEvidenceV1, GenerationQualificationPlatformEvidenceV1Input,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityResultRecordV1,
    GenerationResourceEvidenceManifestV1, ManagedOllamaCandidateGenerationEvidenceV2Input,
    ModelLicenseControlId, ModelPackageFoundationId,
};
use rewrite_types::Digest;
use rusqlite::TransactionBehavior;

use super::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationReadInput, StoredGenerationQualificationPreregistration,
};
use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

mod codec;
mod context;
mod read;
mod resource_rejection;
mod rows;
mod text;
mod write;
pub use resource_rejection::GenerationResourceRejectionV1Input;

/// Borrowed terminal-evidence records and the independent facts that rederive them.
#[derive(Clone, Copy)]
pub struct GenerationQualificationTerminalEvidenceV1Input<'a> {
    /// Exact schema 9 preregistration closure.
    pub preregistration: GenerationQualificationPreregistrationReadInput<'a>,
    /// Exact platform evidence to store.
    pub platform_evidence: &'a GenerationQualificationPlatformEvidenceV1,
    /// Independently retained platform assessment.
    pub platform_input: GenerationQualificationPlatformEvidenceV1Input,
    /// Exact license evidence to store.
    pub license_evidence: &'a GenerationQualificationLicenseEvidenceV1,
    /// Independently retained license assessment.
    pub license_input: GenerationQualificationLicenseEvidenceV1Input,
    /// Stable model-package foundation named by the license assessment.
    pub model_package_foundation_id: &'a ModelPackageFoundationId,
    /// Exact model-license control named by the license assessment.
    pub model_license_control_id: &'a ModelLicenseControlId,
    /// Managed-evidence facts in target-attempt encounter order.
    pub managed_evidence_inputs: &'a [ManagedOllamaCandidateGenerationEvidenceV2Input],
    /// Exact attempt-ledger manifest to store.
    pub attempt_ledger_manifest: &'a GenerationAttemptLedgerManifestV1,
    /// Repeatability results in preregistered repetition order.
    pub repeatability_results: &'a [GenerationRepeatabilityResultRecordV1],
    /// Exact repeatability manifest to store.
    pub repeatability_manifest: &'a GenerationRepeatabilityEvidenceManifestV1,
    /// Exact resource manifest to store.
    pub resource_manifest: &'a GenerationResourceEvidenceManifestV1,
    /// Resource-evidence digests in semantic order.
    pub resource_evidence_record_digests: &'a [Digest],
    /// Exact human-adjudication manifest to store.
    pub human_manifest: &'a GenerationHumanAdjudicationEvidenceManifestV1,
    /// Human-adjudication digests in semantic order.
    pub human_evidence_record_digests: &'a [Digest],
    /// Exact operation receipt to store.
    pub receipt: &'a GenerationQualificationOperationReceiptV1,
    /// Independently retained runner terminal facts.
    pub receipt_input: GenerationQualificationOperationReceiptV1Input,
    /// Exact interruption record when the receipt is noncompleted.
    pub phase_interruption: Option<&'a GenerationQualificationPhaseInterruptionRecordV1>,
    /// Independently retained interruption facts.
    pub phase_interruption_input: Option<&'a GenerationQualificationPhaseInterruptionRecordV1Input>,
}

/// Independent facts required to cold-read one terminal-evidence cohort.
#[derive(Clone, Copy)]
pub struct GenerationQualificationTerminalEvidenceV1ReadInput<'a> {
    /// Exact schema 9 preregistration closure.
    pub preregistration: GenerationQualificationPreregistrationReadInput<'a>,
    /// Independently retained platform assessment.
    pub platform_input: GenerationQualificationPlatformEvidenceV1Input,
    /// Independently retained license assessment.
    pub license_input: GenerationQualificationLicenseEvidenceV1Input,
    /// Stable model-package foundation named by the license assessment.
    pub model_package_foundation_id: &'a ModelPackageFoundationId,
    /// Exact model-license control named by the license assessment.
    pub model_license_control_id: &'a ModelLicenseControlId,
    /// Managed-evidence facts in target-attempt encounter order.
    pub managed_evidence_inputs: &'a [ManagedOllamaCandidateGenerationEvidenceV2Input],
    /// Resource-evidence digests in semantic order.
    pub resource_evidence_record_digests: &'a [Digest],
    /// Human-adjudication digests in semantic order.
    pub human_evidence_record_digests: &'a [Digest],
    /// Independently retained runner terminal facts.
    pub receipt_input: GenerationQualificationOperationReceiptV1Input,
    /// Interruption facts when a row is required. Absence expects no row.
    pub phase_interruption_input: Option<&'a GenerationQualificationPhaseInterruptionRecordV1Input>,
}

/// Owned terminal-evidence cohort reconstructed from durable rows.
///
/// The value is inert. It grants no live, qualification, or activation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredGenerationQualificationTerminalEvidenceV1 {
    preregistration: StoredGenerationQualificationPreregistration,
    platform_evidence: GenerationQualificationPlatformEvidenceV1,
    license_evidence: GenerationQualificationLicenseEvidenceV1,
    attempt_ledger_manifest: GenerationAttemptLedgerManifestV1,
    repeatability_results: Vec<GenerationRepeatabilityResultRecordV1>,
    repeatability_manifest: GenerationRepeatabilityEvidenceManifestV1,
    resource_manifest: GenerationResourceEvidenceManifestV1,
    human_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    receipt: GenerationQualificationOperationReceiptV1,
    phase_interruption: Option<GenerationQualificationPhaseInterruptionRecordV1>,
}

impl StoredGenerationQualificationTerminalEvidenceV1 {
    /// Returns the cold-loaded preregistration closure.
    #[must_use]
    pub const fn preregistration(&self) -> &StoredGenerationQualificationPreregistration {
        &self.preregistration
    }

    /// Returns the exact platform evidence.
    #[must_use]
    pub const fn platform_evidence(&self) -> &GenerationQualificationPlatformEvidenceV1 {
        &self.platform_evidence
    }

    /// Returns the exact license evidence.
    #[must_use]
    pub const fn license_evidence(&self) -> &GenerationQualificationLicenseEvidenceV1 {
        &self.license_evidence
    }

    /// Returns the exact attempt-ledger manifest.
    #[must_use]
    pub const fn attempt_ledger_manifest(&self) -> &GenerationAttemptLedgerManifestV1 {
        &self.attempt_ledger_manifest
    }

    /// Returns repeatability results in preregistered repetition order.
    #[must_use]
    pub fn repeatability_results(&self) -> &[GenerationRepeatabilityResultRecordV1] {
        &self.repeatability_results
    }

    /// Returns the exact repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(&self) -> &GenerationRepeatabilityEvidenceManifestV1 {
        &self.repeatability_manifest
    }

    /// Returns the exact resource manifest.
    #[must_use]
    pub const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        &self.resource_manifest
    }

    /// Returns the exact human-adjudication manifest.
    #[must_use]
    pub const fn human_manifest(&self) -> &GenerationHumanAdjudicationEvidenceManifestV1 {
        &self.human_manifest
    }

    /// Returns the exact operation receipt.
    #[must_use]
    pub const fn receipt(&self) -> &GenerationQualificationOperationReceiptV1 {
        &self.receipt
    }

    /// Returns the interruption record when one was stored.
    #[must_use]
    pub const fn phase_interruption(
        &self,
    ) -> Option<&GenerationQualificationPhaseInterruptionRecordV1> {
        self.phase_interruption.as_ref()
    }
}

/// Per-record outcome of one atomic terminal-evidence write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationTerminalEvidenceV1WriteDisposition {
    /// Platform-evidence write outcome.
    pub platform: WriteDisposition,
    /// License-evidence write outcome.
    pub license: WriteDisposition,
    /// Attempt-ledger manifest write outcome.
    pub attempt_ledger: WriteDisposition,
    /// Repeatability-result write outcome. `None` when the slice is empty.
    pub repeatability_results: Option<WriteDisposition>,
    /// Repeatability-manifest write outcome.
    pub repeatability_manifest: WriteDisposition,
    /// Resource-manifest write outcome.
    pub resource: WriteDisposition,
    /// Human-adjudication manifest write outcome.
    pub human: WriteDisposition,
    /// Operation-receipt write outcome.
    pub receipt: WriteDisposition,
    /// Interruption write outcome. `None` when the record is omitted.
    pub phase_interruption: Option<WriteDisposition>,
}

/// Failure of one atomic terminal-evidence transaction.
pub enum GenerationQualificationTerminalEvidenceV1TransactionError<E> {
    /// Durable storage or typed relationship validation failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for GenerationQualificationTerminalEvidenceV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationQualificationTerminalEvidenceV1TransactionError::Store",
            Self::Gate(_) => "GenerationQualificationTerminalEvidenceV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for GenerationQualificationTerminalEvidenceV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation qualification terminal evidence storage failed",
            Self::Gate(_) => "generation qualification terminal evidence gate rejected the commit",
        })
    }
}

impl<E> Error for GenerationQualificationTerminalEvidenceV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically stores and cold-revalidates one schema 11 terminal-evidence cohort.
    ///
    /// The preregistration and attempt ledger are cold-read on the open immediate
    /// transaction before any cohort row is inserted. Gates run before lock
    /// acquisition, after acquisition, and before commit. The stored value grants
    /// no live, qualification, or activation authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing dependency, corrupt
    /// attempt prefix, immutable conflict, invalid operation closure, or commit
    /// failure. No qualification record is written.
    pub fn transact_generation_qualification_terminal_evidence_v1<E>(
        &mut self,
        input: GenerationQualificationTerminalEvidenceV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<
        GenerationQualificationTerminalEvidenceV1WriteDisposition,
        GenerationQualificationTerminalEvidenceV1TransactionError<E>,
    > {
        gate().map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Store)?;
        gate().map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Gate)?;
        let prepared = codec::prepare(&transaction, &input)
            .map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Store)?;
        let disposition = write::insert_cohort(&transaction, &prepared)
            .map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Store)?;
        let stored = read::load(&transaction, &prepared.read)
            .map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Store)?;
        match stored {
            Some(value) if codec::matches_prepared(&value, &prepared) => {}
            Some(_) => {
                return Err(
                    GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                        StoreError::ImmutableConflict,
                    ),
                );
            }
            None => {
                return Err(
                    GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                        StoreError::CorruptRecord,
                    ),
                );
            }
        }
        gate().map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationQualificationTerminalEvidenceV1TransactionError::Store)?;
        Ok(disposition)
    }

    /// Cold-reads one terminal-evidence cohort.
    ///
    /// `Ok(None)` means the cohort is absent. A present receipt whose chain does
    /// not rederive is [`StoreError::CorruptRecord`]. The read does not modify
    /// durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when preregistration is missing, stored bytes or
    /// indexed columns disagree, or the read modifies the database.
    pub fn generation_qualification_terminal_evidence_v1(
        &self,
        input: GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    ) -> StoreResult<Option<StoredGenerationQualificationTerminalEvidenceV1>> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load(&transaction, &input)?;
        transaction.commit()?;
        if self.connection.total_changes() == before {
            Ok(stored)
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

#[cfg(test)]
#[path = "generation_qualification_terminal_evidence/tests.rs"]
pub(crate) mod tests;
