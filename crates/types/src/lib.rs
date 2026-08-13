//! Stable domain contracts shared by every product interface.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bounded_serde;
mod cancellation;
mod claim;
mod claim_comparison;
mod digest;
mod document;
mod record;
mod rewrite;
mod validation;

pub use cancellation::{CancellationToken, Cancelled};
pub use claim::{
    CLAIM_CONFIDENCE_PARTS_PER_MILLION, CLAIM_EVIDENCE_SCHEMA_VERSION, ClaimEvidence,
    ClaimEvidenceError, ClaimEvidenceSet, ClaimExtractionStatus, ClaimModality, ClaimPolarity,
    MAX_CLAIMS_PER_UNIT, MAX_EVIDENCE_SPANS_PER_CLAIM, MAX_EXTRACTOR_ID_BYTES,
};
pub use claim_comparison::{
    CLAIM_COMPARATOR_VERSION, CLAIM_COMPARISON_SCHEMA_VERSION, ClaimComparisonCounts,
    ClaimComparisonEvidence,
};
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
    CandidateAssessment, EvidenceContractError, GateEvidence, GateEvidenceDetails, GateResult,
    GateStatus, InvariantEvidenceSummary, MAX_GATE_EVIDENCE_CODE_BYTES,
    MAX_GATE_EVIDENCE_MESSAGE_BYTES, MAX_GATES_PER_CANDIDATE, MAX_SEMANTIC_EVIDENCE_ITEMS,
    SemanticAssessment, SemanticEvidence, SemanticEvidenceCode, SemanticEvidenceDetails, Severity,
};

/// Current schema version for in-memory and serialized prototype contracts.
pub const SCHEMA_VERSION: u32 = 1;
