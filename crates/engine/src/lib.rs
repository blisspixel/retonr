//! Deterministic rewrite planning, validation, selection, and abstention.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod claim_comparison;
mod engine;
#[cfg(test)]
#[path = "engine_test_support.rs"]
mod engine_test_support;
#[cfg(test)]
#[path = "engine_tests.rs"]
mod engine_tests;
mod generator;
mod policy;
mod protection;
#[cfg(test)]
mod protection_tests;
mod selection;
mod semantic;
#[cfg(test)]
mod semantic_binding_tests;
mod structure;

pub use claim_comparison::{ClaimComparisonError, ClaimEvidenceComparator};
pub use engine::{
    EngineError, EngineOutcome, MAX_GENERATED_CANDIDATES, MAX_GENERATED_TEXT_BYTES, RewriteEngine,
};
pub use generator::{
    CandidateGenerator, GenerationError, GenerationRequest, PreparedCandidateGenerator,
    ProvidedCandidateGenerator,
};
pub use policy::validate_rewrite_options;
pub use protection::{
    MAX_PROTECTED_OCCURRENCES, MAX_PROTECTED_TERM_BYTES, MAX_PROTECTED_TERM_TOTAL_BYTES,
    MAX_PROTECTED_TERMS, MAX_PROTECTED_TEXT_BYTES, ProtectedKind, ProtectedValue, ProtectionError,
    ProtectionPlan,
};
pub use rewrite_types::CancellationToken;
pub use selection::select_best;
pub use semantic::{LiteralSemanticEvaluator, SemanticEvaluator};
pub use structure::{StructureAssessment, StructureValidator};
