//! Feature-gated synthetic generation-qualification integration fixture.

use rewrite_model::{
    ComputeBackend, ExecutionPlacement, GenerationQualificationDecisionRuleV1,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationQualificationPlatformAssessmentPolicyId, GenerationSystemRecordV1Relations,
    HostAcceleratorScopeV1, HostArchitectureV1Input, HostEnvironmentV1Input,
    HostExecutionClassV1Input, HostExecutionProfileV1, HostHardwareEnvelopeV1Input,
    HostOperatingSystemV1Input, ModelLicenseControlId, ObserverBinaryAssertionModeV1, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem,
    generation_qualification_plan_failure_policy_digest,
};
use rewrite_model_store::{
    GenerationQualificationPlanFoundationV1Input,
    GenerationQualificationPreregistrationFoundationV1Input,
};
use rewrite_types::{CancellationToken, Digest};

use super::generation_qualification_assessment_policy::{
    license_policy_json_for_test, platform_policy_json_for_test,
};
use super::{
    GenerationQualificationAssessmentPolicySourceDisposition,
    GenerationQualificationAssessmentPolicyVerifier,
    GenerationQualificationPlatformAssessmentCompiler,
    GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    GenerationQualificationPlatformPortableRelations,
    GenerationQualificationRequestProjectionCaseAuthoritiesV1,
    GenerationQualificationReviewedPlatformAuthorities, GenerationSystemPolicyPermission,
    GenerationSystemPolicyPurpose, ModelLicenseControlCompiler, ModelLicenseControlVerifier,
    ModelLicensePermission, ProductionGenerationQualificationLicenseAssessmentPolicySource,
    ProductionGenerationQualificationPlatformAssessmentPolicySource,
    ProductionModelLicenseApprovalPolicy, VerifiedCurrentHostEnvironment,
    VerifiedGenerationQualificationLicenseAssessmentPolicy,
    VerifiedGenerationQualificationPlatformAssessment,
    VerifiedGenerationQualificationPlatformAssessmentPolicy, VerifiedGenerationSystemPolicy,
    VerifiedModelLicenseControl,
};
use crate::{
    VerifiedAdmittedRuntime, VerifiedManagedGenerationPath, VerifiedManagedOllamaModelPackageLease,
};

use super::generation_qualification_request_builder::fixture_support as request_support;

/// Closed synthetic outcome selected by the feature-gated integration fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntheticGenerationQualificationScenario {
    /// Both synthetic assessment authorities permit local qualification traffic.
    TrafficEligible,
    /// Platform is positive while the license authority is negative.
    LicenseRejected,
    /// Both authorities are negative, exercising platform precedence.
    BothRejected,
}

/// Exact draft inputs owned or borrowed by one synthetic callback invocation.
pub struct SyntheticGenerationQualificationDraftInput<'records, 'store> {
    /// Complete portable plan foundation that must be stored before preregistration.
    pub foundation: GenerationQualificationPreregistrationFoundationV1Input<'records>,
    /// Complete operation-policy relationship closure.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    /// Independently constructed policy input.
    pub operation_policy_input: GenerationQualificationOperationPolicyV1Input,
    /// Exact semantic-order source and builder authorities.
    pub case_authorities: Vec<GenerationQualificationRequestProjectionCaseAuthoritiesV1<'store>>,
}

/// Synthetic retained owners needed by the production platform compiler.
pub struct SyntheticGenerationQualificationPlatformOwners<'authority> {
    admitted_runtime: &'authority VerifiedAdmittedRuntime,
    generation_path: &'authority VerifiedManagedGenerationPath,
    frozen_components:
        &'authority rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet,
    generation_system_policy: VerifiedGenerationSystemPolicy,
    assessment_policy: VerifiedGenerationQualificationPlatformAssessmentPolicy,
    current_host: VerifiedCurrentHostEnvironment,
}

impl SyntheticGenerationQualificationPlatformOwners<'_> {
    /// Runs the exact production platform compiler for this synthetic fixture.
    ///
    /// # Errors
    ///
    /// Returns the production compiler's content-free relationship or observation error.
    pub fn assess<'owners>(
        &'owners self,
        portable: GenerationQualificationPlatformPortableRelations<'_>,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedGenerationQualificationPlatformAssessment<'owners>,
        super::GenerationQualificationPlatformAssessmentError,
    > {
        let authorities = GenerationQualificationReviewedPlatformAuthorities {
            admitted_runtime: self.admitted_runtime,
            generation_path: self.generation_path,
            frozen_components: self.frozen_components,
            generation_system_policy: &self.generation_system_policy,
            assessment_policy: &self.assessment_policy,
        };
        match self.assessment_policy.source_disposition() {
            GenerationQualificationAssessmentPolicySourceDisposition::Approved => {
                GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
                    portable,
                    authorities,
                    &self.current_host,
                    cancellation,
                )
            }
            GenerationQualificationAssessmentPolicySourceDisposition::Denied => {
                GenerationQualificationPlatformAssessmentCompiler::assess_policy_denied(
                    portable,
                    authorities,
                    cancellation,
                )
            }
        }
    }
}

/// Synthetic retained structural proof needed by the production license compiler.
pub struct SyntheticGenerationQualificationLicenseProof<'lease> {
    control: VerifiedModelLicenseControl<'lease>,
    selected_lease: &'lease VerifiedManagedOllamaModelPackageLease,
}

impl<'lease> SyntheticGenerationQualificationLicenseProof<'lease> {
    /// Returns the retained structural proof.
    #[must_use]
    pub const fn control(&self) -> &VerifiedModelLicenseControl<'lease> {
        &self.control
    }

    /// Returns the exact selected model package lease.
    #[must_use]
    pub const fn selected_lease(&self) -> &'lease VerifiedManagedOllamaModelPackageLease {
        self.selected_lease
    }
}

/// Runs one explicitly synthetic full fixture inside a lifetime-bounded callback.
///
/// This API exists only under the non-default `test-support` feature. It exposes
/// no individual approval-root constructor and keeps production validation active.
/// The traffic-eligible scenario supplies exact synthetic approval roots to the
/// callback so downstream state-machine tests can exercise the positive boundary.
///
/// # Panics
///
/// Panics if the repository's fixed synthetic records no longer satisfy the
/// production validators. Such a panic identifies a stale integration fixture.
#[expect(
    clippy::too_many_lines,
    reason = "the callback keeps one complete synthetic authority closure visibly local"
)]
pub fn with_synthetic_generation_qualification_fixture<T>(
    scenario: SyntheticGenerationQualificationScenario,
    use_fixture: impl for<'records, 'store, 'lease> FnOnce(
        SyntheticGenerationQualificationDraftInput<'records, 'store>,
        SyntheticGenerationQualificationPlatformOwners<'records>,
        SyntheticGenerationQualificationLicenseProof<'lease>,
        VerifiedGenerationQualificationLicenseAssessmentPolicy,
        ProductionModelLicenseApprovalPolicy,
    ) -> T,
) -> T {
    let current_host = VerifiedCurrentHostEnvironment::exact_test_fixture(host_input())
        .expect("synthetic current host");
    let digests = current_host.digest_set();
    let mut fixture = request_support::Fixture::new_with_platform_digests(
        request_support::SOURCE,
        vec!["Acme".to_owned()],
        digests.operating_system_digest().digest().clone(),
        digests.architecture_digest().digest().clone(),
        digests.execution_class_digest().digest().clone(),
        digests.hardware_envelope_digest().digest().clone(),
    );
    let attempt_plan = fixture.plan();
    let request_support::Fixture {
        runtime,
        model_set,
        model_package,
        characterized,
        builder,
        baseline_builder,
        cluster,
        deterministic_contract,
        case,
        suite,
        repetition,
        source,
    } = fixture;
    let request_support::PlanFixture {
        target_attempt,
        baseline_attempt,
        selection_policy,
        ..
    } = attempt_plan;
    let target_system = builder.generation_system().clone();
    let baseline_system = baseline_builder.generation_system().clone();
    let attempts = vec![target_attempt, baseline_attempt];

    let control = structural_control(&runtime.model_lease);
    let traffic_eligible = matches!(
        scenario,
        SyntheticGenerationQualificationScenario::TrafficEligible
    );
    let license_policy = license_policy(control.control_id(), traffic_eligible);
    let production_policy = if traffic_eligible {
        ProductionModelLicenseApprovalPolicy::exact_test_policy(
            control.control_id().clone(),
            ModelLicensePermission::LocalGeneration,
        )
    } else {
        ProductionModelLicenseApprovalPolicy::new()
    };
    let platform_approved = matches!(
        scenario,
        SyntheticGenerationQualificationScenario::TrafficEligible
            | SyntheticGenerationQualificationScenario::LicenseRejected
    );
    let platform_bindings =
        GenerationQualificationPlatformAssessmentPolicyV1Bindings::from_current_host_environment(
            &current_host,
        );
    let platform_policy = platform_policy(&platform_bindings, platform_approved);
    let policy_input = operation_input(
        platform_policy.policy_id().clone(),
        license_policy.policy_id().clone(),
    );
    let systems = sorted_systems(&target_system, &baseline_system);
    let qualification_plan = GenerationQualificationPlanV1::new(
        &suite,
        std::slice::from_ref(&repetition),
        &systems,
        &attempts,
        plan_input(
            &policy_input,
            selection_policy.selection_policy_id().digest().clone(),
        ),
    )
    .expect("synthetic qualification plan");
    let system_relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: &runtime.runtime.runtime_manifest,
        runtime_build: &runtime.runtime.runtime_build,
        effective_runtime_state: &runtime.runtime.runtime_state,
        model_artifact_set: &model_set,
        model_package_manifest: &model_package,
        effective_package_evidence_v2: characterized.evidence(),
    };
    let relations = GenerationQualificationOperationPolicyV1Relations {
        suite: &suite,
        plan: &qualification_plan,
        repetitions: std::slice::from_ref(&repetition),
        planned_attempts: &attempts,
        target_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &target_system,
            relations: system_relations,
        },
        baseline_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &baseline_system,
            relations: system_relations,
        },
    };
    let lease = source.acquire();
    let generation_policy = request_support::policy_for(
        &target_system,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
    );
    let platform_owners = SyntheticGenerationQualificationPlatformOwners {
        admitted_runtime: &runtime.runtime.admitted,
        generation_path: &runtime.runtime.path,
        frozen_components: &runtime.runtime.frozen,
        generation_system_policy: generation_policy,
        assessment_policy: platform_policy,
        current_host,
    };
    let license_proof = SyntheticGenerationQualificationLicenseProof {
        control,
        selected_lease: &runtime.model_lease,
    };
    use_fixture(
        SyntheticGenerationQualificationDraftInput {
            foundation: GenerationQualificationPreregistrationFoundationV1Input {
                runtime_artifact_sets: std::slice::from_ref(&runtime.runtime.runtime_set),
                plan_foundation: GenerationQualificationPlanFoundationV1Input {
                    clusters: std::slice::from_ref(&cluster),
                    deterministic_case_contracts: std::slice::from_ref(&deterministic_contract),
                    cases: std::slice::from_ref(&case),
                    suite: &suite,
                    repetitions: std::slice::from_ref(&repetition),
                    generation_systems: &systems,
                    planned_attempts: &attempts,
                    candidate_selection_policy: &selection_policy,
                    plan: &qualification_plan,
                },
            },
            operation_policy_relations: relations,
            operation_policy_input: policy_input,
            case_authorities: vec![GenerationQualificationRequestProjectionCaseAuthoritiesV1 {
                source: lease,
                cluster: cluster.clone(),
                target_builder: builder,
                baseline_builder,
            }],
        },
        platform_owners,
        license_proof,
        license_policy,
        production_policy,
    )
}

fn sorted_systems(
    target: &rewrite_model::GenerationSystemRecordV1,
    baseline: &rewrite_model::GenerationSystemRecordV1,
) -> Vec<rewrite_model::GenerationSystemRecordV1> {
    let mut systems = vec![target.clone(), baseline.clone()];
    systems.sort_unstable_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    systems
}

fn operation_input(
    platform_policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    license_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
) -> GenerationQualificationOperationPolicyV1Input {
    GenerationQualificationOperationPolicyV1Input {
        limits: request_support::common_limits(),
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id: platform_policy_id,
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id: license_policy_id,
        attempt_ledger_policy_digest: digest("synthetic attempt ledger policy"),
        repeatability_policy_digest: digest("synthetic repeatability policy"),
        resource_policy_digest: digest("synthetic resource policy"),
        human_adjudication_policy_digest: digest("synthetic adjudication policy"),
    }
}

fn plan_input(
    input: &GenerationQualificationOperationPolicyV1Input,
    selection_policy_digest: Digest,
) -> GenerationQualificationPlanV1Input {
    GenerationQualificationPlanV1Input {
        limits: GenerationQualificationPlanLimitsV1::new(
            1,
            2,
            input.limits.maximum_complete_input_bytes(),
            16,
            256,
            input.limits.maximum_output_bytes(),
        )
        .expect("synthetic plan limits"),
        selection_policy_digest,
        failure_policy_digest: generation_qualification_plan_failure_policy_digest(
            input.decision_rule,
            &input.platform_assessment_policy_id,
            input.required_license_permission,
            &input.license_assessment_policy_id,
            &input.attempt_ledger_policy_digest,
            &input.repeatability_policy_digest,
            &input.resource_policy_digest,
            &input.human_adjudication_policy_digest,
        ),
    }
}

fn structural_control(
    lease: &VerifiedManagedOllamaModelPackageLease,
) -> VerifiedModelLicenseControl<'_> {
    let review = ModelLicenseControlCompiler::reviewer_json_for_test(
        lease,
        ModelLicensePermission::LocalGeneration,
    );
    let compiled = ModelLicenseControlCompiler::compile(
        lease,
        ModelLicensePermission::LocalGeneration,
        &review,
        &CancellationToken::new(),
    )
    .expect("synthetic model-license control");
    ModelLicenseControlVerifier::verify_structural(
        compiled.canonical_bytes(),
        lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("synthetic structural proof")
}

fn license_policy(
    control_id: &ModelLicenseControlId,
    approved: bool,
) -> VerifiedGenerationQualificationLicenseAssessmentPolicy {
    let bytes = license_policy_json_for_test(control_id);
    let denied = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &bytes,
        control_id,
        &ProductionGenerationQualificationLicenseAssessmentPolicySource::new(),
    )
    .expect("synthetic denied license policy");
    if approved {
        let source =
            ProductionGenerationQualificationLicenseAssessmentPolicySource::exact_test_source(
                denied.policy_id().clone(),
            );
        GenerationQualificationAssessmentPolicyVerifier::verify_license(&bytes, control_id, &source)
            .expect("synthetic approved license policy")
    } else {
        denied
    }
}

fn platform_policy(
    bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    approved: bool,
) -> VerifiedGenerationQualificationPlatformAssessmentPolicy {
    let bytes = platform_policy_json_for_test(bindings);
    let denied = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &bytes,
        bindings,
        &ProductionGenerationQualificationPlatformAssessmentPolicySource::new(),
    )
    .expect("synthetic denied platform policy");
    if approved {
        let source =
            ProductionGenerationQualificationPlatformAssessmentPolicySource::exact_test_source(
                denied.policy_id().clone(),
            );
        GenerationQualificationAssessmentPolicyVerifier::verify_platform(&bytes, bindings, &source)
            .expect("synthetic approved platform policy")
    } else {
        denied
    }
}

fn host_input() -> HostEnvironmentV1Input {
    HostEnvironmentV1Input {
        operating_system: HostOperatingSystemV1Input {
            family: RuntimeOperatingSystem::Linux,
            version: "6.12.10-synthetic".to_owned(),
        },
        architecture: HostArchitectureV1Input {
            instruction_set: RuntimeArchitecture::X86_64,
            abi: RuntimeAbi::LinuxGnuLibc,
        },
        execution_class: HostExecutionClassV1Input {
            profile: HostExecutionProfileV1::ManagedLinuxNativeCpu,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
            observer_binary_assertion_mode: ObserverBinaryAssertionModeV1::Disabled,
            accelerator_scope: HostAcceleratorScopeV1::NotAssessedForManagedNativeCpu,
        },
        hardware_envelope: HostHardwareEnvelopeV1Input {
            cpu_model: "Synthetic Native CPU".to_owned(),
            physical_core_count: 4,
            logical_core_count: 8,
            total_system_memory_mib: 16_384,
            memory_rounding_granularity_mib: 1_024,
        },
    }
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
