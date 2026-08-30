use rewrite_model::ArtifactId;
use rewrite_types::{Digest, ReasonCode, RewriteStatus};

use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationReport, ExpectedOutput,
    GenerationDeterministicCaseContractV1, GenerationDeterministicCaseContractV1Input,
    ReferenceJudgment, TransformationCoverage,
};

use super::identity::{derive_semantic_to_lexicographic_permutation, derive_suite_pair_digest};
use super::projection::projected_suite_size;
use super::report::{summarize, validate_aggregate};
use super::*;

#[test]
fn compiler_error_debug_is_redacted() {
    let error = CandidateDeterministicCompilerError::Relationship {
        semantic_index: 7,
        relationship: CandidateDeterministicCompilerRelationship::CaseClosure,
    };
    let debug = format!("{error:?}");
    assert!(debug.contains("CaseClosure"));
    assert!(debug.contains("semantic_index: 7"));
}

#[test]
fn semantic_permutation_is_a_complete_checked_bijection() {
    let projected = vec![
        (0, evaluation_case("zeta"), evaluation_case("zeta")),
        (1, evaluation_case("alpha"), evaluation_case("alpha")),
        (2, evaluation_case("middle"), evaluation_case("middle")),
    ];
    assert_eq!(
        derive_semantic_to_lexicographic_permutation(&projected).expect("permutation"),
        [2, 0, 1]
    );

    let duplicate = vec![
        (0, evaluation_case("same"), evaluation_case("same")),
        (1, evaluation_case("same"), evaluation_case("same")),
    ];
    assert!(matches!(
        derive_semantic_to_lexicographic_permutation(&duplicate),
        Err(CandidateDeterministicCompilerError::Relationship {
            relationship: CandidateDeterministicCompilerRelationship::CasePermutation,
            ..
        })
    ));
    let substituted = vec![(0, evaluation_case("alpha"), evaluation_case("other"))];
    assert!(derive_semantic_to_lexicographic_permutation(&substituted).is_err());
    let duplicate_semantic = vec![
        (0, evaluation_case("alpha"), evaluation_case("alpha")),
        (0, evaluation_case("beta"), evaluation_case("beta")),
    ];
    assert!(derive_semantic_to_lexicographic_permutation(&duplicate_semantic).is_err());
    let out_of_range = vec![(1, evaluation_case("alpha"), evaluation_case("alpha"))];
    assert!(derive_semantic_to_lexicographic_permutation(&out_of_range).is_err());
}

#[test]
fn suite_pair_digest_framing_and_candidate_order_are_frozen() {
    let suite = Digest::sha256(b"suite");
    let material = Digest::sha256(b"material");
    let digest = derive_suite_pair_digest(&suite, &material, &[2, 0, 1], b"{}", b"[]")
        .expect("suite-pair digest");
    assert_eq!(
        digest.as_str(),
        "c7957b5bd4fc365659e435ad709587edb5838b21c83a12c6b372489569b26f55"
    );
    assert_ne!(
        digest,
        derive_suite_pair_digest(&suite, &material, &[2, 0, 1], b"[]", b"{}")
            .expect("reversed suite-pair digest")
    );
    assert_ne!(
        digest,
        derive_suite_pair_digest(&suite, &material, &[0, 1, 2], b"{}", b"[]")
            .expect("changed permutation digest")
    );
}

#[test]
fn portable_projection_bound_accepts_exact_limit_and_rejects_next_byte() {
    let contract = case_contract("bound", b"x");
    let base = projected_suite_size(std::slice::from_ref(&contract), 0)
        .expect("portable projected base size");
    let remaining = MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES - base;
    assert_eq!(
        projected_suite_size(std::slice::from_ref(&contract), remaining),
        Some(MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES)
    );
    assert_eq!(
        projected_suite_size(std::slice::from_ref(&contract), remaining + 1),
        Some(MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES + 1)
    );
    assert_eq!(projected_suite_size(&[], usize::MAX), None);
}

#[test]
fn report_summary_rejects_usize_values_outside_portable_u32() {
    let Ok(too_large) = usize::try_from(u64::from(u32::MAX) + 1) else {
        return;
    };
    let report = EvaluationReport {
        schema_version: EVALUATION_SCHEMA_VERSION,
        total: too_large,
        passed: 0,
        categories: Vec::new(),
        transformation_coverage: TransformationCoverage {
            acceptable: 0,
            rewritten: 0,
        },
        failures: Vec::new(),
    };
    assert!(matches!(
        summarize(&report),
        Err(CandidateDeterministicCompilerError::ReportCountOverflow)
    ));
}

#[test]
fn aggregate_closure_rejects_a_kernel_summary_disagreement() {
    let coverage =
        rewrite_model::CandidateDeterministicTransformationCoverageV1::new(1, 1).expect("coverage");
    let summary =
        rewrite_model::CandidateDeterministicReportSummaryV1::new(1, 1, coverage).expect("summary");
    let aggregate = crate::hybrid_scorecard::deterministic::DeterministicReportPairAggregate {
        total: 2,
        passed: 1,
        transformation_coverage: TransformationCoverage {
            acceptable: 2,
            rewritten: 2,
        },
        success: false,
    };
    assert!(matches!(
        validate_aggregate(&aggregate, summary, summary),
        Err(CandidateDeterministicCompilerError::Relationship {
            relationship: CandidateDeterministicCompilerRelationship::ReportClosure,
            ..
        })
    ));
}

#[test]
fn every_compiler_error_debug_variant_is_content_free() {
    let errors = vec![
        CandidateDeterministicCompilerError::Cancelled,
        CandidateDeterministicCompilerError::CandidateBatchSet {
            side: CandidateDeterministicCompilerSide::CandidateB,
            source: VerifiedCandidateBatchSetError::Cancelled,
        },
        CandidateDeterministicCompilerError::AuthorityValidation {
            candidate_a: Some(Box::new(VerifiedCandidateBatchSetError::Cancelled)),
            candidate_b: None,
            case_material: Some(Box::new(VerifiedGenerationCaseMaterialError::Cancelled)),
            cancelled: true,
        },
        CandidateDeterministicCompilerError::CaseMaterial {
            source: VerifiedGenerationCaseMaterialError::Cancelled,
        },
        CandidateDeterministicCompilerError::ProjectedSuiteTooLarge,
        CandidateDeterministicCompilerError::CaseProjection {
            semantic_index: 3,
            source: GenerationDeterministicCaseContractError::SourceNotUtf8,
        },
        CandidateDeterministicCompilerError::CompatibilityKernel {
            source: HybridScorecardError::InvalidDeterministicSuites,
        },
        CandidateDeterministicCompilerError::ReportCountOverflow,
        CandidateDeterministicCompilerError::PortableContract {
            source: GenerationQualificationContractError::EncodingOverflow,
        },
        CandidateDeterministicCompilerError::PrimaryAndFinalValidation {
            primary: Box::new(CandidateDeterministicCompilerError::Cancelled),
            final_validation: Box::new(CandidateDeterministicCompilerError::Cancelled),
        },
    ];
    for error in errors {
        let debug = format!("{error:?}");
        assert!(debug.contains("kind"));
        assert!(!debug.contains("source bytes"));
    }
}

#[test]
fn validation_and_error_translation_helpers_cover_closed_variants() {
    assert!(authority_validation_result(None, None, None, false).is_ok());
    assert!(matches!(
        authority_validation_result(None, None, None, true),
        Err(CandidateDeterministicCompilerError::AuthorityValidation {
            cancelled: true,
            ..
        })
    ));
    assert!(matches!(
        batch_error(
            CandidateDeterministicCompilerSide::CandidateA,
            VerifiedCandidateBatchSetError::Cancelled,
        ),
        CandidateDeterministicCompilerError::CandidateBatchSet {
            side: CandidateDeterministicCompilerSide::CandidateA,
            ..
        }
    ));
    assert!(matches!(
        kernel_error(HybridScorecardError::InvalidDeterministicReport),
        CandidateDeterministicCompilerError::CompatibilityKernel { .. }
    ));
    assert!(matches!(
        portable_error(GenerationQualificationContractError::EncodingOverflow),
        CandidateDeterministicCompilerError::PortableContract { .. }
    ));
}

fn evaluation_case(id: &str) -> EvaluationCase {
    EvaluationCase {
        id: id.to_owned(),
        category: "fixture".to_owned(),
        source: "source".to_owned(),
        candidate: "candidate".to_owned(),
        protected_terms: vec!["term".to_owned()],
        reference_judgment: ReferenceJudgment::Unacceptable,
        expected_status: RewriteStatus::Abstained,
        expected_reason: Some(ReasonCode::ProtectedValueChanged),
        expected_output: ExpectedOutput::Source,
    }
}

fn case_contract(key: &str, source: &[u8]) -> GenerationDeterministicCaseContractV1 {
    let source_digest = Digest::sha256(source);
    GenerationDeterministicCaseContractV1::new(GenerationDeterministicCaseContractV1Input {
        case_key: key.to_owned(),
        source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
        source_digest,
        source_byte_count: source.len() as u64,
        language_digest: Digest::sha256(b"language"),
        mode_digest: Digest::sha256(b"mode"),
        format_digest: Digest::sha256(b"format"),
        evaluation_category: "fixture".to_owned(),
        protected_terms: vec!["term".to_owned()],
        reference_judgment: ReferenceJudgment::Unacceptable,
        expected_status: RewriteStatus::Abstained,
        expected_reason: Some(ReasonCode::ProtectedValueChanged),
        expected_output: ExpectedOutput::Source,
        rubric_clause_ids: vec!["fidelity".to_owned()],
    })
    .expect("case contract")
}
