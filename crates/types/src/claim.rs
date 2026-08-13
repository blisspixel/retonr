use std::collections::HashSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Digest, RewriteUnitId, SourceSpan};

/// Current content-redacted claim-evidence contract version.
pub const CLAIM_EVIDENCE_SCHEMA_VERSION: u32 = 1;
/// Maximum claims retained for one rewrite unit.
pub const MAX_CLAIMS_PER_UNIT: usize = 256;
/// Maximum source spans retained for one claim.
pub const MAX_EVIDENCE_SPANS_PER_CLAIM: usize = 16;
/// Maximum bytes in an extractor identifier or version.
pub const MAX_EXTRACTOR_ID_BYTES: usize = 64;
/// Parts per million used to represent an exact confidence threshold.
pub const CLAIM_CONFIDENCE_PARTS_PER_MILLION: u32 = 1_000_000;

/// Completion state reported by an independent claim extractor.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimExtractionStatus {
    /// The extractor completed its configured procedure without truncation.
    Complete,
    /// The extractor returned partial or truncated evidence.
    Partial,
    /// The extractor declined to make an extraction.
    Abstained,
    /// The extraction procedure failed.
    Failed,
}

/// Polarity reported by a claim extractor.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimPolarity {
    /// The extracted proposition is affirmed.
    Affirmed,
    /// The extracted proposition is negated.
    Negated,
    /// The extractor could not determine polarity reliably.
    Unknown,
}

/// Modal force reported by a claim extractor.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimModality {
    /// The proposition is presented as an assertion.
    Asserted,
    /// The proposition expresses a requirement or obligation.
    Required,
    /// The proposition expresses permission.
    Permitted,
    /// The proposition expresses possibility or uncertainty.
    Possible,
    /// The proposition depends on an explicit condition.
    Conditional,
    /// The extractor could not determine modal force reliably.
    Unknown,
}

/// One content-redacted proposition reported by an independent extractor.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
pub struct ClaimEvidence {
    claim_id: Digest,
    subject_id: Option<Digest>,
    predicate_id: Digest,
    object_id: Option<Digest>,
    polarity: ClaimPolarity,
    modality: ClaimModality,
    condition_count: u16,
    attributed: bool,
    evidence_spans: Vec<SourceSpan>,
    confidence_ppm: u32,
}

impl ClaimEvidence {
    /// Creates one bounded, content-redacted claim.
    ///
    /// IDs are extractor-defined digests of canonical internal representations.
    /// They are comparison evidence, not proof of meaning or anonymization.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimEvidenceError`] when confidence or evidence spans are invalid.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors one atomic claim"
    )]
    pub fn new(
        claim_id: Digest,
        subject_id: Option<Digest>,
        predicate_id: Digest,
        object_id: Option<Digest>,
        polarity: ClaimPolarity,
        modality: ClaimModality,
        condition_count: u16,
        attributed: bool,
        mut evidence_spans: Vec<SourceSpan>,
        confidence: f32,
    ) -> Result<Self, ClaimEvidenceError> {
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(ClaimEvidenceError::InvalidConfidence);
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "finite validated unit confidence rounds into a bounded ppm integer"
        )]
        let confidence_ppm =
            (f64::from(confidence) * f64::from(CLAIM_CONFIDENCE_PARTS_PER_MILLION)).round() as u32;
        if evidence_spans.is_empty() || evidence_spans.len() > MAX_EVIDENCE_SPANS_PER_CLAIM {
            return Err(ClaimEvidenceError::InvalidSpanCount);
        }
        if evidence_spans.iter().any(|span| span.is_empty()) {
            return Err(ClaimEvidenceError::EmptySpan);
        }
        evidence_spans.sort_unstable_by_key(|span| (span.start(), span.end()));
        if evidence_spans.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ClaimEvidenceError::DuplicateSpan);
        }
        Ok(Self {
            claim_id,
            subject_id,
            predicate_id,
            object_id,
            polarity,
            modality,
            condition_count,
            attributed,
            evidence_spans,
            confidence_ppm,
        })
    }

    /// Returns the extractor-defined claim identity.
    #[must_use]
    pub const fn claim_id(&self) -> &Digest {
        &self.claim_id
    }

    /// Returns the extractor-defined subject identity when present.
    #[must_use]
    pub const fn subject_id(&self) -> Option<&Digest> {
        self.subject_id.as_ref()
    }

    /// Returns the extractor-defined predicate identity.
    #[must_use]
    pub const fn predicate_id(&self) -> &Digest {
        &self.predicate_id
    }

    /// Returns the extractor-defined object identity when present.
    #[must_use]
    pub const fn object_id(&self) -> Option<&Digest> {
        self.object_id.as_ref()
    }

    /// Returns the extracted polarity.
    #[must_use]
    pub const fn polarity(&self) -> ClaimPolarity {
        self.polarity
    }

    /// Returns the extracted modal force.
    #[must_use]
    pub const fn modality(&self) -> ClaimModality {
        self.modality
    }

    /// Returns the number of extracted conditions.
    #[must_use]
    pub const fn condition_count(&self) -> u16 {
        self.condition_count
    }

    /// Returns whether the proposition has an explicit attribution.
    #[must_use]
    pub const fn attributed(&self) -> bool {
        self.attributed
    }

    /// Returns source spans supporting the extraction.
    #[must_use]
    pub fn evidence_spans(&self) -> &[SourceSpan] {
        &self.evidence_spans
    }

    /// Returns the canonical extractor confidence in parts per million.
    #[must_use]
    pub const fn confidence_ppm(&self) -> u32 {
        self.confidence_ppm
    }
}

/// Bounded claims extracted from one exact rewrite-unit text.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
pub struct ClaimEvidenceSet {
    schema_version: u32,
    extractor_id: String,
    extractor_version: String,
    extractor_manifest_digest: Digest,
    extraction_status: ClaimExtractionStatus,
    minimum_confidence_ppm: u32,
    unit_id: RewriteUnitId,
    text_digest: Digest,
    text_bytes: u64,
    evidence_digest: Digest,
    claims: Vec<ClaimEvidence>,
}

impl ClaimEvidenceSet {
    /// Builds a set after checking identity, bounds, status, and source spans.
    ///
    /// The manifest digest binds the extractor implementation, model artifact,
    /// prompt, runtime, and configuration selected by the caller. Raw text is
    /// used only for validation and is not retained.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimEvidenceError`] when the set is malformed or exceeds a bound.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor binds the complete extraction identity"
    )]
    pub fn new(
        extractor_id: impl Into<String>,
        extractor_version: impl Into<String>,
        extractor_manifest_digest: Digest,
        extraction_status: ClaimExtractionStatus,
        minimum_confidence_ppm: u32,
        unit_id: RewriteUnitId,
        text: &str,
        mut claims: Vec<ClaimEvidence>,
    ) -> Result<Self, ClaimEvidenceError> {
        let extractor_id = extractor_id.into();
        let extractor_version = extractor_version.into();
        if !valid_component(&extractor_id) || !valid_component(&extractor_version) {
            return Err(ClaimEvidenceError::InvalidExtractorIdentity);
        }
        if minimum_confidence_ppm > CLAIM_CONFIDENCE_PARTS_PER_MILLION {
            return Err(ClaimEvidenceError::InvalidConfidenceThreshold);
        }
        if claims.len() > MAX_CLAIMS_PER_UNIT {
            return Err(ClaimEvidenceError::TooManyClaims);
        }
        claims.sort_unstable_by(|left, right| {
            left.claim_id().as_str().cmp(right.claim_id().as_str())
        });
        let mut identities = HashSet::with_capacity(claims.len());
        for claim in &claims {
            if !identities.insert(claim.claim_id()) {
                return Err(ClaimEvidenceError::DuplicateClaim);
            }
            for span in claim.evidence_spans() {
                if span.end() > text.len()
                    || !text.is_char_boundary(span.start())
                    || !text.is_char_boundary(span.end())
                {
                    return Err(ClaimEvidenceError::InvalidSourceSpan);
                }
            }
        }
        let text_digest = Digest::sha256(text.as_bytes());
        let evidence_digest = evidence_digest(
            &extractor_manifest_digest,
            extraction_status,
            minimum_confidence_ppm,
            &unit_id,
            &text_digest,
            text.len() as u64,
            &claims,
        );
        Ok(Self {
            schema_version: CLAIM_EVIDENCE_SCHEMA_VERSION,
            extractor_id,
            extractor_version,
            extractor_manifest_digest,
            extraction_status,
            minimum_confidence_ppm,
            unit_id,
            text_digest,
            text_bytes: text.len() as u64,
            evidence_digest,
            claims,
        })
    }

    /// Returns the claim-evidence schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the stable extractor identifier.
    #[must_use]
    pub fn extractor_id(&self) -> &str {
        &self.extractor_id
    }
    /// Returns the exact extractor implementation version.
    #[must_use]
    pub fn extractor_version(&self) -> &str {
        &self.extractor_version
    }
    /// Returns the digest of the complete effective extractor manifest.
    #[must_use]
    pub const fn extractor_manifest_digest(&self) -> &Digest {
        &self.extractor_manifest_digest
    }
    /// Returns the extraction completion state.
    #[must_use]
    pub const fn extraction_status(&self) -> ClaimExtractionStatus {
        self.extraction_status
    }
    /// Returns the confidence threshold in parts per million.
    #[must_use]
    pub const fn minimum_confidence_ppm(&self) -> u32 {
        self.minimum_confidence_ppm
    }
    /// Returns the rewrite unit bound to the evidence.
    #[must_use]
    pub const fn unit_id(&self) -> &RewriteUnitId {
        &self.unit_id
    }
    /// Returns the digest of the exact text examined by the extractor.
    #[must_use]
    pub const fn text_digest(&self) -> &Digest {
        &self.text_digest
    }
    /// Returns the UTF-8 byte length of the examined text.
    #[must_use]
    pub const fn text_bytes(&self) -> u64 {
        self.text_bytes
    }
    /// Returns the digest of the canonical redacted evidence set.
    #[must_use]
    pub const fn evidence_digest(&self) -> &Digest {
        &self.evidence_digest
    }
    /// Returns claims in canonical claim-identity order.
    #[must_use]
    pub fn claims(&self) -> &[ClaimEvidence] {
        &self.claims
    }
}

/// Invalid claim extraction or comparison evidence.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ClaimEvidenceError {
    /// Extractor identifier or version is empty, oversized, or noncanonical.
    #[error("invalid claim extractor identity")]
    InvalidExtractorIdentity,
    /// The configured confidence threshold is outside zero to one.
    #[error("claim confidence threshold is invalid")]
    InvalidConfidenceThreshold,
    /// Extractor returned too many claims for one unit.
    #[error("claim count exceeds the per-unit limit")]
    TooManyClaims,
    /// Two claims use the same extractor-defined identity.
    #[error("claim identities are not unique")]
    DuplicateClaim,
    /// A claim has no span or too many spans.
    #[error("claim evidence span count is invalid")]
    InvalidSpanCount,
    /// A claim evidence span is empty.
    #[error("claim evidence span is empty")]
    EmptySpan,
    /// A claim repeats the same evidence span.
    #[error("claim evidence spans are not unique")]
    DuplicateSpan,
    /// A span is outside the bound text or not on a UTF-8 boundary.
    #[error("claim evidence span is outside the exact source text")]
    InvalidSourceSpan,
    /// Extractor confidence is not finite or outside zero to one.
    #[error("claim confidence must be finite and between zero and one")]
    InvalidConfidence,
    /// Aggregate comparison evidence is inconsistent or unbounded.
    #[error("claim comparison evidence is inconsistent")]
    InvalidComparison,
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_EXTRACTOR_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+' | b':')
        })
}

fn append_part(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value);
}

fn append_digest(bytes: &mut Vec<u8>, value: Option<&Digest>) {
    match value {
        Some(value) => {
            bytes.push(1);
            append_part(bytes, value.as_str().as_bytes());
        }
        None => bytes.push(0),
    }
}

fn evidence_digest(
    manifest: &Digest,
    status: ClaimExtractionStatus,
    confidence_ppm: u32,
    unit_id: &RewriteUnitId,
    text_digest: &Digest,
    text_bytes: u64,
    claims: &[ClaimEvidence],
) -> Digest {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&CLAIM_EVIDENCE_SCHEMA_VERSION.to_be_bytes());
    append_part(&mut bytes, manifest.as_str().as_bytes());
    bytes.push(match status {
        ClaimExtractionStatus::Complete => 0,
        ClaimExtractionStatus::Partial => 1,
        ClaimExtractionStatus::Abstained => 2,
        ClaimExtractionStatus::Failed => 3,
    });
    bytes.extend_from_slice(&confidence_ppm.to_be_bytes());
    append_part(&mut bytes, unit_id.as_str().as_bytes());
    append_part(&mut bytes, text_digest.as_str().as_bytes());
    bytes.extend_from_slice(&text_bytes.to_be_bytes());
    bytes.extend_from_slice(&(claims.len() as u64).to_be_bytes());
    for claim in claims {
        append_part(&mut bytes, claim.claim_id().as_str().as_bytes());
        append_digest(&mut bytes, claim.subject_id());
        append_part(&mut bytes, claim.predicate_id().as_str().as_bytes());
        append_digest(&mut bytes, claim.object_id());
        bytes.push(match claim.polarity() {
            ClaimPolarity::Affirmed => 0,
            ClaimPolarity::Negated => 1,
            ClaimPolarity::Unknown => 2,
        });
        bytes.push(match claim.modality() {
            ClaimModality::Asserted => 0,
            ClaimModality::Required => 1,
            ClaimModality::Permitted => 2,
            ClaimModality::Possible => 3,
            ClaimModality::Conditional => 4,
            ClaimModality::Unknown => 5,
        });
        bytes.extend_from_slice(&claim.condition_count().to_be_bytes());
        bytes.push(u8::from(claim.attributed()));
        bytes.extend_from_slice(&claim.confidence_ppm().to_be_bytes());
        bytes.extend_from_slice(&(claim.evidence_spans().len() as u64).to_be_bytes());
        for span in claim.evidence_spans() {
            bytes.extend_from_slice(&(span.start() as u64).to_be_bytes());
            bytes.extend_from_slice(&(span.end() as u64).to_be_bytes());
        }
    }
    Digest::sha256(&bytes)
}

#[cfg(test)]
#[path = "claim_tests.rs"]
mod tests;
