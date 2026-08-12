use core::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{DocumentId, RewriteUnitId, SCHEMA_VERSION};

/// Rewrite strength selected by the caller.
#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteMode {
    /// Mechanical edits whose lexical token sequence is unchanged.
    Literal,
    /// Minimal generative change with the common fidelity floor.
    Pure,
    /// Moderate generative change with the common fidelity floor.
    #[default]
    Balanced,
    /// Broader generative change with the same fidelity floor.
    Strong,
}

/// Scope at which accepted edits are committed.
#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Atomicity {
    /// All required units pass or the original document is returned.
    #[default]
    Document,
    /// Independently validated units may be committed separately.
    Unit,
    /// Connected regions may be committed separately.
    Region,
}

/// Caller-controlled rewrite policy.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RewriteOptions {
    /// Requested rewrite strength.
    pub mode: RewriteMode,
    /// Requested commit scope.
    pub atomicity: Atomicity,
    /// Exact terms that candidates must preserve.
    pub protected_terms: Vec<String>,
    /// Minimum accepted calibrated semantic confidence.
    pub minimum_semantic_confidence: f32,
}

impl Default for RewriteOptions {
    fn default() -> Self {
        Self {
            mode: RewriteMode::Balanced,
            atomicity: Atomicity::Document,
            protected_terms: Vec::new(),
            minimum_semantic_confidence: 0.95,
        }
    }
}

/// Identifier for one generated candidate.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CandidateId(String);

impl CandidateId {
    /// Creates a candidate identifier scoped to a rewrite unit.
    #[must_use]
    pub fn new(unit: &RewriteUnitId, ordinal: usize) -> Self {
        Self(format!("{}:candidate:{ordinal}", unit.as_str()))
    }

    /// Returns the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for CandidateId {
    type Err = CandidateIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((unit, ordinal)) = value.rsplit_once(":candidate:") else {
            return Err(CandidateIdError);
        };
        unit.parse::<RewriteUnitId>()
            .map_err(|_error| CandidateIdError)?;
        let parsed = ordinal
            .parse::<usize>()
            .map_err(|_error| CandidateIdError)?;
        if parsed.to_string() != ordinal {
            return Err(CandidateIdError);
        }
        Ok(Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for CandidateId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Error returned for a noncanonical candidate identifier.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid candidate identifier")]
pub struct CandidateIdError;

/// Whether a generator returned protected sentinels or restored source literals.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateTextKind {
    /// Candidate still contains engine-issued sentinel tokens.
    Masked,
    /// Candidate contains original literal surfaces.
    Raw,
}

/// Lexicographic scores assigned after validation.
#[derive(Clone, Copy, Debug, Default, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct CandidateRank {
    /// Personal style fit in the inclusive range from zero to one.
    pub style: f32,
    /// Situational channel fit in the inclusive range from zero to one.
    pub channel: f32,
    /// Fluency in the inclusive range from zero to one.
    pub fluency: f32,
    /// Deterministic surface edit cost, where lower is preferred.
    pub edit_cost: u64,
}

/// Candidate returned by a generation strategy.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct GeneratedCandidate {
    /// Candidate identifier.
    pub id: CandidateId,
    /// Unit this candidate replaces.
    pub unit_id: RewriteUnitId,
    /// Generated text in the declared representation.
    pub text: String,
    /// Whether the text contains sentinels or original literals.
    pub text_kind: CandidateTextKind,
    /// Ranking evidence assigned outside the generator in production.
    pub rank: CandidateRank,
}

/// Immutable per-unit plan passed to a generator.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct PlannedUnit {
    /// Unit identifier.
    pub unit_id: RewriteUnitId,
    /// Source text with protected values replaced by sentinels.
    pub masked_text: String,
}

/// Immutable plan for one rewrite transaction.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct TransformationPlan {
    /// Contract schema version.
    pub schema_version: u32,
    /// Source document identifier.
    pub document_id: DocumentId,
    /// Planned units.
    pub units: Vec<PlannedUnit>,
}

impl TransformationPlan {
    /// Creates a plan using the current contract schema.
    #[must_use]
    pub fn new(document_id: DocumentId, units: Vec<PlannedUnit>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            document_id,
            units,
        }
    }
}

/// Replacement accepted by every required validation gate.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct AcceptedEdit {
    /// Unit being replaced.
    pub unit_id: RewriteUnitId,
    /// Fully restored replacement text.
    pub replacement: String,
}

/// Stable outcome category for a rewrite transaction.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteStatus {
    /// At least one validated edit was committed.
    Rewritten,
    /// The document contained no eligible text.
    UnchangedNoEligibleContent,
    /// The original was returned because no candidate was eligible.
    Abstained,
    /// Processing failed before a safe decision could be completed.
    Failed,
}

/// Stable machine-readable explanation for abstention or failure.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// No candidate was returned by the selected strategy.
    NoCandidate,
    /// Candidate metadata violated the generation contract.
    InvalidCandidate,
    /// At least one required sentinel was changed, removed, or duplicated.
    SentinelIntegrity,
    /// An exact protected literal or declared term changed.
    ProtectedValueChanged,
    /// The candidate changed required document structure.
    StructureChanged,
    /// The candidate introduced unsafe control or directionality characters.
    UnsafeText,
    /// Semantic evidence failed the configured policy.
    SemanticMismatch,
    /// Semantic evidence was insufficient for acceptance.
    SemanticUncertain,
    /// The completed document failed adapter verification.
    ReassemblyVerification,
    /// Processing was cancelled by the caller.
    Cancelled,
    /// The requested atomicity is not implemented by the current adapter.
    UnsupportedAtomicity,
}

#[cfg(test)]
mod tests {
    use super::{CandidateId, PlannedUnit, TransformationPlan};
    use crate::{Digest, DocumentId, RewriteUnitId, SCHEMA_VERSION};

    #[test]
    fn transformation_plan_round_trips_with_schema_version() {
        let document = DocumentId::from_digest(&Digest::sha256(b"fixture"));
        let plan = TransformationPlan::new(
            document.clone(),
            vec![PlannedUnit {
                unit_id: RewriteUnitId::new(&document, 0),
                masked_text: "masked fixture".to_owned(),
            }],
        );
        let encoded = serde_json::to_string(&plan).expect("plan serializes");
        let decoded: TransformationPlan =
            serde_json::from_str(&encoded).expect("plan deserializes");
        assert_eq!(decoded, plan);
        assert_eq!(decoded.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn candidate_identifier_deserialization_enforces_its_shape() {
        assert!(serde_json::from_str::<CandidateId>("\"candidate:0\"").is_err());
        let document = DocumentId::from_digest(&Digest::sha256(b"fixture"));
        let candidate = CandidateId::new(&RewriteUnitId::new(&document, 0), 2);
        let encoded = serde_json::to_string(&candidate).expect("candidate ID serializes");
        assert_eq!(
            serde_json::from_str::<CandidateId>(&encoded)
                .expect("canonical candidate ID deserializes"),
            candidate
        );
    }
}
