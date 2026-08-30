use std::fmt;

use rewrite_types::{Digest, ReasonCode, RewriteStatus};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::GenerationCaseManifestV1;
use crate::{ArtifactId, EvaluationCase, ExpectedOutput, ReferenceJudgment};

mod accessors;
mod codec;
use codec::canonical_identity_bytes;

/// Current deterministic generation-case contract schema version.
pub const GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION: u32 = 1;
/// Maximum JSON bytes accepted for one deterministic generation-case contract.
pub const MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES: usize = 1024 * 1024;
/// Maximum protected terms admitted by one deterministic generation case.
pub const MAX_GENERATION_DETERMINISTIC_CASE_PROTECTED_TERMS: usize = 1_024;
/// Maximum rubric clause identifiers admitted by one deterministic generation case.
pub const MAX_GENERATION_DETERMINISTIC_CASE_RUBRIC_CLAUSES: usize = 32;

const MAX_EVALUATION_LABEL_BYTES: usize = 64;

/// Caller-supplied exact facts for one deterministic generation case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationDeterministicCaseContractV1Input {
    /// Canonical case machine key shared with the case manifest.
    pub case_key: String,
    /// Exact immutable source artifact identity.
    pub source_artifact_id: ArtifactId,
    /// SHA-256 digest of the complete source bytes.
    pub source_digest: Digest,
    /// Nonzero complete source byte count.
    pub source_byte_count: u64,
    /// Digest of the case language declaration.
    pub language_digest: Digest,
    /// Digest of the case operation mode.
    pub mode_digest: Digest,
    /// Digest of the case format contract.
    pub format_digest: Digest,
    /// Stable deterministic-report category.
    pub evaluation_category: String,
    /// Exact protected terms in strictly ascending byte order.
    pub protected_terms: Vec<String>,
    /// Predeclared human fixture judgment, without machine-proof authority.
    pub reference_judgment: ReferenceJudgment,
    /// Exact expected transaction status.
    pub expected_status: RewriteStatus,
    /// Exact optional expected transaction reason.
    pub expected_reason: Option<ReasonCode>,
    /// Exact expected output identity.
    pub expected_output: ExpectedOutput,
    /// Exact rubric clause identifiers in strictly ascending byte order.
    pub rubric_clause_ids: Vec<String>,
}

/// Portable exact deterministic and judge expectations for one generation case.
///
/// The record intentionally omits `GenerationCaseId`. Its digest is embedded in
/// the case manifest, so including the manifest identity here would create an
/// identity cycle.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationDeterministicCaseContractV1 {
    schema_version: u32,
    case_key: String,
    source_artifact_id: ArtifactId,
    source_digest: Digest,
    source_byte_count: u64,
    language_digest: Digest,
    mode_digest: Digest,
    format_digest: Digest,
    evaluation_category: String,
    protected_terms: Vec<String>,
    reference_judgment: ReferenceJudgment,
    expected_status: RewriteStatus,
    expected_reason: Option<ReasonCode>,
    expected_output: ExpectedOutput,
    rubric_clause_ids: Vec<String>,
    #[serde(skip)]
    contract_digest: Digest,
}

/// Deterministic generation-case contract validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationDeterministicCaseContractError {
    /// Encoded JSON exceeds the fixed pre-decode ceiling.
    #[error("deterministic generation-case contract exceeds its JSON byte limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, incomplete, duplicated, trailing, or otherwise invalid.
    #[error("deterministic generation-case contract encoding is invalid")]
    InvalidEncoding,
    /// JSON is valid but differs from the one canonical encoding.
    #[error("deterministic generation-case contract encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The portable schema version is unsupported.
    #[error("unsupported deterministic generation-case contract schema {0}")]
    UnsupportedSchema(u32),
    /// The case key cannot project into the current deterministic suite contract.
    #[error("deterministic generation-case key is invalid")]
    InvalidCaseKey,
    /// The evaluation category cannot project into the current suite contract.
    #[error("deterministic generation-case evaluation category is invalid")]
    InvalidEvaluationCategory,
    /// The source identity, digest, or byte count is internally inconsistent.
    #[error("deterministic generation-case source binding is invalid")]
    InvalidSourceBinding,
    /// Protected terms are empty, excessive, or contain empty, duplicated, or unordered entries.
    #[error("deterministic generation-case protected terms are invalid")]
    InvalidProtectedTerms,
    /// Rubric clause IDs are empty, excessive, or contain empty, duplicated, or unordered entries.
    #[error("deterministic generation-case rubric clause IDs are invalid")]
    InvalidRubricClauseIds,
    /// Canonical identity bytes exceed the fixed contract ceiling.
    #[error("deterministic generation-case canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// A case manifest carries different source, policy, or contract facts.
    #[error("deterministic generation-case manifest relationship does not match")]
    CaseManifestMismatch,
    /// Supplied source bytes do not match the contract's exact source binding.
    #[error("deterministic generation-case source material does not match")]
    SourceMaterialMismatch,
    /// Exact source bytes are not UTF-8 and cannot enter an `EvaluationCase`.
    #[error("deterministic generation-case source is not UTF-8")]
    SourceNotUtf8,
}

impl GenerationDeterministicCaseContractV1 {
    /// Creates one bounded contract before its digest is embedded in a case manifest.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationDeterministicCaseContractError`] for invalid labels,
    /// source facts, ordered sets, or excessive canonical bytes.
    pub fn new(
        input: GenerationDeterministicCaseContractV1Input,
    ) -> Result<Self, GenerationDeterministicCaseContractError> {
        Self::from_wire(GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION, input)
    }

    fn from_wire(
        schema_version: u32,
        input: GenerationDeterministicCaseContractV1Input,
    ) -> Result<Self, GenerationDeterministicCaseContractError> {
        validate_input(schema_version, &input)?;
        let canonical = canonical_identity_bytes(schema_version, &input)?;
        let record = Self {
            schema_version,
            case_key: input.case_key,
            source_artifact_id: input.source_artifact_id,
            source_digest: input.source_digest,
            source_byte_count: input.source_byte_count,
            language_digest: input.language_digest,
            mode_digest: input.mode_digest,
            format_digest: input.format_digest,
            evaluation_category: input.evaluation_category,
            protected_terms: input.protected_terms,
            reference_judgment: input.reference_judgment,
            expected_status: input.expected_status,
            expected_reason: input.expected_reason,
            expected_output: input.expected_output,
            rubric_clause_ids: input.rubric_clause_ids,
            contract_digest: Digest::sha256(&canonical),
        };
        if serde_json::to_vec(&record)
            .map_err(|_| GenerationDeterministicCaseContractError::InvalidEncoding)?
            .len()
            > MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES
        {
            return Err(GenerationDeterministicCaseContractError::EncodedRecordTooLarge);
        }
        Ok(record)
    }

    /// Parses one canonical bounded contract and validates its case-manifest relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationDeterministicCaseContractError`] for excessive,
    /// malformed, noncanonical, unsupported, invalid, or cross-case input.
    pub fn from_json_bytes(
        bytes: &[u8],
        case: &GenerationCaseManifestV1,
    ) -> Result<Self, GenerationDeterministicCaseContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            case_key: String,
            source_artifact_id: ArtifactId,
            source_digest: Digest,
            source_byte_count: u64,
            language_digest: Digest,
            mode_digest: Digest,
            format_digest: Digest,
            evaluation_category: String,
            protected_terms: Vec<String>,
            reference_judgment: ReferenceJudgment,
            expected_status: RewriteStatus,
            expected_reason: Option<ReasonCode>,
            expected_output: ExpectedOutput,
            rubric_clause_ids: Vec<String>,
        }

        if bytes.len() > MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES {
            return Err(GenerationDeterministicCaseContractError::EncodedRecordTooLarge);
        }
        if bytes.first() != Some(&b'{') || bytes.last() != Some(&b'}') {
            return Err(GenerationDeterministicCaseContractError::InvalidEncoding);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationDeterministicCaseContractError::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            GenerationDeterministicCaseContractV1Input {
                case_key: wire.case_key,
                source_artifact_id: wire.source_artifact_id,
                source_digest: wire.source_digest,
                source_byte_count: wire.source_byte_count,
                language_digest: wire.language_digest,
                mode_digest: wire.mode_digest,
                format_digest: wire.format_digest,
                evaluation_category: wire.evaluation_category,
                protected_terms: wire.protected_terms,
                reference_judgment: wire.reference_judgment,
                expected_status: wire.expected_status,
                expected_reason: wire.expected_reason,
                expected_output: wire.expected_output,
                rubric_clause_ids: wire.rubric_clause_ids,
            },
        )?;
        record.validate_canonical_json(bytes)?;
        record.validate_case_manifest(case)?;
        Ok(record)
    }

    /// Serializes the exact canonical JSON representation.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationDeterministicCaseContractError`] only if the checked
    /// record cannot be serialized or exceeds its fixed ceiling.
    pub fn to_canonical_json_bytes(
        &self,
    ) -> Result<Vec<u8>, GenerationDeterministicCaseContractError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| GenerationDeterministicCaseContractError::InvalidEncoding)?;
        if bytes.len() > MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES {
            return Err(GenerationDeterministicCaseContractError::EncodedRecordTooLarge);
        }
        Ok(bytes)
    }

    /// Rechecks every field shared with the supplied case manifest.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationDeterministicCaseContractError::CaseManifestMismatch`]
    /// unless all shared fields and the complete contract digest match exactly.
    pub fn validate_case_manifest(
        &self,
        case: &GenerationCaseManifestV1,
    ) -> Result<(), GenerationDeterministicCaseContractError> {
        if self.case_key != case.case_key()
            || &self.source_artifact_id != case.source_artifact_id()
            || &self.source_digest != case.source_digest()
            || self.source_byte_count != case.source_byte_count()
            || &self.language_digest != case.language_digest()
            || &self.mode_digest != case.mode_digest()
            || &self.format_digest != case.format_digest()
            || &self.contract_digest != case.case_contract_digest()
        {
            return Err(GenerationDeterministicCaseContractError::CaseManifestMismatch);
        }
        Ok(())
    }

    /// Projects exact source and candidate text into the existing inert suite case.
    ///
    /// This projection creates no live or qualification authority. The verified
    /// material layer must supply `source_bytes` through its retained source lease
    /// and revalidate that lease around evaluation.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationDeterministicCaseContractError`] unless the source
    /// bytes match the exact size and digest and contain UTF-8.
    pub fn project_evaluation_case(
        &self,
        source_bytes: &[u8],
        candidate: impl Into<String>,
    ) -> Result<EvaluationCase, GenerationDeterministicCaseContractError> {
        let source_length = u64::try_from(source_bytes.len())
            .map_err(|_| GenerationDeterministicCaseContractError::SourceMaterialMismatch)?;
        if source_length != self.source_byte_count
            || Digest::sha256(source_bytes) != self.source_digest
        {
            return Err(GenerationDeterministicCaseContractError::SourceMaterialMismatch);
        }
        let source = std::str::from_utf8(source_bytes)
            .map_err(|_| GenerationDeterministicCaseContractError::SourceNotUtf8)?;
        Ok(EvaluationCase {
            id: self.case_key.clone(),
            category: self.evaluation_category.clone(),
            source: source.to_owned(),
            candidate: candidate.into(),
            protected_terms: self.protected_terms.clone(),
            reference_judgment: self.reference_judgment,
            expected_status: self.expected_status,
            expected_reason: self.expected_reason,
            expected_output: self.expected_output,
        })
    }

    fn validate_canonical_json(
        &self,
        bytes: &[u8],
    ) -> Result<(), GenerationDeterministicCaseContractError> {
        if self.to_canonical_json_bytes()? == bytes {
            Ok(())
        } else {
            Err(GenerationDeterministicCaseContractError::NonCanonicalEncoding)
        }
    }
}

impl fmt::Debug for GenerationDeterministicCaseContractV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationDeterministicCaseContractV1")
            .field("schema_version", &self.schema_version)
            .field("contract_digest", &self.contract_digest)
            .field("source_byte_count", &self.source_byte_count)
            .field("protected_term_count", &self.protected_terms.len())
            .field("rubric_clause_count", &self.rubric_clause_ids.len())
            .finish_non_exhaustive()
    }
}

fn validate_input(
    schema_version: u32,
    input: &GenerationDeterministicCaseContractV1Input,
) -> Result<(), GenerationDeterministicCaseContractError> {
    if schema_version != GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION {
        return Err(GenerationDeterministicCaseContractError::UnsupportedSchema(
            schema_version,
        ));
    }
    if !valid_machine_key(&input.case_key, MAX_EVALUATION_LABEL_BYTES) {
        return Err(GenerationDeterministicCaseContractError::InvalidCaseKey);
    }
    if !valid_label(&input.evaluation_category, MAX_EVALUATION_LABEL_BYTES) {
        return Err(GenerationDeterministicCaseContractError::InvalidEvaluationCategory);
    }
    if input.source_byte_count == 0 || input.source_artifact_id.digest() != &input.source_digest {
        return Err(GenerationDeterministicCaseContractError::InvalidSourceBinding);
    }
    if !valid_ordered_text_set(
        &input.protected_terms,
        MAX_GENERATION_DETERMINISTIC_CASE_PROTECTED_TERMS,
        false,
    ) {
        return Err(GenerationDeterministicCaseContractError::InvalidProtectedTerms);
    }
    if !valid_ordered_text_set(
        &input.rubric_clause_ids,
        MAX_GENERATION_DETERMINISTIC_CASE_RUBRIC_CLAUSES,
        true,
    ) {
        return Err(GenerationDeterministicCaseContractError::InvalidRubricClauseIds);
    }
    Ok(())
}

fn valid_ordered_text_set(values: &[String], maximum: usize, machine_keys: bool) -> bool {
    !values.is_empty()
        && values.len() <= maximum
        && values.iter().all(|value| {
            if machine_keys {
                valid_machine_key(value, MAX_EVALUATION_LABEL_BYTES)
            } else {
                !value.is_empty()
            }
        })
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_machine_key(value: &str, maximum: usize) -> bool {
    if value.is_empty() || value.len() > maximum {
        return false;
    }
    let mut previous_separator = false;
    for byte in value.bytes() {
        let separator = matches!(byte, b'-' | b'_');
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || separator)
            || (separator && previous_separator)
        {
            return false;
        }
        previous_separator = separator;
    }
    !matches!(value.as_bytes().first(), Some(b'-' | b'_'))
        && !matches!(value.as_bytes().last(), Some(b'-' | b'_'))
}

fn valid_label(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

#[cfg(test)]
mod tests;
