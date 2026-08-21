use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use crate::OutputContract;

/// Current neutral local-judge attempt output contract version.
pub const LOCAL_JUDGE_ATTEMPT_OUTPUT_SCHEMA_VERSION: u32 = 1;
/// Maximum UTF-8 JSON bytes accepted for one local-judge attempt.
pub const MAX_LOCAL_JUDGE_ATTEMPT_OUTPUT_BYTES: usize = 64 * 1024;
/// Maximum bytes in a case or rubric-clause label.
pub const MAX_LOCAL_JUDGE_LABEL_BYTES: usize = 64;
/// Maximum cited rubric clauses in one local-judge attempt.
pub const MAX_LOCAL_JUDGE_RUBRIC_CLAUSES: usize = 32;
/// Maximum cited byte spans for each presented input.
pub const MAX_LOCAL_JUDGE_BYTE_SPANS: usize = 32;

const LOCAL_JUDGE_ATTEMPT_SCHEMA: &str = r##"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"urn:retonr:local-judge-attempt-output:v1","type":"object","additionalProperties":false,"required":["schema_version","case_id","choice","rubric_clauses","source_spans","first_candidate_spans","second_candidate_spans"],"properties":{"schema_version":{"type":"integer","const":1},"case_id":{"type":"string","minLength":1,"maxLength":64,"pattern":"^[a-z0-9_-]+$"},"choice":{"type":"string","enum":["first","second","tie","abstain"]},"rubric_clauses":{"type":"array","minItems":1,"maxItems":32,"uniqueItems":true,"items":{"type":"string","minLength":1,"maxLength":64,"pattern":"^[a-z0-9_-]+$"}},"source_spans":{"$ref":"#/$defs/spans"},"first_candidate_spans":{"$ref":"#/$defs/spans"},"second_candidate_spans":{"$ref":"#/$defs/spans"}},"$defs":{"spans":{"type":"array","maxItems":32,"items":{"$ref":"#/$defs/span"}},"span":{"type":"object","additionalProperties":false,"required":["start","end"],"properties":{"start":{"type":"integer","minimum":0,"maximum":4294967295},"end":{"type":"integer","minimum":0,"maximum":4294967295}}}}}"##;

/// Choice relative to the presented candidate order.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalJudgeChoice {
    /// Prefer the first presented candidate.
    First,
    /// Prefer the second presented candidate.
    Second,
    /// The candidates are tied under the cited rubric clauses.
    Tie,
    /// Available evidence is insufficient for a choice.
    Abstain,
}

/// One nonempty half-open byte span in a presented input.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalJudgeByteSpan {
    /// Inclusive byte offset.
    pub start: u32,
    /// Exclusive byte offset.
    pub end: u32,
}

/// One bounded neutral local-judge attempt result.
///
/// Span offsets are relative to the separately presented source, first
/// candidate, or second candidate byte sequence named by each field. Parsing
/// proves only structural bounds. The caller must check exact input lengths and
/// UTF-8 boundaries before using the spans.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalJudgeAttemptOutput {
    /// Output contract version.
    pub schema_version: u32,
    /// Stable case label supplied in the judge input.
    pub case_id: String,
    /// Choice relative to presentation order.
    pub choice: LocalJudgeChoice,
    /// Sorted unique rubric-clause labels cited by this attempt.
    pub rubric_clauses: Vec<String>,
    /// Sorted non-overlapping spans into the source bytes.
    pub source_spans: Vec<LocalJudgeByteSpan>,
    /// Sorted non-overlapping spans into the first candidate bytes.
    pub first_candidate_spans: Vec<LocalJudgeByteSpan>,
    /// Sorted non-overlapping spans into the second candidate bytes.
    pub second_candidate_spans: Vec<LocalJudgeByteSpan>,
}

/// Structural local-judge attempt parsing failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LocalJudgeAttemptOutputError {
    /// Serialized JSON exceeds the hard response ceiling.
    #[error("local judge attempt output exceeds the byte limit")]
    TooLarge,
    /// JSON is malformed, incomplete, incorrectly typed, or has unknown fields.
    #[error("invalid local judge attempt output JSON")]
    InvalidJson,
    /// The output contract version is unsupported.
    #[error("unsupported local judge attempt output schema version {0}")]
    UnsupportedSchema(u32),
    /// The case identifier is not a canonical bounded label.
    #[error("invalid local judge attempt case identifier")]
    InvalidCaseId,
    /// Rubric clauses are empty, unbounded, malformed, duplicated, or unordered.
    #[error("invalid local judge attempt rubric clauses")]
    InvalidRubricClauses,
    /// Source spans are unbounded, empty, overlapping, or unordered.
    #[error("invalid local judge attempt source spans")]
    InvalidSourceSpans,
    /// First-candidate spans are unbounded, empty, overlapping, or unordered.
    #[error("invalid local judge attempt first-candidate spans")]
    InvalidFirstCandidateSpans,
    /// Second-candidate spans are unbounded, empty, overlapping, or unordered.
    #[error("invalid local judge attempt second-candidate spans")]
    InvalidSecondCandidateSpans,
}

/// Returns the exact provider-neutral local-judge attempt output contract.
///
/// Contract admission does not attest a judge, runtime, model, prompt, or input.
#[must_use]
pub fn local_judge_attempt_output_contract() -> OutputContract {
    OutputContract {
        schema_digest: Digest::sha256(LOCAL_JUDGE_ATTEMPT_SCHEMA.as_bytes()),
        schema_json: LOCAL_JUDGE_ATTEMPT_SCHEMA.to_owned(),
    }
}

/// Parses and structurally validates one neutral local-judge attempt output.
///
/// This parser deliberately does not check offsets against actual inputs or
/// require offsets to fall on UTF-8 boundaries. The evaluation layer owns those
/// input-specific checks.
///
/// # Errors
///
/// Returns [`LocalJudgeAttemptOutputError`] for byte, JSON, version, label,
/// clause, ordering, overlap, or span failures.
pub fn parse_local_judge_attempt_output(
    input: &str,
) -> Result<LocalJudgeAttemptOutput, LocalJudgeAttemptOutputError> {
    if input.len() > MAX_LOCAL_JUDGE_ATTEMPT_OUTPUT_BYTES {
        return Err(LocalJudgeAttemptOutputError::TooLarge);
    }
    let output: LocalJudgeAttemptOutput =
        serde_json::from_str(input).map_err(|_error| LocalJudgeAttemptOutputError::InvalidJson)?;
    if output.schema_version != LOCAL_JUDGE_ATTEMPT_OUTPUT_SCHEMA_VERSION {
        return Err(LocalJudgeAttemptOutputError::UnsupportedSchema(
            output.schema_version,
        ));
    }
    if !valid_label(&output.case_id) {
        return Err(LocalJudgeAttemptOutputError::InvalidCaseId);
    }
    if output.rubric_clauses.is_empty()
        || output.rubric_clauses.len() > MAX_LOCAL_JUDGE_RUBRIC_CLAUSES
        || output
            .rubric_clauses
            .iter()
            .any(|clause| !valid_label(clause))
        || !output
            .rubric_clauses
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    {
        return Err(LocalJudgeAttemptOutputError::InvalidRubricClauses);
    }
    validate_spans(&output.source_spans)
        .map_err(|()| LocalJudgeAttemptOutputError::InvalidSourceSpans)?;
    validate_spans(&output.first_candidate_spans)
        .map_err(|()| LocalJudgeAttemptOutputError::InvalidFirstCandidateSpans)?;
    validate_spans(&output.second_candidate_spans)
        .map_err(|()| LocalJudgeAttemptOutputError::InvalidSecondCandidateSpans)?;
    Ok(output)
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LOCAL_JUDGE_LABEL_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn validate_spans(spans: &[LocalJudgeByteSpan]) -> Result<(), ()> {
    if spans.len() > MAX_LOCAL_JUDGE_BYTE_SPANS
        || spans.iter().any(|span| span.start >= span.end)
        || !spans.windows(2).all(|pair| pair[0].end <= pair[1].start)
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
