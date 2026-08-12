use rewrite_types::{
    CandidateId, CandidateRank, CandidateTextKind, Digest, DocumentIr, GateResult,
    GeneratedCandidate, MediaType, RewriteMode, RewriteOptions, RewriteUnit, RewriteUnitId,
    SemanticAssessment, SourceSpan, StructuralFingerprint,
};

use super::StructureValidator;
use crate::{
    CancellationToken, CandidateGenerator, GenerationError, GenerationRequest, SemanticEvaluator,
};

pub(super) struct PassStructure;

impl StructureValidator for PassStructure {
    fn validate(&self, _unit: &RewriteUnit, _candidate: &str) -> GateResult {
        GateResult::pass("structure")
    }
}

pub(super) struct NoNewlineChange;

impl StructureValidator for NoNewlineChange {
    fn validate(&self, unit: &RewriteUnit, candidate: &str) -> GateResult {
        if unit.text.matches('\n').count() == candidate.matches('\n').count() {
            GateResult::pass("structure")
        } else {
            GateResult::fail("structure", "newline", "newline count changed")
        }
    }
}

pub(super) struct EmptyGenerator;

impl CandidateGenerator for EmptyGenerator {
    fn id(&self) -> &'static str {
        "empty-fixture"
    }

    fn generate(
        &self,
        _request: &GenerationRequest,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        Ok(Vec::new())
    }
}

pub(super) struct ErrorGenerator(pub(super) GenerationError);

impl CandidateGenerator for ErrorGenerator {
    fn id(&self) -> &'static str {
        "error-fixture"
    }

    fn generate(
        &self,
        _request: &GenerationRequest,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        Err(self.0.clone())
    }
}

pub(super) struct MismatchedUnitGenerator;

impl CandidateGenerator for MismatchedUnitGenerator {
    fn id(&self) -> &'static str {
        "mismatched-unit-fixture"
    }

    fn generate(
        &self,
        request: &GenerationRequest,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        let other_document = rewrite_types::DocumentId::from_digest(&Digest::sha256(b"other"));
        Ok(vec![GeneratedCandidate {
            id: CandidateId::new(&request.unit_id, 0),
            unit_id: RewriteUnitId::new(&other_document, 0),
            text: "Hello.".to_owned(),
            text_kind: CandidateTextKind::Raw,
            rank: CandidateRank::default(),
        }])
    }
}

pub(super) struct MaskedEchoGenerator;

impl CandidateGenerator for MaskedEchoGenerator {
    fn id(&self) -> &'static str {
        "masked-echo-fixture"
    }

    fn generate(
        &self,
        request: &GenerationRequest,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        Ok(vec![GeneratedCandidate {
            id: CandidateId::new(&request.unit_id, 0),
            unit_id: request.unit_id.clone(),
            text: format!("{}!", request.masked_source),
            text_kind: CandidateTextKind::Masked,
            rank: CandidateRank::default(),
        }])
    }
}

pub(super) struct InvalidRankGenerator;

impl CandidateGenerator for InvalidRankGenerator {
    fn id(&self) -> &'static str {
        "invalid-rank-fixture"
    }

    fn generate(
        &self,
        request: &GenerationRequest,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        Ok(vec![GeneratedCandidate {
            id: CandidateId::new(&request.unit_id, 0),
            unit_id: request.unit_id.clone(),
            text: "Hello.".to_owned(),
            text_kind: CandidateTextKind::Raw,
            rank: CandidateRank {
                style: f32::NAN,
                ..CandidateRank::default()
            },
        }])
    }
}

pub(super) struct CancellingGenerator;

impl CandidateGenerator for CancellingGenerator {
    fn id(&self) -> &'static str {
        "cancelling-fixture"
    }

    fn generate(
        &self,
        request: &GenerationRequest,
        cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        cancellation.cancel();
        Ok(vec![GeneratedCandidate {
            id: CandidateId::new(&request.unit_id, 0),
            unit_id: request.unit_id.clone(),
            text: format!("{}.", request.masked_source),
            text_kind: CandidateTextKind::Masked,
            rank: CandidateRank::default(),
        }])
    }
}

pub(super) struct FixedSemantic(pub(super) SemanticAssessment);

impl SemanticEvaluator for FixedSemantic {
    fn id(&self) -> &'static str {
        "fixed-semantic-fixture"
    }

    fn evaluate(&self, _source: &str, _candidate: &str, _mode: RewriteMode) -> SemanticAssessment {
        self.0
    }
}

pub(super) fn document(text: &str) -> DocumentIr {
    let digest = Digest::sha256(text.as_bytes());
    let document_id = rewrite_types::DocumentId::from_digest(&digest);
    DocumentIr::new(
        digest,
        MediaType::PlainText,
        vec![RewriteUnit {
            id: RewriteUnitId::new(&document_id, 0),
            source_span: SourceSpan::new(0, text.len()).expect("valid fixture span"),
            text: text.to_owned(),
        }],
        StructuralFingerprint {
            kind: "fixture".to_owned(),
            digest: Digest::sha256(b"fixture"),
        },
    )
    .expect("valid fixture document")
}

pub(super) fn two_unit_document() -> DocumentIr {
    let digest = Digest::sha256(b"HelloWorld");
    let document_id = rewrite_types::DocumentId::from_digest(&digest);
    DocumentIr::new(
        digest,
        MediaType::PlainText,
        vec![
            RewriteUnit {
                id: RewriteUnitId::new(&document_id, 0),
                source_span: SourceSpan::new(0, 5).expect("valid fixture span"),
                text: "Hello".to_owned(),
            },
            RewriteUnit {
                id: RewriteUnitId::new(&document_id, 1),
                source_span: SourceSpan::new(5, 10).expect("valid fixture span"),
                text: "World".to_owned(),
            },
        ],
        StructuralFingerprint {
            kind: "fixture".to_owned(),
            digest: Digest::sha256(b"fixture"),
        },
    )
    .expect("valid fixture document")
}

pub(super) fn literal_options() -> RewriteOptions {
    RewriteOptions {
        mode: RewriteMode::Literal,
        ..RewriteOptions::default()
    }
}
