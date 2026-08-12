use core::cmp::Ordering;

use rewrite_types::GeneratedCandidate;

/// Selects the strongest eligible candidate using a stable lexicographic order.
///
/// Style, channel fit, and fluency are maximized in that order. Edit cost and
/// candidate identifier break ties. Callers must filter hard-gate failures first.
#[must_use]
pub fn select_best<'a>(
    candidates: impl IntoIterator<Item = &'a GeneratedCandidate>,
) -> Option<&'a GeneratedCandidate> {
    candidates
        .into_iter()
        .max_by(|left, right| compare_candidates(left, right))
}

pub(crate) fn compare_candidates(
    left: &GeneratedCandidate,
    right: &GeneratedCandidate,
) -> Ordering {
    left.rank
        .style
        .total_cmp(&right.rank.style)
        .then_with(|| left.rank.channel.total_cmp(&right.rank.channel))
        .then_with(|| left.rank.fluency.total_cmp(&right.rank.fluency))
        .then_with(|| right.rank.edit_cost.cmp(&left.rank.edit_cost))
        .then_with(|| right.id.as_str().cmp(left.id.as_str()))
}

#[cfg(test)]
mod tests {
    use rewrite_types::{
        CandidateId, CandidateRank, CandidateTextKind, GeneratedCandidate, RewriteUnitId,
    };

    use super::select_best;

    fn candidate(ordinal: usize, rank: CandidateRank) -> GeneratedCandidate {
        let document = rewrite_types::DocumentId::from_digest(&rewrite_types::Digest::sha256(b"x"));
        let unit = RewriteUnitId::new(&document, 0);
        GeneratedCandidate {
            id: CandidateId::new(&unit, ordinal),
            unit_id: unit,
            text: String::new(),
            text_kind: CandidateTextKind::Raw,
            rank,
        }
    }

    #[test]
    fn style_outranks_every_soft_tie_breaker() {
        let lower_style = candidate(
            0,
            CandidateRank {
                style: 0.8,
                channel: 1.0,
                fluency: 1.0,
                edit_cost: 0,
            },
        );
        let higher_style = candidate(
            1,
            CandidateRank {
                style: 0.9,
                channel: 0.0,
                fluency: 0.0,
                edit_cost: 100,
            },
        );
        assert_eq!(
            select_best([&lower_style, &higher_style]).map(|item| item.id.as_str()),
            Some(higher_style.id.as_str())
        );
    }

    #[test]
    fn lower_edit_cost_breaks_equal_score() {
        let expensive = candidate(
            0,
            CandidateRank {
                edit_cost: 10,
                ..CandidateRank::default()
            },
        );
        let cheap = candidate(
            1,
            CandidateRank {
                edit_cost: 2,
                ..CandidateRank::default()
            },
        );
        assert_eq!(
            select_best([&expensive, &cheap]).map(|item| item.id.as_str()),
            Some(cheap.id.as_str())
        );
    }
}
