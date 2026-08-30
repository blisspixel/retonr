use super::*;

#[test]
fn legacy_eager_preparation_preserves_frozen_request_vectors() {
    let (candidate_a, candidate_b) = suites();
    let plan = plan(&candidate_a, &candidate_b);
    let rubric = rubric();
    let clauses = rubric
        .clauses
        .iter()
        .map(|clause| (clause.id.as_str(), clause))
        .collect::<BTreeMap<_, _>>();
    let model = model_binding();
    let plan_digest = hybrid_scorecard_plan_digest(&plan).expect("plan digest");
    let attempts = prepare_attempts::<()>(
        &plan,
        &candidate_a,
        &candidate_b,
        &clauses,
        &model,
        &plan_digest,
    )
    .expect("prepared attempts");
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].presentation, JudgePresentation::CandidateBFirst);
    assert_eq!(
        attempts[0].request.sampling.seed,
        Some(16_201_717_743_242_601_372)
    );
    assert_eq!(
        attempts[0].request.binding_digest().as_str(),
        "a8b90a2574c5ff4b35e14fdf356832ad0ffa4b506468432a1374b500a5a91ecb"
    );
    assert_eq!(attempts[1].presentation, JudgePresentation::CandidateAFirst);
    assert_eq!(
        attempts[1].request.sampling.seed,
        Some(11_769_455_392_107_554_815)
    );
    assert_eq!(
        attempts[1].request.binding_digest().as_str(),
        "247806efa2f2c7f12ca5a02bfe56f7333bc9dcfd7c53bbd693c51454ee4d0fc5"
    );
}
