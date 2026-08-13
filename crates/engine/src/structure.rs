use rewrite_types::{GateResult, ReasonCode, RewriteUnit};

/// Product-owned structural outcome returned by an adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureAssessment {
    /// The candidate preserves the format-specific structural contract.
    Preserved,
    /// The candidate introduced a control or other unsafe text construct.
    UnsafeText,
    /// The candidate changed a format-specific structural invariant.
    Changed,
}

/// Adapter-owned structural validation applied to restored candidate text.
pub trait StructureValidator: Send + Sync {
    /// Returns a closed outcome for the candidate's source-bound structure.
    fn validate(&self, unit: &RewriteUnit, candidate: &str) -> StructureAssessment;
}

pub(crate) fn retained_gate(assessment: StructureAssessment) -> (GateResult, ReasonCode) {
    match assessment {
        StructureAssessment::Preserved => {
            (GateResult::pass("structure"), ReasonCode::StructureChanged)
        }
        StructureAssessment::UnsafeText => (
            GateResult::fail(
                "structure",
                "unsafe_text_control",
                "candidate introduced an unsafe text control",
            ),
            ReasonCode::UnsafeText,
        ),
        StructureAssessment::Changed => (
            GateResult::fail(
                "structure",
                "structure_changed",
                "candidate changed a source structural invariant",
            ),
            ReasonCode::StructureChanged,
        ),
    }
}
