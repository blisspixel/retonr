use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current canonical local-judge rubric schema version.
pub const LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized rubric bytes.
pub const MAX_LOCAL_JUDGE_RUBRIC_BYTES: usize = 256 * 1024;
/// Maximum clauses in one local-judge rubric.
pub const MAX_LOCAL_JUDGE_RUBRIC_CLAUSES: usize = 32;

const MAX_RUBRIC_LABEL_BYTES: usize = 64;
const MAX_RUBRIC_INSTRUCTION_BYTES: usize = 2_048;
const RUBRIC_DIGEST_DOMAIN: &[u8] = b"retonr:local-judge-rubric:v1\0";

/// Versioned rubric with clauses in canonical identifier order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalJudgeRubric {
    /// Rubric schema version.
    pub schema_version: u32,
    /// Nonempty clauses sorted by exact identifier.
    pub clauses: Vec<LocalJudgeRubricClause>,
}

/// One exact local-judge rubric clause.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalJudgeRubricClause {
    /// Stable lowercase machine identifier.
    pub id: String,
    /// Complete instruction presented to the judge.
    pub instruction: String,
}

/// Canonical local-judge rubric failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LocalJudgeRubricError {
    /// Serialized input exceeds the hard byte ceiling.
    #[error("local judge rubric exceeds the byte limit")]
    TooLarge,
    /// JSON is malformed, incorrectly typed, or contains unknown fields.
    #[error("invalid local judge rubric JSON")]
    InvalidJson,
    /// The rubric version is unsupported.
    #[error("unsupported local judge rubric schema version {0}")]
    UnsupportedSchema(u32),
    /// Clauses are empty, unbounded, malformed, duplicated, or unordered.
    #[error("invalid local judge rubric clauses")]
    InvalidClauses,
}

/// Parses and validates a canonical local-judge rubric.
///
/// # Errors
///
/// Returns [`LocalJudgeRubricError`] for size, JSON, schema, ordering, label,
/// or instruction failures.
pub fn parse_local_judge_rubric(input: &str) -> Result<LocalJudgeRubric, LocalJudgeRubricError> {
    if input.len() > MAX_LOCAL_JUDGE_RUBRIC_BYTES {
        return Err(LocalJudgeRubricError::TooLarge);
    }
    let rubric: LocalJudgeRubric =
        serde_json::from_str(input).map_err(|_error| LocalJudgeRubricError::InvalidJson)?;
    validate_rubric(&rubric)?;
    Ok(rubric)
}

/// Computes the domain-separated digest of one validated canonical rubric.
///
/// # Errors
///
/// Returns [`LocalJudgeRubricError`] when the rubric is invalid or oversized.
pub fn local_judge_rubric_digest(
    rubric: &LocalJudgeRubric,
) -> Result<Digest, LocalJudgeRubricError> {
    validate_rubric(rubric)?;
    let encoded =
        serde_json::to_vec(rubric).map_err(|_error| LocalJudgeRubricError::InvalidJson)?;
    let mut material = Vec::with_capacity(RUBRIC_DIGEST_DOMAIN.len() + encoded.len() + 8);
    material.extend_from_slice(RUBRIC_DIGEST_DOMAIN);
    material.extend_from_slice(&(encoded.len() as u64).to_be_bytes());
    material.extend_from_slice(&encoded);
    Ok(Digest::sha256(&material))
}

pub(super) fn validate_rubric(rubric: &LocalJudgeRubric) -> Result<(), LocalJudgeRubricError> {
    if rubric.schema_version != LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION {
        return Err(LocalJudgeRubricError::UnsupportedSchema(
            rubric.schema_version,
        ));
    }
    let encoded =
        serde_json::to_vec(rubric).map_err(|_error| LocalJudgeRubricError::InvalidJson)?;
    if encoded.len() > MAX_LOCAL_JUDGE_RUBRIC_BYTES {
        return Err(LocalJudgeRubricError::TooLarge);
    }
    if rubric.clauses.is_empty()
        || rubric.clauses.len() > MAX_LOCAL_JUDGE_RUBRIC_CLAUSES
        || rubric.clauses.iter().any(|clause| {
            !valid_label(&clause.id)
                || clause.instruction.is_empty()
                || clause.instruction.len() > MAX_RUBRIC_INSTRUCTION_BYTES
                || clause.instruction.trim() != clause.instruction
                || clause.instruction.chars().any(char::is_control)
        })
        || !rubric
            .clauses
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
    {
        return Err(LocalJudgeRubricError::InvalidClauses);
    }
    Ok(())
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RUBRIC_LABEL_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

#[cfg(test)]
mod tests;
