#[path = "tests/support.rs"]
mod support;

use rewrite_model::{
    GenerationQualificationLicenseDecisionV1, GenerationQualificationLicenseReasonV1,
    ModelLicenseControlId, StructuredCompletionRequestBindingId,
};
use rewrite_types::{CancellationToken, Digest};

use super::*;
use crate::candidate_attempt_precursor::tests::support::{Fixture, changed_state};
use crate::{ModelLicensePermission, ProductionModelLicenseApprovalPolicy};
use support::{StaticFixture, assessment_policy, compiler_input, structural_control};

#[test]
fn exact_dual_approval_derives_positive_inert_evidence() {
    let live = Fixture::new();
    let proof = structural_control(&live.model_lease, ModelLicensePermission::LocalGeneration);
    let policy = assessment_policy(proof.control_id(), true);
    let fixture = StaticFixture::new(&live, policy, "positive");
    let production = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        proof.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );
    let input = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    let authority = GenerationQualificationLicenseAssessmentCompiler::compile(
        &input,
        &CancellationToken::new(),
    )
    .expect("positive assessment");

    assert_eq!(
        authority.portable_evidence().decision(),
        GenerationQualificationLicenseDecisionV1::LocalUseOnly
    );
    assert_eq!(
        authority.portable_evidence().reason(),
        GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration
    );
    authority
        .revalidate_live_proof(&CancellationToken::new())
        .expect("fresh live proof");
    authority
        .revalidate_against(&input, &CancellationToken::new())
        .expect("complete assessment closure revalidates");
    authority
        .revalidate_portable(input.portable_relations(), &CancellationToken::new())
        .expect("portable closure revalidates without exposing live owners");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        authority.revalidate_against(&input, &cancellation),
        Err(GenerationQualificationLicenseAssessmentError::Cancelled)
    ));
}

#[test]
fn either_policy_denial_derives_the_same_deterministic_rejection() {
    for assessment_approved in [false, true] {
        let live = Fixture::new();
        let proof = structural_control(&live.model_lease, ModelLicensePermission::LocalGeneration);
        let policy = assessment_policy(proof.control_id(), assessment_approved);
        let fixture = StaticFixture::new(&live, policy, "denied");
        let production = if assessment_approved {
            ProductionModelLicenseApprovalPolicy::new()
        } else {
            ProductionModelLicenseApprovalPolicy::exact_test_policy(
                proof.control_id().clone(),
                ModelLicensePermission::LocalGeneration,
            )
        };
        let authority = GenerationQualificationLicenseAssessmentCompiler::compile(
            &compiler_input(&fixture, &live, &proof, &live.model_lease, &production),
            &CancellationToken::new(),
        )
        .expect("negative evidence remains valid");

        assert_eq!(
            authority.portable_evidence().decision(),
            GenerationQualificationLicenseDecisionV1::Rejected
        );
        assert_eq!(
            authority.portable_evidence().reason(),
            GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied
        );
    }
}

#[test]
fn policy_target_projection_and_relation_substitutions_fail_closed() {
    let live = Fixture::new();
    let proof = structural_control(&live.model_lease, ModelLicensePermission::LocalGeneration);
    let policy = assessment_policy(proof.control_id(), true);
    let fixture = StaticFixture::new(&live, policy, "primary");
    let other_policy = assessment_policy(proof.control_id(), true);
    let other = StaticFixture::new(&live, other_policy, "foreign");
    let production = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        proof.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );

    let mut target = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    target.target_generation_system = fixture.baseline();
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &target,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::Relationship(
            GenerationQualificationLicenseAssessmentRelationship::OperationTarget
        ))
    ));

    let changed_runtime_state = changed_state(&live.runtime);
    let mut relations = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    relations
        .target_generation_system_relations
        .effective_runtime_state = &changed_runtime_state;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &relations,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::TargetSystem(
            _
        ))
    ));

    let mut projection = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    projection.request_projection = &other.projection;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &projection,
            &CancellationToken::new()
        ),
        Err(
            GenerationQualificationLicenseAssessmentError::RequestProjection(_)
                | GenerationQualificationLicenseAssessmentError::Relationship(
                    GenerationQualificationLicenseAssessmentRelationship::ProjectionOperation
                )
        )
    ));

    let mut foreign_policy_input = fixture.operation_policy_input.clone();
    foreign_policy_input.resource_policy_digest = Digest::sha256(b"foreign resource policy");
    let mut policy_input = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    policy_input.operation_policy_input = &foreign_policy_input;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &policy_input,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::OperationPolicy(_))
    ));

    let mut foreign_projection_inputs = fixture.projection_inputs.clone();
    foreign_projection_inputs[0].structured_completion_request_binding_id =
        StructuredCompletionRequestBindingId::from_derived_digest(Digest::sha256(
            b"foreign structured request",
        ));
    let mut projection_input =
        compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    projection_input.request_projection_entry_inputs = &foreign_projection_inputs;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &projection_input,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::RequestProjection(_))
    ));
}

#[test]
fn assessment_control_permission_and_live_lease_substitutions_fail_closed() {
    let live = Fixture::new();
    let other_live = Fixture::new();
    let proof = structural_control(&live.model_lease, ModelLicensePermission::LocalGeneration);
    let policy = assessment_policy(proof.control_id(), true);
    let fixture = StaticFixture::new(&live, policy, "substitution");
    let production = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        proof.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );

    let mut lease = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    lease.selected_model_package_lease = &other_live.model_lease;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &lease,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::ModelLicenseControl(_))
    ));

    let foreign_package = Fixture::new_model_variant(true);
    let mut package = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    package.selected_model_package_lease = &foreign_package.model_lease;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &package,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::Relationship(
            GenerationQualificationLicenseAssessmentRelationship::ModelArtifactSet
                | GenerationQualificationLicenseAssessmentRelationship::ModelPackage
        ))
    ));

    let foreign_control = ModelLicenseControlId::from_derived_digest(Digest::sha256(b"foreign"));
    let foreign_policy = assessment_policy(&foreign_control, true);
    let mut assessment = compiler_input(&fixture, &live, &proof, &live.model_lease, &production);
    assessment.assessment_policy = &foreign_policy;
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &assessment,
            &CancellationToken::new()
        ),
        Err(GenerationQualificationLicenseAssessmentError::Relationship(
            GenerationQualificationLicenseAssessmentRelationship::AssessmentPolicy
        ))
    ));

    let foreign_static = StaticFixture::new(&live, foreign_policy, "foreign control");
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &compiler_input(
                &foreign_static,
                &live,
                &proof,
                &live.model_lease,
                &production,
            ),
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationLicenseAssessmentError::Relationship(
            GenerationQualificationLicenseAssessmentRelationship::ModelLicenseControl
        ))
    ));

    let redistribution = structural_control(
        &live.model_lease,
        ModelLicensePermission::PackageRedistribution,
    );
    let redistribution_policy = assessment_policy(redistribution.control_id(), true);
    let redistribution_fixture = StaticFixture::new(&live, redistribution_policy, "redistribution");
    let redistribution_root = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        redistribution.control_id().clone(),
        ModelLicensePermission::PackageRedistribution,
    );
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &compiler_input(
                &redistribution_fixture,
                &live,
                &redistribution,
                &live.model_lease,
                &redistribution_root,
            ),
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationLicenseAssessmentError::Relationship(
            GenerationQualificationLicenseAssessmentRelationship::Permission
        ))
    ));
}

#[test]
fn cancellation_and_fresh_post_derivation_drift_discard_evidence() {
    let live = Fixture::new();
    let proof = structural_control(&live.model_lease, ModelLicensePermission::LocalGeneration);
    let policy = assessment_policy(proof.control_id(), true);
    let fixture = StaticFixture::new(&live, policy, "drift");
    let production = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        proof.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        GenerationQualificationLicenseAssessmentCompiler::compile(
            &compiler_input(&fixture, &live, &proof, &live.model_lease, &production),
            &cancelled,
        ),
        Err(GenerationQualificationLicenseAssessmentError::Cancelled)
    ));

    let final_cancellation = CancellationToken::new();
    assert!(matches!(
        compile_with_post_derivation(
            &compiler_input(&fixture, &live, &proof, &live.model_lease, &production),
            &final_cancellation,
            || final_cancellation.cancel(),
        ),
        Err(GenerationQualificationLicenseAssessmentError::Cancelled)
    ));

    let drift_live = Fixture::new();
    let drift_proof = structural_control(
        &drift_live.model_lease,
        ModelLicensePermission::LocalGeneration,
    );
    let drift_policy = assessment_policy(drift_proof.control_id(), true);
    let drift_fixture = StaticFixture::new(&drift_live, drift_policy, "fresh drift");
    let drift_production = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        drift_proof.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );
    assert!(matches!(
        compile_with_post_derivation(
            &compiler_input(
                &drift_fixture,
                &drift_live,
                &drift_proof,
                &drift_live.model_lease,
                &drift_production,
            ),
            &CancellationToken::new(),
            || drift_live.add_model_member(),
        ),
        Err(GenerationQualificationLicenseAssessmentError::ModelLicenseControl(_))
    ));
}

#[test]
fn debug_surface_is_redacted() {
    let live = Fixture::new();
    let proof = structural_control(&live.model_lease, ModelLicensePermission::LocalGeneration);
    let policy = assessment_policy(proof.control_id(), false);
    let fixture = StaticFixture::new(&live, policy, "debug");
    let production = ProductionModelLicenseApprovalPolicy::new();
    let authority = GenerationQualificationLicenseAssessmentCompiler::compile(
        &compiler_input(&fixture, &live, &proof, &live.model_lease, &production),
        &CancellationToken::new(),
    )
    .expect("negative assessment");
    let debug = format!("{authority:?}");

    assert!(debug.contains("Rejected"));
    assert!(!debug.contains(proof.control_id().digest().as_str()));
    assert!(!debug.contains(live.model_lease.foundation_id().digest().as_str()));
    assert!(
        !debug.contains(
            fixture
                .operation_policy
                .operation_policy_id()
                .digest()
                .as_str()
        )
    );
}
