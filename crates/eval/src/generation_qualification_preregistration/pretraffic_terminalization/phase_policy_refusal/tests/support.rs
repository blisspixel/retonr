use rewrite_app::{
    GenerationQualificationLicenseAssessmentCompiler, GenerationQualificationPhasePolicyVerifier,
    ProductionGenerationQualificationHumanAdjudicationPolicySource,
    ProductionGenerationQualificationResourcePolicySource,
    SyntheticGenerationQualificationScenario,
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    VerifiedGenerationQualificationResourcePolicy, with_synthetic_generation_qualification_fixture,
};
use rewrite_model::{
    GenerationQualificationOperationLimitsV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationPlanV1,
    GenerationQualificationPlanV1Input, GenerationSystemRecordV1,
    generation_qualification_plan_failure_policy_digest,
};
use rewrite_model_store::{
    GenerationQualificationPlanFoundationV1Input,
    GenerationQualificationPreregistrationFoundationV1Input,
};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::{HUMAN_POLICY_DIGEST, HUMAN_POLICY_JSON, RESOURCE_POLICY_DIGEST, RESOURCE_POLICY_JSON};
use crate::{
    GenerationQualificationOperationDraft, GenerationQualificationPreregistrationRepository,
    PreparedGenerationQualificationOperation,
};

pub(super) fn with_prepared(
    elapsed_override: Option<u32>,
    use_prepared: impl for<'records, 'store, 'platform, 'proof, 'lease> FnOnce(
        PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
        VerifiedGenerationQualificationResourcePolicy,
        VerifiedGenerationQualificationHumanAdjudicationPolicy,
    ),
) {
    with_fixture(elapsed_override, |prepared, resource, human, _foreign| {
        use_prepared(prepared, resource, human);
    });
}

pub(super) fn with_prepared_and_foreign_human(
    use_prepared: impl for<'records, 'store, 'platform, 'proof, 'lease> FnOnce(
        PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
        VerifiedGenerationQualificationResourcePolicy,
        VerifiedGenerationQualificationHumanAdjudicationPolicy,
    ),
) {
    with_fixture(None, |prepared, resource, _human, foreign| {
        use_prepared(prepared, resource, foreign);
    });
}

fn with_fixture(
    elapsed_override: Option<u32>,
    use_prepared: impl for<'records, 'store, 'platform, 'proof, 'lease> FnOnce(
        PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
        VerifiedGenerationQualificationResourcePolicy,
        VerifiedGenerationQualificationHumanAdjudicationPolicy,
        VerifiedGenerationQualificationHumanAdjudicationPolicy,
    ),
) {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |mut input, platform_owners, license_proof, license_policy, production_policy| {
            bind_phase_policies(&mut input.operation_policy_input, elapsed_override);
            let original_relations = input.operation_policy_relations;
            let systems = sorted_systems(original_relations);
            let qualification_plan = GenerationQualificationPlanV1::new(
                original_relations.suite,
                original_relations.repetitions,
                &systems,
                original_relations.planned_attempts,
                plan_input(original_relations.plan, &input.operation_policy_input),
            )
            .expect("phase-policy-bound qualification plan");
            let operation_policy_relations = GenerationQualificationOperationPolicyV1Relations {
                plan: &qualification_plan,
                ..original_relations
            };
            let foundation = GenerationQualificationPreregistrationFoundationV1Input {
                runtime_artifact_sets: input.foundation.runtime_artifact_sets,
                plan_foundation: GenerationQualificationPlanFoundationV1Input {
                    plan: &qualification_plan,
                    ..input.foundation.plan_foundation
                },
            };
            let mut foreign_input = input.operation_policy_input.clone();
            let foreign_human_json = HUMAN_POLICY_JSON.replace_ascii(b"7627468237172680543", b"91");
            foreign_input.human_adjudication_policy_digest = phase_policy_digest(
                b"retonr:generation-qualification-human-adjudication-policy:v1\0",
                &foreign_human_json,
            );
            let foreign_qualification_plan = GenerationQualificationPlanV1::new(
                original_relations.suite,
                original_relations.repetitions,
                &systems,
                original_relations.planned_attempts,
                plan_input(original_relations.plan, &foreign_input),
            )
            .expect("foreign phase-policy-bound qualification plan");
            let foreign_policy = GenerationQualificationOperationPolicyV1::new(
                GenerationQualificationOperationPolicyV1Relations {
                    plan: &foreign_qualification_plan,
                    ..original_relations
                },
                foreign_input,
            )
            .expect("foreign operation policy");
            let foreign_human = verify_denied_human_policy(&foreign_human_json, &foreign_policy);
            let cancellation = CancellationToken::new();
            let draft = GenerationQualificationOperationDraft::begin(
                operation_policy_relations,
                input.operation_policy_input,
                &cancellation,
            )
            .expect("draft");
            let projected = draft
                .project(input.case_authorities, &cancellation)
                .expect("projection");
            let portable = projected.platform_portable_relations();
            let (resource, human) = verified_denied_phase_policies(portable.operation_policy);
            let platform = platform_owners
                .assess(projected.platform_portable_relations(), &cancellation)
                .expect("platform assessment");
            let license_input = projected.license_assessment_input(
                &license_policy,
                license_proof.control(),
                license_proof.selected_lease(),
                &production_policy,
            );
            let license = GenerationQualificationLicenseAssessmentCompiler::compile(
                &license_input,
                &cancellation,
            )
            .expect("license assessment");
            let directory = tempdir().expect("temporary repository directory");
            let mut repository = GenerationQualificationPreregistrationRepository::open(
                &directory.path().join("qualification.db"),
            )
            .expect("durable repository");
            let prepared = projected
                .finish(
                    &mut repository,
                    foundation,
                    platform,
                    license,
                    license_policy,
                    production_policy,
                    &cancellation,
                )
                .expect("prepared operation");
            use_prepared(prepared, resource, human, foreign_human);
        },
    );
}

fn bind_phase_policies(
    input: &mut GenerationQualificationOperationPolicyV1Input,
    elapsed_override: Option<u32>,
) {
    if let Some(maximum_elapsed_milliseconds) = elapsed_override {
        input.limits = limits_with_elapsed(input.limits, maximum_elapsed_milliseconds);
    }
    input.resource_policy_digest = digest(RESOURCE_POLICY_DIGEST);
    input.human_adjudication_policy_digest = digest(HUMAN_POLICY_DIGEST);
}

fn sorted_systems(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
) -> Vec<GenerationSystemRecordV1> {
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
    systems
}

fn verified_denied_phase_policies(
    operation_policy: &GenerationQualificationOperationPolicyV1,
) -> (
    VerifiedGenerationQualificationResourcePolicy,
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
) {
    let resource = GenerationQualificationPhasePolicyVerifier::verify_resource(
        RESOURCE_POLICY_JSON,
        operation_policy,
        &ProductionGenerationQualificationResourcePolicySource::new(),
    )
    .expect("resource policy authority");
    let human = verify_denied_human_policy(HUMAN_POLICY_JSON, operation_policy);
    (resource, human)
}

fn verify_denied_human_policy(
    policy_json: &[u8],
    operation_policy: &GenerationQualificationOperationPolicyV1,
) -> VerifiedGenerationQualificationHumanAdjudicationPolicy {
    GenerationQualificationPhasePolicyVerifier::verify_human_adjudication(
        policy_json,
        operation_policy,
        &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
    )
    .expect("human policy authority")
}

fn plan_input(
    original: &GenerationQualificationPlanV1,
    operation: &rewrite_model::GenerationQualificationOperationPolicyV1Input,
) -> GenerationQualificationPlanV1Input {
    GenerationQualificationPlanV1Input {
        limits: original.limits(),
        selection_policy_digest: original.selection_policy_digest().clone(),
        failure_policy_digest: generation_qualification_plan_failure_policy_digest(
            operation.decision_rule,
            &operation.platform_assessment_policy_id,
            operation.required_license_permission,
            &operation.license_assessment_policy_id,
            &operation.attempt_ledger_policy_digest,
            &operation.repeatability_policy_digest,
            &operation.resource_policy_digest,
            &operation.human_adjudication_policy_digest,
        ),
    }
}

fn limits_with_elapsed(
    limits: GenerationQualificationOperationLimitsV1,
    maximum_elapsed_milliseconds: u32,
) -> GenerationQualificationOperationLimitsV1 {
    GenerationQualificationOperationLimitsV1::new(
        limits.maximum_source_bytes(),
        limits.maximum_complete_input_bytes(),
        limits.maximum_context_tokens(),
        limits.maximum_output_tokens(),
        limits.maximum_output_bytes(),
        limits.maximum_candidates_per_completion(),
        limits.maximum_candidate_bytes(),
        limits.maximum_aggregate_candidate_bytes(),
        limits.maximum_predeclared_attempts(),
        limits.maximum_concurrent_attempts(),
        maximum_elapsed_milliseconds,
    )
    .expect("valid synthetic operation limits")
}

fn digest(hex: &str) -> Digest {
    Digest::from_sha256_hex(hex.to_owned()).expect("fixed digest")
}

fn phase_policy_digest(domain: &[u8], bytes: &[u8]) -> Digest {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(
        u64::try_from(bytes.len())
            .expect("bounded test policy")
            .to_be_bytes(),
    );
    hasher.update(bytes);
    digest(&format!("{:x}", hasher.finalize()))
}

trait ReplaceAscii {
    fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
}

impl ReplaceAscii for [u8] {
    fn replace_ascii(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
        let position = self
            .windows(from.len())
            .position(|window| window == from)
            .expect("fixture value");
        let mut output = Vec::with_capacity(self.len() - from.len() + to.len());
        output.extend_from_slice(&self[..position]);
        output.extend_from_slice(to);
        output.extend_from_slice(&self[position + from.len()..]);
        output
    }
}
