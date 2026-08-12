use rewrite_types::{
    CandidateId, CandidateRank, CandidateTextKind, GeneratedCandidate, RewriteMode, RewriteUnitId,
};
use thiserror::Error;

use crate::{CancellationToken, ProtectedValue};

/// Immutable request passed to a candidate generation strategy.
#[derive(Clone, Debug)]
pub struct GenerationRequest {
    /// Unit being rewritten.
    pub unit_id: RewriteUnitId,
    /// Source text with protected values replaced by sentinels.
    pub masked_source: String,
    /// Protected values and their issued sentinel tokens.
    pub protected_values: Vec<ProtectedValue>,
    /// Requested rewrite strength.
    pub mode: RewriteMode,
}

/// Candidate generation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GenerationError {
    /// Generation was cancelled cooperatively.
    #[error("candidate generation was cancelled")]
    Cancelled,
    /// The configured provider could not produce a candidate.
    #[error("candidate provider failed: {0}")]
    Provider(String),
}

/// Port implemented by local candidate generation backends.
pub trait CandidateGenerator: Send + Sync {
    /// Stable generation strategy identifier.
    fn id(&self) -> &'static str;

    /// Produces complete candidates for one rewrite unit.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationError`] for cancellation or provider failure.
    fn generate(
        &self,
        request: &GenerationRequest,
        cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError>;
}

/// Model-free generator that presents caller-supplied raw candidates to the engine.
///
/// This implementation exists for deterministic evaluation and the private
/// candidate-check interface. It is not a semantic evaluator.
#[derive(Clone, Debug)]
pub struct ProvidedCandidateGenerator {
    candidates: Vec<String>,
}

/// Generator that presents already constructed candidates to the common engine.
///
/// Grounded strategies use this boundary after generation. The engine still
/// validates unit identity, representation, bounds, sentinels, structure, and
/// semantic evidence for every candidate.
#[derive(Clone, Debug)]
pub struct PreparedCandidateGenerator {
    candidates: Vec<GeneratedCandidate>,
}

impl PreparedCandidateGenerator {
    /// Creates a generator from untrusted preconstructed candidates.
    #[must_use]
    pub fn new(candidates: Vec<GeneratedCandidate>) -> Self {
        Self { candidates }
    }
}

impl CandidateGenerator for PreparedCandidateGenerator {
    fn id(&self) -> &'static str {
        "prepared-candidate-v1"
    }

    fn generate(
        &self,
        _request: &GenerationRequest,
        cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        if cancellation.is_cancelled() {
            Err(GenerationError::Cancelled)
        } else {
            Ok(self.candidates.clone())
        }
    }
}

impl ProvidedCandidateGenerator {
    /// Creates a generator from restored candidate text.
    #[must_use]
    pub fn new(candidates: Vec<String>) -> Self {
        Self { candidates }
    }
}

impl CandidateGenerator for ProvidedCandidateGenerator {
    fn id(&self) -> &'static str {
        "provided-candidate-v1"
    }

    fn generate(
        &self,
        request: &GenerationRequest,
        cancellation: &CancellationToken,
    ) -> Result<Vec<GeneratedCandidate>, GenerationError> {
        if cancellation.is_cancelled() {
            return Err(GenerationError::Cancelled);
        }

        Ok(self
            .candidates
            .iter()
            .enumerate()
            .map(|(ordinal, text)| GeneratedCandidate {
                id: CandidateId::new(&request.unit_id, ordinal),
                unit_id: request.unit_id.clone(),
                text: text.clone(),
                text_kind: CandidateTextKind::Raw,
                rank: CandidateRank::default(),
            })
            .collect())
    }
}
