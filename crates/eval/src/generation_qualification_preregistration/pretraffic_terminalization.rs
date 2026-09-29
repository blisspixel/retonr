//! Exact terminalization for policy-directed pretraffic rejection.

use std::{fmt, time::Instant};

use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationRecordV1, GenerationQualificationTerminalSetPlan,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationResourceEvidenceManifestV1, GenerationResourceEvidenceManifestV1Relations,
    PlannedGenerationQualificationTerminalSet,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

mod phase_policy_refusal;
mod relations;

pub use phase_policy_refusal::{
    FinalizedPhasePolicyRefusedPretrafficGenerationQualification,
    GenerationQualificationPhasePolicyRefusalError,
};

use relations::{
    phase_scope, receipt_relations, receipt_relations_from_evidence, record_relations,
    record_relations_from_evidence,
};

use super::{
    GenerationQualificationPreparationDisposition, GenerationQualificationPreparationError,
    PreparedGenerationQualificationOperation, check_gate,
    prepared::{
        PreparedGenerationQualificationValidationError,
        PreparedGenerationQualificationValidationView,
    },
};

/// Content-redacted failure while closing a rejected pretraffic operation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPretrafficTerminalizationError {
    /// Fresh validation of the retained Prepared authority failed.
    #[error("generation qualification prepared authority does not match")]
    PreparedAuthority,
    /// A traffic-eligible operation cannot use the rejection terminalizer.
    #[error("generation qualification operation is traffic eligible")]
    TrafficEligible,
    /// The monotonic elapsed interval could not be represented exactly.
    #[error("generation qualification operation elapsed time is unavailable")]
    ElapsedUnavailable,
    /// One exact skipped phase manifest could not be derived or revalidated.
    #[error("generation qualification skipped phase closure does not match")]
    PhaseClosureMismatch,
    /// The exact terminal operation receipt could not be derived or revalidated.
    #[error("generation qualification operation receipt does not match")]
    ReceiptMismatch,
    /// The compact inert rejected record could not be derived or revalidated.
    #[error("generation qualification record does not match")]
    QualificationRecordMismatch,
    /// Multiple independently retained validation stages failed in one pass.
    #[error("generation qualification terminal validation failures were aggregated")]
    ValidationAggregation,
    /// The original absolute operation deadline was reached.
    #[error("generation qualification preparation deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("generation qualification preparation was cancelled")]
    Cancelled,
}

struct RejectedPretrafficEvidence {
    attempt_ledger_manifest: GenerationAttemptLedgerManifestV1,
    repeatability_manifest: GenerationRepeatabilityEvidenceManifestV1,
    resource_manifest: GenerationResourceEvidenceManifestV1,
    human_adjudication_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    receipt: GenerationQualificationOperationReceiptV1,
    record: GenerationQualificationRecordV1,
}

/// Closed, noncloneable pretraffic rejection retaining its exact Prepared authority.
///
/// This value owns inert skipped manifests, the exact terminal receipt, and the
/// compact rejected record. It grants no launch, network, generation, traffic,
/// qualification-verification, activation, or live-use authority.
///
/// ```compile_fail
/// use rewrite_eval::FinalizedRejectedPretrafficGenerationQualification;
/// fn cannot_clone(
///     value: &FinalizedRejectedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
/// ) {
///     let _forged = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::FinalizedRejectedPretrafficGenerationQualification;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(
///     value: &FinalizedRejectedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
/// ) {
///     require_serialize(value);
/// }
/// ```
pub struct FinalizedRejectedPretrafficGenerationQualification<
    'records,
    'store,
    'platform,
    'proof,
    'lease,
> {
    prepared: PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
    evidence: RejectedPretrafficEvidence,
}

impl<'records, 'store, 'platform, 'proof, 'lease>
    PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>
{
    /// Consumes a policy-rejected Prepared state and derives its exact terminal closure.
    ///
    /// The traffic-eligible disposition fails closed and this method performs no
    /// managed launch, network, candidate, judge, or model traffic operation.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for traffic eligibility, expiry,
    /// cancellation, monotonic clock failure, or any substituted exact relation.
    pub fn finalize_rejected_pretraffic(
        self,
        cancellation: &CancellationToken,
    ) -> Result<
        FinalizedRejectedPretrafficGenerationQualification<
            'records,
            'store,
            'platform,
            'proof,
            'lease,
        >,
        GenerationQualificationPretrafficTerminalizationError,
    > {
        FinalizedRejectedPretrafficGenerationQualification::from_prepared(self, cancellation)
    }
}

impl<'records, 'store, 'platform, 'proof, 'lease>
    FinalizedRejectedPretrafficGenerationQualification<'records, 'store, 'platform, 'proof, 'lease>
{
    fn from_prepared(
        mut prepared: PreparedGenerationQualificationOperation<
            'records,
            'store,
            'platform,
            'proof,
            'lease,
        >,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationPretrafficTerminalizationError> {
        let evidence = prepared
            .with_validated_view(cancellation, |view| {
                derive_rejected_pretraffic_evidence(&view, cancellation)
            })
            .map_err(map_validated_view_error)?;
        Ok(Self { prepared, evidence })
    }

    /// Returns the exact inert terminal receipt.
    #[must_use]
    pub const fn receipt(&self) -> &GenerationQualificationOperationReceiptV1 {
        &self.evidence.receipt
    }

    /// Returns the compact inert qualification record.
    #[must_use]
    pub const fn record(&self) -> &GenerationQualificationRecordV1 {
        &self.evidence.record
    }

    /// Returns the exact skipped attempt-ledger manifest.
    #[must_use]
    pub const fn attempt_ledger_manifest(&self) -> &GenerationAttemptLedgerManifestV1 {
        &self.evidence.attempt_ledger_manifest
    }

    /// Returns the exact skipped repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(&self) -> &GenerationRepeatabilityEvidenceManifestV1 {
        &self.evidence.repeatability_manifest
    }

    /// Returns the exact skipped resource manifest.
    #[must_use]
    pub const fn resource_manifest(&self) -> &GenerationResourceEvidenceManifestV1 {
        &self.evidence.resource_manifest
    }

    /// Returns the exact skipped human-adjudication manifest.
    #[must_use]
    pub const fn human_adjudication_manifest(
        &self,
    ) -> &GenerationHumanAdjudicationEvidenceManifestV1 {
        &self.evidence.human_adjudication_manifest
    }

    /// Freshly revalidates the retained Prepared authority and exact terminal closure.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for original deadline expiry,
    /// cancellation, retained authority drift, or any changed terminal relation.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationPretrafficTerminalizationError> {
        let Self { prepared, evidence } = self;
        prepared
            .with_validated_view(cancellation, |view| {
                validate_rejected_pretraffic_evidence(&view, evidence, cancellation)
            })
            .map_err(map_validated_view_error)
    }
}

impl fmt::Debug for FinalizedRejectedPretrafficGenerationQualification<'_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalizedRejectedPretrafficGenerationQualification")
            .field(
                "operation_receipt_id",
                self.evidence.receipt.operation_receipt_id(),
            )
            .field(
                "generation_qualification_id",
                self.evidence.record.generation_qualification_id(),
            )
            .field("status", &self.evidence.record.status())
            .finish_non_exhaustive()
    }
}

fn derive_rejected_pretraffic_evidence(
    view: &PreparedGenerationQualificationValidationView<'_>,
    cancellation: &CancellationToken,
) -> Result<RejectedPretrafficEvidence, GenerationQualificationPretrafficTerminalizationError> {
    ensure_rejected_disposition(view.disposition)?;
    check_terminal_gate(view.deadline, cancellation)?;
    let manifests = derive_skipped_manifests(view)?;
    let receipt_input = rejected_receipt_input(checked_elapsed_nanoseconds(
        view.started,
        view.deadline,
        cancellation,
    )?);
    let (receipt, None) =
        PlannedGenerationQualificationTerminalSet::plan(GenerationQualificationTerminalSetPlan {
            receipt_relations: receipt_relations(view, &manifests),
            receipt_input,
            interruption: None,
        })
        .map_err(|_| GenerationQualificationPretrafficTerminalizationError::ReceiptMismatch)?
        .into_parts()
    else {
        return Err(GenerationQualificationPretrafficTerminalizationError::ReceiptMismatch);
    };
    let record = GenerationQualificationRecordV1::new(&record_relations(
        view,
        &manifests,
        &receipt,
        receipt_input,
    ))
    .map_err(|_| {
        GenerationQualificationPretrafficTerminalizationError::QualificationRecordMismatch
    })?;
    check_terminal_gate(view.deadline, cancellation)?;
    Ok(RejectedPretrafficEvidence {
        attempt_ledger_manifest: manifests.attempt_ledger,
        repeatability_manifest: manifests.repeatability,
        resource_manifest: manifests.resource,
        human_adjudication_manifest: manifests.human_adjudication,
        receipt_input,
        receipt,
        record,
    })
}

fn validate_rejected_pretraffic_evidence(
    view: &PreparedGenerationQualificationValidationView<'_>,
    evidence: &RejectedPretrafficEvidence,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPretrafficTerminalizationError> {
    ensure_rejected_disposition(view.disposition)?;
    check_terminal_gate(view.deadline, cancellation)?;
    validate_manifests(view, evidence)?;
    let relations = receipt_relations_from_evidence(view, evidence);
    evidence
        .receipt
        .validate_against(relations, evidence.receipt_input)
        .map_err(|_| GenerationQualificationPretrafficTerminalizationError::ReceiptMismatch)?;
    evidence
        .record
        .validate_against(&record_relations_from_evidence(view, evidence))
        .map_err(|_| {
            GenerationQualificationPretrafficTerminalizationError::QualificationRecordMismatch
        })?;
    check_terminal_gate(view.deadline, cancellation)
}

struct SkippedPhaseManifests {
    attempt_ledger: GenerationAttemptLedgerManifestV1,
    repeatability: GenerationRepeatabilityEvidenceManifestV1,
    resource: GenerationResourceEvidenceManifestV1,
    human_adjudication: GenerationHumanAdjudicationEvidenceManifestV1,
}

fn derive_skipped_manifests(
    view: &PreparedGenerationQualificationValidationView<'_>,
) -> Result<SkippedPhaseManifests, GenerationQualificationPretrafficTerminalizationError> {
    let scope = phase_scope(view);
    let status = GenerationQualificationPhaseStatusV1::Skipped;
    let attempt_ledger_manifest =
        GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
            scope,
            phase_policy_digest: view.operation_policy.attempt_ledger_policy_digest(),
            planned_attempts: view.operation_policy_relations.planned_attempts,
            attempt_records: &[],
            status,
        })
        .map_err(|_| GenerationQualificationPretrafficTerminalizationError::PhaseClosureMismatch)?;
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
    .map_err(|_| GenerationQualificationPretrafficTerminalizationError::PhaseClosureMismatch)?;
    let resource_manifest =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: view.operation_policy.resource_policy_digest(),
            evidence_record_digests: &[],
            status,
        })
        .map_err(|_| GenerationQualificationPretrafficTerminalizationError::PhaseClosureMismatch)?;
    let human_adjudication_manifest = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: view.operation_policy.human_adjudication_policy_digest(),
            evidence_record_digests: &[],
            status,
        },
    )
    .map_err(|_| GenerationQualificationPretrafficTerminalizationError::PhaseClosureMismatch)?;
    Ok(SkippedPhaseManifests {
        attempt_ledger: attempt_ledger_manifest,
        repeatability: repeatability_manifest,
        resource: resource_manifest,
        human_adjudication: human_adjudication_manifest,
    })
}

fn validate_manifests(
    view: &PreparedGenerationQualificationValidationView<'_>,
    evidence: &RejectedPretrafficEvidence,
) -> Result<(), GenerationQualificationPretrafficTerminalizationError> {
    let expected = derive_skipped_manifests(view)?;
    if expected.attempt_ledger == evidence.attempt_ledger_manifest
        && expected.repeatability == evidence.repeatability_manifest
        && expected.resource == evidence.resource_manifest
        && expected.human_adjudication == evidence.human_adjudication_manifest
    {
        Ok(())
    } else {
        Err(GenerationQualificationPretrafficTerminalizationError::PhaseClosureMismatch)
    }
}

const fn rejected_receipt_input(
    elapsed_nanoseconds: u64,
) -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds,
        peak_concurrent_attempts: 0,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Completed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    }
}

fn checked_elapsed_nanoseconds(
    started: Instant,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<u64, GenerationQualificationPretrafficTerminalizationError> {
    check_terminal_gate(deadline, cancellation)?;
    let observed = Instant::now();
    let elapsed = observed
        .checked_duration_since(started)
        .ok_or(GenerationQualificationPretrafficTerminalizationError::ElapsedUnavailable)?;
    let elapsed = u64::try_from(elapsed.as_nanos())
        .map_err(|_| GenerationQualificationPretrafficTerminalizationError::ElapsedUnavailable)?;
    if observed >= deadline {
        Err(GenerationQualificationPretrafficTerminalizationError::DeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Err(GenerationQualificationPretrafficTerminalizationError::Cancelled)
    } else {
        Ok(elapsed)
    }
}

const fn ensure_rejected_disposition(
    disposition: GenerationQualificationPreparationDisposition,
) -> Result<(), GenerationQualificationPretrafficTerminalizationError> {
    match disposition {
        GenerationQualificationPreparationDisposition::PlatformRejected
        | GenerationQualificationPreparationDisposition::LicenseRejected => Ok(()),
        GenerationQualificationPreparationDisposition::TrafficEligible => {
            Err(GenerationQualificationPretrafficTerminalizationError::TrafficEligible)
        }
    }
}

fn check_terminal_gate(
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPretrafficTerminalizationError> {
    check_gate(deadline, cancellation).map_err(map_preparation_error)
}

fn map_validated_view_error(
    error: PreparedGenerationQualificationValidationError<
        GenerationQualificationPretrafficTerminalizationError,
    >,
) -> GenerationQualificationPretrafficTerminalizationError {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            map_preparation_error(error)
        }
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::InitialAndFinal {
            initial,
            final_validation,
        } => {
            let _ = (initial, final_validation);
            GenerationQualificationPretrafficTerminalizationError::ValidationAggregation
        }
        PreparedGenerationQualificationValidationError::CallbackAndFinal {
            callback,
            final_validation,
        } => {
            let _ = (callback, final_validation);
            GenerationQualificationPretrafficTerminalizationError::ValidationAggregation
        }
    }
}

const fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> GenerationQualificationPretrafficTerminalizationError {
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => {
            GenerationQualificationPretrafficTerminalizationError::DeadlineExceeded
        }
        GenerationQualificationPreparationError::Cancelled => {
            GenerationQualificationPretrafficTerminalizationError::Cancelled
        }
        _ => GenerationQualificationPretrafficTerminalizationError::PreparedAuthority,
    }
}

#[cfg(test)]
#[path = "pretraffic_terminalization/tests.rs"]
mod tests;
