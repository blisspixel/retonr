use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{CandidateAssessment, CandidateId, Digest, DocumentId, ReasonCode, RewriteStatus};

/// Current rewrite-record contract version.
pub const REWRITE_RECORD_SCHEMA_VERSION: u32 = 2;

/// Current redacted generation-provenance contract version.
pub const GENERATION_PROVENANCE_SCHEMA_VERSION: u32 = 1;

/// Persistence-neutral identity of the runtime used for one generation call.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationRuntimeProvenance {
    /// Backend implementation identifier.
    pub backend: String,
    /// Exact runtime version observed for discovery and generation.
    pub version: String,
    /// Executable or runtime-package digest when the backend can prove it.
    pub digest: Option<Digest>,
}

/// Optional bounded resource observations for one generation call.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationUsageProvenance {
    /// Runtime-reported input token count.
    pub input_tokens: Option<u64>,
    /// Runtime-reported output token count.
    pub output_tokens: Option<u64>,
    /// Runtime-reported generation duration in microseconds.
    pub generation_micros: Option<u64>,
}

/// Content-redacted evidence for one completed candidate-generation call.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationProvenance {
    /// Generation-provenance contract version.
    pub schema_version: u32,
    /// Stable generation-strategy implementation identifier.
    pub strategy_id: String,
    /// Exact runtime observed for discovery and generation.
    pub runtime: GenerationRuntimeProvenance,
    /// Content-derived installed artifact identity.
    pub artifact_id: Digest,
    /// Exact artifact digest rechecked after generation.
    pub artifact_digest: Digest,
    /// Digest of the versioned instruction template.
    pub prompt_template_digest: Digest,
    /// Digest of the complete serialized backend input.
    pub input_digest: Digest,
    /// Digest of the exact structured-output schema.
    pub output_schema_digest: Digest,
    /// Number of candidates requested and returned.
    pub candidate_count: u8,
    /// Optional bounded resource observations reported by the runtime.
    pub usage: GenerationUsageProvenance,
}

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
    /// Redacted provenance when a candidate-generation call completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<GenerationProvenance>,
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
            schema_version: REWRITE_RECORD_SCHEMA_VERSION,
            document_id,
            source_digest,
            output_digest,
            status,
            reason,
            selected_candidates,
            assessments,
            generation: None,
        }
    }

    /// Attaches the redacted evidence for one completed generation call.
    #[must_use]
    pub fn with_generation(mut self, generation: GenerationProvenance) -> Self {
        self.generation = Some(generation);
        self
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        GENERATION_PROVENANCE_SCHEMA_VERSION, GenerationProvenance, GenerationRuntimeProvenance,
        GenerationUsageProvenance, REWRITE_RECORD_SCHEMA_VERSION, RewriteRecord,
    };
    use crate::{Digest, DocumentId, RewriteStatus};

    fn record() -> RewriteRecord {
        let source = Digest::sha256(b"source");
        RewriteRecord::new(
            DocumentId::from_digest(&source),
            source,
            Digest::sha256(b"output"),
            RewriteStatus::Rewritten,
            None,
            Vec::new(),
            Vec::new(),
        )
    }

    fn generation() -> GenerationProvenance {
        GenerationProvenance {
            schema_version: GENERATION_PROVENANCE_SCHEMA_VERSION,
            strategy_id: "grounded-structured-v1".to_owned(),
            runtime: GenerationRuntimeProvenance {
                backend: "fixture".to_owned(),
                version: "1.2.3".to_owned(),
                digest: Some(Digest::sha256(b"runtime")),
            },
            artifact_id: Digest::sha256(b"artifact-id"),
            artifact_digest: Digest::sha256(b"artifact"),
            prompt_template_digest: Digest::sha256(b"template"),
            input_digest: Digest::sha256(b"private input"),
            output_schema_digest: Digest::sha256(b"schema"),
            candidate_count: 2,
            usage: GenerationUsageProvenance {
                input_tokens: Some(12),
                output_tokens: Some(8),
                generation_micros: Some(100),
            },
        }
    }

    #[test]
    fn model_free_record_omits_generation_and_uses_current_version() {
        let record = record();
        let encoded = serde_json::to_value(record).expect("record serializes");
        assert_eq!(encoded["schema_version"], REWRITE_RECORD_SCHEMA_VERSION);
        assert!(encoded.get("generation").is_none());
    }

    #[test]
    fn generation_is_nested_and_contains_no_raw_input() {
        let encoded = serde_json::to_string(&record().with_generation(generation()))
            .expect("record serializes");
        assert!(encoded.contains("grounded-structured-v1"));
        assert!(!encoded.contains("private input"));
    }

    #[test]
    fn prior_record_without_generation_remains_readable() {
        let mut encoded = serde_json::to_value(record()).expect("record serializes");
        let object = encoded.as_object_mut().expect("record is an object");
        object.insert("schema_version".to_owned(), Value::from(1));
        object.remove("generation");
        let decoded: RewriteRecord = serde_json::from_value(encoded).expect("v1 record decodes");
        assert_eq!(decoded.schema_version, 1);
        assert!(decoded.generation.is_none());
    }
}
