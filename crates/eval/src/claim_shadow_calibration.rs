use std::collections::BTreeSet;

use rewrite_app::{CandidateCheckRequest, CandidateCheckService, PreparedClaimShadow};
use rewrite_types::{
    CancellationToken, ClaimEvidence, ClaimEvidenceSet, ClaimExtractionStatus, ClaimModality,
    ClaimPolarity, Digest, DocumentId, ExtractorManifest, GateStatus, ReasonCode, RewriteStatus,
    RewriteUnitId, SourceSpan,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current claim-shadow calibration corpus schema version.
pub const CLAIM_SHADOW_CALIBRATION_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized calibration corpus size accepted by the runner.
pub const MAX_CLAIM_SHADOW_CALIBRATION_BYTES: usize = 16 * 1024 * 1024;
/// Maximum cases accepted in one calibration corpus.
pub const MAX_CLAIM_SHADOW_CALIBRATION_CASES: usize = 256;

const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_PREDICATE_BYTES: usize = 64;
const CALIBRATION_CONFIDENCE_PPM: u32 = 800_000;

/// Versioned fixture set for independent shadow-mode claim calibration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimShadowCalibrationCorpus {
    /// Corpus contract version.
    pub schema_version: u32,
    /// Stable corpus identity.
    pub corpus_id: String,
    /// Independently reportable cases.
    pub cases: Vec<ClaimShadowCalibrationCase>,
}

/// One source and candidate pair with expected engine and shadow outcomes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimShadowCalibrationCase {
    /// Stable fixture identifier.
    pub id: String,
    /// Synthetic source text.
    pub source: String,
    /// Synthetic candidate text.
    pub candidate: String,
    /// Required transaction status from the literal hard gates.
    pub expected_status: RewriteStatus,
    /// Required reason when the case should abstain.
    pub expected_reason: Option<ReasonCode>,
    /// Required informational shadow outcome.
    pub expected_shadow: ExpectedShadowOutcome,
    /// Fixture-assigned source claim identity, empty when no join is prepared.
    #[serde(default)]
    pub source_predicate: String,
    /// Fixture-assigned candidate claim identity, empty when no join is prepared.
    #[serde(default)]
    pub candidate_predicate: String,
}

/// Informational shadow outcome expected after a completed join.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedShadowOutcome {
    /// Comparison found no missing, novel, or conflicting claim.
    Preserved,
    /// Comparison found a missing, novel, or conflicting claim.
    Conflict,
    /// Comparison retained polarity, modality, or confidence uncertainty.
    Uncertain,
    /// No shadow gate was recorded.
    Absent,
}

/// Content-free result of one calibration run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimShadowCalibrationReport {
    /// Corpus contract version.
    pub schema_version: u32,
    /// Stable corpus identity.
    pub corpus_id: String,
    /// Total calibrated cases.
    pub total: usize,
    /// Cases whose engine status, reason, and shadow outcome matched.
    pub passed: usize,
    /// Cases where attaching shadow changed hard-gate acceptance.
    pub authority_violations: usize,
    /// Content-free failure records.
    pub failures: Vec<ClaimShadowCalibrationFailure>,
}

impl ClaimShadowCalibrationReport {
    /// Returns whether every case matched and shadow never changed acceptance.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failures.is_empty() && self.authority_violations == 0
    }
}

/// Content-free mismatch for one calibration case.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ClaimShadowCalibrationFailure {
    /// Stable fixture identifier.
    pub id: String,
    /// Required transaction status.
    pub expected_status: RewriteStatus,
    /// Observed transaction status when a transaction completed.
    pub actual_status: Option<RewriteStatus>,
    /// Required informational shadow outcome.
    pub expected_shadow: ExpectedShadowOutcome,
    /// Observed informational shadow outcome.
    pub actual_shadow: ExpectedShadowOutcome,
    /// Whether attaching shadow changed hard-gate acceptance.
    pub authority_violation: bool,
    /// Redacted operational category when the case could not be executed.
    pub error: Option<&'static str>,
}

/// Calibration corpus parsing or execution failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClaimShadowCalibrationError {
    /// Serialized input exceeds the corpus byte bound.
    #[error("claim-shadow calibration corpus exceeds the supported byte limit")]
    TooLarge,
    /// Corpus JSON is invalid or contains an unknown field or enum value.
    #[error("invalid claim-shadow calibration corpus")]
    InvalidJson,
    /// Corpus contract version is unsupported.
    #[error("unsupported claim-shadow calibration schema version {0}")]
    UnsupportedSchema(u32),
    /// Corpus identity or case-count state is invalid.
    #[error("claim-shadow calibration corpus contract is invalid")]
    InvalidCorpus,
    /// Corpus contains more cases than the declared bound.
    #[error("claim-shadow calibration corpus exceeds the case-count limit")]
    TooManyCases,
    /// Case fields, predicates, or expected outcomes are inconsistent.
    #[error("claim-shadow calibration case {index} is invalid")]
    InvalidCase {
        /// Zero-based case position.
        index: usize,
    },
    /// Case identifier is not unique within the corpus.
    #[error("claim-shadow calibration case {index} has a duplicate identifier")]
    DuplicateCaseId {
        /// Zero-based case position.
        index: usize,
    },
}

/// Parses and validates one independent claim-shadow calibration corpus.
///
/// # Errors
///
/// Returns [`ClaimShadowCalibrationError`] for a size, JSON, version, identity,
/// or case-contract violation.
pub fn parse_claim_shadow_calibration(
    input: &str,
) -> Result<ClaimShadowCalibrationCorpus, ClaimShadowCalibrationError> {
    if input.len() > MAX_CLAIM_SHADOW_CALIBRATION_BYTES {
        return Err(ClaimShadowCalibrationError::TooLarge);
    }
    let corpus: ClaimShadowCalibrationCorpus =
        serde_json::from_str(input).map_err(|_error| ClaimShadowCalibrationError::InvalidJson)?;
    validate_corpus(&corpus)?;
    Ok(corpus)
}

/// Runs every calibration case through the application candidate-check path.
///
/// Fixture-assigned claim identities are compared independently from generation.
/// A recorded conflict cannot change hard-gate acceptance.
#[must_use]
pub fn run_claim_shadow_calibration(
    corpus: &ClaimShadowCalibrationCorpus,
) -> ClaimShadowCalibrationReport {
    let mut failures = Vec::new();
    let mut authority_violations = 0;
    for case in &corpus.cases {
        match evaluate_case(case) {
            Ok(None) => {}
            Ok(Some(failure)) => {
                if failure.authority_violation {
                    authority_violations += 1;
                }
                failures.push(failure);
            }
            Err(error) => failures.push(ClaimShadowCalibrationFailure {
                id: case.id.clone(),
                expected_status: case.expected_status,
                actual_status: None,
                expected_shadow: case.expected_shadow,
                actual_shadow: ExpectedShadowOutcome::Absent,
                authority_violation: false,
                error: Some(error),
            }),
        }
    }
    ClaimShadowCalibrationReport {
        schema_version: CLAIM_SHADOW_CALIBRATION_SCHEMA_VERSION,
        corpus_id: corpus.corpus_id.clone(),
        total: corpus.cases.len(),
        passed: corpus.cases.len().saturating_sub(failures.len()),
        authority_violations,
        failures,
    }
}

fn validate_corpus(
    corpus: &ClaimShadowCalibrationCorpus,
) -> Result<(), ClaimShadowCalibrationError> {
    if corpus.schema_version != CLAIM_SHADOW_CALIBRATION_SCHEMA_VERSION {
        return Err(ClaimShadowCalibrationError::UnsupportedSchema(
            corpus.schema_version,
        ));
    }
    if !valid_label(&corpus.corpus_id) || corpus.cases.is_empty() {
        return Err(ClaimShadowCalibrationError::InvalidCorpus);
    }
    if corpus.cases.len() > MAX_CLAIM_SHADOW_CALIBRATION_CASES {
        return Err(ClaimShadowCalibrationError::TooManyCases);
    }
    let mut ids = BTreeSet::new();
    for (index, case) in corpus.cases.iter().enumerate() {
        if !valid_case(case) {
            return Err(ClaimShadowCalibrationError::InvalidCase { index });
        }
        if !ids.insert(case.id.as_str()) {
            return Err(ClaimShadowCalibrationError::DuplicateCaseId { index });
        }
    }
    Ok(())
}

fn valid_case(case: &ClaimShadowCalibrationCase) -> bool {
    if !valid_label(&case.id)
        || !valid_text(&case.source)
        || !valid_text(&case.candidate)
        || (case.expected_status == RewriteStatus::Abstained) != case.expected_reason.is_some()
        || case.expected_status == RewriteStatus::Failed
    {
        return false;
    }
    match case.expected_shadow {
        ExpectedShadowOutcome::Absent => {
            case.source_predicate.is_empty() && case.candidate_predicate.is_empty()
        }
        ExpectedShadowOutcome::Preserved | ExpectedShadowOutcome::Uncertain => {
            valid_predicate(&case.source_predicate)
                && case.source_predicate == case.candidate_predicate
        }
        ExpectedShadowOutcome::Conflict => {
            valid_predicate(&case.source_predicate)
                && valid_predicate(&case.candidate_predicate)
                && case.source_predicate != case.candidate_predicate
        }
    }
}

fn evaluate_case(
    case: &ClaimShadowCalibrationCase,
) -> Result<Option<ClaimShadowCalibrationFailure>, &'static str> {
    let request = CandidateCheckRequest {
        source: case.source.as_bytes().to_vec(),
        candidate: case.candidate.clone(),
        protected_terms: Vec::new(),
    };
    let without =
        CandidateCheckService::check(request.clone()).map_err(|error| map_app_error(&error))?;
    let observer = match case.expected_shadow {
        ExpectedShadowOutcome::Absent => None,
        ExpectedShadowOutcome::Uncertain => Some(fixture_shadow(case, ClaimPolarity::Unknown)?),
        ExpectedShadowOutcome::Preserved | ExpectedShadowOutcome::Conflict => {
            Some(fixture_shadow(case, ClaimPolarity::Affirmed)?)
        }
    };
    let with = CandidateCheckService::check_with_claim_shadow(
        request,
        &CancellationToken::new(),
        observer
            .as_ref()
            .map(|shadow| shadow as &dyn rewrite_app::ClaimShadowObserver),
    )
    .map_err(|error| map_app_error(&error))?;
    let actual_shadow = observed_shadow(&with);
    let authority_violation = without.record.status != with.record.status
        || without.record.reason != with.record.reason
        || without.output != with.output;
    let matched = with.record.status == case.expected_status
        && with.record.reason == case.expected_reason
        && actual_shadow == case.expected_shadow
        && !authority_violation;
    if matched {
        Ok(None)
    } else {
        Ok(Some(ClaimShadowCalibrationFailure {
            id: case.id.clone(),
            expected_status: case.expected_status,
            actual_status: Some(with.record.status),
            expected_shadow: case.expected_shadow,
            actual_shadow,
            authority_violation,
            error: None,
        }))
    }
}

fn fixture_shadow(
    case: &ClaimShadowCalibrationCase,
    polarity: ClaimPolarity,
) -> Result<PreparedClaimShadow, &'static str> {
    let unit_id = RewriteUnitId::new(
        &DocumentId::from_digest(&Digest::sha256(case.source.as_bytes())),
        0,
    );
    let source = fixture_evidence(&unit_id, &case.source, &case.source_predicate, polarity)?;
    let candidate = fixture_evidence(
        &unit_id,
        &case.candidate,
        &case.candidate_predicate,
        polarity,
    )?;
    PreparedClaimShadow::from_evidence_sets(&source, &candidate)
        .map_err(|_error| "claim comparison failed")
}

fn fixture_evidence(
    unit_id: &RewriteUnitId,
    text: &str,
    predicate: &str,
    polarity: ClaimPolarity,
) -> Result<ClaimEvidenceSet, &'static str> {
    let span = SourceSpan::new(0, text.len()).map_err(|_error| "invalid evidence span")?;
    // Predicate-stable IDs let punctuation-only pairs preserve without a surface-text conflict.
    let identity = Digest::sha256(predicate.as_bytes());
    let claim = ClaimEvidence::from_canonical(
        identity.clone(),
        None,
        identity,
        None,
        polarity,
        ClaimModality::Asserted,
        0,
        false,
        vec![span],
        CALIBRATION_CONFIDENCE_PPM,
    )
    .map_err(|_error| "invalid fixture claim")?;
    ClaimEvidenceSet::new(
        "shadow-calibration",
        "1.0.0",
        calibration_manifest()?.manifest_digest(),
        ClaimExtractionStatus::Complete,
        500_000,
        unit_id.clone(),
        text,
        vec![claim],
    )
    .map_err(|_error| "invalid fixture evidence")
}

fn calibration_manifest() -> Result<ExtractorManifest, &'static str> {
    ExtractorManifest::new(
        "shadow-calibration",
        "1.0.0",
        Digest::sha256(b"calibration-subject"),
        Digest::sha256(b"calibration-prompt"),
        Digest::sha256(b"calibration-output"),
        Digest::sha256(b"calibration-operation"),
        Digest::sha256(b"calibration-confidence"),
        Digest::sha256(b"calibration-language"),
    )
    .map_err(|_error| "invalid calibration extractor")
}

fn observed_shadow(result: &rewrite_app::CandidateCheckResult) -> ExpectedShadowOutcome {
    let Some(gate) = result.record.assessments.first().and_then(|assessment| {
        assessment
            .gates
            .iter()
            .find(|gate| gate.gate_id == "claim_comparison_shadow")
    }) else {
        return ExpectedShadowOutcome::Absent;
    };
    match gate.status {
        GateStatus::Pass => ExpectedShadowOutcome::Preserved,
        GateStatus::Fail => ExpectedShadowOutcome::Conflict,
        GateStatus::Uncertain => ExpectedShadowOutcome::Uncertain,
        GateStatus::NotApplicable => ExpectedShadowOutcome::Absent,
    }
}

fn map_app_error(error: &rewrite_app::AppError) -> &'static str {
    match error {
        rewrite_app::AppError::CandidateTooLarge { .. } => "candidate too large",
        rewrite_app::AppError::TextAdapter(_) => "unsupported text",
        rewrite_app::AppError::Engine(_) | rewrite_app::AppError::Protection(_) => "engine policy",
        rewrite_app::AppError::Grounded(_)
        | rewrite_app::AppError::GroundedUnavailable
        | rewrite_app::AppError::GroundedSelectionMismatch
        | rewrite_app::AppError::GroundedRuntimeUnavailable
        | rewrite_app::AppError::GroundedRepository => "grounded unavailable",
        rewrite_app::AppError::ClaimExtraction(_) => "claim extraction",
    }
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

fn valid_predicate(value: &str) -> bool {
    valid_label(value) && value.len() <= MAX_PREDICATE_BYTES
}

fn valid_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

#[cfg(test)]
mod tests;
