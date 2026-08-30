use rewrite_model::{
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations,
};

use super::support::*;
use super::*;

#[test]
fn denied_compilers_derive_exact_one_item_failed_manifests_and_revalidate() {
    with_fixture(|fixture| {
        let resource = GenerationQualificationResourcePolicyDeniedCompiler::compile(
            &fixture.operation,
            fixture.scope(),
            fixture.resource_policy(false),
        )
        .expect("resource denial authority");
        let human = GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
            &fixture.operation,
            fixture.scope(),
            fixture.human_policy(false),
        )
        .expect("human denial authority");

        assert_eq!(resource.manifest().evidence_item_count(), 1);
        assert_eq!(human.manifest().evidence_item_count(), 1);
        assert_eq!(
            resource.manifest().status(),
            GenerationQualificationPhaseStatusV1::Failed
        );
        assert_eq!(
            human.manifest().status(),
            GenerationQualificationPhaseStatusV1::Failed
        );
        assert_eq!(
            resource.policy_digest(),
            fixture.operation.resource_policy_digest()
        );
        assert_eq!(
            human.policy_digest(),
            fixture.operation.human_adjudication_policy_digest()
        );
        assert_eq!(
            resource.denial_record().generation_system_id(),
            fixture.operation.target_generation_system_id()
        );
        assert_eq!(
            human.denial_record().generation_qualification_plan_id(),
            fixture.operation.generation_qualification_plan_id()
        );
        assert_eq!(
            resource.denial_record().suite_manifest_id(),
            fixture.operation.suite_manifest_id()
        );

        let resource_digest = resource
            .denial_record()
            .resource_policy_denial_record_id()
            .digest()
            .clone();
        let exact_resource = GenerationResourceEvidenceManifestV1::new(
            GenerationResourceEvidenceManifestV1Relations {
                scope: fixture.scope(),
                phase_policy_digest: fixture.operation.resource_policy_digest(),
                evidence_record_digests: std::slice::from_ref(&resource_digest),
                status: GenerationQualificationPhaseStatusV1::Failed,
            },
        )
        .expect("exact resource manifest");
        assert_eq!(resource.manifest(), &exact_resource);

        let human_digest = human
            .denial_record()
            .human_adjudication_policy_denial_record_id()
            .digest()
            .clone();
        let exact_human = GenerationHumanAdjudicationEvidenceManifestV1::new(
            GenerationHumanAdjudicationEvidenceManifestV1Relations {
                scope: fixture.scope(),
                phase_policy_digest: fixture.operation.human_adjudication_policy_digest(),
                evidence_record_digests: std::slice::from_ref(&human_digest),
                status: GenerationQualificationPhaseStatusV1::Failed,
            },
        )
        .expect("exact human manifest");
        assert_eq!(human.manifest(), &exact_human);

        resource.revalidate().expect("fresh resource closure");
        human.revalidate().expect("fresh human closure");
    });
}

#[test]
fn approved_policies_are_refused_without_inert_evidence() {
    with_fixture(|fixture| {
        assert!(matches!(
            GenerationQualificationResourcePolicyDeniedCompiler::compile(
                &fixture.operation,
                fixture.scope(),
                fixture.resource_policy(true),
            ),
            Err(GenerationQualificationPhaseDenialError::PolicyNotDenied)
        ));
        assert!(matches!(
            GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
                &fixture.operation,
                fixture.scope(),
                fixture.human_policy(true),
            ),
            Err(GenerationQualificationPhaseDenialError::PolicyNotDenied)
        ));
    });
}

#[test]
fn every_scope_component_and_alternate_phase_policy_is_refused() {
    with_fixture(|fixture| {
        let target_scope = GenerationQualificationPhaseScopeV1 {
            generation_system: fixture.relations.baseline_system.generation_system,
            qualification_plan: &fixture.plan,
            suite: fixture.relations.suite,
        };
        assert_scope_error(
            GenerationQualificationResourcePolicyDeniedCompiler::compile(
                &fixture.operation,
                target_scope,
                fixture.resource_policy(false),
            ),
        );

        let plan_scope = GenerationQualificationPhaseScopeV1 {
            generation_system: fixture.relations.target_system.generation_system,
            qualification_plan: fixture.relations.plan,
            suite: fixture.relations.suite,
        };
        assert_scope_error(
            GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
                &fixture.operation,
                plan_scope,
                fixture.human_policy(false),
            ),
        );

        let suite_scope = GenerationQualificationPhaseScopeV1 {
            generation_system: fixture.relations.target_system.generation_system,
            qualification_plan: &fixture.plan,
            suite: &fixture.foreign_suite,
        };
        assert_scope_error(
            GenerationQualificationResourcePolicyDeniedCompiler::compile(
                &fixture.operation,
                suite_scope,
                fixture.resource_policy(false),
            ),
        );

        assert_policy_error(
            GenerationQualificationResourcePolicyDeniedCompiler::compile(
                &fixture.operation,
                fixture.scope(),
                fixture.alternate_resource_policy(),
            ),
        );
        assert_policy_error(
            GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
                &fixture.operation,
                fixture.scope(),
                fixture.alternate_human_policy(),
            ),
        );
    });
}

#[test]
fn substituted_operation_and_cross_phase_records_are_refused() {
    with_fixture(|fixture| {
        let (substituted_plan, substituted) = fixture.swapped_operation();
        let substituted_scope = GenerationQualificationPhaseScopeV1 {
            generation_system: fixture.relations.target_system.generation_system,
            qualification_plan: &substituted_plan,
            suite: fixture.relations.suite,
        };
        assert_policy_error(
            GenerationQualificationResourcePolicyDeniedCompiler::compile(
                &substituted,
                substituted_scope,
                fixture.resource_policy(false),
            ),
        );
        assert_policy_error(
            GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
                &substituted,
                substituted_scope,
                fixture.human_policy(false),
            ),
        );

        let resource = GenerationQualificationResourcePolicyDeniedCompiler::compile(
            &fixture.operation,
            fixture.scope(),
            fixture.resource_policy(false),
        )
        .expect("resource authority");
        let human = GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
            &fixture.operation,
            fixture.scope(),
            fixture.human_policy(false),
        )
        .expect("human authority");
        let human_record_digest = human
            .denial_record()
            .human_adjudication_policy_denial_record_id()
            .digest()
            .clone();
        let cross_resource = GenerationResourceEvidenceManifestV1::new(
            GenerationResourceEvidenceManifestV1Relations {
                scope: fixture.scope(),
                phase_policy_digest: fixture.operation.resource_policy_digest(),
                evidence_record_digests: std::slice::from_ref(&human_record_digest),
                status: GenerationQualificationPhaseStatusV1::Failed,
            },
        )
        .expect("structurally inert cross-phase manifest");
        assert_ne!(resource.manifest(), &cross_resource);
        assert_ne!(
            resource
                .denial_record()
                .resource_policy_denial_record_id()
                .digest(),
            human
                .denial_record()
                .human_adjudication_policy_denial_record_id()
                .digest()
        );
    });
}

#[test]
fn authority_debug_is_content_redacted() {
    with_fixture(|fixture| {
        let operation_digest = fixture.operation.operation_policy_id().digest().as_str();
        let target_digest = fixture
            .operation
            .target_generation_system_id()
            .digest()
            .as_str();
        let resource_digest = fixture.operation.resource_policy_digest().as_str();
        let resource = GenerationQualificationResourcePolicyDeniedCompiler::compile(
            &fixture.operation,
            fixture.scope(),
            fixture.resource_policy(false),
        )
        .expect("resource authority");
        let human = GenerationQualificationHumanAdjudicationPolicyDeniedCompiler::compile(
            &fixture.operation,
            fixture.scope(),
            fixture.human_policy(false),
        )
        .expect("human authority");
        for debug in [format!("{resource:?}"), format!("{human:?}")] {
            for secret in [operation_digest, target_digest, resource_digest] {
                assert!(!debug.contains(secret));
            }
            assert!(debug.contains("Failed"));
        }
    });
}

fn assert_scope_error<T>(result: Result<T, GenerationQualificationPhaseDenialError>) {
    assert!(matches!(
        result.err(),
        Some(GenerationQualificationPhaseDenialError::ScopeMismatch)
    ));
}

fn assert_policy_error<T>(result: Result<T, GenerationQualificationPhaseDenialError>) {
    assert!(matches!(
        result.err(),
        Some(GenerationQualificationPhaseDenialError::Policy(
            GenerationQualificationPhasePolicyError::OperationPolicyMismatch
        ))
    ));
}
