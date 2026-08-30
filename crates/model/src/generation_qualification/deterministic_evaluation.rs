//! Portable deterministic-evaluation identities and aggregate facts.
//!
//! Report framing in this module is inert. The eval-owned compiler must validate
//! exact `EvaluationReport` structure and prove that each supplied summary equals
//! that report before it may construct an authoritative evaluation result.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::append_u64;
use super::{GenerationQualificationContractError, MAX_GENERATION_SUITE_CASES};

mod record;

pub use record::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
};

/// Domain for one candidate-specific deterministic report digest.
pub const CANDIDATE_DETERMINISTIC_REPORT_DIGEST_DOMAIN: &[u8] =
    b"retonr:candidate-deterministic-report:v1\0";
/// Domain for one permutation-aware ordered pair of deterministic suites.
pub const CANDIDATE_DETERMINISTIC_SUITE_PAIR_DIGEST_DOMAIN: &[u8] =
    b"retonr:candidate-deterministic-suite-pair:v1\0";
/// Existing compatibility domain retained for the ordered report-pair digest.
pub const CANDIDATE_DETERMINISTIC_REPORT_PAIR_DIGEST_DOMAIN: &[u8] =
    b"retonr:hybrid-scorecard-report-pair:v1\0";
/// Existing fixed deterministic-policy domain used by the compatibility kernel.
pub const CANDIDATE_DETERMINISTIC_POLICY_DOMAIN: &[u8] =
    b"retonr:hybrid-scorecard-deterministic-policy:v1\0";

/// Maximum JSON bytes accepted for one deterministic evaluation record.
pub const MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES: usize = 16 * 1_024;
/// Maximum report bytes accepted for inert digest framing of either candidate.
pub const MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_CANDIDATE_DETERMINISTIC_REPORT_CASES: u32 = 256;
const MAX_COMBINED_DETERMINISTIC_CASES: u32 = 2 * MAX_CANDIDATE_DETERMINISTIC_REPORT_CASES;
const _: () = assert!(MAX_GENERATION_SUITE_CASES == 256);

/// Returns the frozen deterministic-policy digest named by candidate records.
#[must_use]
pub fn candidate_deterministic_policy_digest() -> Digest {
    Digest::sha256(CANDIDATE_DETERMINISTIC_POLICY_DOMAIN)
}

/// Integer transformation coverage across one or both candidate reports.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDeterministicTransformationCoverageV1 {
    acceptable: u32,
    rewritten: u32,
}

impl CandidateDeterministicTransformationCoverageV1 {
    /// Creates bounded coverage whose rewritten count is a subset of acceptable.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] when either count exceeds
    /// the two-report ceiling or rewritten exceeds acceptable.
    pub fn new(
        acceptable: u32,
        rewritten: u32,
    ) -> Result<Self, GenerationQualificationContractError> {
        if acceptable > MAX_COMBINED_DETERMINISTIC_CASES || rewritten > acceptable {
            return Err(
                GenerationQualificationContractError::InvalidDeterministicReportRelationship,
            );
        }
        Ok(Self {
            acceptable,
            rewritten,
        })
    }

    /// Returns independently acceptable changed candidates.
    #[must_use]
    pub const fn acceptable(self) -> u32 {
        self.acceptable
    }

    /// Returns acceptable changed candidates emitted as rewrites.
    #[must_use]
    pub const fn rewritten(self) -> u32 {
        self.rewritten
    }
}

/// Derived deterministic outcome across the exact two candidate reports.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDeterministicEvaluationStatusV1 {
    /// Every case in both reports passed.
    Passed,
    /// At least one case in either report failed.
    Failed,
}

/// Bounded aggregate facts asserted by the eval-owned report validator.
///
/// This type validates arithmetic only. It does not inspect a report or prove that
/// these values were extracted from the report bytes later framed with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateDeterministicReportSummaryV1 {
    total: u32,
    passed: u32,
    transformation_coverage: CandidateDeterministicTransformationCoverageV1,
}

impl CandidateDeterministicReportSummaryV1 {
    /// Creates one nonempty, bounded, internally consistent report summary.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for zero or excessive
    /// totals, passed counts above total, or coverage above total.
    pub fn new(
        total: u32,
        passed: u32,
        transformation_coverage: CandidateDeterministicTransformationCoverageV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if total == 0
            || total > MAX_CANDIDATE_DETERMINISTIC_REPORT_CASES
            || passed > total
            || transformation_coverage.acceptable() > total
        {
            return Err(
                GenerationQualificationContractError::InvalidDeterministicReportRelationship,
            );
        }
        Ok(Self {
            total,
            passed,
            transformation_coverage,
        })
    }

    /// Returns the asserted report case count.
    #[must_use]
    pub const fn total(self) -> u32 {
        self.total
    }

    /// Returns the asserted report passing-case count.
    #[must_use]
    pub const fn passed(self) -> u32 {
        self.passed
    }

    /// Returns the asserted report transformation coverage.
    #[must_use]
    pub const fn transformation_coverage(self) -> CandidateDeterministicTransformationCoverageV1 {
        self.transformation_coverage
    }
}

/// Inert digest framing and asserted aggregates for exact ordered report bytes.
///
/// This value proves only bounded framing and arithmetic. Before constructing it,
/// the eval-owned compiler must validate both exact `EvaluationReport` values and
/// prove each summary equals its report. This model layer cannot perform that
/// semantic validation and grants no execution or qualification authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateDeterministicReportRelationshipV1 {
    candidate_a_report_digest: Digest,
    candidate_b_report_digest: Digest,
    report_pair_digest: Digest,
    candidate_a_summary: CandidateDeterministicReportSummaryV1,
    candidate_b_summary: CandidateDeterministicReportSummaryV1,
}

impl CandidateDeterministicReportRelationshipV1 {
    /// Frames two bounded report encodings and their arithmetic-only summaries.
    ///
    /// Candidate order is identity-significant. The individual report digests use
    /// the candidate report domain, while the pair retains the compatibility
    /// hybrid report-pair domain. This constructor does not parse report bytes or
    /// compare their contents with the summaries.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] when either report is
    /// empty, exceeds its fixed byte ceiling, or cannot be length-framed.
    #[expect(
        clippy::similar_names,
        reason = "candidate A and B are the frozen identity-significant report positions"
    )]
    pub fn new(
        candidate_a_report_bytes: &[u8],
        candidate_b_report_bytes: &[u8],
        candidate_a_summary: CandidateDeterministicReportSummaryV1,
        candidate_b_summary: CandidateDeterministicReportSummaryV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_report_bytes(candidate_a_report_bytes)?;
        validate_report_bytes(candidate_b_report_bytes)?;
        Ok(Self {
            candidate_a_report_digest: framed_digest(
                CANDIDATE_DETERMINISTIC_REPORT_DIGEST_DOMAIN,
                &[candidate_a_report_bytes],
            )?,
            candidate_b_report_digest: framed_digest(
                CANDIDATE_DETERMINISTIC_REPORT_DIGEST_DOMAIN,
                &[candidate_b_report_bytes],
            )?,
            report_pair_digest: framed_digest(
                CANDIDATE_DETERMINISTIC_REPORT_PAIR_DIGEST_DOMAIN,
                &[candidate_a_report_bytes, candidate_b_report_bytes],
            )?,
            candidate_a_summary,
            candidate_b_summary,
        })
    }

    /// Returns the candidate A report digest.
    #[must_use]
    pub const fn candidate_a_report_digest(&self) -> &Digest {
        &self.candidate_a_report_digest
    }

    /// Returns the candidate B report digest.
    #[must_use]
    pub const fn candidate_b_report_digest(&self) -> &Digest {
        &self.candidate_b_report_digest
    }

    /// Returns the ordered compatibility report-pair digest.
    #[must_use]
    pub const fn report_pair_digest(&self) -> &Digest {
        &self.report_pair_digest
    }

    /// Returns candidate A's arithmetic-only report summary.
    #[must_use]
    pub const fn candidate_a_summary(&self) -> CandidateDeterministicReportSummaryV1 {
        self.candidate_a_summary
    }

    /// Returns candidate B's arithmetic-only report summary.
    #[must_use]
    pub const fn candidate_b_summary(&self) -> CandidateDeterministicReportSummaryV1 {
        self.candidate_b_summary
    }
}

fn validate_report_bytes(bytes: &[u8]) -> Result<(), GenerationQualificationContractError> {
    if bytes.is_empty() || bytes.len() > MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES {
        return Err(GenerationQualificationContractError::InvalidDeterministicReportRelationship);
    }
    Ok(())
}

fn framed_digest(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<Digest, GenerationQualificationContractError> {
    let capacity = fields.iter().try_fold(domain.len(), |total, field| {
        total
            .checked_add(8)
            .and_then(|value| value.checked_add(field.len()))
            .ok_or(GenerationQualificationContractError::EncodingOverflow)
    })?;
    let mut canonical = Vec::with_capacity(capacity);
    canonical.extend_from_slice(domain);
    for field in fields {
        append_u64(
            &mut canonical,
            u64::try_from(field.len())
                .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?,
        );
        canonical.extend_from_slice(field);
    }
    Ok(Digest::sha256(&canonical))
}
