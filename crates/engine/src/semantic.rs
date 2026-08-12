use rewrite_types::{GateStatus, RewriteMode, SemanticAssessment};

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
            };
        }

        let source_tokens = lexical_tokens(source);
        let candidate_tokens = lexical_tokens(candidate);
        if source_tokens == candidate_tokens {
            SemanticAssessment {
                status: GateStatus::Pass,
                confidence: Some(1.0),
            }
        } else {
            SemanticAssessment {
                status: GateStatus::Uncertain,
                confidence: None,
            }
        }
    }
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

        let lexical = evaluator.evaluate("Hello world", "Hello there", RewriteMode::Literal);
        assert_eq!(lexical.status, GateStatus::Uncertain);

        let non_literal = evaluator.evaluate("Hello world", "Hello world", RewriteMode::Balanced);
        assert_eq!(non_literal.status, GateStatus::Uncertain);

        let moved_newline =
            evaluator.evaluate("Hello\nworld\n", "Hello world\n\n", RewriteMode::Literal);
        assert_eq!(moved_newline.status, GateStatus::Uncertain);
    }
}
