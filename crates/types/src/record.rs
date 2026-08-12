use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    CandidateAssessment, CandidateId, Digest, DocumentId, ReasonCode, RewriteStatus, SCHEMA_VERSION,
};

/// Redacted audit record for one completed rewrite transaction.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RewriteRecord {
    /// Contract schema version.
    pub schema_version: u32,
    /// Source document identifier.
    pub document_id: DocumentId,
    /// Digest of the complete source bytes.
    pub source_digest: Digest,
    /// Digest of the returned output bytes.
    pub output_digest: Digest,
    /// Transaction outcome.
    pub status: RewriteStatus,
    /// Stable explanation for abstention or failure.
    pub reason: Option<ReasonCode>,
    /// Selected candidates in rewrite-unit order.
    pub selected_candidates: Vec<CandidateId>,
    /// Candidate gate results without raw text.
    pub assessments: Vec<CandidateAssessment>,
}

impl RewriteRecord {
    /// Creates a record using the current contract schema.
    #[must_use]
    pub fn new(
        document_id: DocumentId,
        source_digest: Digest,
        output_digest: Digest,
        status: RewriteStatus,
        reason: Option<ReasonCode>,
        selected_candidates: Vec<CandidateId>,
        assessments: Vec<CandidateAssessment>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            document_id,
            source_digest,
            output_digest,
            status,
            reason,
            selected_candidates,
            assessments,
        }
    }
}
