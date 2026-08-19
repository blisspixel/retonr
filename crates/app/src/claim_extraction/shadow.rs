use rewrite_engine::{ClaimEvidenceComparator, ClaimShadowObserver};
use rewrite_inference::InferenceBackend;
use rewrite_types::{ClaimComparisonEvidence, ClaimEvidenceSet, Digest, RewriteUnitId};

use super::{
    ClaimExtractionContext, ClaimExtractionError, ClaimExtractionService, ClaimShadowJoinBinding,
};

/// Independently produced comparison retained for one restored unit pair.
///
/// The observer returns evidence only when the examined text matches the bound
/// comparison. It cannot authorize a rewrite.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedClaimShadow {
    comparison: ClaimComparisonEvidence,
}

impl PreparedClaimShadow {
    /// Retains one validated comparison for later informational recording.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimExtractionError::Comparison`] when the comparison is not a
    /// valid evidence object.
    pub fn from_comparison(
        comparison: ClaimComparisonEvidence,
    ) -> Result<Self, ClaimExtractionError> {
        comparison.validate().map_err(|_error| {
            ClaimExtractionError::Comparison(rewrite_engine::ClaimComparisonError::InvalidAggregate)
        })?;
        Ok(Self { comparison })
    }

    /// Compares two independently produced evidence sets and retains the result.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimExtractionError::Comparison`] when the sets cannot be
    /// compared or the aggregate is invalid.
    pub fn from_evidence_sets(
        source: &ClaimEvidenceSet,
        candidate: &ClaimEvidenceSet,
    ) -> Result<Self, ClaimExtractionError> {
        let comparison = ClaimEvidenceComparator::compare(source, candidate)
            .map_err(ClaimExtractionError::Comparison)?;
        Self::from_comparison(comparison)
    }

    /// Returns the retained comparison evidence.
    #[must_use]
    pub const fn comparison(&self) -> &ClaimComparisonEvidence {
        &self.comparison
    }

    fn matches(&self, unit_id: &RewriteUnitId, source: &str, candidate: &str) -> bool {
        self.comparison.unit_id() == unit_id
            && self.comparison.source_text_digest() == &Digest::sha256(source.as_bytes())
            && self.comparison.candidate_text_digest() == &Digest::sha256(candidate.as_bytes())
            && self.comparison.source_text_bytes() == source.len() as u64
            && self.comparison.candidate_text_bytes() == candidate.len() as u64
    }
}

impl ClaimShadowObserver for PreparedClaimShadow {
    fn observe(
        &self,
        unit_id: &RewriteUnitId,
        source: &str,
        candidate: &str,
    ) -> Option<ClaimComparisonEvidence> {
        self.matches(unit_id, source, candidate)
            .then(|| self.comparison.clone())
    }
}

/// Zero or more prepared comparisons for candidates of one unit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreparedClaimShadowSet {
    shadows: Vec<PreparedClaimShadow>,
}

impl PreparedClaimShadowSet {
    /// Creates a set from independently prepared comparisons.
    #[must_use]
    pub fn new(shadows: Vec<PreparedClaimShadow>) -> Self {
        Self { shadows }
    }

    /// Returns whether no comparison was retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shadows.is_empty()
    }

    /// Returns the retained comparisons in preparation order.
    #[must_use]
    pub fn shadows(&self) -> &[PreparedClaimShadow] {
        &self.shadows
    }
}

impl ClaimShadowObserver for PreparedClaimShadowSet {
    fn observe(
        &self,
        unit_id: &RewriteUnitId,
        source: &str,
        candidate: &str,
    ) -> Option<ClaimComparisonEvidence> {
        self.shadows
            .iter()
            .find_map(|shadow| shadow.observe(unit_id, source, candidate))
    }
}

/// Result of attempting one informational shadow join.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "boxing would change the public join DTO"
)]
pub enum ClaimShadowJoinDisposition {
    /// Completed comparison bound to the examined unit pair.
    Recorded(PreparedClaimShadow),
    /// No comparison was available. Hard gates stay unchanged.
    Skipped,
}

/// Prepares independently produced comparison evidence for the engine shadow gate.
///
/// Backend unavailability, malformed payloads, and incomplete extraction skip the
/// join. Cancellation still discards partial work. The result has no rewrite
/// authority.
pub struct ClaimShadowJoinService<'a> {
    extraction: ClaimExtractionService<'a>,
}

impl<'a> ClaimShadowJoinService<'a> {
    /// Creates a join service over an already constructed inference backend.
    #[must_use]
    pub const fn new(backend: &'a dyn InferenceBackend) -> Self {
        Self {
            extraction: ClaimExtractionService::new(backend),
        }
    }

    /// Extracts one restored pair and retains comparison evidence when complete.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimExtractionError`] for an invalid binding or cancellation.
    /// Unavailable or unusable extraction results are [`ClaimShadowJoinDisposition::Skipped`].
    pub async fn prepare(
        &self,
        binding: &ClaimShadowJoinBinding,
        unit_id: RewriteUnitId,
        source: &str,
        candidate: &str,
        context: ClaimExtractionContext<'_>,
    ) -> Result<ClaimShadowJoinDisposition, ClaimExtractionError> {
        let request = binding.extraction_request(unit_id, source, candidate);
        match self.extraction.extract(request, context).await {
            Ok(pair) => match pair.comparison {
                Some(comparison) => PreparedClaimShadow::from_comparison(comparison)
                    .map(ClaimShadowJoinDisposition::Recorded),
                None => Ok(ClaimShadowJoinDisposition::Skipped),
            },
            Err(
                ClaimExtractionError::Unavailable
                | ClaimExtractionError::InvalidPayload
                | ClaimExtractionError::PayloadMismatch
                | ClaimExtractionError::InvalidEvidence(_)
                | ClaimExtractionError::Comparison(_)
                | ClaimExtractionError::Backend(_),
            ) => Ok(ClaimShadowJoinDisposition::Skipped),
            Err(error) => Err(error),
        }
    }

    /// Prepares comparisons for every restored candidate of one source unit.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimExtractionError`] for an invalid binding or cancellation.
    pub async fn prepare_for_candidates(
        &self,
        binding: &ClaimShadowJoinBinding,
        unit_id: RewriteUnitId,
        source: &str,
        restored_candidates: impl IntoIterator<Item = &str>,
        context: ClaimExtractionContext<'_>,
    ) -> Result<PreparedClaimShadowSet, ClaimExtractionError> {
        let mut shadows = Vec::new();
        for candidate in restored_candidates {
            match self
                .prepare(binding, unit_id.clone(), source, candidate, context)
                .await?
            {
                ClaimShadowJoinDisposition::Recorded(shadow) => shadows.push(shadow),
                ClaimShadowJoinDisposition::Skipped => {}
            }
        }
        Ok(PreparedClaimShadowSet::new(shadows))
    }
}

#[cfg(test)]
mod tests;
