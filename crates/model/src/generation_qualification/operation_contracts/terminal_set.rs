//! In-memory terminal set for one qualification operation.
//!
//! The set is one receipt plus an interruption only when the caller supplies
//! interruption facts for a noncompleted receipt. It does not persist, change
//! the store schema, construct a qualification record, or grant live authority.

use std::fmt;

use super::{
    GenerationQualificationOperationContractError, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationOperationReceiptV1,
    GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1,
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationPhaseInterruptionRecordV1Input,
    GenerationQualificationPhaseInterruptionRecordV1Relations,
};

/// Receipt facts and the optional interruption facts for one terminal set.
#[derive(Clone)]
pub struct GenerationQualificationTerminalSetPlan<'a> {
    /// Exact records required to derive the operation receipt.
    pub receipt_relations: GenerationQualificationOperationReceiptV1Relations<'a>,
    /// Runner-derived terminal facts for that receipt.
    pub receipt_input: GenerationQualificationOperationReceiptV1Input,
    /// Interruption facts. Present only for a noncompleted receipt.
    pub interruption: Option<GenerationQualificationTerminalInterruptionPlan<'a>>,
}

/// Recursive policy closure and interruption facts bound to the derived receipt.
#[derive(Clone)]
pub struct GenerationQualificationTerminalInterruptionPlan<'a> {
    /// Complete policy dependency closure.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'a>,
    /// Independently retained operation-policy input.
    pub operation_policy_input: &'a GenerationQualificationOperationPolicyV1Input,
    /// Phase, checkpoint, attempt, and primary reason.
    pub input: GenerationQualificationPhaseInterruptionRecordV1Input,
}

/// Inert receipt and optional interruption derived from one plan.
///
/// This value grants no execution, persistence, activation, qualification, or
/// live-use authority.
#[derive(Clone, Eq, PartialEq)]
pub struct PlannedGenerationQualificationTerminalSet {
    receipt: GenerationQualificationOperationReceiptV1,
    interruption: Option<GenerationQualificationPhaseInterruptionRecordV1>,
}

impl PlannedGenerationQualificationTerminalSet {
    /// Derives the receipt and binds an interruption only to a noncompleted receipt.
    ///
    /// # Errors
    ///
    /// Returns a content-free error when the receipt closure is invalid, an
    /// interruption accompanies a completed receipt, or the interruption does
    /// not close over that same receipt.
    pub fn plan(
        plan: GenerationQualificationTerminalSetPlan<'_>,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        let GenerationQualificationTerminalSetPlan {
            receipt_relations,
            receipt_input,
            interruption,
        } = plan;
        let receipt =
            GenerationQualificationOperationReceiptV1::new(receipt_relations, receipt_input)?;
        let interruption = match interruption {
            None => None,
            Some(interruption) => Some(interruption_for_receipt(
                receipt_relations,
                receipt_input,
                &receipt,
                interruption,
            )?),
        };
        Ok(Self {
            receipt,
            interruption,
        })
    }

    /// Returns the derived operation receipt.
    #[must_use]
    pub const fn receipt(&self) -> &GenerationQualificationOperationReceiptV1 {
        &self.receipt
    }

    /// Returns the interruption bound to this receipt, when the plan supplied one.
    #[must_use]
    pub const fn interruption(&self) -> Option<&GenerationQualificationPhaseInterruptionRecordV1> {
        self.interruption.as_ref()
    }

    /// Splits the set into the receipt and its optional interruption.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        GenerationQualificationOperationReceiptV1,
        Option<GenerationQualificationPhaseInterruptionRecordV1>,
    ) {
        (self.receipt, self.interruption)
    }
}

fn interruption_for_receipt(
    receipt_relations: GenerationQualificationOperationReceiptV1Relations<'_>,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    receipt: &GenerationQualificationOperationReceiptV1,
    interruption: GenerationQualificationTerminalInterruptionPlan<'_>,
) -> Result<
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationOperationContractError,
> {
    if receipt.terminal_status() == GenerationQualificationOperationTerminalStatusV1::Completed {
        return Err(GenerationQualificationOperationContractError::InvalidInterruptionClosure);
    }
    GenerationQualificationPhaseInterruptionRecordV1::new(
        &GenerationQualificationPhaseInterruptionRecordV1Relations {
            operation_policy: receipt_relations.operation_policy,
            operation_policy_relations: interruption.operation_policy_relations,
            operation_policy_input: interruption.operation_policy_input,
            operation_receipt: receipt,
            operation_receipt_relations: receipt_relations,
            operation_receipt_input: receipt_input,
        },
        interruption.input,
    )
}

impl fmt::Debug for PlannedGenerationQualificationTerminalSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlannedGenerationQualificationTerminalSet")
            .field("operation_receipt_id", self.receipt.operation_receipt_id())
            .field(
                "phase_interruption_record_id",
                &self.interruption.as_ref().map(
                    GenerationQualificationPhaseInterruptionRecordV1::phase_interruption_record_id,
                ),
            )
            .finish_non_exhaustive()
    }
}
