//! Active-owned target attempt-record prefix and one-shot ledger sealing.

use std::fmt;

use rewrite_model::{
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1,
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, PlannedCandidateAttemptV1,
};
use thiserror::Error;

use super::ActiveGenerationQualificationOperation;
use crate::VerifiedCompletedManagedCandidateAttempt;
use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};
use crate::generation_qualification_preregistration::prepared::{
    PreparedGenerationQualificationMandatoryFinalizationError,
    PreparedGenerationQualificationValidationView,
};

#[derive(Clone)]
pub(super) struct ActiveCandidateAttemptScope {
    pub(super) planned_attempt: PlannedCandidateAttemptV1,
    pub(super) precursor: CandidateGenerationAttemptPrecursorV1,
    pub(super) target: bool,
}

impl ActiveCandidateAttemptScope {
    pub(super) fn new(
        planned_attempt: PlannedCandidateAttemptV1,
        precursor: CandidateGenerationAttemptPrecursorV1,
        target: bool,
    ) -> Self {
        Self {
            planned_attempt,
            precursor,
            target,
        }
    }
}

pub(super) struct PendingActiveCandidateAttempt {
    pub(super) scope: ActiveCandidateAttemptScope,
    pub(super) completed_attempt: Option<Box<VerifiedCompletedManagedCandidateAttempt>>,
}

impl PendingActiveCandidateAttempt {
    pub(super) fn new(
        scope: ActiveCandidateAttemptScope,
        completed_attempt: Box<VerifiedCompletedManagedCandidateAttempt>,
    ) -> Self {
        Self {
            scope,
            completed_attempt: Some(completed_attempt),
        }
    }
}

/// Noncloneable Active-owned target attempt-ledger closure.
///
/// Its records and manifest remain inert portable evidence. The private binding
/// proves only that this in-memory closure came from one exact Active operation.
///
/// ```compile_fail
/// use rewrite_eval::ActiveGenerationQualificationAttemptLedgerClosure;
/// fn cannot_clone(value: ActiveGenerationQualificationAttemptLedgerClosure) {
///     let _forged = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::ActiveGenerationQualificationAttemptLedgerClosure;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(value: &ActiveGenerationQualificationAttemptLedgerClosure) {
///     require_serialize(value);
/// }
/// ```
pub struct ActiveGenerationQualificationAttemptLedgerClosure {
    active_binding: ActiveGenerationQualificationBinding,
    manifest: GenerationAttemptLedgerManifestV1,
    target_attempt_records: Vec<CandidateGenerationAttemptRecordV1>,
    target_attempt_receipts: Vec<CandidateGenerationReceiptV1>,
}

impl ActiveGenerationQualificationAttemptLedgerClosure {
    pub(super) fn new(
        active_binding: ActiveGenerationQualificationBinding,
        manifest: GenerationAttemptLedgerManifestV1,
        target_attempt_records: Vec<CandidateGenerationAttemptRecordV1>,
        target_attempt_receipts: Vec<CandidateGenerationReceiptV1>,
    ) -> Self {
        Self {
            active_binding,
            manifest,
            target_attempt_records,
            target_attempt_receipts,
        }
    }

    /// Returns the exact target attempt-ledger manifest.
    #[must_use]
    pub const fn manifest(&self) -> &GenerationAttemptLedgerManifestV1 {
        &self.manifest
    }

    /// Returns the exact retained target record prefix in target plan order.
    #[must_use]
    pub fn target_attempt_records(&self) -> &[CandidateGenerationAttemptRecordV1] {
        &self.target_attempt_records
    }

    /// Returns every exact completed target receipt in target record order.
    #[must_use]
    pub fn target_attempt_receipts(&self) -> &[CandidateGenerationReceiptV1] {
        &self.target_attempt_receipts
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        subject.accepts(&self.active_binding)
    }
}

impl fmt::Debug for ActiveGenerationQualificationAttemptLedgerClosure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationAttemptLedgerClosure")
            .field(
                "attempt_ledger_manifest_id",
                self.manifest.attempt_ledger_manifest_id(),
            )
            .field(
                "target_attempt_record_count",
                &self.target_attempt_records.len(),
            )
            .field(
                "target_attempt_receipt_count",
                &self.target_attempt_receipts.len(),
            )
            .finish_non_exhaustive()
    }
}

/// Stable category for one-shot attempt-ledger sealing failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveGenerationQualificationAttemptLedgerErrorKind {
    /// The ledger was already sealed.
    AlreadySealed,
    /// Candidate execution or settlement has not reached a sealable boundary.
    NotReady,
    /// Fresh mandatory authority validation failed.
    MandatoryFinalization,
    /// The exact portable ledger contract rejected the retained closure.
    EvidenceCompilation,
    /// Ledger compilation and mandatory finalization both failed.
    PrimaryAndFinalization,
}

/// Content-redacted failure while sealing the target attempt ledger.
#[derive(Error)]
pub enum ActiveGenerationQualificationAttemptLedgerError {
    /// The ledger was already sealed.
    #[error("active generation qualification attempt ledger is already sealed")]
    AlreadySealed,
    /// Candidate execution or settlement has not reached a sealable boundary.
    #[error("active generation qualification attempt ledger is not ready")]
    NotReady,
    /// Fresh mandatory authority validation failed.
    #[error("active generation qualification mandatory finalization failed")]
    MandatoryFinalization,
    /// The exact portable ledger contract rejected the retained closure.
    #[error("active generation qualification attempt ledger compilation failed")]
    EvidenceCompilation(#[source] GenerationQualificationPhaseEvidenceError),
    /// Ledger compilation and mandatory finalization both failed.
    #[error("active generation qualification ledger and finalization both failed")]
    PrimaryAndFinalization {
        /// Content-redacted primary ledger failure.
        primary: Box<ActiveGenerationQualificationAttemptLedgerError>,
    },
}

impl ActiveGenerationQualificationAttemptLedgerError {
    /// Returns the stable content-free failure category.
    #[must_use]
    pub const fn kind(&self) -> ActiveGenerationQualificationAttemptLedgerErrorKind {
        match self {
            Self::AlreadySealed => {
                ActiveGenerationQualificationAttemptLedgerErrorKind::AlreadySealed
            }
            Self::NotReady => ActiveGenerationQualificationAttemptLedgerErrorKind::NotReady,
            Self::MandatoryFinalization => {
                ActiveGenerationQualificationAttemptLedgerErrorKind::MandatoryFinalization
            }
            Self::EvidenceCompilation(_) => {
                ActiveGenerationQualificationAttemptLedgerErrorKind::EvidenceCompilation
            }
            Self::PrimaryAndFinalization { .. } => {
                ActiveGenerationQualificationAttemptLedgerErrorKind::PrimaryAndFinalization
            }
        }
    }
}

impl fmt::Debug for ActiveGenerationQualificationAttemptLedgerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationAttemptLedgerError")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Seals the exact target record prefix into one process-bound closure.
    ///
    /// Final evidence construction deliberately uses fresh uncancelled mandatory
    /// validation. Sealing is allowed only after the whole candidate sequence was
    /// settled or after a terminal failure, and it can happen only once.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for repeated sealing, pending or incomplete
    /// candidate work, retained authority drift, or a rejected portable closure.
    pub fn seal_target_attempt_ledger(
        &mut self,
    ) -> Result<
        &ActiveGenerationQualificationAttemptLedgerClosure,
        ActiveGenerationQualificationAttemptLedgerError,
    > {
        if self.attempt_ledger().is_some() {
            return Err(ActiveGenerationQualificationAttemptLedgerError::AlreadySealed);
        }
        if self.started_candidate_attempt.is_some() || self.pending_completed_candidate.is_some() {
            return Err(ActiveGenerationQualificationAttemptLedgerError::NotReady);
        }
        let records = &self.target_attempt_records;
        let receipts = &self.target_attempt_receipts;
        let next_candidate_attempt = self.next_candidate_attempt;
        let terminal = self.terminal;
        let manifest = self
            .prepared
            .with_mandatory_finalization_validated_view(|view| {
                compile_manifest(&view, records, receipts, next_candidate_attempt, terminal)
            })
            .map_err(map_finalization_error)?;
        self.attempt_ledger = Some(ActiveGenerationQualificationAttemptLedgerClosure::new(
            self.subject.binding(),
            manifest,
            std::mem::take(&mut self.target_attempt_records),
            std::mem::take(&mut self.target_attempt_receipts),
        ));
        self.attempt_ledger.as_ref().ok_or(
            ActiveGenerationQualificationAttemptLedgerError::EvidenceCompilation(
                GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
            ),
        )
    }
}

pub(super) fn compile_manifest(
    view: &PreparedGenerationQualificationValidationView<'_>,
    records: &[CandidateGenerationAttemptRecordV1],
    receipts: &[CandidateGenerationReceiptV1],
    next_candidate_attempt: usize,
    terminal: bool,
) -> Result<GenerationAttemptLedgerManifestV1, ActiveGenerationQualificationAttemptLedgerError> {
    let planned = view.operation_policy_relations.planned_attempts;
    validate_receipt_closure(view, records, receipts)?;
    if !terminal && next_candidate_attempt != planned.len() {
        return Err(ActiveGenerationQualificationAttemptLedgerError::NotReady);
    }
    let target_count = planned
        .iter()
        .filter(|attempt| {
            attempt.generation_system_id() == view.operation_policy.target_generation_system_id()
        })
        .count();
    let completed = records.iter().all(|record| {
        matches!(
            record.outcome(),
            CandidateGenerationAttemptOutcomeV1::Completed { .. }
        )
    });
    let status = derive_status(records.len(), target_count, completed, terminal)?;
    GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system: view
                .operation_policy_relations
                .target_system
                .generation_system,
            qualification_plan: view.operation_policy_relations.plan,
            suite: view.operation_policy_relations.suite,
        },
        phase_policy_digest: view.operation_policy.attempt_ledger_policy_digest(),
        planned_attempts: planned,
        attempt_records: records,
        status,
    })
    .map_err(ActiveGenerationQualificationAttemptLedgerError::EvidenceCompilation)
}

fn validate_receipt_closure(
    view: &PreparedGenerationQualificationValidationView<'_>,
    records: &[CandidateGenerationAttemptRecordV1],
    receipts: &[CandidateGenerationReceiptV1],
) -> Result<(), ActiveGenerationQualificationAttemptLedgerError> {
    validate_receipt_closure_ids(
        records,
        receipts,
        view.operation_policy.target_generation_system_id(),
        view.operation_policy.generation_qualification_plan_id(),
        view.operation_policy.suite_manifest_id(),
    )
}

fn validate_receipt_closure_ids(
    records: &[CandidateGenerationAttemptRecordV1],
    receipts: &[CandidateGenerationReceiptV1],
    target_generation_system_id: &rewrite_model::GenerationSystemId,
    qualification_plan_id: &rewrite_model::GenerationQualificationPlanId,
    suite_manifest_id: &rewrite_model::GenerationSuiteManifestId,
) -> Result<(), ActiveGenerationQualificationAttemptLedgerError> {
    let mut receipts = receipts.iter();
    for record in records {
        let CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id,
            receipt_id,
            ..
        } = record.outcome()
        else {
            continue;
        };
        let receipt = receipts.next().ok_or_else(receipt_closure_error)?;
        if receipt.planned_attempt_id() != planned_attempt_id
            || receipt.receipt_id() != receipt_id
            || receipt.generation_system_id() != target_generation_system_id
            || receipt.qualification_plan_id() != qualification_plan_id
            || receipt.suite_manifest_id() != suite_manifest_id
        {
            return Err(receipt_closure_error());
        }
    }
    if receipts.next().is_some() {
        Err(receipt_closure_error())
    } else {
        Ok(())
    }
}

const fn receipt_closure_error() -> ActiveGenerationQualificationAttemptLedgerError {
    ActiveGenerationQualificationAttemptLedgerError::EvidenceCompilation(
        GenerationQualificationPhaseEvidenceError::RelationshipMismatch,
    )
}

fn derive_status(
    record_count: usize,
    target_count: usize,
    all_completed: bool,
    terminal: bool,
) -> Result<GenerationQualificationPhaseStatusV1, ActiveGenerationQualificationAttemptLedgerError> {
    if record_count == target_count && all_completed {
        Ok(GenerationQualificationPhaseStatusV1::Passed)
    } else if terminal && record_count > 0 {
        Ok(GenerationQualificationPhaseStatusV1::Failed)
    } else if terminal {
        Ok(GenerationQualificationPhaseStatusV1::Skipped)
    } else {
        Err(ActiveGenerationQualificationAttemptLedgerError::NotReady)
    }
}

fn map_finalization_error(
    error: PreparedGenerationQualificationMandatoryFinalizationError<
        ActiveGenerationQualificationAttemptLedgerError,
    >,
) -> ActiveGenerationQualificationAttemptLedgerError {
    match error {
        PreparedGenerationQualificationMandatoryFinalizationError::Callback(error) => error,
        PreparedGenerationQualificationMandatoryFinalizationError::Initial(_)
        | PreparedGenerationQualificationMandatoryFinalizationError::Final(_)
        | PreparedGenerationQualificationMandatoryFinalizationError::InitialAndFinal { .. } => {
            ActiveGenerationQualificationAttemptLedgerError::MandatoryFinalization
        }
        PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal {
            callback,
            ..
        } => ActiveGenerationQualificationAttemptLedgerError::PrimaryAndFinalization {
            primary: Box::new(callback),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verified_candidate_batch_set::tests::support::scenario;

    #[test]
    fn closed_status_is_derived_without_caller_input() {
        assert_eq!(
            derive_status(2, 2, true, false).expect("complete target closure"),
            GenerationQualificationPhaseStatusV1::Passed
        );
        assert_eq!(
            derive_status(1, 2, true, true).expect("terminal prefix"),
            GenerationQualificationPhaseStatusV1::Failed
        );
        assert_eq!(
            derive_status(0, 2, true, true).expect("terminal before target record"),
            GenerationQualificationPhaseStatusV1::Skipped
        );
        assert_eq!(
            derive_status(1, 2, true, false)
                .expect_err("live prefix cannot seal")
                .kind(),
            ActiveGenerationQualificationAttemptLedgerErrorKind::NotReady
        );
    }

    #[test]
    fn completed_target_records_require_exact_ordered_receipts() {
        let scenario = scenario("active-ledger-receipts");
        let records = scenario
            .batches
            .iter()
            .map(|batch| batch.portable_attempt_record().clone())
            .collect::<Vec<_>>();
        let receipts = scenario
            .batches
            .iter()
            .map(|batch| batch.portable_receipt().clone())
            .collect::<Vec<_>>();
        let validate = |receipts: &[CandidateGenerationReceiptV1]| {
            validate_receipt_closure_ids(
                &records,
                receipts,
                scenario.input.generation_system.generation_system_id(),
                scenario.input.qualification_plan.qualification_plan_id(),
                scenario.input.suite.suite_manifest_id(),
            )
        };
        validate(&receipts).expect("exact receipt closure");
        assert!(validate(&receipts[..1]).is_err());
        let mut reordered = receipts.clone();
        reordered.reverse();
        assert!(validate(&reordered).is_err());
        let mut extra = receipts.clone();
        extra.push(receipts[0].clone());
        assert!(validate(&extra).is_err());
    }
}
