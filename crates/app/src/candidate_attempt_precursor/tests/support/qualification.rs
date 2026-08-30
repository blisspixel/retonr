use rewrite_inference::{
    GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, ReasoningPolicy, SamplingParameters,
    candidate_output_contract,
};
use rewrite_model::{
    ArtifactId, CandidateOutputCeilingsV1, EffectiveRuntimeState, EffectiveRuntimeStateInput,
    FrozenExternalComponentSetId, GenerationCaseManifestV1, GenerationCaseManifestV1Input,
    GenerationClusterRecordV1, GenerationQualificationPlanLimitsV1, GenerationQualificationPlanV1,
    GenerationQualificationPlanV1Input, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, GenerationSystemRecordV1Input, GenerationSystemRecordV1Relations,
    ManagedGenerationPathId, PlannedCandidateAttemptV1, PlannedCandidateAttemptV1Input,
    PlannedCandidateAttemptV1Relations, RuntimeAdmissionJoinId,
};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;

use crate::{
    CandidateAttemptPrecursorCompilationError, CandidateAttemptPrecursorCompilationInput,
    CandidateAttemptPrecursorCompiler, GenerationSystemPolicyBindingsV1,
    GenerationSystemPolicyBindingsV1Input, GenerationSystemPolicyCompiler,
    GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose,
    GenerationSystemPolicyVerifier, ProductionGenerationSystemPolicyApproval,
    ReleasedGenerationEffectivePackageV2, StaticModelInterpretationV1,
    VerifiedGenerationSystemPolicy, VerifiedManagedOllamaLaunchPlan,
};

use super::{Fixture, RuntimeFixture, launch};

#[cfg(test)]
#[path = "qualification/substitutions.rs"]
mod substitutions;
#[cfg(test)]
pub(in crate::candidate_attempt_precursor::tests) use substitutions::portable_and_policy_substitution_cases;

const SOURCE: &str = "source bytes";

pub(crate) struct QualificationFixture {
    pub(crate) characterized_package: ReleasedGenerationEffectivePackageV2,
    pub(crate) expected_runtime_state: EffectiveRuntimeState,
    pub(crate) static_model: StaticModelInterpretationV1,
    pub(crate) generation_policy: VerifiedGenerationSystemPolicy,
    pub(crate) generation_system: GenerationSystemRecordV1,
    pub(crate) cluster: GenerationClusterRecordV1,
    pub(crate) case: GenerationCaseManifestV1,
    pub(crate) suite: GenerationSuiteManifestV1,
    pub(crate) repetition: GenerationRepetitionRecordV1,
    pub(crate) planned_attempt: PlannedCandidateAttemptV1,
    pub(crate) qualification_plan: GenerationQualificationPlanV1,
    pub(crate) generation_request: GenerationRequest,
    pub(crate) output_ceilings: CandidateOutputCeilingsV1,
}

impl QualificationFixture {
    pub(crate) fn new(
        fixture: &Fixture,
        launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
    ) -> Self {
        Self::with_grounded_request(fixture, launch_plan, "grounded request")
    }

    pub(crate) fn with_grounded_request(
        fixture: &Fixture,
        launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
        grounded_request: &str,
    ) -> Self {
        Self::with_policy(
            fixture,
            launch_plan,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            grounded_request,
        )
    }

    fn with_policy(
        fixture: &Fixture,
        launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
        permission: GenerationSystemPolicyPermission,
        purpose: GenerationSystemPolicyPurpose,
        grounded_request: &str,
    ) -> Self {
        let output = candidate_output_contract();
        let bindings = bindings(output.schema_digest.clone());
        Self::with_policy_and_bindings(
            fixture,
            launch_plan,
            permission,
            purpose,
            grounded_request,
            &bindings,
            output,
        )
    }

    pub(crate) fn with_exact_bindings(
        fixture: &Fixture,
        launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
        bindings: &GenerationSystemPolicyBindingsV1,
    ) -> Self {
        let output = candidate_output_contract();
        assert_eq!(bindings.output_schema_digest(), &output.schema_digest);
        Self::with_policy_and_bindings(
            fixture,
            launch_plan,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
            "grounded request",
            bindings,
            output,
        )
    }

    fn with_policy_and_bindings(
        fixture: &Fixture,
        launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
        permission: GenerationSystemPolicyPermission,
        purpose: GenerationSystemPolicyPurpose,
        grounded_request: &str,
        bindings: &GenerationSystemPolicyBindingsV1,
        output: rewrite_inference::OutputContract,
    ) -> Self {
        let expected_runtime_state = fixture.runtime.runtime_state.clone();
        let characterized_package = characterized(fixture, launch_plan, &expected_runtime_state);
        let static_model =
            StaticModelInterpretationV1::derive(launch_plan).expect("static interpretation");
        let generation_policy = verified_policy(bindings, permission, purpose);
        let generation_system = system(
            fixture,
            launch_plan,
            &expected_runtime_state,
            &characterized_package,
            &static_model,
            bindings,
        );
        let source_digest = Digest::sha256(SOURCE.as_bytes());
        let cluster =
            GenerationClusterRecordV1::new("candidate", digest("cluster policy")).expect("cluster");
        let case = GenerationCaseManifestV1::new(
            &cluster,
            GenerationCaseManifestV1Input {
                case_key: "candidate-case".to_owned(),
                source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
                source_digest,
                source_byte_count: SOURCE.len() as u64,
                case_contract_digest: digest("case contract"),
                language_digest: bindings.language_digest().clone(),
                mode_digest: bindings.mode_digest().clone(),
                format_digest: bindings.format_digest().clone(),
            },
        )
        .expect("case");
        let suite =
            GenerationSuiteManifestV1::new(digest("suite protocol"), std::slice::from_ref(&case))
                .expect("suite");
        let repetition = GenerationRepetitionRecordV1::new(&suite, 0, digest("repetition policy"))
            .expect("repetition");
        let output_ceilings =
            CandidateOutputCeilingsV1::new(1, 1_024, 1_024).expect("output ceilings");
        let generation_request = request(launch_plan, output_ceilings);
        let planned_attempt = PlannedCandidateAttemptV1::new(
            PlannedCandidateAttemptV1Relations {
                suite: &suite,
                case: &case,
                cluster: &cluster,
                repetition: &repetition,
                generation_system: &generation_system,
            },
            PlannedCandidateAttemptV1Input {
                attempt_ordinal: 0,
                declared_seed: 7,
                grounded_request_digest: digest(grounded_request),
                generation_request_binding_id: generation_request.generation_request_binding_id(),
                candidate_output_contract_digest: output.schema_digest,
                output_ceilings,
            },
        )
        .expect("planned attempt");
        let limits = GenerationQualificationPlanLimitsV1::new(1, 1, 4_096, 16, 256, 1024 * 1024)
            .expect("plan limits");
        let qualification_plan = GenerationQualificationPlanV1::new(
            &suite,
            std::slice::from_ref(&repetition),
            std::slice::from_ref(&generation_system),
            std::slice::from_ref(&planned_attempt),
            GenerationQualificationPlanV1Input {
                limits,
                selection_policy_digest: digest("selection policy"),
                failure_policy_digest: digest("failure policy"),
            },
        )
        .expect("qualification plan");
        Self {
            characterized_package,
            expected_runtime_state,
            static_model,
            generation_policy,
            generation_system,
            cluster,
            case,
            suite,
            repetition,
            planned_attempt,
            qualification_plan,
            generation_request,
            output_ceilings,
        }
    }
}

pub(crate) fn compilation_input<'a, 'model>(
    runtime: &'a mut RuntimeFixture,
    qualification: &'a QualificationFixture,
    launch_plan: VerifiedManagedOllamaLaunchPlan<'model>,
) -> CandidateAttemptPrecursorCompilationInput<'a, 'model, 'a, 'a> {
    CandidateAttemptPrecursorCompilationInput {
        qualification_plan: &qualification.qualification_plan,
        planned_attempt: &qualification.planned_attempt,
        generation_system: &qualification.generation_system,
        runtime_manifest: &runtime.runtime_manifest,
        runtime_package: &mut runtime.runtime_package,
        runtime_build: &runtime.runtime_build,
        admitted_runtime: &runtime.admitted,
        generation_path: &runtime.path,
        frozen_components: &runtime.frozen,
        launch_plan,
        characterized_package: &qualification.characterized_package,
        expected_runtime_state: &qualification.expected_runtime_state,
        static_model: &qualification.static_model,
        generation_request: qualification.generation_request.clone(),
        output_ceilings: qualification.output_ceilings,
        generation_policy: &qualification.generation_policy,
    }
}

pub(crate) fn characterized(
    fixture: &Fixture,
    launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
    state: &EffectiveRuntimeState,
) -> ReleasedGenerationEffectivePackageV2 {
    let derived = crate::generation_effective_package::derive_managed_judge_batch(
        &crate::generation_effective_package::ManagedJudgeBatchInputs {
            runtime_manifest: &fixture.runtime.runtime_manifest,
            model_package: launch_plan.package(),
            runtime_build: &fixture.runtime.runtime_build,
            effective_state: state,
            admitted_runtime: &fixture.runtime.admitted,
            generation_path: &fixture.runtime.path,
            frozen_components: &fixture.runtime.frozen,
            prepared_isolation: &fixture.runtime.prepared_isolation,
            license_control_id: launch_plan.model_license_control_id(),
        },
        &CancellationToken::new(),
    )
    .expect("exact effective package evidence");
    ReleasedGenerationEffectivePackageV2::exact_candidate_precursor_test_fixture(
        derived.evidence,
        launch_plan.foundation_id().clone(),
        launch_plan.model_license_control_id().clone(),
        fixture
            .runtime
            .runtime_package
            .evidence()
            .artifact_set_id()
            .clone(),
        fixture
            .runtime
            .runtime_package
            .evidence()
            .runtime_package_manifest_id()
            .clone(),
        launch_plan.package().artifact_set_id().clone(),
        launch_plan.package().model_package_manifest_id().clone(),
    )
}

pub(crate) fn system(
    fixture: &Fixture,
    launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
    state: &EffectiveRuntimeState,
    characterized: &ReleasedGenerationEffectivePackageV2,
    static_model: &StaticModelInterpretationV1,
    bindings: &GenerationSystemPolicyBindingsV1,
) -> GenerationSystemRecordV1 {
    GenerationSystemRecordV1::new(
        GenerationSystemRecordV1Relations {
            runtime_package_manifest: &fixture.runtime.runtime_manifest,
            runtime_build: &fixture.runtime.runtime_build,
            effective_runtime_state: state,
            model_artifact_set: launch_plan.model_artifact_set_manifest(),
            model_package_manifest: launch_plan.model_package_manifest(),
            effective_package_evidence_v2: characterized.evidence(),
        },
        GenerationSystemRecordV1Input {
            runtime_admission_join_id: RuntimeAdmissionJoinId::from_derived_digest(
                fixture
                    .runtime
                    .admitted
                    .admitted_runtime_id()
                    .digest()
                    .clone(),
            ),
            managed_generation_path_id: ManagedGenerationPathId::from_derived_digest(
                fixture.runtime.path.generation_path_id().clone(),
            ),
            frozen_external_component_set_id: FrozenExternalComponentSetId::from_derived_digest(
                fixture.runtime.frozen.frozen_set_id().digest().clone(),
            ),
            model_artifact_id: launch_plan.model_target().artifact_id().clone(),
            static_model_binding_digest: static_model.binding_digest().clone(),
            strategy_digest: bindings.strategy_digest().clone(),
            planner_digest: bindings.planner_digest().clone(),
            validator_digest: bindings.validator_digest().clone(),
            adapter_digest: bindings.adapter_digest().clone(),
            prompt_digest: bindings.prompt_digest().clone(),
            output_schema_digest: bindings.output_schema_digest().clone(),
            request_policy_digest: bindings.request_policy_digest().clone(),
            language_digest: bindings.language_digest().clone(),
            mode_digest: bindings.mode_digest().clone(),
            format_digest: bindings.format_digest().clone(),
            operating_system_digest: bindings.operating_system_digest().clone(),
            architecture_digest: bindings.architecture_digest().clone(),
            execution_class_digest: bindings.execution_class_digest().clone(),
            hardware_envelope_digest: bindings.hardware_envelope_digest().clone(),
        },
    )
    .expect("generation system")
}

fn request(
    launch_plan: &VerifiedManagedOllamaLaunchPlan<'_>,
    ceilings: CandidateOutputCeilingsV1,
) -> GenerationRequest {
    GenerationRequest {
        schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
        artifact_id: launch_plan.model_target().artifact_id().clone(),
        artifact_digest: launch_plan.model_target().artifact_id().digest().clone(),
        input: "complete grounded source bytes".to_owned(),
        output: candidate_output_contract(),
        candidate_count: ceilings.candidate_count(),
        source_byte_count: SOURCE.len() as u64,
        source_byte_limit: 4_096,
        input_byte_limit: 4_096,
        context_token_limit: 2_048,
        output_token_limit: 256,
        candidate_byte_limit: ceilings.maximum_candidate_bytes(),
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

fn bindings(output_schema_digest: Digest) -> GenerationSystemPolicyBindingsV1 {
    bindings_with_strategy(output_schema_digest, "strategy")
}

pub(crate) fn bindings_with_strategy(
    output_schema_digest: Digest,
    strategy: &str,
) -> GenerationSystemPolicyBindingsV1 {
    bindings_with_platform(
        output_schema_digest,
        strategy,
        digest("operating system"),
        digest("architecture"),
        digest("execution class"),
        digest("hardware envelope"),
    )
}

pub(crate) fn bindings_with_platform(
    output_schema_digest: Digest,
    strategy: &str,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
) -> GenerationSystemPolicyBindingsV1 {
    GenerationSystemPolicyBindingsV1::new(GenerationSystemPolicyBindingsV1Input {
        strategy_digest: digest(strategy),
        planner_digest: digest("planner"),
        validator_digest: digest("validator"),
        adapter_digest: digest("adapter"),
        prompt_digest: digest("prompt"),
        output_schema_digest,
        request_policy_digest: digest("request policy"),
        language_digest: digest("language"),
        mode_digest: digest("mode"),
        format_digest: digest("format"),
        operating_system_digest,
        architecture_digest,
        execution_class_digest,
        hardware_envelope_digest,
    })
}

pub(crate) fn verified_policy(
    bindings: &GenerationSystemPolicyBindingsV1,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
) -> VerifiedGenerationSystemPolicy {
    let review = serde_json::to_vec(&Review {
        decision: ReviewDecision::Approved,
        bindings,
        permission,
        procedure_id: crate::GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_ID,
        procedure_version: crate::GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_VERSION,
        purpose,
        schema_version: crate::GENERATION_SYSTEM_POLICY_REVIEW_SCHEMA_VERSION,
    })
    .expect("review JSON");
    let compiled = GenerationSystemPolicyCompiler::compile(bindings, permission, purpose, &review)
        .expect("compile generation policy");
    let approval = ProductionGenerationSystemPolicyApproval::exact_test_policy(
        compiled.control_id().clone(),
        permission,
        purpose,
    );
    GenerationSystemPolicyVerifier::verify(
        compiled.canonical_bytes(),
        bindings,
        permission,
        purpose,
        &approval,
    )
    .expect("verify generation policy")
}

pub(crate) fn changed_state(runtime: &RuntimeFixture) -> EffectiveRuntimeState {
    EffectiveRuntimeState::new(
        &runtime.runtime_build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "candidate-precursor-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("changed snapshot"),
            launch_policy_digest: digest("launch policy"),
            loaded_components_digest: digest("loaded components"),
            effective_configuration_digest: digest("effective configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("runtime execution class"),
            isolation_policy_digest: digest("isolation"),
            effective_context_tokens: 8_192,
            compute_backend: rewrite_model::ComputeBackend::NativeCpu,
            placement: rewrite_model::ExecutionPlacement::CpuOnly,
        },
    )
    .expect("changed runtime state")
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewDecision {
    Approved,
}

#[derive(Serialize)]
struct Review<'a> {
    decision: ReviewDecision,
    bindings: &'a GenerationSystemPolicyBindingsV1,
    permission: GenerationSystemPolicyPermission,
    procedure_id: &'static str,
    procedure_version: u32,
    purpose: GenerationSystemPolicyPurpose,
    schema_version: u32,
}
