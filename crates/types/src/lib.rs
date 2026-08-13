//! Stable domain contracts shared by every product interface.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod cancellation;
mod digest;
mod document;
mod record;
mod rewrite;
mod validation;

pub use cancellation::{CancellationToken, Cancelled};
pub use digest::{Digest, DigestError};
pub use document::{
    DocumentError, DocumentId, DocumentIr, IdentifierError, MediaType, RewriteUnit, RewriteUnitId,
    SourceSpan, SpanError, StructuralFingerprint,
};
pub use record::{
    GENERATION_PROVENANCE_SCHEMA_VERSION, GenerationProvenance, GenerationRuntimeProvenance,
    GenerationUsageProvenance, REWRITE_RECORD_SCHEMA_VERSION, RewriteRecord,
};
pub use rewrite::{
    AcceptedEdit, Atomicity, CandidateId, CandidateIdError, CandidateRank, CandidateTextKind,
    GeneratedCandidate, PlannedUnit, ReasonCode, RewriteMode, RewriteOptions, RewriteStatus,
    TransformationPlan,
};
pub use validation::{
    CandidateAssessment, GateEvidence, GateResult, GateStatus, SemanticAssessment, Severity,
};

/// Current schema version for in-memory and serialized prototype contracts.
pub const SCHEMA_VERSION: u32 = 1;
