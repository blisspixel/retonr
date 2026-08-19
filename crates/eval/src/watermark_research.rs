//! Research-only fixtures that refuse to treat style as a watermark.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current watermark-research corpus schema version.
pub const WATERMARK_RESEARCH_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized corpus size.
pub const MAX_WATERMARK_RESEARCH_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASES: usize = 128;
const MAX_TEXT_BYTES: usize = 4 * 1024;

/// Development corpus that catalogs refusal and observation cases only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatermarkResearchCorpus {
    /// Corpus contract version.
    pub schema_version: u32,
    /// Stable corpus identity.
    pub corpus_id: String,
    /// Always true for this corpus.
    pub research_only: bool,
    /// Live rewrite authority granted by this file.
    pub live_engine_authority: LiveEngineAuthority,
    /// Whether a public embedder produced marked text.
    pub known_mark_generation: KnownMarkGeneration,
    /// Cases.
    pub cases: Vec<WatermarkResearchCase>,
}

/// Authority this corpus may grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveEngineAuthority {
    /// No generation, ranking, or acceptance authority.
    None,
}

/// Whether marked text was generated under a named scheme.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnownMarkGeneration {
    /// No public embedder was run.
    NotPerformed,
}

/// One research case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatermarkResearchCase {
    /// Stable case identifier.
    pub id: String,
    /// Expected library outcome.
    pub expected_outcome: WatermarkResearchOutcome,
    /// Optional paired unmarked control identifier.
    pub unmarked_control_id: Option<String>,
    /// Synthetic text used by the case.
    pub source: String,
    /// Why the case exists.
    pub notes: String,
}

/// Honest outcomes admitted by this corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkResearchOutcome {
    /// Style or folklore was offered as a mark and must be refused.
    RefusedStyleAsMark,
    /// Literal bytes or comments were inventoried without a detector verdict.
    LiteralObservation,
    /// A structured-text-shaped comment is unparsed and untrusted.
    CarrierShapeUnparsed,
    /// Matched prose without the bait pattern or extra carrier.
    UnmarkedControl,
}

/// Content-free summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WatermarkResearchSummary {
    /// Corpus contract version.
    pub schema_version: u32,
    /// Stable corpus identity.
    pub corpus_id: String,
    /// Total cases.
    pub total: usize,
    /// Style-as-mark refusals.
    pub refused_style_as_mark: usize,
    /// Unmarked controls.
    pub unmarked_controls: usize,
}

/// Watermark-research corpus failure.
#[derive(Debug, Error)]
pub enum WatermarkResearchError {
    /// Serialized input exceeds the byte bound.
    #[error("watermark-research corpus exceeds the supported byte limit")]
    TooLarge,
    /// JSON is invalid or contains an unknown field.
    #[error("invalid watermark-research corpus: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// Unsupported schema version.
    #[error("unsupported watermark-research schema version {0}")]
    UnsupportedSchema(u32),
    /// Corpus contract is invalid.
    #[error("watermark-research corpus contract is invalid")]
    InvalidCorpus,
    /// One case failed validation.
    #[error("watermark-research case {index} is invalid")]
    InvalidCase {
        /// Zero-based case position.
        index: usize,
    },
    /// Duplicate case identifier.
    #[error("watermark-research case {index} has a duplicate identifier")]
    DuplicateCaseId {
        /// Zero-based case position.
        index: usize,
    },
}

impl WatermarkResearchCorpus {
    /// Returns a content-free summary.
    #[must_use]
    pub fn summary(&self) -> WatermarkResearchSummary {
        WatermarkResearchSummary {
            schema_version: self.schema_version,
            corpus_id: self.corpus_id.clone(),
            total: self.cases.len(),
            refused_style_as_mark: self
                .cases
                .iter()
                .filter(|case| {
                    case.expected_outcome == WatermarkResearchOutcome::RefusedStyleAsMark
                })
                .count(),
            unmarked_controls: self
                .cases
                .iter()
                .filter(|case| case.expected_outcome == WatermarkResearchOutcome::UnmarkedControl)
                .count(),
        }
    }
}

/// Parses and validates one watermark-research corpus.
///
/// # Errors
///
/// Returns [`WatermarkResearchError`] for size, JSON, or contract failures.
pub fn parse_watermark_research_corpus(
    input: &str,
) -> Result<WatermarkResearchCorpus, WatermarkResearchError> {
    if input.len() > MAX_WATERMARK_RESEARCH_BYTES {
        return Err(WatermarkResearchError::TooLarge);
    }
    let corpus: WatermarkResearchCorpus = serde_json::from_str(input)?;
    validate_corpus(&corpus)?;
    Ok(corpus)
}

fn validate_corpus(corpus: &WatermarkResearchCorpus) -> Result<(), WatermarkResearchError> {
    if corpus.schema_version != WATERMARK_RESEARCH_SCHEMA_VERSION
        || !valid_label(&corpus.corpus_id)
        || !corpus.research_only
        || corpus.live_engine_authority != LiveEngineAuthority::None
        || corpus.known_mark_generation != KnownMarkGeneration::NotPerformed
        || corpus.cases.is_empty()
        || corpus.cases.len() > MAX_CASES
    {
        return Err(WatermarkResearchError::InvalidCorpus);
    }
    let identifiers = corpus
        .cases
        .iter()
        .map(|case| case.id.as_str())
        .collect::<BTreeSet<_>>();
    if identifiers.len() != corpus.cases.len() {
        let mut seen = BTreeSet::new();
        for (index, case) in corpus.cases.iter().enumerate() {
            if !seen.insert(case.id.as_str()) {
                return Err(WatermarkResearchError::DuplicateCaseId { index });
            }
        }
    }
    for (index, case) in corpus.cases.iter().enumerate() {
        if !valid_case(case, &identifiers) {
            return Err(WatermarkResearchError::InvalidCase { index });
        }
    }
    Ok(())
}

fn valid_case(case: &WatermarkResearchCase, identifiers: &BTreeSet<&str>) -> bool {
    if !valid_label(&case.id)
        || !valid_text(&case.source)
        || !valid_text(&case.notes)
        || case.source.contains("watermark_free")
        || case.notes.contains("watermark_free")
        || case.notes.contains("is watermarked")
    {
        return false;
    }
    match case.expected_outcome {
        WatermarkResearchOutcome::RefusedStyleAsMark => case
            .unmarked_control_id
            .as_deref()
            .is_some_and(|id| identifiers.contains(id)),
        WatermarkResearchOutcome::UnmarkedControl => case.unmarked_control_id.is_none(),
        WatermarkResearchOutcome::LiteralObservation
        | WatermarkResearchOutcome::CarrierShapeUnparsed => true,
    }
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(|ch| ch.is_control() && ch != '\n')
}

#[cfg(test)]
mod tests;
