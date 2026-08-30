use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationPlanV1,
    GenerationQualificationPlanV1Input, generation_qualification_plan_failure_policy_digest,
};

use super::*;
use crate::{
    SyntheticGenerationQualificationDraftInput, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};

#[test]
fn public_verifiers_and_revalidation_require_exact_operation_policy_digests() {
    let resource_json = resource_policy_json_for_test(resource_limits());
    let human_json = human_policy_json_for_test(47);
    let resource_digest = resource_policy_digest_for_test(&resource_json);
    let human_digest = human_policy_digest_for_test(&human_json);

    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::BothRejected,
        |draft, _platform, _license, _license_policy, _production_license| {
            let exact = operation_policy(&draft, resource_digest.clone(), human_digest.clone());
            let resource = GenerationQualificationPhasePolicyVerifier::verify_resource(
                &resource_json,
                &exact,
                &ProductionGenerationQualificationResourcePolicySource::new(),
            )
            .expect("exact resource operation binding");
            let human = GenerationQualificationPhasePolicyVerifier::verify_human_adjudication(
                &human_json,
                &exact,
                &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
            )
            .expect("exact human operation binding");
            resource
                .revalidate_operation_policy(&exact)
                .expect("resource revalidation");
            human
                .revalidate_operation_policy(&exact)
                .expect("human revalidation");

            let substituted =
                operation_policy(&draft, human_digest.clone(), resource_digest.clone());
            assert_eq!(
                resource.revalidate_operation_policy(&substituted),
                Err(GenerationQualificationPhasePolicyError::OperationPolicyMismatch)
            );
            assert_eq!(
                human.revalidate_operation_policy(&substituted),
                Err(GenerationQualificationPhasePolicyError::OperationPolicyMismatch)
            );
            assert!(matches!(
                GenerationQualificationPhasePolicyVerifier::verify_resource(
                    &resource_json,
                    &substituted,
                    &ProductionGenerationQualificationResourcePolicySource::new(),
                ),
                Err(GenerationQualificationPhasePolicyError::OperationPolicyMismatch)
            ));
            assert!(matches!(
                GenerationQualificationPhasePolicyVerifier::verify_human_adjudication(
                    &human_json,
                    &substituted,
                    &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
                ),
                Err(GenerationQualificationPhasePolicyError::OperationPolicyMismatch)
            ));
        },
    );
}

fn operation_policy(
    draft: &SyntheticGenerationQualificationDraftInput<'_, '_>,
    resource_policy_digest: Digest,
    human_adjudication_policy_digest: Digest,
) -> GenerationQualificationOperationPolicyV1 {
    let relations = draft.operation_policy_relations;
    let mut input = draft.operation_policy_input.clone();
    input.resource_policy_digest = resource_policy_digest;
    input.human_adjudication_policy_digest = human_adjudication_policy_digest;
    let mut systems = vec![
        relations.target_system.generation_system.clone(),
        relations.baseline_system.generation_system.clone(),
    ];
    systems.sort_unstable_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    let plan = GenerationQualificationPlanV1::new(
        relations.suite,
        relations.repetitions,
        &systems,
        relations.planned_attempts,
        GenerationQualificationPlanV1Input {
            limits: relations.plan.limits(),
            selection_policy_digest: relations.plan.selection_policy_digest().clone(),
            failure_policy_digest: failure_policy_digest(&input),
        },
    )
    .expect("phase-policy test plan");
    GenerationQualificationOperationPolicyV1::new(
        GenerationQualificationOperationPolicyV1Relations {
            suite: relations.suite,
            plan: &plan,
            repetitions: relations.repetitions,
            planned_attempts: relations.planned_attempts,
            target_system: relations.target_system,
            baseline_system: relations.baseline_system,
        },
        input,
    )
    .expect("phase-policy test operation")
}

fn failure_policy_digest(input: &GenerationQualificationOperationPolicyV1Input) -> Digest {
    generation_qualification_plan_failure_policy_digest(
        input.decision_rule,
        &input.platform_assessment_policy_id,
        input.required_license_permission,
        &input.license_assessment_policy_id,
        &input.attempt_ledger_policy_digest,
        &input.repeatability_policy_digest,
        &input.resource_policy_digest,
        &input.human_adjudication_policy_digest,
    )
}
