use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::EvaluationCase;

use super::{
    CandidateDeterministicCompilerError, CandidateDeterministicCompilerRelationship,
    relationship_error,
};

const SUITE_PAIR_DOMAIN: &[u8] = rewrite_model::CANDIDATE_DETERMINISTIC_SUITE_PAIR_DIGEST_DOMAIN;
const DIGEST_TEXT_BYTES: usize = 64;

pub(super) fn derive_semantic_to_lexicographic_permutation(
    projected: &[(usize, EvaluationCase, EvaluationCase)],
) -> Result<Vec<u32>, CandidateDeterministicCompilerError> {
    let entries = projected
        .iter()
        .map(|(semantic_index, case_a, case_b)| {
            (*semantic_index, case_a.id.as_str(), case_b.id.as_str())
        })
        .collect::<Vec<_>>();
    derive_case_key_permutation(&entries)
}

pub(crate) fn derive_case_key_permutation(
    entries: &[(usize, &str, &str)],
) -> Result<Vec<u32>, CandidateDeterministicCompilerError> {
    let mut semantic_seen = vec![false; entries.len()];
    let mut ordered = entries
        .iter()
        .map(|(semantic_index, case_a, case_b)| {
            let slot = semantic_seen.get_mut(*semantic_index).ok_or_else(|| {
                relationship_error(
                    *semantic_index,
                    CandidateDeterministicCompilerRelationship::CasePermutation,
                )
            })?;
            if case_a != case_b || *slot {
                return Err(relationship_error(
                    *semantic_index,
                    CandidateDeterministicCompilerRelationship::CasePermutation,
                ));
            }
            *slot = true;
            Ok((*case_a, *semantic_index))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if semantic_seen.iter().any(|seen| !seen) {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::CasePermutation,
        ));
    }
    ordered.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if ordered.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(relationship_error(
            0,
            CandidateDeterministicCompilerRelationship::CasePermutation,
        ));
    }
    let mut permutation = vec![u32::MAX; entries.len()];
    for (lexicographic_index, (_, semantic_index)) in ordered.into_iter().enumerate() {
        let lexicographic_index = u32::try_from(lexicographic_index).map_err(|_| {
            relationship_error(
                semantic_index,
                CandidateDeterministicCompilerRelationship::CasePermutation,
            )
        })?;
        permutation[semantic_index] = lexicographic_index;
    }
    Ok(permutation)
}

#[expect(
    clippy::similar_names,
    reason = "candidate A and B JSON are identity-significant ordered inputs"
)]
pub(super) fn derive_suite_pair_digest(
    suite_id_digest: &Digest,
    material_digest: &Digest,
    permutation: &[u32],
    candidate_a_json: &[u8],
    candidate_b_json: &[u8],
) -> Result<Digest, CandidateDeterministicCompilerError> {
    let _canonical_length = SUITE_PAIR_DOMAIN
        .len()
        .checked_add(DIGEST_TEXT_BYTES * 2)
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(permutation.len().checked_mul(4)?))
        .and_then(|value| value.checked_add(8)?.checked_add(candidate_a_json.len()))
        .and_then(|value| value.checked_add(8)?.checked_add(candidate_b_json.len()))
        .ok_or(CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)?;
    let count = u64::try_from(permutation.len())
        .map_err(|_| CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)?;
    let length_a = u64::try_from(candidate_a_json.len())
        .map_err(|_| CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)?;
    let length_b = u64::try_from(candidate_b_json.len())
        .map_err(|_| CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)?;
    let mut hasher = Sha256::new();
    hasher.update(SUITE_PAIR_DOMAIN);
    hasher.update(suite_id_digest.as_str().as_bytes());
    hasher.update(material_digest.as_str().as_bytes());
    hasher.update(count.to_be_bytes());
    for index in permutation {
        hasher.update(index.to_be_bytes());
    }
    hasher.update(length_a.to_be_bytes());
    hasher.update(candidate_a_json);
    hasher.update(length_b.to_be_bytes());
    hasher.update(candidate_b_json);
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| CandidateDeterministicCompilerError::ProjectedSuiteTooLarge)
}
