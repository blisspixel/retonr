use rewrite_types::{
    Digest, GateStatus, RewriteMode, RewriteUnitId, SemanticAssessment, SemanticEvidence,
    SemanticEvidenceCode, SemanticEvidenceDetails,
};

pub(crate) fn evidence_matches(
    assessment: &SemanticAssessment,
    unit_id: &RewriteUnitId,
    source: &str,
    candidate: &str,
) -> bool {
    let source_digest = Digest::sha256(source.as_bytes());
    let candidate_digest = Digest::sha256(candidate.as_bytes());
    assessment.evidence.iter().all(|evidence| {
        let Some(SemanticEvidenceDetails::ClaimComparison(comparison)) = evidence.details.as_ref()
        else {
            return true;
        };
        comparison.unit_id() == unit_id
            && comparison.source_text_digest() == &source_digest
            && comparison.candidate_text_digest() == &candidate_digest
            && comparison.source_text_bytes() == source.len() as u64
            && comparison.candidate_text_bytes() == candidate.len() as u64
    })
}

/// Port implemented by an independently qualified semantic evaluator.
pub trait SemanticEvaluator: Send + Sync {
    /// Stable evaluator identifier.
    fn id(&self) -> &'static str;

    /// Assesses whether a restored candidate remains semantically eligible.
    fn evaluate(&self, source: &str, candidate: &str, mode: RewriteMode) -> SemanticAssessment;
}

/// Conservative model-free evaluator for the literal strategy only.
///
/// It accepts exact alphanumeric token-sequence equality. Any lexical change or
/// non-literal mode is uncertain, not equivalent.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteralSemanticEvaluator;

impl SemanticEvaluator for LiteralSemanticEvaluator {
    fn id(&self) -> &'static str {
        "literal-token-sequence-v1"
    }

    fn evaluate(&self, source: &str, candidate: &str, mode: RewriteMode) -> SemanticAssessment {
        if mode != RewriteMode::Literal {
            return SemanticAssessment {
                status: GateStatus::Uncertain,
                confidence: None,
                evidence: vec![evidence(SemanticEvidenceCode::UnsupportedMode)],
            };
        }

        let source_tokens = lexical_tokens(source);
        let candidate_tokens = lexical_tokens(candidate);
        if source_tokens == candidate_tokens {
            SemanticAssessment {
                status: GateStatus::Pass,
                confidence: Some(1.0),
                evidence: vec![evidence(SemanticEvidenceCode::LiteralTokensEqual)],
            }
        } else {
            SemanticAssessment {
                status: GateStatus::Uncertain,
                confidence: None,
                evidence: vec![evidence(SemanticEvidenceCode::LiteralTokensChanged)],
            }
        }
    }
}

const fn evidence(code: SemanticEvidenceCode) -> SemanticEvidence {
    SemanticEvidence::new(code, None)
}

fn lexical_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character.is_alphanumeric() {
            current.extend(character.to_lowercase());
            continue;
        }
        if !current.is_empty() {
            tokens.push(core::mem::take(&mut current));
        }
        if character == '\r' || character == '\n' {
            if character == '\r' && characters.peek() == Some(&'\n') {
                characters.next();
            }
            tokens.push("\n".to_owned());
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use rewrite_types::{GateStatus, RewriteMode};

    use super::{LiteralSemanticEvaluator, SemanticEvaluator};

    #[test]
    fn accepts_only_literal_token_equality() {
        let evaluator = LiteralSemanticEvaluator;
        let punctuation = evaluator.evaluate("Hello world", "Hello, world!", RewriteMode::Literal);
        assert_eq!(punctuation.status, GateStatus::Pass);
        assert_eq!(
            punctuation.evidence[0].code.as_str(),
            "literal_tokens_equal"
        );

        let lexical = evaluator.evaluate("Hello world", "Hello there", RewriteMode::Literal);
        assert_eq!(lexical.status, GateStatus::Uncertain);
        assert_eq!(lexical.evidence[0].code.as_str(), "literal_tokens_changed");

        let non_literal = evaluator.evaluate("Hello world", "Hello world", RewriteMode::Balanced);
        assert_eq!(non_literal.status, GateStatus::Uncertain);
        assert_eq!(non_literal.evidence[0].code.as_str(), "unsupported_mode");

        let moved_newline =
            evaluator.evaluate("Hello\nworld\n", "Hello world\n\n", RewriteMode::Literal);
        assert_eq!(moved_newline.status, GateStatus::Uncertain);
    }
}
