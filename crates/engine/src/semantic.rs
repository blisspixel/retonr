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

/// The first literal token that differs, without the token text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiteralTokenChange {
    /// Both sides have a token here and the tokens differ.
    Altered {
        /// Byte offset of the source token.
        source_at: usize,
        /// Byte offset of the candidate token.
        candidate_at: usize,
    },
    /// The source has a token the candidate does not.
    Missing {
        /// Byte offset of the source token.
        source_at: usize,
    },
    /// The candidate has a token the source does not.
    Added {
        /// Byte offset of the candidate token.
        candidate_at: usize,
    },
}

pub(crate) fn first_literal_token_change(
    source: &str,
    candidate: &str,
) -> Option<LiteralTokenChange> {
    let source_tokens = lexical_token_spans(source);
    let candidate_tokens = lexical_token_spans(candidate);
    for index in 0..source_tokens.len().max(candidate_tokens.len()) {
        match (source_tokens.get(index), candidate_tokens.get(index)) {
            (Some(left), Some(right)) if left.text != right.text => {
                return Some(LiteralTokenChange::Altered {
                    source_at: left.at,
                    candidate_at: right.at,
                });
            }
            (Some(left), None) => {
                return Some(LiteralTokenChange::Missing { source_at: left.at });
            }
            (None, Some(right)) => {
                return Some(LiteralTokenChange::Added {
                    candidate_at: right.at,
                });
            }
            _ => {}
        }
    }
    None
}

struct LexicalToken {
    text: String,
    at: usize,
}

fn lexical_tokens(text: &str) -> Vec<String> {
    lexical_token_spans(text)
        .into_iter()
        .map(|token| token.text)
        .collect()
}

fn lexical_token_spans(text: &str) -> Vec<LexicalToken> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_at = 0;
    let mut index = 0;
    while let Some((character, width)) = char_at(text, index) {
        if character.is_alphanumeric() {
            if current.is_empty() {
                current_at = index;
            }
            current.extend(character.to_lowercase());
            index += width;
            continue;
        }
        let next_is_digit =
            char_at(text, index + width).is_some_and(|(next, _)| next.is_ascii_digit());
        if character == '.' && current.is_empty() && next_is_digit {
            current_at = index;
            current.push('.');
            index += width;
            continue;
        }
        if !current.is_empty() {
            tokens.push(LexicalToken {
                text: core::mem::take(&mut current),
                at: current_at,
            });
        }
        if character == '\r' || character == '\n' {
            let newline_at = index;
            index += width;
            if character == '\r' && char_at(text, index).is_some_and(|(next, _)| next == '\n') {
                index += 1;
            }
            tokens.push(LexicalToken {
                text: "\n".to_owned(),
                at: newline_at,
            });
            continue;
        }
        index += width;
    }
    if !current.is_empty() {
        tokens.push(LexicalToken {
            text: current,
            at: current_at,
        });
    }
    tokens
}

fn char_at(text: &str, index: usize) -> Option<(char, usize)> {
    let character = text.get(index..)?.chars().next()?;
    Some((character, character.len_utf8()))
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

        let source = "Revenue was -2% year over year, which is better than the -7% we forecast.\n";
        let candidate = "Revenue was -2% year over year, better than the -7% we forecast.\n";
        let change = super::first_literal_token_change(source, candidate).expect("token change");
        assert_eq!(
            change,
            super::LiteralTokenChange::Altered {
                source_at: source.find("which").expect("source word"),
                candidate_at: candidate.find("better").expect("candidate word"),
            }
        );
        assert!(super::first_literal_token_change("Hello world", "Hello, world!").is_none());

        let leading_decimal =
            evaluator.evaluate("Value is .5.", "Value is 5.", RewriteMode::Literal);
        assert_eq!(leading_decimal.status, GateStatus::Uncertain);
    }
}
