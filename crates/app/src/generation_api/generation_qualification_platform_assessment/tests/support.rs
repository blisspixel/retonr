use rewrite_inference::candidate_output_contract;
use rewrite_model::{
    GenerationQualificationDecisionRuleV1, GenerationQualificationLicenseAssessmentPolicyId,
    GenerationQualificationLicensePermissionV1, GenerationQualificationOperationLimitsV1,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations, GenerationRequestBindingId,
    GenerationSystemRecordV1, GenerationSystemRecordV1Relations, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
    StructuredCompletionRequestBindingId, generation_qualification_plan_failure_policy_digest,
};
use rewrite_types::Digest;
use serde::Serialize;

use crate::candidate_attempt_precursor::tests::support::{
    Fixture, QualificationFixture, bindings_with_platform, bindings_with_strategy, launch, system,
    verified_policy,
};
use crate::{
    GenerationQualificationAssessmentPolicyVerifier,
    GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    GenerationQualificationPlatformPortableRelations,
    ProductionGenerationQualificationPlatformAssessmentPolicySource,
    VerifiedCurrentHostEnvironment, VerifiedGenerationQualificationPlatformAssessmentPolicy,
};

pub(super) struct AssessmentFixture {
    pub(super) fixture: Fixture,
    pub(super) qualification: QualificationFixture,
    pub(super) systems: Vec<GenerationSystemRecordV1>,
    pub(super) attempts: Vec<PlannedCandidateAttemptV1>,
    pub(super) plan: GenerationQualificationPlanV1,
    pub(super) operation_input: GenerationQualificationOperationPolicyV1Input,
    pub(super) operation: GenerationQualificationOperationPolicyV1,
    pub(super) projection_inputs: Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    pub(super) projection: GenerationQualificationRequestProjectionV1,
    pub(super) platform_bindings: GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    pub(super) denied_platform_policy: VerifiedGenerationQualificationPlatformAssessmentPolicy,
    pub(super) approved_platform_policy: VerifiedGenerationQualificationPlatformAssessmentPolicy,
    target_index: usize,
}

impl AssessmentFixture {
    pub(super) fn new(target: RuntimeTarget) -> Self {
        let target_bindings =
            bindings_with_strategy(candidate_output_contract().schema_digest, "strategy");
        Self::with_bindings(target, &target_bindings, "resource policy")
    }

    pub(super) fn with_operation_resource_policy(target: RuntimeTarget, label: &str) -> Self {
        let target_bindings =
            bindings_with_strategy(candidate_output_contract().schema_digest, "strategy");
        Self::with_bindings(target, &target_bindings, label)
    }

    pub(super) fn with_current_host(current_host: &VerifiedCurrentHostEnvironment) -> Self {
        let digests = current_host.digest_set();
        let target_bindings = bindings_with_platform(
            candidate_output_contract().schema_digest,
            "strategy",
            digests.operating_system_digest().digest().clone(),
            digests.architecture_digest().digest().clone(),
            digests.execution_class_digest().digest().clone(),
            digests.hardware_envelope_digest().digest().clone(),
        );
        Self::with_bindings(
            current_host.runtime_target(),
            &target_bindings,
            "resource policy",
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one fixture builds the complete portable and app-owned assessment closure"
    )]
    fn with_bindings(
        target: RuntimeTarget,
        target_bindings: &crate::GenerationSystemPolicyBindingsV1,
        resource_policy_label: &str,
    ) -> Self {
        let fixture = Fixture::new_with_runtime_target(target);
        let launch_plan = launch(&fixture.model_lease, "v1");
        let qualification =
            QualificationFixture::with_exact_bindings(&fixture, &launch_plan, target_bindings);
        let baseline_bindings = bindings_with_strategy(
            candidate_output_contract().schema_digest,
            "platform assessment baseline strategy",
        );
        let baseline = system(
            &fixture,
            &launch_plan,
            &qualification.expected_runtime_state,
            &qualification.characterized_package,
            &qualification.static_model,
            &baseline_bindings,
        );
        let target_id = qualification
            .generation_system
            .generation_system_id()
            .clone();
        let mut systems = vec![qualification.generation_system.clone(), baseline];
        systems.sort_by(|left, right| {
            left.generation_system_id()
                .digest()
                .as_str()
                .cmp(right.generation_system_id().digest().as_str())
        });
        let target_index = systems
            .iter()
            .position(|system| system.generation_system_id() == &target_id)
            .expect("target remains present");
        let attempts = systems
            .iter()
            .enumerate()
            .map(|(ordinal, generation_system)| {
                PlannedCandidateAttemptV1::new(
                    PlannedCandidateAttemptV1Relations {
                        suite: &qualification.suite,
                        case: &qualification.case,
                        cluster: &qualification.cluster,
                        repetition: &qualification.repetition,
                        generation_system,
                    },
                    PlannedCandidateAttemptV1Input {
                        attempt_ordinal: u32::try_from(ordinal).expect("attempt ordinal"),
                        declared_seed: 10 + u64::try_from(ordinal).expect("seed ordinal"),
                        grounded_request_digest: digest(&format!("grounded request {ordinal}")),
                        generation_request_binding_id:
                            GenerationRequestBindingId::from_derived_digest(digest(&format!(
                                "generation request {ordinal}"
                            ))),
                        candidate_output_contract_digest: candidate_output_contract().schema_digest,
                        output_ceilings: qualification.output_ceilings,
                    },
                )
                .expect("planned attempt")
            })
            .collect::<Vec<_>>();

        let platform_bindings = GenerationQualificationPlatformAssessmentPolicyV1Bindings {
            runtime_target: reviewed_target(),
            operating_system_digest: systems[target_index].operating_system_digest().clone(),
            architecture_digest: systems[target_index].architecture_digest().clone(),
            execution_class_digest: systems[target_index].execution_class_digest().clone(),
            hardware_envelope_digest: systems[target_index].hardware_envelope_digest().clone(),
        };
        let policy_json = platform_policy_json(&platform_bindings);
        let production_source =
            ProductionGenerationQualificationPlatformAssessmentPolicySource::new();
        let denied_platform_policy =
            GenerationQualificationAssessmentPolicyVerifier::verify_platform(
                &policy_json,
                &platform_bindings,
                &production_source,
            )
            .expect("structurally verified denied platform policy");
        let approved_source =
            ProductionGenerationQualificationPlatformAssessmentPolicySource::exact_test_source(
                denied_platform_policy.policy_id().clone(),
            );
        let approved_platform_policy =
            GenerationQualificationAssessmentPolicyVerifier::verify_platform(
                &policy_json,
                &platform_bindings,
                &approved_source,
            )
            .expect("approved platform policy");

        let operation_input = operation_input(
            denied_platform_policy.policy_id().clone(),
            resource_policy_label,
        );
        let plan = GenerationQualificationPlanV1::new(
            &qualification.suite,
            std::slice::from_ref(&qualification.repetition),
            &systems,
            &attempts,
            GenerationQualificationPlanV1Input {
                limits: GenerationQualificationPlanLimitsV1::new(
                    1,
                    2,
                    4_096,
                    16,
                    256,
                    qualification.output_ceilings.maximum_envelope_bytes(),
                )
                .expect("plan limits"),
                selection_policy_digest: digest("selection policy"),
                failure_policy_digest: generation_qualification_plan_failure_policy_digest(
                    operation_input.decision_rule,
                    &operation_input.platform_assessment_policy_id,
                    operation_input.required_license_permission,
                    &operation_input.license_assessment_policy_id,
                    &operation_input.attempt_ledger_policy_digest,
                    &operation_input.repeatability_policy_digest,
                    &operation_input.resource_policy_digest,
                    &operation_input.human_adjudication_policy_digest,
                ),
            },
        )
        .expect("qualification plan");
        let operation = GenerationQualificationOperationPolicyV1::new(
            operation_relations(
                &fixture,
                &qualification,
                &systems,
                target_index,
                &attempts,
                &plan,
            ),
            operation_input.clone(),
        )
        .expect("operation policy");
        let projection_inputs = attempts
            .iter()
            .enumerate()
            .map(
                |(index, _)| GenerationQualificationRequestProjectionEntryV1Input {
                    structured_completion_request_binding_id:
                        StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                            "structured request {index}"
                        ))),
                    complete_input_byte_count: 64,
                    context_token_limit: 2_048,
                    output_token_limit: 256,
                },
            )
            .collect::<Vec<_>>();
        let projection = GenerationQualificationRequestProjectionV1::new(
            GenerationQualificationRequestProjectionV1Relations {
                operation_policy: &operation,
                qualification_plan: &plan,
                suite: &qualification.suite,
                planned_attempts: &attempts,
            },
            &projection_inputs,
        )
        .expect("request projection");
        assert_eq!(qualification.generation_policy.bindings(), target_bindings);

        Self {
            fixture,
            qualification,
            systems,
            attempts,
            plan,
            operation_input,
            operation,
            projection_inputs,
            projection,
            platform_bindings,
            denied_platform_policy,
            approved_platform_policy,
            target_index,
        }
    }

    pub(super) fn portable(&self) -> GenerationQualificationPlatformPortableRelations<'_> {
        GenerationQualificationPlatformPortableRelations {
            operation_policy: &self.operation,
            operation_policy_relations: operation_relations(
                &self.fixture,
                &self.qualification,
                &self.systems,
                self.target_index,
                &self.attempts,
                &self.plan,
            ),
            operation_policy_input: &self.operation_input,
            request_projection: &self.projection,
            request_projection_relations: GenerationQualificationRequestProjectionV1Relations {
                operation_policy: &self.operation,
                qualification_plan: &self.plan,
                suite: &self.qualification.suite,
                planned_attempts: &self.attempts,
            },
            request_projection_entry_inputs: &self.projection_inputs,
        }
    }

    pub(super) fn target_system(&self) -> &GenerationSystemRecordV1 {
        &self.systems[self.target_index]
    }
}

fn operation_relations<'a>(
    fixture: &'a Fixture,
    qualification: &'a QualificationFixture,
    systems: &'a [GenerationSystemRecordV1],
    target_index: usize,
    attempts: &'a [PlannedCandidateAttemptV1],
    plan: &'a GenerationQualificationPlanV1,
) -> GenerationQualificationOperationPolicyV1Relations<'a> {
    let baseline_index = usize::from(target_index == 0);
    let model = fixture.model_lease.private_view();
    let system_relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: &fixture.runtime.runtime_manifest,
        runtime_build: &fixture.runtime.runtime_build,
        effective_runtime_state: &qualification.expected_runtime_state,
        model_artifact_set: model.artifact_set_manifest(),
        model_package_manifest: model.model_package_manifest(),
        effective_package_evidence_v2: qualification.characterized_package.evidence(),
    };
    GenerationQualificationOperationPolicyV1Relations {
        suite: &qualification.suite,
        plan,
        repetitions: std::slice::from_ref(&qualification.repetition),
        planned_attempts: attempts,
        target_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &systems[target_index],
            relations: system_relations,
        },
        baseline_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &systems[baseline_index],
            relations: system_relations,
        },
    }
}

fn operation_input(
    platform_policy_id: rewrite_model::GenerationQualificationPlatformAssessmentPolicyId,
    resource_policy_label: &str,
) -> GenerationQualificationOperationPolicyV1Input {
    let output =
        rewrite_model::CandidateOutputCeilingsV1::new(1, 1_024, 1_024).expect("output limits");
    GenerationQualificationOperationPolicyV1Input {
        limits: GenerationQualificationOperationLimitsV1::new(
            4_096,
            4_096,
            2_048,
            256,
            output.maximum_envelope_bytes(),
            1,
            1_024,
            1_024,
            2,
            1,
            60_000,
        )
        .expect("operation limits"),
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id: platform_policy_id,
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id:
            GenerationQualificationLicenseAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"platform-assessment-license-test"}"#,
            )
            .expect("license policy ID"),
        attempt_ledger_policy_digest: digest("attempt ledger policy"),
        repeatability_policy_digest: digest("repeatability policy"),
        resource_policy_digest: digest(resource_policy_label),
        human_adjudication_policy_digest: digest("human adjudication policy"),
    }
}

fn platform_policy_json(
    bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
) -> Vec<u8> {
    #[derive(Serialize)]
    #[expect(
        clippy::struct_field_names,
        reason = "field names intentionally freeze the canonical policy wire contract"
    )]
    struct Policy<'a> {
        authority: &'static str,
        policy: &'static str,
        decision_rule: &'static str,
        procedure_id: &'static str,
        procedure_version: u32,
        profile: &'static str,
        runtime_target: RuntimeTarget,
        operating_system_digest: &'a Digest,
        architecture_digest: &'a Digest,
        execution_class_digest: &'a Digest,
        hardware_envelope_digest: &'a Digest,
        schema_version: u32,
    }
    serde_json::to_vec(&Policy {
        authority: "none",
        policy: "generation_qualification_platform_assessment",
        decision_rule: "exact_reviewed_profile_and_current_host",
        procedure_id: crate::GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_ID,
        procedure_version: crate::GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_VERSION,
        profile: "managed_linux_native_cpu",
        runtime_target: bindings.runtime_target,
        operating_system_digest: &bindings.operating_system_digest,
        architecture_digest: &bindings.architecture_digest,
        execution_class_digest: &bindings.execution_class_digest,
        hardware_envelope_digest: &bindings.hardware_envelope_digest,
        schema_version: crate::GENERATION_QUALIFICATION_ASSESSMENT_POLICY_SCHEMA_VERSION,
    })
    .expect("platform policy JSON")
}

pub(super) fn foreign_generation_policy() -> crate::VerifiedGenerationSystemPolicy {
    let bindings = bindings_with_strategy(
        candidate_output_contract().schema_digest,
        "foreign platform assessment strategy",
    );
    verified_policy(
        &bindings,
        crate::GenerationSystemPolicyPermission::ConstructGenerationSystem,
        crate::GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
    )
}

pub(super) fn foreign_platform_policy(
    fixture: &AssessmentFixture,
) -> VerifiedGenerationQualificationPlatformAssessmentPolicy {
    let mut bindings = fixture.platform_bindings.clone();
    bindings.hardware_envelope_digest = digest("foreign platform hardware");
    let json = platform_policy_json(&bindings);
    let denied = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &json,
        &bindings,
        &ProductionGenerationQualificationPlatformAssessmentPolicySource::new(),
    )
    .expect("foreign structurally valid policy");
    let source = ProductionGenerationQualificationPlatformAssessmentPolicySource::exact_test_source(
        denied.policy_id().clone(),
    );
    GenerationQualificationAssessmentPolicyVerifier::verify_platform(&json, &bindings, &source)
        .expect("foreign approved policy")
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

pub(super) fn reviewed_target() -> RuntimeTarget {
    RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("reviewed target")
}
