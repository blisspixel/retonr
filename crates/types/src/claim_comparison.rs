use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use crate::{
    CLAIM_CONFIDENCE_PARTS_PER_MILLION, CLAIM_EVIDENCE_SCHEMA_VERSION, ClaimEvidenceError, Digest,
    MAX_CLAIMS_PER_UNIT, RewriteUnitId,
};

/// Current claim-comparison evidence contract version.
pub const CLAIM_COMPARISON_SCHEMA_VERSION: u32 = 1;
/// Current deterministic claim-comparator implementation version.
pub const CLAIM_COMPARATOR_VERSION: u32 = 1;

/// Bounded counts from comparing two claim-evidence sets.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimComparisonCounts {
    /// Claims extracted from the source.
    pub source_claims: u32,
    /// Claims extracted from the candidate.
    pub candidate_claims: u32,
    /// Claims aligned by exact redacted relation identities.
    pub aligned_claims: u32,
    /// Source claims with no eligible candidate alignment.
    pub missing_claims: u32,
    /// Candidate claims with no eligible source alignment.
    pub novel_claims: u32,
    /// Aligned known polarities that differ.
    pub polarity_conflicts: u32,
    /// Aligned known modalities that differ.
    pub modality_conflicts: u32,
    /// Aligned claims with condition, attribution, or residual identity differences.
    pub relationship_conflicts: u32,
    /// Source claims with unknown polarity.
    pub source_unknown_polarity: u32,
    /// Candidate claims with unknown polarity.
    pub candidate_unknown_polarity: u32,
    /// Source claims with unknown modality.
    pub source_unknown_modality: u32,
    /// Candidate claims with unknown modality.
    pub candidate_unknown_modality: u32,
    /// Source claims below the declared extractor confidence threshold.
    pub source_below_confidence: u32,
    /// Candidate claims below the declared extractor confidence threshold.
    pub candidate_below_confidence: u32,
}

impl ClaimComparisonCounts {
    /// Validates count accounting and contract bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimEvidenceError`] when any count is inconsistent or unbounded.
    pub fn validate(self) -> Result<Self, ClaimEvidenceError> {
        let maximum = u32::try_from(MAX_CLAIMS_PER_UNIT)
            .map_err(|_error| ClaimEvidenceError::InvalidComparison)?;
        let source_accounted = self
            .aligned_claims
            .checked_add(self.missing_claims)
            .ok_or(ClaimEvidenceError::InvalidComparison)?;
        let candidate_accounted = self
            .aligned_claims
            .checked_add(self.novel_claims)
            .ok_or(ClaimEvidenceError::InvalidComparison)?;
        let source_subsets = [
            self.source_unknown_polarity,
            self.source_unknown_modality,
            self.source_below_confidence,
        ];
        let candidate_subsets = [
            self.candidate_unknown_polarity,
            self.candidate_unknown_modality,
            self.candidate_below_confidence,
        ];
        if self.source_claims > maximum
            || self.candidate_claims > maximum
            || source_accounted != self.source_claims
            || candidate_accounted != self.candidate_claims
            || self.polarity_conflicts > self.aligned_claims
            || self.modality_conflicts > self.aligned_claims
            || self.relationship_conflicts > self.aligned_claims
            || source_subsets
                .into_iter()
                .any(|count| count > self.source_claims)
            || candidate_subsets
                .into_iter()
                .any(|count| count > self.candidate_claims)
        {
            return Err(ClaimEvidenceError::InvalidComparison);
        }
        Ok(self)
    }

    /// Returns whether the extractor reported unresolved claim uncertainty.
    #[must_use]
    pub const fn has_uncertainty(self) -> bool {
        self.source_unknown_polarity > 0
            || self.candidate_unknown_polarity > 0
            || self.source_unknown_modality > 0
            || self.candidate_unknown_modality > 0
            || self.source_below_confidence > 0
            || self.candidate_below_confidence > 0
    }

    /// Returns whether the comparison reports any missing, novel, or conflicting claim.
    #[must_use]
    pub const fn has_difference(self) -> bool {
        self.missing_claims > 0
            || self.novel_claims > 0
            || self.polarity_conflicts > 0
            || self.modality_conflicts > 0
            || self.relationship_conflicts > 0
    }
}

impl<'de> Deserialize<'de> for ClaimComparisonCounts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            source_claims: u32,
            candidate_claims: u32,
            aligned_claims: u32,
            missing_claims: u32,
            novel_claims: u32,
            polarity_conflicts: u32,
            modality_conflicts: u32,
            relationship_conflicts: u32,
            source_unknown_polarity: u32,
            candidate_unknown_polarity: u32,
            source_unknown_modality: u32,
            candidate_unknown_modality: u32,
            source_below_confidence: u32,
            candidate_below_confidence: u32,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self {
            source_claims: wire.source_claims,
            candidate_claims: wire.candidate_claims,
            aligned_claims: wire.aligned_claims,
            missing_claims: wire.missing_claims,
            novel_claims: wire.novel_claims,
            polarity_conflicts: wire.polarity_conflicts,
            modality_conflicts: wire.modality_conflicts,
            relationship_conflicts: wire.relationship_conflicts,
            source_unknown_polarity: wire.source_unknown_polarity,
            candidate_unknown_polarity: wire.candidate_unknown_polarity,
            source_unknown_modality: wire.source_unknown_modality,
            candidate_unknown_modality: wire.candidate_unknown_modality,
            source_below_confidence: wire.source_below_confidence,
            candidate_below_confidence: wire.candidate_below_confidence,
        }
        .validate()
        .map_err(D::Error::custom)
    }
}

/// Redacted aggregate bound to the exact evidence sets that were compared.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ClaimComparisonEvidence {
    schema_version: u32,
    claim_schema_version: u32,
    comparator_version: u32,
    extractor_manifest_digest: Digest,
    unit_id: RewriteUnitId,
    source_text_digest: Digest,
    source_text_bytes: u64,
    candidate_text_digest: Digest,
    candidate_text_bytes: u64,
    source_evidence_digest: Digest,
    candidate_evidence_digest: Digest,
    minimum_confidence_ppm: u32,
    counts: ClaimComparisonCounts,
}

impl ClaimComparisonEvidence {
    /// Builds an aggregate bound to exact redacted evidence identities.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimEvidenceError`] when the counts violate the bounded contract.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor binds both exact inputs"
    )]
    pub fn new(
        extractor_manifest_digest: Digest,
        unit_id: RewriteUnitId,
        source_text_digest: Digest,
        source_text_bytes: u64,
        candidate_text_digest: Digest,
        candidate_text_bytes: u64,
        source_evidence_digest: Digest,
        candidate_evidence_digest: Digest,
        minimum_confidence_ppm: u32,
        counts: ClaimComparisonCounts,
    ) -> Result<Self, ClaimEvidenceError> {
        let evidence = Self {
            schema_version: CLAIM_COMPARISON_SCHEMA_VERSION,
            claim_schema_version: CLAIM_EVIDENCE_SCHEMA_VERSION,
            comparator_version: CLAIM_COMPARATOR_VERSION,
            extractor_manifest_digest,
            unit_id,
            source_text_digest,
            source_text_bytes,
            candidate_text_digest,
            candidate_text_bytes,
            source_evidence_digest,
            candidate_evidence_digest,
            minimum_confidence_ppm,
            counts,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    /// Validates versions, bounds, and count consistency.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimEvidenceError`] when a version or count is invalid.
    pub fn validate(&self) -> Result<(), ClaimEvidenceError> {
        if self.schema_version != CLAIM_COMPARISON_SCHEMA_VERSION
            || self.claim_schema_version != CLAIM_EVIDENCE_SCHEMA_VERSION
            || self.comparator_version != CLAIM_COMPARATOR_VERSION
            || self.minimum_confidence_ppm > CLAIM_CONFIDENCE_PARTS_PER_MILLION
            || (self.source_text_bytes > 0 && self.counts.source_claims == 0)
        {
            return Err(ClaimEvidenceError::InvalidComparison);
        }
        self.counts.validate()?;
        Ok(())
    }

    /// Returns the exact extractor-manifest identity.
    #[must_use]
    pub const fn extractor_manifest_digest(&self) -> &Digest {
        &self.extractor_manifest_digest
    }
    /// Returns the rewrite unit compared.
    #[must_use]
    pub const fn unit_id(&self) -> &RewriteUnitId {
        &self.unit_id
    }
    /// Returns the source text digest.
    #[must_use]
    pub const fn source_text_digest(&self) -> &Digest {
        &self.source_text_digest
    }
    /// Returns the source UTF-8 byte length.
    #[must_use]
    pub const fn source_text_bytes(&self) -> u64 {
        self.source_text_bytes
    }
    /// Returns the candidate text digest.
    #[must_use]
    pub const fn candidate_text_digest(&self) -> &Digest {
        &self.candidate_text_digest
    }
    /// Returns the candidate UTF-8 byte length.
    #[must_use]
    pub const fn candidate_text_bytes(&self) -> u64 {
        self.candidate_text_bytes
    }
    /// Returns the source evidence-set digest.
    #[must_use]
    pub const fn source_evidence_digest(&self) -> &Digest {
        &self.source_evidence_digest
    }
    /// Returns the candidate evidence-set digest.
    #[must_use]
    pub const fn candidate_evidence_digest(&self) -> &Digest {
        &self.candidate_evidence_digest
    }
    /// Returns the bounded comparison counts.
    #[must_use]
    pub const fn counts(&self) -> ClaimComparisonCounts {
        self.counts
    }
}

impl<'de> Deserialize<'de> for ClaimComparisonEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            claim_schema_version: u32,
            comparator_version: u32,
            extractor_manifest_digest: Digest,
            unit_id: RewriteUnitId,
            source_text_digest: Digest,
            source_text_bytes: u64,
            candidate_text_digest: Digest,
            candidate_text_bytes: u64,
            source_evidence_digest: Digest,
            candidate_evidence_digest: Digest,
            minimum_confidence_ppm: u32,
            counts: ClaimComparisonCounts,
        }
        let wire = Wire::deserialize(deserializer)?;
        let evidence = Self {
            schema_version: wire.schema_version,
            claim_schema_version: wire.claim_schema_version,
            comparator_version: wire.comparator_version,
            extractor_manifest_digest: wire.extractor_manifest_digest,
            unit_id: wire.unit_id,
            source_text_digest: wire.source_text_digest,
            source_text_bytes: wire.source_text_bytes,
            candidate_text_digest: wire.candidate_text_digest,
            candidate_text_bytes: wire.candidate_text_bytes,
            source_evidence_digest: wire.source_evidence_digest,
            candidate_evidence_digest: wire.candidate_evidence_digest,
            minimum_confidence_ppm: wire.minimum_confidence_ppm,
            counts: wire.counts,
        };
        evidence.validate().map_err(D::Error::custom)?;
        Ok(evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::{ClaimComparisonCounts, ClaimComparisonEvidence};
    use crate::{ClaimEvidenceError, Digest, DocumentId, MAX_CLAIMS_PER_UNIT, RewriteUnitId};

    fn counts() -> ClaimComparisonCounts {
        ClaimComparisonCounts {
            source_claims: 1,
            candidate_claims: 1,
            aligned_claims: 1,
            missing_claims: 0,
            novel_claims: 0,
            polarity_conflicts: 0,
            modality_conflicts: 0,
            relationship_conflicts: 0,
            source_unknown_polarity: 0,
            candidate_unknown_polarity: 0,
            source_unknown_modality: 0,
            candidate_unknown_modality: 0,
            source_below_confidence: 0,
            candidate_below_confidence: 0,
        }
    }

    #[test]
    fn rejects_unbounded_or_inconsistent_counts() {
        let mut invalid = counts();
        invalid.source_claims =
            u32::try_from(MAX_CLAIMS_PER_UNIT).expect("claim limit fits in u32") + 1;
        invalid.missing_claims = invalid.source_claims - 1;
        assert_eq!(
            invalid.validate(),
            Err(ClaimEvidenceError::InvalidComparison)
        );

        let unit = RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(b"u")), 0);
        let invalid_threshold = ClaimComparisonEvidence::new(
            Digest::sha256(b"manifest"),
            unit.clone(),
            Digest::sha256(b"s"),
            1,
            Digest::sha256(b"c"),
            1,
            Digest::sha256(b"se"),
            Digest::sha256(b"ce"),
            1_000_001,
            counts(),
        );
        assert_eq!(
            invalid_threshold,
            Err(ClaimEvidenceError::InvalidComparison)
        );

        let empty = ClaimComparisonCounts {
            source_claims: 0,
            candidate_claims: 0,
            aligned_claims: 0,
            missing_claims: 0,
            novel_claims: 0,
            polarity_conflicts: 0,
            modality_conflicts: 0,
            relationship_conflicts: 0,
            source_unknown_polarity: 0,
            candidate_unknown_polarity: 0,
            source_unknown_modality: 0,
            candidate_unknown_modality: 0,
            source_below_confidence: 0,
            candidate_below_confidence: 0,
        };
        assert_eq!(
            ClaimComparisonEvidence::new(
                Digest::sha256(b"manifest"),
                unit,
                Digest::sha256(b"s"),
                1,
                Digest::sha256(b"c"),
                1,
                Digest::sha256(b"se"),
                Digest::sha256(b"ce"),
                0,
                empty,
            ),
            Err(ClaimEvidenceError::InvalidComparison)
        );
        let mut invalid = counts();
        invalid.source_unknown_polarity = 2;
        assert_eq!(
            invalid.validate(),
            Err(ClaimEvidenceError::InvalidComparison)
        );
    }

    #[test]
    fn bound_record_round_trips_and_rejects_unknown_fields() {
        let unit = RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(b"u")), 0);
        let evidence = ClaimComparisonEvidence::new(
            Digest::sha256(b"manifest"),
            unit,
            Digest::sha256(b"s"),
            1,
            Digest::sha256(b"c"),
            1,
            Digest::sha256(b"se"),
            Digest::sha256(b"ce"),
            900_000,
            counts(),
        )
        .expect("valid bound comparison");
        let encoded = serde_json::to_string(&evidence).expect("comparison serializes");
        assert_eq!(
            serde_json::from_str::<ClaimComparisonEvidence>(&encoded)
                .expect("comparison deserializes"),
            evidence
        );
        let altered = encoded.replacen('{', "{\"unexpected\":1,", 1);
        assert!(serde_json::from_str::<ClaimComparisonEvidence>(&altered).is_err());
    }
}
