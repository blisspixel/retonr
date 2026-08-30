use rewrite_types::{CancellationToken, Digest};

use super::*;
use crate::candidate_judge_preparation::tests::{prepare, ready_config};
use crate::generation_case_material::VerifiedGenerationCaseMaterialError;
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::{CandidateJudgePreparationOutcome, HybridScorecardError};

#[test]
fn reversed_semantic_subset_has_checked_lexicographic_projection() {
    let material = Fixture::judge_triple_reversed_eligible_order();
    let prepared = prepare(
        &material,
        "projection-reversed",
        ready_config(vec![0, 2]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("passed reversed subset must prepare")
    };
    let plan = &ready.compatibility_projection.plan;
    assert_eq!(
        plan.cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>(),
        ["eligible-rewrite", "eligible-zeta"]
    );
    assert_eq!(
        ready.compatibility_projection.semantic_to_lexicographic,
        [1, 0]
    );
    assert_eq!(
        plan.cases[0].candidate_a_system_digest,
        *ready
            .judge_plan()
            .candidate_a_generation_system_id()
            .digest()
    );
    assert_eq!(
        plan.cases[0].candidate_b_system_digest,
        *ready
            .judge_plan()
            .candidate_b_generation_system_id()
            .digest()
    );
    assert_eq!(
        crate::hybrid_scorecard_plan_digest(plan).expect("legacy-compatible plan digest"),
        ready.compatibility_projection.plan_digest
    );
    assert_eq!(
        ready.compatibility_projection.plan_digest.as_str(),
        "cdd9b2be10482dbb4158b68e3af904269a519ffadf0c48217f50b82cfa54a124"
    );
    let json = serde_json::to_string(plan).expect("content-free plan JSON");
    for raw in [
        "Zeta 7 needs polish.",
        "Acme 42 needs polish.",
        "Zeta, 7 needs polish!",
        "Acme, 42 needs polish!",
    ] {
        assert!(!json.contains(raw));
    }
}

#[test]
fn equal_candidate_bytes_use_only_the_exact_private_projection_path() {
    let material = Fixture::judge_pair();
    let prepared = prepare(
        &material,
        "equal-selected",
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("equal exact candidates must remain judgeable")
    };
    let plan = &ready.compatibility_projection.plan;
    assert_eq!(
        plan.cases[0].candidate_a_digest,
        plan.cases[0].candidate_b_digest
    );
    assert!(matches!(
        crate::hybrid_scorecard_plan_digest(plan),
        Err(HybridScorecardError::InvalidCase { index: 0 })
    ));
    assert_eq!(
        ready.compatibility_projection.semantic_to_lexicographic,
        [0]
    );
    assert_eq!(
        ready.compatibility_projection.plan_digest.as_str(),
        "56427b1f3a24767698371419e3ef56d9c9cf98b7708d45b12d60340c868a7482"
    );
}

#[test]
fn projection_failure_mapping_is_content_free_and_lossless() {
    assert!(matches!(
        compatibility_error(),
        CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::CompatibilityProjection
        )
    ));
    let material = || VerifiedGenerationCaseMaterialError::Cancelled;
    let callback = || compatibility_error();
    let errors = [
        map_traversal_error(VerifiedGenerationCaseMaterialTraversalError::Material {
            source: material(),
        }),
        map_traversal_error(
            VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
                primary: material(),
                final_validation: Box::new(material()),
            },
        ),
        map_traversal_error(VerifiedGenerationCaseMaterialTraversalError::Callback {
            semantic_index: 0,
            source: callback(),
        }),
        map_traversal_error(
            VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
                semantic_index: 0,
                source: callback(),
                final_validation: Box::new(material()),
            },
        ),
        map_traversal_error(
            VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
                semantic_index: 0,
                source: callback(),
                source_validation: Box::new(material()),
                final_validation: None,
            },
        ),
        map_traversal_error(
            VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
                semantic_index: 0,
                source: callback(),
                source_validation: Box::new(material()),
                final_validation: Some(Box::new(material())),
            },
        ),
    ];
    for error in errors {
        let debug = format!("{error:?}");
        assert!(debug.contains("kind"));
        assert!(!debug.contains(Digest::sha256(b"raw candidate").as_str()));
    }

    let active = CancellationToken::new();
    assert_eq!(
        candidate_digest(b"candidate", &active).expect("candidate digest"),
        Digest::sha256(b"candidate")
    );
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        candidate_digest(b"candidate", &cancelled),
        Err(CandidateJudgePreparationError::AuthorityValidation {
            cancelled: true,
            ..
        })
    ));
}
