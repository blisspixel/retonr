//! Deterministic rewrite planning, validation, selection, and abstention.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod engine;
mod generator;
mod protection;
mod selection;
mod semantic;

pub use engine::{
    EngineError, EngineOutcome, MAX_GENERATED_CANDIDATES, MAX_GENERATED_TEXT_BYTES,
    MAX_PROTECTED_TERM_BYTES, MAX_PROTECTED_TERM_TOTAL_BYTES, MAX_PROTECTED_TERMS, RewriteEngine,
    StructureValidator, validate_rewrite_options,
};
pub use generator::{
    CandidateGenerator, GenerationError, GenerationRequest, PreparedCandidateGenerator,
    ProvidedCandidateGenerator,
};
pub use protection::{ProtectedKind, ProtectedValue, ProtectionError, ProtectionPlan};
pub use rewrite_types::CancellationToken;
pub use selection::select_best;
pub use semantic::{LiteralSemanticEvaluator, SemanticEvaluator};
