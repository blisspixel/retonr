use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, CandidateOutputCeilingsV1,
    CandidateSelectionPolicyV1, ComputeBackend, EffectivePackageEvidenceMode,
    EffectivePackageEvidenceRoleV2, EffectivePackageEvidenceV2, EffectivePackageEvidenceV2Input,
    EffectivePackageMemberPurpose, EffectivePackageMemberUseV2, EffectiveRuntimeState,
    EffectiveRuntimeStateInput, EmbeddedModelComponentPurpose, ExecutionPlacement,
    GenerationCaseManifestV1, GenerationCaseManifestV1Input, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1, GenerationQualificationDecisionRuleV1,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationOperationLimitsV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationQualificationPlatformAssessmentPolicyId,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations, GenerationRepetitionRecordV1,
    GenerationRequestBindingId, GenerationSuiteManifestV1, GenerationSystemRecordV1,
    ModelPackageManifest, ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout,
    PackageTransformation, PackageTransformationDisposition, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations, RuntimeAbi,
    RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildMode, RuntimeOperatingSystem,
    RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget, StructuredCompletionRequestBindingId,
    generation_qualification_plan_failure_policy_digest,
};
use rewrite_types::Digest;

use crate::{
    ArtifactStateStore, GenerationQualificationPlanFoundationV1Input,
    GenerationSystemFoundationV1Input,
};

#[path = "support/system.rs"]
mod system;
pub(crate) use system::SystemFixture;
#[path = "support/alternate.rs"]
mod alternate;
#[path = "support/persistence.rs"]
mod persistence;
pub(crate) use persistence::persist_plan_foundation;
#[path = "support/plan.rs"]
mod plan;
#[path = "support/system_data.rs"]
mod system_data;
use system_data::{embedded_component, member_evidence, path, source};

const CANDIDATE_BYTES: u64 = 1_024;
const OUTPUT_ENVELOPE_BYTES: u64 = 6_428;

pub(crate) struct Fixture {
    pub(crate) system: SystemFixture,
    pub(crate) clusters: Vec<GenerationClusterRecordV1>,
    pub(crate) deterministic_case_contracts: Vec<GenerationDeterministicCaseContractV1>,
    pub(crate) cases: Vec<GenerationCaseManifestV1>,
    pub(crate) suite: GenerationSuiteManifestV1,
    pub(crate) repetitions: Vec<GenerationRepetitionRecordV1>,
    pub(crate) systems: Vec<GenerationSystemRecordV1>,
    pub(crate) attempts: Vec<PlannedCandidateAttemptV1>,
    pub(crate) selection_policy: CandidateSelectionPolicyV1,
    pub(crate) plan: GenerationQualificationPlanV1,
    pub(crate) policy_input: GenerationQualificationOperationPolicyV1Input,
    pub(crate) policy: GenerationQualificationOperationPolicyV1,
    pub(crate) entry_inputs: Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    pub(crate) projection: GenerationQualificationRequestProjectionV1,
}

impl Fixture {
    pub(crate) fn relations(&self) -> GenerationQualificationOperationPolicyV1Relations<'_> {
        GenerationQualificationOperationPolicyV1Relations {
            suite: &self.suite,
            plan: &self.plan,
            repetitions: &self.repetitions,
            planned_attempts: &self.attempts,
            target_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: &self.systems[0],
                relations: self.system.relations(),
            },
            baseline_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: &self.systems[1],
                relations: self.system.relations(),
            },
        }
    }
}

#[expect(clippy::too_many_lines, reason = "complete exact policy closure")]
pub(crate) fn fixture() -> Fixture {
    let system = system_fixture();
    let cluster = GenerationClusterRecordV1::new("store", digest("cluster")).expect("cluster");
    let deterministic_case_contract = plan::deterministic_case_contract();
    let case = GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: "case".to_owned(),
            source_artifact_id: artifact("case source"),
            source_digest: digest("case source"),
            source_byte_count: 11,
            case_contract_digest: deterministic_case_contract.contract_digest().clone(),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("case");
    let cases = vec![case];
    let suite = GenerationSuiteManifestV1::new(digest("suite protocol"), &cases).expect("suite");
    let repetitions = vec![
        GenerationRepetitionRecordV1::new(&suite, 0, digest("repetition policy"))
            .expect("repetition"),
    ];
    let mut systems = ["target", "baseline"]
        .iter()
        .map(|label| {
            GenerationSystemRecordV1::new(system.relations(), system.input(label)).expect("system")
        })
        .collect::<Vec<_>>();
    systems.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    let ceilings =
        CandidateOutputCeilingsV1::new(1, CANDIDATE_BYTES, CANDIDATE_BYTES).expect("ceilings");
    assert_eq!(ceilings.maximum_envelope_bytes(), OUTPUT_ENVELOPE_BYTES);
    let attempts = systems
        .iter()
        .enumerate()
        .map(|(index, generation_system)| {
            PlannedCandidateAttemptV1::new(
                PlannedCandidateAttemptV1Relations {
                    suite: &suite,
                    case: &cases[0],
                    cluster: &cluster,
                    repetition: &repetitions[0],
                    generation_system,
                },
                PlannedCandidateAttemptV1Input {
                    attempt_ordinal: u32::try_from(index).expect("ordinal"),
                    declared_seed: u64::try_from(index + 10).expect("seed"),
                    grounded_request_digest: digest(&format!("grounded {index}")),
                    generation_request_binding_id: GenerationRequestBindingId::from_derived_digest(
                        digest(&format!("request {index}")),
                    ),
                    candidate_output_contract_digest: digest("output contract"),
                    output_ceilings: ceilings,
                },
            )
            .expect("attempt")
        })
        .collect::<Vec<_>>();
    let selection_policy = CandidateSelectionPolicyV1::new(&suite, &[0]).expect("selection policy");
    let policy_input = policy_input(u32::try_from(attempts.len()).expect("attempt count"));
    let plan = GenerationQualificationPlanV1::new(
        &suite,
        &repetitions,
        &systems,
        &attempts,
        GenerationQualificationPlanV1Input {
            limits: GenerationQualificationPlanLimitsV1::new(
                1,
                u32::try_from(attempts.len()).expect("attempt count"),
                4_096,
                128,
                256,
                OUTPUT_ENVELOPE_BYTES,
            )
            .expect("plan limits"),
            selection_policy_digest: selection_policy.selection_policy_id().digest().clone(),
            failure_policy_digest: generation_qualification_plan_failure_policy_digest(
                policy_input.decision_rule,
                &policy_input.platform_assessment_policy_id,
                policy_input.required_license_permission,
                &policy_input.license_assessment_policy_id,
                &policy_input.attempt_ledger_policy_digest,
                &policy_input.repeatability_policy_digest,
                &policy_input.resource_policy_digest,
                &policy_input.human_adjudication_policy_digest,
            ),
        },
    )
    .expect("plan");
    let relations = GenerationQualificationOperationPolicyV1Relations {
        suite: &suite,
        plan: &plan,
        repetitions: &repetitions,
        planned_attempts: &attempts,
        target_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &systems[0],
            relations: system.relations(),
        },
        baseline_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &systems[1],
            relations: system.relations(),
        },
    };
    let policy = GenerationQualificationOperationPolicyV1::new(relations, policy_input.clone())
        .expect("policy");
    let entry_inputs = attempts
        .iter()
        .enumerate()
        .map(
            |(index, _)| GenerationQualificationRequestProjectionEntryV1Input {
                structured_completion_request_binding_id:
                    StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                        "structured {index}"
                    ))),
                complete_input_byte_count: 128,
                context_token_limit: 4_096,
                output_token_limit: 1_024,
            },
        )
        .collect::<Vec<_>>();
    let projection = GenerationQualificationRequestProjectionV1::new(
        GenerationQualificationRequestProjectionV1Relations {
            operation_policy: &policy,
            qualification_plan: &plan,
            suite: &suite,
            planned_attempts: &attempts,
        },
        &entry_inputs,
    )
    .expect("projection");
    Fixture {
        system,
        clusters: vec![cluster],
        deterministic_case_contracts: vec![deterministic_case_contract],
        cases,
        suite,
        repetitions,
        systems,
        attempts,
        selection_policy,
        plan,
        policy_input,
        policy,
        entry_inputs,
        projection,
    }
}

fn policy_input(attempt_count: u32) -> GenerationQualificationOperationPolicyV1Input {
    GenerationQualificationOperationPolicyV1Input {
        limits: GenerationQualificationOperationLimitsV1::new(
            1_024,
            4_096,
            4_096,
            1_024,
            OUTPUT_ENVELOPE_BYTES,
            1,
            CANDIDATE_BYTES,
            CANDIDATE_BYTES,
            attempt_count,
            1,
            60_000,
        )
        .expect("operation limits"),
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id:
            GenerationQualificationPlatformAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"platform-v1"}"#,
            )
            .expect("platform policy"),
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id:
            GenerationQualificationLicenseAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"license-v1"}"#,
            )
            .expect("license policy"),
        attempt_ledger_policy_digest: digest("ledger policy"),
        repeatability_policy_digest: digest("repeatability policy"),
        resource_policy_digest: digest("resource policy"),
        human_adjudication_policy_digest: digest("human policy"),
    }
}

#[expect(clippy::too_many_lines, reason = "complete typed package closure")]
pub(crate) fn system_fixture() -> SystemFixture {
    let runtime_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("runtime"), 10, path("bin/runtime")),
        ArtifactSetMember::new(artifact("runtime license"), 11, path("legal/license.txt")),
        ArtifactSetMember::new(
            artifact("runtime provenance"),
            12,
            path("legal/provenance.txt"),
        ),
    ])
    .expect("runtime set");
    let runtime_package = RuntimePackageManifest::new(
        &runtime_set,
        "ollama",
        "0.32.15",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        source("runtime"),
        PackageTransformation::Untransformed {
            evidence_digest: digest("runtime unchanged"),
        },
        vec![
            RuntimePackageMember::new(
                artifact("runtime"),
                10,
                path("bin/runtime"),
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                artifact("runtime license"),
                11,
                path("legal/license.txt"),
                vec![RuntimePackageMemberRole::LicenseText],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            RuntimePackageMember::new(
                artifact("runtime provenance"),
                12,
                path("legal/provenance.txt"),
                vec![RuntimePackageMemberRole::ProvenanceRecord],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("runtime package");
    let runtime_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        &runtime_package,
    )
    .expect("runtime build");
    let runtime_state = EffectiveRuntimeState::new(
        &runtime_build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("snapshot"),
            launch_policy_digest: digest("launch"),
            loaded_components_digest: digest("components"),
            effective_configuration_digest: digest("configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("runtime class"),
            isolation_policy_digest: digest("isolation"),
            effective_context_tokens: 8_192,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
        },
    )
    .expect("runtime state");
    let model_artifact_id = artifact("model");
    let model_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("model license"), 10, path("legal/license.txt")),
        ArtifactSetMember::new(
            artifact("model provenance"),
            11,
            path("legal/provenance.txt"),
        ),
        ArtifactSetMember::new(model_artifact_id.clone(), 12, path("model/model.gguf")),
    ])
    .expect("model set");
    let model_package = ModelPackageManifest::new(
        &model_set,
        "gguf",
        1,
        source("model"),
        PackageTransformation::Untransformed {
            evidence_digest: digest("model unchanged"),
        },
        vec![
            ModelPackageMember::new(
                artifact("model license"),
                10,
                path("legal/license.txt"),
                vec![ModelPackageMemberRole::LicenseText],
            ),
            ModelPackageMember::new(
                artifact("model provenance"),
                11,
                path("legal/provenance.txt"),
                vec![ModelPackageMemberRole::ProvenanceRecord],
            ),
            ModelPackageMember::new(
                model_artifact_id.clone(),
                12,
                path("model/model.gguf"),
                vec![ModelPackageMemberRole::ModelWeights],
            ),
        ],
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        vec![
            embedded_component(
                EmbeddedModelComponentPurpose::ModelConfiguration,
                "general.architecture",
                "model config",
            ),
            embedded_component(
                EmbeddedModelComponentPurpose::GenerationConfiguration,
                "generation.defaults",
                "generation config",
            ),
            embedded_component(
                EmbeddedModelComponentPurpose::Tokenizer,
                "tokenizer.ggml",
                "tokenizer",
            ),
            embedded_component(
                EmbeddedModelComponentPurpose::PromptTemplate,
                "tokenizer.chat_template",
                "template",
            ),
        ],
    )
    .expect("model package");
    let effective_package = EffectivePackageEvidenceV2::new(
        &model_set,
        &runtime_build,
        &runtime_state,
        EffectivePackageEvidenceV2Input {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: "managed-package".to_owned(),
            evidence_contract_schema_version: 2,
            member_evidence: vec![
                member_evidence(
                    "legal/license.txt",
                    "model license",
                    10,
                    EffectivePackageMemberUseV2::EvidenceOnly {
                        roles: vec![EffectivePackageEvidenceRoleV2::LicenseText],
                    },
                ),
                member_evidence(
                    "legal/provenance.txt",
                    "model provenance",
                    11,
                    EffectivePackageMemberUseV2::EvidenceOnly {
                        roles: vec![EffectivePackageEvidenceRoleV2::ProvenanceRecord],
                    },
                ),
                member_evidence(
                    "model/model.gguf",
                    "model",
                    12,
                    EffectivePackageMemberUseV2::Effective {
                        purposes: vec![
                            EffectivePackageMemberPurpose::ModelWeights,
                            EffectivePackageMemberPurpose::ModelConfiguration,
                            EffectivePackageMemberPurpose::TokenizerModel,
                            EffectivePackageMemberPurpose::PromptTemplate,
                        ],
                    },
                ),
            ],
            artifact_set_completeness_evidence_digest: digest("complete"),
            acquisition_evidence_digest: digest("acquisition"),
            license_review_evidence_digest: digest("license"),
            transformation: PackageTransformationDisposition::Untransformed {
                evidence_digest: digest("binding"),
            },
            runtime_load_closure_evidence_digest: digest("closure"),
            exclusion_isolation_evidence_digest: digest("exclusion"),
        },
    )
    .expect("effective package");
    SystemFixture {
        runtime_set,
        runtime_package,
        runtime_build,
        runtime_state,
        model_set,
        model_package,
        model_artifact_id,
        effective_package,
    }
}

fn artifact(label: &str) -> ArtifactId {
    ArtifactId::from_digest(digest(label))
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
