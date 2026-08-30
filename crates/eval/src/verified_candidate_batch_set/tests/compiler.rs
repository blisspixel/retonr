use rewrite_model::CandidateDeterministicEvaluationStatusV1;
use rewrite_types::CancellationToken;

use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::{
    CandidateDeterministicCompilerError, CandidateDeterministicCompilerRelationship,
    VerifiedCandidateBatchSet, compile_candidate_deterministic_evaluation,
};

use super::support::{OfflineBatchFailureControl, paired_scenario};
use super::{VerifiedCandidateBatchSetCore, offline_authority};

#[test]
fn offline_backed_authorities_compile_one_exact_inert_record() {
    let material_fixture = Fixture::pair();
    let material = material_fixture.verify().expect("material authority");
    let (candidate_a, candidate_b, _) = authorities(&material_fixture, "compiler");
    let record = compile_candidate_deterministic_evaluation(
        &candidate_a,
        &candidate_b,
        &material,
        &CancellationToken::new(),
    )
    .expect("compiler returns exact record");
    assert_eq!(record.total(), 4);
    assert_eq!(record.passed(), 4);
    assert_eq!(
        record.status(),
        CandidateDeterministicEvaluationStatusV1::Passed
    );
    assert_eq!(
        record.case_material_set_digest(),
        material.case_material_set_digest()
    );
    assert_eq!(
        record.suite_pair_digest().as_str(),
        "74529c109ed25aac9ac2fe35d57c34da4156a403c59dc3142396db90acd3da75"
    );
    assert_eq!(
        record.deterministic_evaluation_id().digest().as_str(),
        "ba91485bc57792149a848f0c5215a1d5bc8ee104a708608d7e404954839ed42a"
    );

    let swapped = compile_candidate_deterministic_evaluation(
        &candidate_b,
        &candidate_a,
        &material,
        &CancellationToken::new(),
    )
    .expect("swapped compiler record");
    assert_eq!(
        record.candidate_a_receipt_set_id(),
        swapped.candidate_b_receipt_set_id()
    );
    assert_eq!(
        record.candidate_b_receipt_set_id(),
        swapped.candidate_a_receipt_set_id()
    );
    assert_ne!(record.suite_pair_digest(), swapped.suite_pair_digest());
    assert_ne!(
        record.deterministic_evaluation_id(),
        swapped.deterministic_evaluation_id()
    );

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        compile_candidate_deterministic_evaluation(
            &candidate_a,
            &candidate_b,
            &material,
            &cancelled,
        ),
        Err(CandidateDeterministicCompilerError::Cancelled)
    ));
}

#[test]
fn deterministic_hard_gate_failure_is_a_valid_inert_record() {
    let material_fixture = Fixture::pair();
    let material = material_fixture.verify().expect("material authority");
    let (candidate_a, candidate_b, _) = authorities(&material_fixture, "hard-gate-failed");
    let record = compile_candidate_deterministic_evaluation(
        &candidate_a,
        &candidate_b,
        &material,
        &CancellationToken::new(),
    )
    .expect("failed deterministic result is still a record");
    assert_eq!(record.total(), 4);
    assert_eq!(record.passed(), 0);
    assert_eq!(
        record.status(),
        CandidateDeterministicEvaluationStatusV1::Failed
    );
}

#[test]
fn rejects_foreign_material_and_non_utf8_exact_source() {
    let material_fixture = Fixture::pair();
    let (candidate_a, candidate_b, _) = authorities(&material_fixture, "foreign-material");
    let foreign_fixture = Fixture::pair_with_protocol("foreign protocol");
    let foreign = foreign_fixture
        .verify()
        .expect("foreign material authority");
    assert!(matches!(
        compile_candidate_deterministic_evaluation(
            &candidate_a,
            &candidate_b,
            &foreign,
            &CancellationToken::new(),
        ),
        Err(CandidateDeterministicCompilerError::Relationship {
            relationship: CandidateDeterministicCompilerRelationship::SuiteClosure,
            ..
        })
    ));

    let non_utf8_fixture = Fixture::non_utf8();
    let non_utf8 = non_utf8_fixture
        .verify()
        .expect("non-UTF8 material authority");
    let (candidate_a, candidate_b, _) = authorities(&non_utf8_fixture, "non-utf8");
    assert!(matches!(
        compile_candidate_deterministic_evaluation(
            &candidate_a,
            &candidate_b,
            &non_utf8,
            &CancellationToken::new(),
        ),
        Err(CandidateDeterministicCompilerError::CaseProjection {
            semantic_index: 0,
            ..
        })
    ));
}

#[test]
fn final_and_dual_failures_preserve_every_validation_error() {
    let material_fixture = Fixture::pair();
    let material = material_fixture.verify().expect("material authority");
    let (candidate_a, candidate_b, control) = authorities(&material_fixture, "post-validation");
    assert_eq!(control.revalidation_calls(), 2);
    control.fail_on_call(11);
    assert!(matches!(
        compile_candidate_deterministic_evaluation(
            &candidate_a,
            &candidate_b,
            &material,
            &CancellationToken::new(),
        ),
        Err(CandidateDeterministicCompilerError::AuthorityValidation {
            candidate_a: Some(_),
            ..
        })
    ));

    let (candidate_a, candidate_b, control) = authorities(&material_fixture, "dual-failure");
    control.fail_on_call(7);
    let foreign_fixture = Fixture::pair_with_protocol("dual foreign protocol");
    let foreign = foreign_fixture
        .verify()
        .expect("foreign material authority");
    let error = compile_candidate_deterministic_evaluation(
        &candidate_a,
        &candidate_b,
        &foreign,
        &CancellationToken::new(),
    )
    .expect_err("primary and final validation must both survive");
    let CandidateDeterministicCompilerError::PrimaryAndFinalValidation {
        primary,
        final_validation,
    } = error
    else {
        panic!("expected lossless dual failure")
    };
    assert!(matches!(
        *primary,
        CandidateDeterministicCompilerError::Relationship {
            relationship: CandidateDeterministicCompilerRelationship::SuiteClosure,
            ..
        }
    ));
    assert!(matches!(
        *final_validation,
        CandidateDeterministicCompilerError::AuthorityValidation {
            candidate_a: Some(_),
            ..
        }
    ));
}

fn authorities(
    material: &Fixture,
    suffix: &str,
) -> (
    VerifiedCandidateBatchSet,
    VerifiedCandidateBatchSet,
    OfflineBatchFailureControl,
) {
    let paired = paired_scenario(&material.suite, &material.cases, suffix);
    let control = paired.candidate_a.batches[0].failure_control();
    let candidate_a = offline_authority(
        VerifiedCandidateBatchSetCore::verify(
            paired.candidate_a.input,
            paired.candidate_a.batches,
            &CancellationToken::new(),
        )
        .expect("candidate A authority"),
    );
    let candidate_b = offline_authority(
        VerifiedCandidateBatchSetCore::verify(
            paired.candidate_b.input,
            paired.candidate_b.batches,
            &CancellationToken::new(),
        )
        .expect("candidate B authority"),
    );
    (candidate_a, candidate_b, control)
}
