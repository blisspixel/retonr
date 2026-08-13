use std::collections::HashSet;

use rewrite_types::{CandidateRank, GeneratedCandidate, ReasonCode, RewriteOptions, RewriteUnitId};

use crate::protection::protected_terms_are_valid;
use crate::{EngineError, MAX_GENERATED_CANDIDATES, MAX_GENERATED_TEXT_BYTES};

/// Validates caller-controlled rewrite policy before generation begins.
///
/// # Errors
///
/// Returns [`EngineError`] when semantic confidence or protected terms violate
/// the bounded engine contract.
pub fn validate_rewrite_options(options: &RewriteOptions) -> Result<(), EngineError> {
    if !options.minimum_semantic_confidence.is_finite()
        || !(0.0..=1.0).contains(&options.minimum_semantic_confidence)
    {
        return Err(EngineError::InvalidSemanticConfidence);
    }
    if !protected_terms_are_valid(&options.protected_terms) {
        return Err(EngineError::InvalidProtectedTerms);
    }
    Ok(())
}

pub(crate) fn candidate_contract_is_valid(candidate: &GeneratedCandidate) -> bool {
    candidate.id.is_scoped_to(&candidate.unit_id)
        && candidate.text.len() <= MAX_GENERATED_TEXT_BYTES
        && candidate_rank_is_valid(candidate.rank)
}

fn candidate_batch_identity_is_valid(
    unit_id: &RewriteUnitId,
    candidates: &[GeneratedCandidate],
) -> bool {
    let mut identities = HashSet::with_capacity(candidates.len());
    candidates.iter().all(|candidate| {
        candidate.unit_id == *unit_id
            && candidate.id.is_scoped_to(unit_id)
            && identities.insert(candidate.id.as_str())
    })
}

const fn candidate_count_reason(count: usize) -> Option<ReasonCode> {
    if count == 0 {
        Some(ReasonCode::NoCandidate)
    } else if count > MAX_GENERATED_CANDIDATES {
        Some(ReasonCode::InvalidCandidate)
    } else {
        None
    }
}

pub(crate) fn candidate_batch_reason(
    unit_id: &RewriteUnitId,
    candidates: &[GeneratedCandidate],
) -> Option<ReasonCode> {
    candidate_count_reason(candidates.len()).or_else(|| {
        (!candidate_batch_identity_is_valid(unit_id, candidates))
            .then_some(ReasonCode::InvalidCandidate)
    })
}

fn candidate_rank_is_valid(rank: CandidateRank) -> bool {
    [rank.style, rank.channel, rank.fluency]
        .into_iter()
        .all(|score| score.is_finite() && (0.0..=1.0).contains(&score))
}

pub(crate) const fn preferred_reason(
    current: Option<ReasonCode>,
    candidate: ReasonCode,
) -> ReasonCode {
    match current {
        Some(current) if reason_priority(current) <= reason_priority(candidate) => current,
        _ => candidate,
    }
}

pub(crate) const fn reason_priority(reason: ReasonCode) -> u8 {
    match reason {
        ReasonCode::SentinelIntegrity => 0,
        ReasonCode::ProtectedValueChanged => 1,
        ReasonCode::UnsafeText => 2,
        ReasonCode::StructureChanged => 3,
        ReasonCode::SemanticMismatch => 4,
        ReasonCode::SemanticUncertain => 5,
        ReasonCode::InvalidCandidate => 6,
        ReasonCode::NoCandidate => 7,
        ReasonCode::ReassemblyVerification => 8,
        ReasonCode::Cancelled => 9,
        ReasonCode::UnsupportedAtomicity => 10,
    }
}
