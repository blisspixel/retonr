use rewrite_types::{
    ClaimComparisonCounts, ClaimComparisonEvidence, ClaimEvidence, ClaimEvidenceSet,
    ClaimExtractionStatus, ClaimModality, ClaimPolarity,
};
use thiserror::Error;

/// Deterministic comparator for two independently extracted claim-evidence sets.
///
/// It aligns only exact extractor-defined subject, predicate, and object identities.
/// Ambiguous or incomplete evidence fails closed. The result is evidence for a
/// later semantic policy, not a semantic decision.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClaimEvidenceComparator;

impl ClaimEvidenceComparator {
    /// Compares source and candidate evidence from the same exact extractor.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimComparisonError`] when evidence is incompatible, incomplete,
    /// ambiguous, or cannot be represented within the bounded aggregate.
    pub fn compare(
        source: &ClaimEvidenceSet,
        candidate: &ClaimEvidenceSet,
    ) -> Result<ClaimComparisonEvidence, ClaimComparisonError> {
        validate_compatible_sets(source, candidate)?;
        let mut candidate_used = vec![false; candidate.claims().len()];
        let mut counts = base_counts(source, candidate)?;

        for source_claim in source.claims() {
            let mut matches = candidate
                .claims()
                .iter()
                .enumerate()
                .filter(|(_, candidate_claim)| same_relation(source_claim, candidate_claim));
            let Some((candidate_index, candidate_claim)) = matches.next() else {
                counts.missing_claims = increment(counts.missing_claims)?;
                continue;
            };
            if matches.next().is_some() || candidate_used[candidate_index] {
                return Err(ClaimComparisonError::AmbiguousAlignment);
            }
            candidate_used[candidate_index] = true;
            counts.aligned_claims = increment(counts.aligned_claims)?;
            compare_aligned(source_claim, candidate_claim, &mut counts)?;
        }

        counts.novel_claims = candidate_used.iter().try_fold(0_u32, |count, used| {
            if *used { Ok(count) } else { increment(count) }
        })?;
        ClaimComparisonEvidence::new(
            source.extractor_manifest_digest().clone(),
            source.unit_id().clone(),
            source.text_digest().clone(),
            source.text_bytes(),
            candidate.text_digest().clone(),
            candidate.text_bytes(),
            source.evidence_digest().clone(),
            candidate.evidence_digest().clone(),
            source.minimum_confidence_ppm(),
            counts,
        )
        .map_err(|_error| ClaimComparisonError::InvalidAggregate)
    }
}

fn validate_compatible_sets(
    source: &ClaimEvidenceSet,
    candidate: &ClaimEvidenceSet,
) -> Result<(), ClaimComparisonError> {
    if source.extractor_id() != candidate.extractor_id()
        || source.extractor_version() != candidate.extractor_version()
        || source.extractor_manifest_digest() != candidate.extractor_manifest_digest()
        || source.schema_version() != candidate.schema_version()
        || source.minimum_confidence_ppm() != candidate.minimum_confidence_ppm()
    {
        return Err(ClaimComparisonError::ExtractorMismatch);
    }
    if source.unit_id() != candidate.unit_id() {
        return Err(ClaimComparisonError::UnitMismatch);
    }
    if source.extraction_status() != ClaimExtractionStatus::Complete
        || candidate.extraction_status() != ClaimExtractionStatus::Complete
    {
        return Err(ClaimComparisonError::IncompleteExtraction);
    }
    if source.text_bytes() > 0 && source.claims().is_empty() {
        return Err(ClaimComparisonError::InsufficientEvidence);
    }
    Ok(())
}

fn base_counts(
    source: &ClaimEvidenceSet,
    candidate: &ClaimEvidenceSet,
) -> Result<ClaimComparisonCounts, ClaimComparisonError> {
    let mut counts = ClaimComparisonCounts {
        source_claims: count(source.claims().len())?,
        candidate_claims: count(candidate.claims().len())?,
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
    for claim in source.claims() {
        count_uncertainty(claim, source.minimum_confidence_ppm(), true, &mut counts)?;
    }
    for claim in candidate.claims() {
        count_uncertainty(
            claim,
            candidate.minimum_confidence_ppm(),
            false,
            &mut counts,
        )?;
    }
    Ok(counts)
}

fn count_uncertainty(
    claim: &ClaimEvidence,
    threshold_ppm: u32,
    source: bool,
    counts: &mut ClaimComparisonCounts,
) -> Result<(), ClaimComparisonError> {
    let below = claim.confidence_ppm() < threshold_ppm;
    if source {
        if claim.polarity() == ClaimPolarity::Unknown {
            counts.source_unknown_polarity = increment(counts.source_unknown_polarity)?;
        }
        if claim.modality() == ClaimModality::Unknown {
            counts.source_unknown_modality = increment(counts.source_unknown_modality)?;
        }
        if below {
            counts.source_below_confidence = increment(counts.source_below_confidence)?;
        }
    } else {
        if claim.polarity() == ClaimPolarity::Unknown {
            counts.candidate_unknown_polarity = increment(counts.candidate_unknown_polarity)?;
        }
        if claim.modality() == ClaimModality::Unknown {
            counts.candidate_unknown_modality = increment(counts.candidate_unknown_modality)?;
        }
        if below {
            counts.candidate_below_confidence = increment(counts.candidate_below_confidence)?;
        }
    }
    Ok(())
}

fn compare_aligned(
    source: &ClaimEvidence,
    candidate: &ClaimEvidence,
    counts: &mut ClaimComparisonCounts,
) -> Result<(), ClaimComparisonError> {
    let known_polarity = source.polarity() != ClaimPolarity::Unknown
        && candidate.polarity() != ClaimPolarity::Unknown;
    let polarity_changed = known_polarity && source.polarity() != candidate.polarity();
    let known_modality = source.modality() != ClaimModality::Unknown
        && candidate.modality() != ClaimModality::Unknown;
    let modality_changed = known_modality && source.modality() != candidate.modality();
    if polarity_changed {
        counts.polarity_conflicts = increment(counts.polarity_conflicts)?;
    }
    if modality_changed {
        counts.modality_conflicts = increment(counts.modality_conflicts)?;
    }
    if source.condition_count() != candidate.condition_count()
        || source.attributed() != candidate.attributed()
        || (source.claim_id() != candidate.claim_id() && !polarity_changed && !modality_changed)
    {
        counts.relationship_conflicts = increment(counts.relationship_conflicts)?;
    }
    Ok(())
}

fn same_relation(source: &ClaimEvidence, candidate: &ClaimEvidence) -> bool {
    source.subject_id() == candidate.subject_id()
        && source.predicate_id() == candidate.predicate_id()
        && source.object_id() == candidate.object_id()
}

fn count(value: usize) -> Result<u32, ClaimComparisonError> {
    u32::try_from(value).map_err(|_error| ClaimComparisonError::CountOverflow)
}

fn increment(value: u32) -> Result<u32, ClaimComparisonError> {
    value
        .checked_add(1)
        .ok_or(ClaimComparisonError::CountOverflow)
}

/// Reason deterministic claim comparison could not produce usable evidence.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ClaimComparisonError {
    /// Evidence came from different effective extractor identities or policies.
    #[error("claim evidence extractor identities differ")]
    ExtractorMismatch,
    /// Evidence refers to different rewrite units.
    #[error("claim evidence rewrite units differ")]
    UnitMismatch,
    /// One extraction did not complete its configured procedure.
    #[error("claim extraction did not complete")]
    IncompleteExtraction,
    /// A nonempty source produced no comparison-eligible claims.
    #[error("claim extraction produced insufficient source evidence")]
    InsufficientEvidence,
    /// Exact role identities do not provide a unique one-to-one alignment.
    #[error("claim evidence alignment is ambiguous")]
    AmbiguousAlignment,
    /// Claim counts cannot be represented in the aggregate contract.
    #[error("claim comparison count overflow")]
    CountOverflow,
    /// Constructed aggregate evidence violated its own invariants.
    #[error("claim comparison aggregate is invalid")]
    InvalidAggregate,
}

#[cfg(test)]
mod tests {
    use rewrite_types::{
        ClaimEvidence, ClaimEvidenceSet, ClaimExtractionStatus, ClaimModality, ClaimPolarity,
        Digest, DocumentId, RewriteUnitId, SourceSpan,
    };

    use super::{ClaimComparisonError, ClaimEvidenceComparator};

    fn unit(seed: &[u8]) -> RewriteUnitId {
        RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(seed)), 0)
    }

    fn claim(
        id: &[u8],
        polarity: ClaimPolarity,
        modality: ClaimModality,
        confidence: f32,
    ) -> ClaimEvidence {
        ClaimEvidence::new(
            Digest::sha256(id),
            Some(Digest::sha256(b"subject")),
            Digest::sha256(b"predicate"),
            Some(Digest::sha256(b"object")),
            polarity,
            modality,
            0,
            false,
            vec![SourceSpan::new(0, 1).expect("valid span")],
            confidence,
        )
        .expect("valid claim fixture")
    }

    fn set(
        unit_id: RewriteUnitId,
        extractor: &str,
        status: ClaimExtractionStatus,
        text: &str,
        claims: Vec<ClaimEvidence>,
    ) -> ClaimEvidenceSet {
        ClaimEvidenceSet::new(
            extractor,
            "1",
            Digest::sha256(extractor.as_bytes()),
            status,
            900_000,
            unit_id,
            text,
            claims,
        )
        .expect("valid evidence set")
    }

    #[test]
    fn binds_comparison_and_reports_known_conflicts() {
        let unit_id = unit(b"unit");
        let source = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "s",
            vec![claim(
                b"source",
                ClaimPolarity::Affirmed,
                ClaimModality::Required,
                1.0,
            )],
        );
        let candidate = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "c",
            vec![claim(
                b"candidate",
                ClaimPolarity::Negated,
                ClaimModality::Possible,
                1.0,
            )],
        );
        let compared = ClaimEvidenceComparator::compare(&source, &candidate)
            .expect("comparison is compatible");
        assert_eq!(compared.unit_id(), &unit_id);
        assert_eq!(compared.source_text_digest(), source.text_digest());
        assert_eq!(
            compared.candidate_evidence_digest(),
            candidate.evidence_digest()
        );
        assert_eq!(compared.counts().polarity_conflicts, 1);
        assert_eq!(compared.counts().modality_conflicts, 1);
    }

    #[test]
    fn retains_uncertainty_without_calling_it_conflict() {
        let unit_id = unit(b"uncertain");
        let source = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "s",
            vec![claim(
                b"same",
                ClaimPolarity::Unknown,
                ClaimModality::Unknown,
                0.5,
            )],
        );
        let candidate = set(
            unit_id,
            "fixture",
            ClaimExtractionStatus::Complete,
            "c",
            vec![claim(
                b"same",
                ClaimPolarity::Affirmed,
                ClaimModality::Required,
                1.0,
            )],
        );
        let counts = ClaimEvidenceComparator::compare(&source, &candidate)
            .expect("comparison retains uncertainty")
            .counts();
        assert_eq!(counts.polarity_conflicts, 0);
        assert_eq!(counts.modality_conflicts, 0);
        assert_eq!(counts.source_unknown_polarity, 1);
        assert_eq!(counts.source_unknown_modality, 1);
        assert_eq!(counts.source_below_confidence, 1);
        assert!(counts.has_uncertainty());
    }

    #[test]
    fn exact_confidence_threshold_is_not_below_threshold() {
        let unit_id = unit(b"threshold");
        let source = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "s",
            vec![claim(
                b"same",
                ClaimPolarity::Affirmed,
                ClaimModality::Asserted,
                0.9,
            )],
        );
        let candidate = set(
            unit_id,
            "fixture",
            ClaimExtractionStatus::Complete,
            "c",
            vec![claim(
                b"same",
                ClaimPolarity::Affirmed,
                ClaimModality::Asserted,
                0.9,
            )],
        );
        let counts = ClaimEvidenceComparator::compare(&source, &candidate)
            .expect("threshold equality is comparison eligible")
            .counts();
        assert_eq!(counts.source_below_confidence, 0);
        assert_eq!(counts.candidate_below_confidence, 0);
    }

    #[test]
    fn rejects_incomplete_empty_and_mismatched_evidence() {
        let unit_id = unit(b"reject");
        let complete = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "s",
            vec![claim(
                b"c",
                ClaimPolarity::Affirmed,
                ClaimModality::Asserted,
                1.0,
            )],
        );
        let partial = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Partial,
            "c",
            Vec::new(),
        );
        assert_eq!(
            ClaimEvidenceComparator::compare(&complete, &partial),
            Err(ClaimComparisonError::IncompleteExtraction)
        );
        let empty = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "s",
            Vec::new(),
        );
        assert_eq!(
            ClaimEvidenceComparator::compare(&empty, &empty),
            Err(ClaimComparisonError::InsufficientEvidence)
        );
        let other = set(
            unit_id,
            "other",
            ClaimExtractionStatus::Complete,
            "c",
            Vec::new(),
        );
        assert_eq!(
            ClaimEvidenceComparator::compare(&complete, &other),
            Err(ClaimComparisonError::ExtractorMismatch)
        );
    }

    #[test]
    fn rejects_ambiguous_alignment() {
        let unit_id = unit(b"ambiguous");
        let source = set(
            unit_id.clone(),
            "fixture",
            ClaimExtractionStatus::Complete,
            "s",
            vec![claim(
                b"source",
                ClaimPolarity::Affirmed,
                ClaimModality::Asserted,
                1.0,
            )],
        );
        let candidate = set(
            unit_id,
            "fixture",
            ClaimExtractionStatus::Complete,
            "cc",
            vec![
                claim(
                    b"one",
                    ClaimPolarity::Affirmed,
                    ClaimModality::Asserted,
                    1.0,
                ),
                claim(
                    b"two",
                    ClaimPolarity::Affirmed,
                    ClaimModality::Asserted,
                    1.0,
                ),
            ],
        );
        assert_eq!(
            ClaimEvidenceComparator::compare(&source, &candidate),
            Err(ClaimComparisonError::AmbiguousAlignment)
        );
    }
}
