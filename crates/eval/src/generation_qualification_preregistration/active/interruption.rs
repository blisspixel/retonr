//! Active-owned terminal closure for interrupted attempt-ledger work.

use std::{fmt, time::Instant};

use rewrite_model::{
    CandidateGenerationAttemptRecordV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationInterruptedPhaseV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseCheckpointV1,
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseInterruptionReasonV1,
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationPhaseInterruptionRecordV1Input, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationTerminalInterruptionPlan,
    GenerationQualificationTerminalSetPlan, GenerationRepeatabilityEvidenceManifestV1,
    GenerationRepeatabilityEvidenceManifestV1Relations, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations, PlannedCandidateAttemptId,
    PlannedGenerationQualificationTerminalSet,
};
use thiserror::Error;

use super::{
    ActiveGenerationQualificationAttemptLedgerClosure,
    ActiveGenerationQualificationAttemptLedgerError, ActiveGenerationQualificationOperation,
    attempt_ledger::compile_manifest,
};
use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};
use crate::generation_qualification_preregistration::prepared::{
    PreparedGenerationQualificationMandatoryFinalizationError,
    PreparedGenerationQualificationValidationView,
};

/// Noncloneable exact terminal evidence for one interrupted Active operation.
///
/// The portable records remain inert. The private binding proves only that this
/// in-memory closure was derived by the same process-local Active owner.
pub struct ActiveGenerationQualificationOperationInterruption {
    active_binding: ActiveGenerationQualificationBinding,
    attempt_ledger: ActiveGenerationQualificationAttemptLedgerClosure,
    repeatability_manifest: GenerationRepeatabilityEvidenceManifestV1,
    resource_manifest: GenerationResourceEvidenceManifestV1,
    human_adjudication_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    receipt: GenerationQualificationOperationReceiptV1,
    interruption_input: GenerationQualificationPhaseInterruptionRecordV1Input,
    interruption: GenerationQualificationPhaseInterruptionRecordV1,
}

impl ActiveGenerationQualificationOperationInterruption {
    /// Returns the exact target attempt-ledger closure.
    #[must_use]
    pub const fn attempt_ledger(&self) -> &ActiveGenerationQualificationAttemptLedgerClosure {
        &self.attempt_ledger
    }

    /// Returns the exact skipped repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(&self) -> &GenerationRepeatabilityEvidenceManifestV1 {
        &self.repeatability_manifest
    }

    /// Returns the exact skipped resource-evidence manifest.
    #[must_use]
    pub const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        &self.resource_manifest
    }

    /// Returns the exact skipped human-adjudication manifest.
    #[must_use]
    pub const fn human_adjudication_manifest(
        &self,
    ) -> &GenerationHumanAdjudicationEvidenceManifestV1 {
        &self.human_adjudication_manifest
    }

    /// Returns the exact noncompleted operation receipt.
    #[must_use]
    pub const fn receipt(&self) -> &GenerationQualificationOperationReceiptV1 {
        &self.receipt
    }

    /// Returns the exact phase-interruption companion record.
    #[must_use]
    pub const fn interruption(&self) -> &GenerationQualificationPhaseInterruptionRecordV1 {
        &self.interruption
    }

    fn matches_active_subject(&self, subject: &ActiveGenerationQualificationSubject) -> bool {
        subject.accepts(&self.active_binding) && self.attempt_ledger.matches_active_subject(subject)
    }
}

impl fmt::Debug for ActiveGenerationQualificationOperationInterruption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationOperationInterruption")
            .field("operation_receipt_id", self.receipt.operation_receipt_id())
            .field(
                "phase_interruption_record_id",
                self.interruption.phase_interruption_record_id(),
            )
            .field("reason", &self.interruption.reason())
            .finish_non_exhaustive()
    }
}

/// Stable category for fresh Active interruption-closure validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveGenerationQualificationOperationInterruptionErrorKind {
    /// No operation interruption was derived.
    Unavailable,
    /// The process-local Active binding did not match.
    ActiveBinding,
    /// Fresh mandatory finalization validation failed.
    MandatoryFinalization,
    /// Exact portable interruption evidence no longer rederived.
    EvidenceCompilation,
    /// Evidence compilation and finalization both failed.
    PrimaryAndFinalization,
}

/// Content-redacted failure while revalidating Active interruption evidence.
#[derive(Error)]
pub enum ActiveGenerationQualificationOperationInterruptionError {
    /// No operation interruption was derived.
    #[error("active generation qualification interruption is unavailable")]
    Unavailable,
    /// The process-local Active binding did not match.
    #[error("active generation qualification interruption binding does not match")]
    ActiveBinding,
    /// Fresh mandatory finalization validation failed.
    #[error("active generation qualification interruption finalization failed")]
    MandatoryFinalization,
    /// Exact portable interruption evidence no longer rederived.
    #[error("active generation qualification interruption evidence does not match")]
    EvidenceCompilation,
    /// Evidence compilation and finalization both failed.
    #[error("active generation qualification interruption validation failures were aggregated")]
    PrimaryAndFinalization,
}

impl ActiveGenerationQualificationOperationInterruptionError {
    /// Returns the stable content-free failure category.
    #[must_use]
    pub const fn kind(&self) -> ActiveGenerationQualificationOperationInterruptionErrorKind {
        match self {
            Self::Unavailable => {
                ActiveGenerationQualificationOperationInterruptionErrorKind::Unavailable
            }
            Self::ActiveBinding => {
                ActiveGenerationQualificationOperationInterruptionErrorKind::ActiveBinding
            }
            Self::MandatoryFinalization => {
                ActiveGenerationQualificationOperationInterruptionErrorKind::MandatoryFinalization
            }
            Self::EvidenceCompilation => {
                ActiveGenerationQualificationOperationInterruptionErrorKind::EvidenceCompilation
            }
            Self::PrimaryAndFinalization => {
                ActiveGenerationQualificationOperationInterruptionErrorKind::PrimaryAndFinalization
            }
        }
    }
}

impl fmt::Debug for ActiveGenerationQualificationOperationInterruptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationOperationInterruptionError")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

pub(super) struct CandidateCloseoutInterruptionFacts {
    pub(super) planned_attempt_id: PlannedCandidateAttemptId,
    pub(super) reason: GenerationQualificationPhaseInterruptionReasonV1,
    pub(super) terminal_status: GenerationQualificationOperationTerminalStatusV1,
    pub(super) observed_at: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CandidateCloseoutInterruptionDisposition {
    Available,
    EvidenceCompilationFailed,
    MandatoryFinalizationFailed,
    PrimaryAndFinalizationFailed,
}

impl CandidateCloseoutInterruptionDisposition {
    pub(super) const fn available(self) -> bool {
        matches!(self, Self::Available)
    }

    pub(super) const fn compilation_failed(self) -> bool {
        matches!(
            self,
            Self::EvidenceCompilationFailed | Self::PrimaryAndFinalizationFailed
        )
    }

    pub(super) const fn finalization_failed(self) -> bool {
        matches!(
            self,
            Self::MandatoryFinalizationFailed | Self::PrimaryAndFinalizationFailed
        )
    }
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    pub(super) fn compile_candidate_closeout_interruption(
        &mut self,
        facts: CandidateCloseoutInterruptionFacts,
    ) -> CandidateCloseoutInterruptionDisposition {
        let target_attempt_records = &self.target_attempt_records;
        let target_attempt_receipts = &self.target_attempt_receipts;
        let next_candidate_attempt = self.next_candidate_attempt;
        let peak_concurrent_attempts = u32::from(self.lifecycle.peak_live_authorities());
        let result = self
            .prepared
            .with_mandatory_finalization_validated_view(|view| {
                let elapsed_nanoseconds =
                    checked_elapsed_nanoseconds(view.started, facts.observed_at)?;
                derive_interruption_evidence(
                    &view,
                    target_attempt_records,
                    target_attempt_receipts,
                    next_candidate_attempt,
                    GenerationQualificationOperationReceiptV1Input {
                        elapsed_nanoseconds,
                        peak_concurrent_attempts,
                        terminal_status: facts.terminal_status,
                        finalization_status:
                            GenerationQualificationOperationFinalizationStatusV1::Passed,
                    },
                    GenerationQualificationPhaseInterruptionRecordV1Input {
                        phase: GenerationQualificationInterruptedPhaseV1::AttemptLedger,
                        checkpoint: GenerationQualificationPhaseCheckpointV1::EvidenceCompilation,
                        planned_attempt_id: Some(facts.planned_attempt_id),
                        reason: facts.reason,
                    },
                )
            });
        match result {
            Ok(evidence) => {
                let attempt_ledger = ActiveGenerationQualificationAttemptLedgerClosure::new(
                    self.subject.binding(),
                    evidence.attempt_ledger_manifest,
                    std::mem::take(&mut self.target_attempt_records),
                    std::mem::take(&mut self.target_attempt_receipts),
                );
                self.operation_interruption =
                    Some(ActiveGenerationQualificationOperationInterruption {
                        active_binding: self.subject.binding(),
                        attempt_ledger,
                        repeatability_manifest: evidence.repeatability_manifest,
                        resource_manifest: evidence.resource_manifest,
                        human_adjudication_manifest: evidence.human_adjudication_manifest,
                        receipt_input: evidence.receipt_input,
                        receipt: evidence.receipt,
                        interruption_input: evidence.interruption_input,
                        interruption: evidence.interruption,
                    });
                CandidateCloseoutInterruptionDisposition::Available
            }
            Err(PreparedGenerationQualificationMandatoryFinalizationError::Callback(_)) => {
                CandidateCloseoutInterruptionDisposition::EvidenceCompilationFailed
            }
            Err(PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal {
                ..
            }) => CandidateCloseoutInterruptionDisposition::PrimaryAndFinalizationFailed,
            Err(
                PreparedGenerationQualificationMandatoryFinalizationError::Initial(_)
                | PreparedGenerationQualificationMandatoryFinalizationError::Final(_)
                | PreparedGenerationQualificationMandatoryFinalizationError::InitialAndFinal {
                    ..
                },
            ) => CandidateCloseoutInterruptionDisposition::MandatoryFinalizationFailed,
        }
    }

    /// Freshly revalidates the exact retained terminal interruption closure.
    ///
    /// This uses independent mandatory finalization authority, so a legitimate
    /// deadline or caller cancellation cannot prevent terminal evidence validation.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when no closure exists, the process-local binding
    /// differs, mandatory finalization fails, or any exact portable relation drifts.
    pub fn revalidate_operation_interruption(
        &mut self,
    ) -> Result<(), ActiveGenerationQualificationOperationInterruptionError> {
        let interruption = self
            .operation_interruption
            .as_ref()
            .ok_or(ActiveGenerationQualificationOperationInterruptionError::Unavailable)?;
        if !interruption.matches_active_subject(&self.subject) {
            return Err(ActiveGenerationQualificationOperationInterruptionError::ActiveBinding);
        }
        let next_candidate_attempt = self.next_candidate_attempt;
        self.prepared
            .with_mandatory_finalization_validated_view(|view| {
                validate_interruption_evidence(&view, interruption, next_candidate_attempt)
            })
            .map_err(|error| map_interruption_validation_error(&error))
    }
}

struct InterruptionEvidence {
    attempt_ledger_manifest: rewrite_model::GenerationAttemptLedgerManifestV1,
    repeatability_manifest: GenerationRepeatabilityEvidenceManifestV1,
    resource_manifest: GenerationResourceEvidenceManifestV1,
    human_adjudication_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    receipt: GenerationQualificationOperationReceiptV1,
    interruption_input: GenerationQualificationPhaseInterruptionRecordV1Input,
    interruption: GenerationQualificationPhaseInterruptionRecordV1,
}

#[derive(Debug, Error)]
enum InterruptionEvidenceError {
    #[error("attempt-ledger closure failed")]
    AttemptLedger(#[source] ActiveGenerationQualificationAttemptLedgerError),
    #[error("phase-manifest closure failed")]
    PhaseManifest(#[source] GenerationQualificationPhaseEvidenceError),
    #[error("operation interruption closure failed")]
    Operation(#[source] GenerationQualificationOperationContractError),
    #[error("operation elapsed time is unavailable")]
    ElapsedUnavailable,
    #[error("operation interruption closure changed")]
    RelationshipMismatch,
}

fn derive_interruption_evidence(
    view: &PreparedGenerationQualificationValidationView<'_>,
    target_attempt_records: &[CandidateGenerationAttemptRecordV1],
    target_attempt_receipts: &[rewrite_model::CandidateGenerationReceiptV1],
    next_candidate_attempt: usize,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    interruption_input: GenerationQualificationPhaseInterruptionRecordV1Input,
) -> Result<InterruptionEvidence, InterruptionEvidenceError> {
    let attempt_ledger_manifest = compile_manifest(
        view,
        target_attempt_records,
        target_attempt_receipts,
        next_candidate_attempt,
        true,
    )
    .map_err(InterruptionEvidenceError::AttemptLedger)?;
    let scope = phase_scope(view);
    let status = GenerationQualificationPhaseStatusV1::Skipped;
    let repeatability_manifest = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: view.operation_policy.repeatability_policy_digest(),
            planned_attempts: view.operation_policy_relations.planned_attempts,
            preregistered_repetitions: view.operation_policy_relations.repetitions,
            results: &[],
            status,
        },
    )
    .map_err(InterruptionEvidenceError::PhaseManifest)?;
    let resource_manifest =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: view.operation_policy.resource_policy_digest(),
            evidence_record_digests: &[],
            status,
        })
        .map_err(InterruptionEvidenceError::PhaseManifest)?;
    let human_adjudication_manifest = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: view.operation_policy.human_adjudication_policy_digest(),
            evidence_record_digests: &[],
            status,
        },
    )
    .map_err(InterruptionEvidenceError::PhaseManifest)?;
    let receipt_relations = receipt_relations(
        view,
        &attempt_ledger_manifest,
        &repeatability_manifest,
        &resource_manifest,
        &human_adjudication_manifest,
    );
    let (receipt, interruption) =
        PlannedGenerationQualificationTerminalSet::plan(GenerationQualificationTerminalSetPlan {
            receipt_relations,
            receipt_input,
            interruption: Some(GenerationQualificationTerminalInterruptionPlan {
                operation_policy_relations: view.operation_policy_relations,
                operation_policy_input: view.operation_policy_input,
                input: interruption_input.clone(),
            }),
        })
        .map_err(InterruptionEvidenceError::Operation)?
        .into_parts();
    let Some(interruption) = interruption else {
        return Err(InterruptionEvidenceError::Operation(
            GenerationQualificationOperationContractError::InvalidInterruptionClosure,
        ));
    };
    Ok(InterruptionEvidence {
        attempt_ledger_manifest,
        repeatability_manifest,
        resource_manifest,
        human_adjudication_manifest,
        receipt_input,
        receipt,
        interruption_input,
        interruption,
    })
}

fn validate_interruption_evidence(
    view: &PreparedGenerationQualificationValidationView<'_>,
    evidence: &ActiveGenerationQualificationOperationInterruption,
    next_candidate_attempt: usize,
) -> Result<(), InterruptionEvidenceError> {
    let expected = derive_interruption_evidence(
        view,
        evidence.attempt_ledger.target_attempt_records(),
        evidence.attempt_ledger.target_attempt_receipts(),
        next_candidate_attempt,
        evidence.receipt_input,
        evidence.interruption_input.clone(),
    )?;
    if expected.attempt_ledger_manifest == *evidence.attempt_ledger.manifest()
        && expected.repeatability_manifest == evidence.repeatability_manifest
        && expected.resource_manifest == evidence.resource_manifest
        && expected.human_adjudication_manifest == evidence.human_adjudication_manifest
        && expected.receipt == evidence.receipt
        && expected.interruption == evidence.interruption
    {
        Ok(())
    } else {
        Err(InterruptionEvidenceError::RelationshipMismatch)
    }
}

fn receipt_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
    attempt_ledger_manifest: &'a rewrite_model::GenerationAttemptLedgerManifestV1,
    repeatability_manifest: &'a GenerationRepeatabilityEvidenceManifestV1,
    resource_manifest: &'a GenerationResourceEvidenceManifestV1,
    human_adjudication_manifest: &'a GenerationHumanAdjudicationEvidenceManifestV1,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: view.operation_policy,
        request_projection: view.request_projection,
        platform_evidence: view.platform_evidence,
        license_evidence: view.license_evidence,
        attempt_ledger_manifest,
        repeatability_manifest,
        resource_manifest,
        human_adjudication_manifest,
    }
}

fn phase_scope<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
) -> GenerationQualificationPhaseScopeV1<'a> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: view
            .operation_policy_relations
            .target_system
            .generation_system,
        qualification_plan: view.operation_policy_relations.plan,
        suite: view.operation_policy_relations.suite,
    }
}

fn checked_elapsed_nanoseconds(
    started: Instant,
    observed: Instant,
) -> Result<u64, InterruptionEvidenceError> {
    let elapsed = observed
        .checked_duration_since(started)
        .ok_or(InterruptionEvidenceError::ElapsedUnavailable)?;
    u64::try_from(elapsed.as_nanos()).map_err(|_| InterruptionEvidenceError::ElapsedUnavailable)
}

fn map_interruption_validation_error(
    error: &PreparedGenerationQualificationMandatoryFinalizationError<InterruptionEvidenceError>,
) -> ActiveGenerationQualificationOperationInterruptionError {
    match error {
        PreparedGenerationQualificationMandatoryFinalizationError::Callback(_) => {
            ActiveGenerationQualificationOperationInterruptionError::EvidenceCompilation
        }
        PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal { .. } => {
            ActiveGenerationQualificationOperationInterruptionError::PrimaryAndFinalization
        }
        PreparedGenerationQualificationMandatoryFinalizationError::Initial(_)
        | PreparedGenerationQualificationMandatoryFinalizationError::Final(_)
        | PreparedGenerationQualificationMandatoryFinalizationError::InitialAndFinal { .. } => {
            ActiveGenerationQualificationOperationInterruptionError::MandatoryFinalization
        }
    }
}

#[cfg(test)]
#[path = "interruption/tests.rs"]
mod tests;
