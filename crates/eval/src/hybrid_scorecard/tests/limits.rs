use rewrite_types::Digest;

use crate::hybrid_scorecard::{
    HybridScorecardCasePlan, HybridScorecardError, JudgeChoice, JudgeObservation,
    JudgePresentation, MAX_HYBRID_SCORECARD_BYTES, hybrid_scorecard_plan_digest,
    run_hybrid_scorecard,
};

use super::{batch, plan, suites};

fn long_clauses() -> Vec<String> {
    (0..32)
        .map(|index| format!("clause-{index:02}-{}", "a".repeat(50)))
        .collect()
}

#[test]
fn in_memory_plans_and_batches_enforce_the_serialized_byte_limit() {
    let (candidate_a, candidate_b) = suites();
    let base = plan(&candidate_a, &candidate_b);
    let mut oversized_plan = base.clone();
    oversized_plan.cases = (0..2_200)
        .map(|index| HybridScorecardCasePlan {
            id: format!("case-{index:04}"),
            cluster_id: format!("cluster-{index:04}"),
            source_digest: Digest::sha256(format!("source-{index}").as_bytes()),
            candidate_a_digest: Digest::sha256(format!("primary-{index}").as_bytes()),
            candidate_b_digest: Digest::sha256(format!("alternate-{index}").as_bytes()),
            candidate_a_system_digest: Digest::sha256(b"generator-a"),
            candidate_b_system_digest: Digest::sha256(b"generator-b"),
            rubric_clauses: long_clauses(),
        })
        .collect();
    assert!(
        serde_json::to_vec(&oversized_plan)
            .expect("serialize oversized plan")
            .len()
            > MAX_HYBRID_SCORECARD_BYTES
    );
    assert_eq!(
        hybrid_scorecard_plan_digest(&oversized_plan),
        Err(HybridScorecardError::TooLarge)
    );

    let mut oversized_batch = batch(&base);
    oversized_batch.observations = (0..2_200)
        .map(|index| JudgeObservation {
            case_id: format!("case-{index:04}"),
            presentation: if index % 2 == 0 {
                JudgePresentation::CandidateAFirst
            } else {
                JudgePresentation::CandidateBFirst
            },
            choice: JudgeChoice::Tie,
            rubric_clauses: long_clauses(),
        })
        .collect();
    assert!(
        serde_json::to_vec(&oversized_batch)
            .expect("serialize oversized batch")
            .len()
            > MAX_HYBRID_SCORECARD_BYTES
    );
    assert_eq!(
        run_hybrid_scorecard(&base, &candidate_a, &candidate_b, &oversized_batch),
        Err(HybridScorecardError::TooLarge)
    );
}
