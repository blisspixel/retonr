use rewrite_model::{
    ArtifactId, CandidateOutputCeilingsV1, CandidateSelectionPolicyV1, GenerationCaseManifestV1,
    GenerationCaseManifestV1Input, GenerationCaseRequestProfileV1, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1, GenerationDeterministicCaseContractV1Input,
    GenerationQualificationDecisionRuleV1, GenerationQualificationLicenseAssessmentPolicyId,
    GenerationQualificationLicensePermissionV1, GenerationQualificationOperationLimitsV1,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationQualificationPlatformAssessmentPolicyId, GenerationRepetitionRecordV1,
    GenerationSuiteManifestV1, GenerationSystemRecordV1, GenerationSystemRecordV1Relations,
    PlannedCandidateAttemptV1, PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
    ReferenceJudgment, generation_qualification_plan_failure_policy_digest,
};
use rewrite_types::{CancellationToken, Digest, RewriteMode, RewriteStatus};

use super::{
    GenerationQualificationRequestBuilderBindingsV1, GenerationQualificationRequestBuilderV1,
    GenerationQualificationRequestBuilderV1Input, PreplannedGenerationQualificationRequestFactsV1,
};
#[cfg(all(feature = "test-support", not(test)))]
use crate::candidate_attempt_precursor::synthetic_test_support::{
    Fixture as RuntimeFixture, characterized, launch, system, verified_policy,
};
#[cfg(test)]
use crate::candidate_attempt_precursor::tests::support::{
    Fixture as RuntimeFixture, characterized, launch, system, verified_policy,
};
use crate::{
    GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose,
    ReleasedGenerationEffectivePackageV2,
};

#[path = "support/components.rs"]
mod components;
pub(crate) use components::{bindings, bindings_with_platform_digests, digest, policy_for};

#[path = "support/source.rs"]
mod source;
pub(crate) use source::{SourceFixture, source_fixture};

pub(crate) const SOURCE: &[u8] = b"Retain Acme 42 exactly.";

pub(crate) struct Fixture {
    pub(crate) runtime: RuntimeFixture,
    pub(crate) model_set: rewrite_model::ArtifactSetManifest,
    pub(crate) model_package: rewrite_model::ModelPackageManifest,
    pub(crate) characterized: ReleasedGenerationEffectivePackageV2,
    pub(crate) builder: GenerationQualificationRequestBuilderV1,
    pub(crate) baseline_builder: GenerationQualificationRequestBuilderV1,
    pub(crate) cluster: GenerationClusterRecordV1,
    #[cfg_attr(
        not(feature = "test-support"),
        expect(
            dead_code,
            reason = "Used by the cross-crate synthetic fixture with test-support"
        )
    )]
    pub(crate) deterministic_contract: GenerationDeterministicCaseContractV1,
    pub(crate) case: GenerationCaseManifestV1,
    pub(crate) suite: GenerationSuiteManifestV1,
    pub(crate) repetition: GenerationRepetitionRecordV1,
    pub(crate) source: SourceFixture,
}

impl Fixture {
    pub(crate) fn new(source: &[u8], protected_terms: Vec<String>) -> Self {
        Self::new_with_platform_digests(
            source,
            protected_terms,
            digest("target operating system"),
            digest("target architecture"),
            digest("target execution class"),
            digest("target hardware envelope"),
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the fixture assembles one complete cross-record trust closure"
    )]
    pub(crate) fn new_with_platform_digests(
        source: &[u8],
        protected_terms: Vec<String>,
        operating_system_digest: Digest,
        architecture_digest: Digest,
        execution_class_digest: Digest,
        hardware_envelope_digest: Digest,
    ) -> Self {
        let protected_terms = if protected_terms.is_empty() {
            vec!["Acme".to_owned()]
        } else {
            protected_terms
        };
        let runtime = RuntimeFixture::new();
        let launch_plan = launch(&runtime.model_lease, "v1");
        let state = runtime.runtime.runtime_state.clone();
        let characterized = characterized(&runtime, &launch_plan, &state);
        let static_model = crate::StaticModelInterpretationV1::derive(&launch_plan)
            .expect("static model interpretation");
        let profile = GenerationCaseRequestProfileV1::new(RewriteMode::Pure);
        let validator = digest("reviewed validator equality binding");
        let adapter = digest("reviewed adapter equality binding");
        let components =
            GenerationQualificationRequestBuilderBindingsV1::derive(&profile, &validator, &adapter)
                .expect("builder bindings");
        let target_bindings = bindings_with_platform_digests(
            &components,
            validator.clone(),
            adapter.clone(),
            operating_system_digest,
            architecture_digest,
            execution_class_digest,
            hardware_envelope_digest,
        );
        let baseline_bindings = bindings(&components, validator, adapter, "baseline");
        let target_system = system(
            &runtime,
            &launch_plan,
            &state,
            &characterized,
            &static_model,
            &target_bindings,
        );
        let baseline_system = system(
            &runtime,
            &launch_plan,
            &state,
            &characterized,
            &static_model,
            &baseline_bindings,
        );
        let model_set = launch_plan.model_artifact_set_manifest().clone();
        let model_package = launch_plan.model_package_manifest().clone();
        let policy = verified_policy(
            &target_bindings,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        );
        let baseline_policy = verified_policy(
            &baseline_bindings,
            GenerationSystemPolicyPermission::ConstructGenerationSystem,
            GenerationSystemPolicyPurpose::ManagedCandidateGeneration,
        );

        let source_digest = Digest::sha256(source);
        let contract = GenerationDeterministicCaseContractV1::new(
            GenerationDeterministicCaseContractV1Input {
                case_key: "request-builder-case".to_owned(),
                source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
                source_digest: source_digest.clone(),
                source_byte_count: source.len() as u64,
                language_digest: profile.language_digest(),
                mode_digest: profile.rewrite_mode_digest(),
                format_digest: profile.format_digest(),
                evaluation_category: "request_builder".to_owned(),
                protected_terms,
                reference_judgment: ReferenceJudgment::Acceptable,
                expected_status: RewriteStatus::Rewritten,
                expected_reason: None,
                expected_output: rewrite_model::ExpectedOutput::Candidate,
                rubric_clause_ids: vec!["fidelity".to_owned()],
            },
        )
        .expect("deterministic contract");
        let cluster = GenerationClusterRecordV1::new("request-builder", digest("cluster policy"))
            .expect("cluster");
        let case = GenerationCaseManifestV1::new(
            &cluster,
            GenerationCaseManifestV1Input {
                case_key: contract.case_key().to_owned(),
                source_artifact_id: contract.source_artifact_id().clone(),
                source_digest: contract.source_digest().clone(),
                source_byte_count: contract.source_byte_count(),
                case_contract_digest: contract.contract_digest().clone(),
                language_digest: profile.language_digest(),
                mode_digest: profile.rewrite_mode_digest(),
                format_digest: profile.format_digest(),
            },
        )
        .expect("case");
        let suite =
            GenerationSuiteManifestV1::new(digest("suite protocol"), std::slice::from_ref(&case))
                .expect("suite");
        let repetition = GenerationRepetitionRecordV1::new(&suite, 0, digest("repetition policy"))
            .expect("repetition");
        let source_fixture = source_fixture(source, &case);
        let builder = GenerationQualificationRequestBuilderV1::new(
            GenerationQualificationRequestBuilderV1Input {
                case: case.clone(),
                deterministic_contract: contract.clone(),
                request_profile: profile.clone(),
                generation_system: target_system,
                generation_policy: policy,
            },
        )
        .expect("request builder");
        let baseline_builder = GenerationQualificationRequestBuilderV1::new(
            GenerationQualificationRequestBuilderV1Input {
                case: case.clone(),
                deterministic_contract: contract.clone(),
                request_profile: profile,
                generation_system: baseline_system,
                generation_policy: baseline_policy,
            },
        )
        .expect("baseline request builder");
        Self {
            runtime,
            model_set,
            model_package,
            characterized,
            builder,
            baseline_builder,
            cluster,
            deterministic_contract: contract,
            case,
            suite,
            repetition,
            source: source_fixture,
        }
    }

    pub(crate) fn plan(&mut self) -> PlanFixture {
        self.plan_with_selection("selection policy")
    }

    pub(crate) fn plan_with_selection(&mut self, selection: &str) -> PlanFixture {
        let limits = common_limits();
        let lease = self.source.acquire();
        let target_facts = self
            .builder
            .derive_preplanning(&lease, 7, limits, &CancellationToken::new())
            .expect("target preplanning")
            .facts()
            .clone();
        drop(lease);
        self.plan_from_target_facts_and_selection(7, &target_facts, selection)
    }

    pub(crate) fn plan_from_target_facts(
        &mut self,
        target_seed: u64,
        target_facts: &PreplannedGenerationQualificationRequestFactsV1,
    ) -> PlanFixture {
        self.plan_from_target_facts_and_selection(target_seed, target_facts, "selection policy")
    }

    fn plan_from_target_facts_and_selection(
        &mut self,
        target_seed: u64,
        target_facts: &PreplannedGenerationQualificationRequestFactsV1,
        selection: &str,
    ) -> PlanFixture {
        let limits = common_limits();
        let lease = self.source.acquire();
        let baseline_facts = self
            .baseline_builder
            .derive_preplanning(&lease, 11, limits, &CancellationToken::new())
            .expect("baseline preplanning")
            .facts()
            .clone();

        let target_attempt = attempt(
            0,
            target_seed,
            target_facts,
            &self.suite,
            &self.case,
            &self.cluster,
            &self.repetition,
            self.builder.generation_system(),
        );
        let baseline_attempt = attempt(
            1,
            11,
            &baseline_facts,
            &self.suite,
            &self.case,
            &self.cluster,
            &self.repetition,
            self.baseline_builder.generation_system(),
        );
        let mut attempts = vec![target_attempt, baseline_attempt];
        let mut systems = vec![
            self.builder.generation_system().clone(),
            self.baseline_builder.generation_system().clone(),
        ];
        systems.sort_unstable_by(|left, right| {
            left.generation_system_id()
                .digest()
                .as_str()
                .cmp(right.generation_system_id().digest().as_str())
        });
        let operation_input = operation_input(limits);
        let selection_policy =
            CandidateSelectionPolicyV1::new(&self.suite, &[0]).expect("selection policy");
        let failure_policy = generation_qualification_plan_failure_policy_digest(
            operation_input.decision_rule,
            &operation_input.platform_assessment_policy_id,
            operation_input.required_license_permission,
            &operation_input.license_assessment_policy_id,
            &operation_input.attempt_ledger_policy_digest,
            &operation_input.repeatability_policy_digest,
            &operation_input.resource_policy_digest,
            &operation_input.human_adjudication_policy_digest,
        );
        let plan = GenerationQualificationPlanV1::new(
            &self.suite,
            std::slice::from_ref(&self.repetition),
            &systems,
            &attempts,
            GenerationQualificationPlanV1Input {
                limits: GenerationQualificationPlanLimitsV1::new(
                    1,
                    2,
                    limits.maximum_complete_input_bytes(),
                    16,
                    256,
                    1024 * 1024,
                )
                .expect("plan limits"),
                selection_policy_digest: if selection == "selection policy" {
                    selection_policy.selection_policy_id().digest().clone()
                } else {
                    digest(selection)
                },
                failure_policy_digest: failure_policy,
            },
        )
        .expect("qualification plan");
        let relations = self.operation_relations(&plan, &attempts);
        let operation = GenerationQualificationOperationPolicyV1::new(relations, operation_input)
            .expect("operation policy");
        PlanFixture {
            target_attempt: attempts.remove(0),
            baseline_attempt: attempts.remove(0),
            selection_policy,
            plan,
            operation,
        }
    }

    fn operation_relations<'a>(
        &'a self,
        plan: &'a GenerationQualificationPlanV1,
        attempts: &'a [PlannedCandidateAttemptV1],
    ) -> GenerationQualificationOperationPolicyV1Relations<'a> {
        let target_relations = self.system_relations();
        let baseline_relations = self.system_relations();
        GenerationQualificationOperationPolicyV1Relations {
            suite: &self.suite,
            plan,
            repetitions: std::slice::from_ref(&self.repetition),
            planned_attempts: attempts,
            target_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: self.builder.generation_system(),
                relations: target_relations,
            },
            baseline_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: self.baseline_builder.generation_system(),
                relations: baseline_relations,
            },
        }
    }

    pub(crate) fn system_relations(&self) -> GenerationSystemRecordV1Relations<'_> {
        GenerationSystemRecordV1Relations {
            runtime_package_manifest: &self.runtime.runtime.runtime_manifest,
            runtime_build: &self.runtime.runtime.runtime_build,
            effective_runtime_state: &self.runtime.runtime.runtime_state,
            model_artifact_set: &self.model_set,
            model_package_manifest: &self.model_package,
            effective_package_evidence_v2: self.characterized.evidence(),
        }
    }
}

pub(crate) struct PlanFixture {
    pub(crate) target_attempt: PlannedCandidateAttemptV1,
    pub(crate) baseline_attempt: PlannedCandidateAttemptV1,
    #[cfg_attr(
        not(feature = "test-support"),
        expect(
            dead_code,
            reason = "Used by the cross-crate synthetic fixture with test-support"
        )
    )]
    pub(crate) selection_policy: CandidateSelectionPolicyV1,
    pub(crate) plan: GenerationQualificationPlanV1,
    pub(crate) operation: GenerationQualificationOperationPolicyV1,
}

pub(crate) fn common_limits() -> GenerationQualificationOperationLimitsV1 {
    let output = CandidateOutputCeilingsV1::new(1, 4_096, 4_096).expect("ceilings");
    GenerationQualificationOperationLimitsV1::new(
        64 * 1_024,
        64 * 1_024,
        2_048,
        256,
        output.maximum_envelope_bytes(),
        1,
        output.maximum_candidate_bytes(),
        output.maximum_aggregate_candidate_bytes(),
        2,
        1,
        10_000,
    )
    .expect("operation limits")
}

#[expect(
    clippy::too_many_arguments,
    reason = "the fixture exposes each exact attempt relation to substitution tests"
)]
pub(crate) fn attempt(
    ordinal: u32,
    seed: u64,
    facts: &PreplannedGenerationQualificationRequestFactsV1,
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    system: &GenerationSystemRecordV1,
) -> PlannedCandidateAttemptV1 {
    PlannedCandidateAttemptV1::new(
        PlannedCandidateAttemptV1Relations {
            suite,
            case,
            cluster,
            repetition,
            generation_system: system,
        },
        PlannedCandidateAttemptV1Input {
            attempt_ordinal: ordinal,
            declared_seed: seed,
            grounded_request_digest: facts.grounded_request_digest().clone(),
            generation_request_binding_id: facts.generation_request_binding_id().clone(),
            candidate_output_contract_digest: facts.candidate_output_contract_digest().clone(),
            output_ceilings: facts.output_ceilings(),
        },
    )
    .expect("planned attempt")
}

pub(crate) fn operation_input(
    limits: GenerationQualificationOperationLimitsV1,
) -> GenerationQualificationOperationPolicyV1Input {
    GenerationQualificationOperationPolicyV1Input {
        limits,
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id:
            GenerationQualificationPlatformAssessmentPolicyId::from_canonical_policy_bytes(
                b"platform assessment policy",
            )
            .expect("platform policy id"),
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id:
            GenerationQualificationLicenseAssessmentPolicyId::from_canonical_policy_bytes(
                b"license assessment policy",
            )
            .expect("license policy id"),
        attempt_ledger_policy_digest: digest("attempt ledger policy"),
        repeatability_policy_digest: digest("repeatability policy"),
        resource_policy_digest: digest("resource policy"),
        human_adjudication_policy_digest: digest("human adjudication policy"),
    }
}
