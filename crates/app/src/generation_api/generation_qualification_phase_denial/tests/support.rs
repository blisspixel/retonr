use rewrite_model::{
    GenerationCaseManifestV1, GenerationCaseManifestV1Input,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input, GenerationSuiteManifestV1,
    generation_qualification_plan_failure_policy_digest,
};
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::{
    GenerationQualificationPhasePolicyVerifier,
    ProductionGenerationQualificationHumanAdjudicationPolicySource,
    ProductionGenerationQualificationResourcePolicySource,
    SyntheticGenerationQualificationDraftInput, SyntheticGenerationQualificationScenario,
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    VerifiedGenerationQualificationResourcePolicy, with_synthetic_generation_qualification_fixture,
};

const RESOURCE_DOMAIN: &[u8] = b"retonr:generation-qualification-resource-policy:v1\0";
const HUMAN_DOMAIN: &[u8] = b"retonr:generation-qualification-human-adjudication-policy:v1\0";

pub(crate) struct Fixture<'records> {
    pub(super) relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    operation_input: GenerationQualificationOperationPolicyV1Input,
    pub(super) plan: GenerationQualificationPlanV1,
    pub(crate) operation: GenerationQualificationOperationPolicyV1,
    pub(super) foreign_suite: GenerationSuiteManifestV1,
}

impl Fixture<'_> {
    pub(super) fn scope(&self) -> GenerationQualificationPhaseScopeV1<'_> {
        GenerationQualificationPhaseScopeV1 {
            generation_system: self.relations.target_system.generation_system,
            qualification_plan: &self.plan,
            suite: self.relations.suite,
        }
    }

    pub(crate) fn resource_policy(
        &self,
        approved: bool,
    ) -> VerifiedGenerationQualificationResourcePolicy {
        let bytes = resource_policy_json(2_000_000_000);
        let denied = GenerationQualificationPhasePolicyVerifier::verify_resource(
            &bytes,
            &self.operation,
            &ProductionGenerationQualificationResourcePolicySource::new(),
        )
        .expect("denied resource policy");
        if approved {
            GenerationQualificationPhasePolicyVerifier::verify_resource(
                &bytes,
                &self.operation,
                &ProductionGenerationQualificationResourcePolicySource::exact_test_source(
                    denied.policy_digest().clone(),
                ),
            )
            .expect("approved resource policy")
        } else {
            denied
        }
    }

    pub(super) fn human_policy(
        &self,
        approved: bool,
    ) -> VerifiedGenerationQualificationHumanAdjudicationPolicy {
        let bytes = human_policy_json(7_627_468_237_172_680_543);
        let denied = GenerationQualificationPhasePolicyVerifier::verify_human_adjudication(
            &bytes,
            &self.operation,
            &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
        )
        .expect("denied human policy");
        if approved {
            GenerationQualificationPhasePolicyVerifier::verify_human_adjudication(
                &bytes,
                &self.operation,
                &ProductionGenerationQualificationHumanAdjudicationPolicySource::exact_test_source(
                    denied.policy_digest().clone(),
                ),
            )
            .expect("approved human policy")
        } else {
            denied
        }
    }

    pub(super) fn alternate_resource_policy(
        &self,
    ) -> VerifiedGenerationQualificationResourcePolicy {
        let bytes = resource_policy_json(2_000_000_001);
        let digest = phase_digest(RESOURCE_DOMAIN, &bytes);
        let (_, operation) = operation(
            self.relations,
            self.operation_input.clone(),
            digest,
            human_policy_digest(),
        );
        GenerationQualificationPhasePolicyVerifier::verify_resource(
            &bytes,
            &operation,
            &ProductionGenerationQualificationResourcePolicySource::new(),
        )
        .expect("alternate resource policy")
    }

    pub(super) fn alternate_human_policy(
        &self,
    ) -> VerifiedGenerationQualificationHumanAdjudicationPolicy {
        let bytes = human_policy_json(7_627_468_237_172_680_544);
        let digest = phase_digest(HUMAN_DOMAIN, &bytes);
        let (_, operation) = operation(
            self.relations,
            self.operation_input.clone(),
            resource_policy_digest(),
            digest,
        );
        GenerationQualificationPhasePolicyVerifier::verify_human_adjudication(
            &bytes,
            &operation,
            &ProductionGenerationQualificationHumanAdjudicationPolicySource::new(),
        )
        .expect("alternate human policy")
    }

    pub(crate) fn swapped_operation(
        &self,
    ) -> (
        GenerationQualificationPlanV1,
        GenerationQualificationOperationPolicyV1,
    ) {
        operation(
            self.relations,
            self.operation_input.clone(),
            human_policy_digest(),
            resource_policy_digest(),
        )
    }
}

pub(crate) fn with_fixture<T>(use_fixture: impl for<'records> FnOnce(Fixture<'records>) -> T) -> T {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::BothRejected,
        |draft, _platform, _license, _license_policy, _production_license| {
            let foreign_suite = foreign_suite(&draft);
            let relations = draft.operation_policy_relations;
            let operation_input = draft.operation_policy_input;
            let (plan, operation) = operation(
                relations,
                operation_input.clone(),
                resource_policy_digest(),
                human_policy_digest(),
            );
            use_fixture(Fixture {
                relations,
                operation_input,
                plan,
                operation,
                foreign_suite,
            })
        },
    )
}

fn operation(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    mut input: GenerationQualificationOperationPolicyV1Input,
    resource_policy_digest: Digest,
    human_adjudication_policy_digest: Digest,
) -> (
    GenerationQualificationPlanV1,
    GenerationQualificationOperationPolicyV1,
) {
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
    .expect("phase-denial test plan");
    let policy = GenerationQualificationOperationPolicyV1::new(
        GenerationQualificationOperationPolicyV1Relations {
            plan: &plan,
            ..relations
        },
        input,
    )
    .expect("phase-denial test operation");
    (plan, policy)
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

fn foreign_suite(
    draft: &SyntheticGenerationQualificationDraftInput<'_, '_>,
) -> GenerationSuiteManifestV1 {
    let authority = &draft.case_authorities[0];
    let case = GenerationCaseManifestV1::new(
        &authority.cluster,
        GenerationCaseManifestV1Input {
            case_key: "foreign-phase-denial-case".to_owned(),
            source_artifact_id: authority.source.source_artifact_id().clone(),
            source_digest: authority.source.source_digest().clone(),
            source_byte_count: authority.source.source_byte_count(),
            case_contract_digest: Digest::sha256(b"foreign case contract"),
            language_digest: Digest::sha256(b"foreign language"),
            mode_digest: Digest::sha256(b"foreign mode"),
            format_digest: Digest::sha256(b"foreign format"),
        },
    )
    .expect("foreign case");
    GenerationSuiteManifestV1::new(Digest::sha256(b"foreign suite"), &[case])
        .expect("foreign suite")
}

fn resource_policy_json(maximum_cleanup_nanoseconds: u64) -> Vec<u8> {
    format!(
        concat!(
            "{{\"authority\":\"none\",",
            "\"decision_rule\":\"all_complete_target_observations_within_declared_limits\",",
            "\"procedure_id\":\"retonr:generation-qualification-resource-policy:procedure\",",
            "\"procedure_version\":1,",
            "\"measurement_profile\":\"managed_local_generation_v1\",",
            "\"maximum_attempt_elapsed_nanoseconds\":30000000000,",
            "\"maximum_first_response_nanoseconds\":5000000000,",
            "\"maximum_cleanup_nanoseconds\":{},",
            "\"maximum_worker_high_water_resident_bytes\":17179869184,",
            "\"maximum_installed_footprint_bytes\":34359738368,",
            "\"required_provider_observations\":[\"prompt_token_count\",",
            "\"generated_token_count\",\"total_duration_nanoseconds\",",
            "\"load_duration_nanoseconds\",\"prompt_evaluation_duration_nanoseconds\",",
            "\"evaluation_duration_nanoseconds\"],\"schema_version\":1}}"
        ),
        maximum_cleanup_nanoseconds
    )
    .into_bytes()
}

fn human_policy_json(presentation_seed: u64) -> Vec<u8> {
    format!(
        concat!(
            "{{\"authority\":\"none\",",
            "\"decision_rule\":\"two_independent_blinded_reviews_then_role_separated_adjudication\",",
            "\"procedure_id\":\"retonr:generation-qualification-human-adjudication-policy:procedure\",",
            "\"procedure_version\":1,",
            "\"presentation_rule\":\"deterministic_blinded_candidate_pair_v1\",",
            "\"presentation_seed\":{},",
            "\"eligible_case_rule\":\"all_cases_in_all_passed_repetitions\",",
            "\"primary_reviewer_count\":2,",
            "\"require_distinct_primary_reviewers\":true,",
            "\"require_role_separated_adjudicator\":true,",
            "\"adjudication_trigger\":\"disagreement_tie_or_abstention\",",
            "\"allowed_outcomes\":[\"acceptable\",\"unacceptable\",\"abstain\"],",
            "\"schema_version\":1}}"
        ),
        presentation_seed
    )
    .into_bytes()
}

fn resource_policy_digest() -> Digest {
    phase_digest(RESOURCE_DOMAIN, &resource_policy_json(2_000_000_000))
}

fn human_policy_digest() -> Digest {
    phase_digest(HUMAN_DOMAIN, &human_policy_json(7_627_468_237_172_680_543))
}

fn phase_digest(domain: &[u8], bytes: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(
        u64::try_from(bytes.len())
            .expect("bounded test policy")
            .to_be_bytes(),
    );
    hasher.update(bytes);
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize())).expect("sha256 digest")
}
