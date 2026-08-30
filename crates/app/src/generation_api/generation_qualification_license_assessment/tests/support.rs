use rewrite_model::{
    CandidateOutputCeilingsV1, GenerationQualificationDecisionRuleV1,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationOperationLimitsV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationQualificationPlatformAssessmentPolicyId,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations, GenerationSystemRecordV1,
    GenerationSystemRecordV1Relations, ModelLicenseControlId, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
    StructuredCompletionRequestBindingId, generation_qualification_plan_failure_policy_digest,
};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;

use crate::candidate_attempt_precursor::tests::support::{
    Fixture, QualificationFixture, bindings_with_strategy, launch, system,
};
use crate::{
    GenerationQualificationAssessmentPolicyVerifier, GenerationQualificationLicenseAssessmentInput,
    ModelLicenseControlCompiler, ModelLicenseControlVerifier, ModelLicensePermission,
    ProductionGenerationQualificationLicenseAssessmentPolicySource,
    ProductionModelLicenseApprovalPolicy, VerifiedGenerationQualificationLicenseAssessmentPolicy,
    VerifiedModelLicenseControl,
};

pub(super) struct StaticFixture {
    pub(super) qualification: QualificationFixture,
    pub(super) systems: Vec<GenerationSystemRecordV1>,
    pub(super) attempts: Vec<PlannedCandidateAttemptV1>,
    pub(super) plan: GenerationQualificationPlanV1,
    pub(super) operation_policy_input: GenerationQualificationOperationPolicyV1Input,
    pub(super) operation_policy: GenerationQualificationOperationPolicyV1,
    pub(super) projection_inputs: Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    pub(super) projection: GenerationQualificationRequestProjectionV1,
    pub(super) assessment_policy: VerifiedGenerationQualificationLicenseAssessmentPolicy,
}

impl StaticFixture {
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture constructs the exact operation, projection, and assessment closure"
    )]
    pub(super) fn new(
        live: &Fixture,
        assessment_policy: VerifiedGenerationQualificationLicenseAssessmentPolicy,
        variant: &str,
    ) -> Self {
        let launch_plan = launch(&live.model_lease, "v1");
        let qualification = QualificationFixture::new(live, &launch_plan);
        let output_schema = qualification
            .planned_attempt
            .candidate_output_contract_digest()
            .clone();
        let baseline_bindings =
            bindings_with_strategy(output_schema, &format!("baseline strategy {variant}"));
        let baseline = system(
            live,
            &launch_plan,
            &qualification.expected_runtime_state,
            &qualification.characterized_package,
            &qualification.static_model,
            &baseline_bindings,
        );
        let mut systems = vec![qualification.generation_system.clone(), baseline];
        systems.sort_by(|left, right| {
            left.generation_system_id()
                .digest()
                .as_str()
                .cmp(right.generation_system_id().digest().as_str())
        });

        let output_ceilings = qualification.output_ceilings;
        let attempts = systems
            .iter()
            .enumerate()
            .map(|(index, generation_system)| {
                PlannedCandidateAttemptV1::new(
                    PlannedCandidateAttemptV1Relations {
                        suite: &qualification.suite,
                        case: &qualification.case,
                        cluster: &qualification.cluster,
                        repetition: &qualification.repetition,
                        generation_system,
                    },
                    PlannedCandidateAttemptV1Input {
                        attempt_ordinal: u32::try_from(index).expect("bounded ordinal"),
                        declared_seed: u64::try_from(index).expect("bounded seed") + 7,
                        grounded_request_digest: digest(&format!(
                            "grounded request {variant} {index}"
                        )),
                        generation_request_binding_id: qualification
                            .generation_request
                            .generation_request_binding_id(),
                        candidate_output_contract_digest: qualification
                            .planned_attempt
                            .candidate_output_contract_digest()
                            .clone(),
                        output_ceilings,
                    },
                )
                .expect("planned attempt")
            })
            .collect::<Vec<_>>();

        let policy_input = operation_policy_input(
            assessment_policy.policy_id().clone(),
            output_ceilings,
            variant,
        );
        let failure_policy_digest = generation_qualification_plan_failure_policy_digest(
            policy_input.decision_rule,
            &policy_input.platform_assessment_policy_id,
            policy_input.required_license_permission,
            &policy_input.license_assessment_policy_id,
            &policy_input.attempt_ledger_policy_digest,
            &policy_input.repeatability_policy_digest,
            &policy_input.resource_policy_digest,
            &policy_input.human_adjudication_policy_digest,
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
                    output_ceilings.maximum_envelope_bytes(),
                )
                .expect("plan limits"),
                selection_policy_digest: digest(&format!("selection {variant}")),
                failure_policy_digest,
            },
        )
        .expect("qualification plan");
        let relations = operation_relations(live, &qualification, &systems, &attempts, &plan);
        let operation_policy =
            GenerationQualificationOperationPolicyV1::new(relations, policy_input.clone())
                .expect("operation policy");
        let projection_inputs = projection_inputs(&attempts);
        let projection = GenerationQualificationRequestProjectionV1::new(
            GenerationQualificationRequestProjectionV1Relations {
                operation_policy: &operation_policy,
                qualification_plan: &plan,
                suite: &qualification.suite,
                planned_attempts: &attempts,
            },
            &projection_inputs,
        )
        .expect("request projection");
        Self {
            qualification,
            systems,
            attempts,
            plan,
            operation_policy_input: policy_input,
            operation_policy,
            projection_inputs,
            projection,
            assessment_policy,
        }
    }

    pub(super) fn operation_relations<'a>(
        &'a self,
        live: &'a Fixture,
    ) -> GenerationQualificationOperationPolicyV1Relations<'a> {
        operation_relations(
            live,
            &self.qualification,
            &self.systems,
            &self.attempts,
            &self.plan,
        )
    }

    pub(super) fn target_relations<'a>(
        &'a self,
        live: &'a Fixture,
    ) -> GenerationSystemRecordV1Relations<'a> {
        system_relations(live, &self.qualification)
    }

    pub(super) fn target(&self) -> &GenerationSystemRecordV1 {
        &self.systems[0]
    }

    pub(super) fn baseline(&self) -> &GenerationSystemRecordV1 {
        &self.systems[1]
    }
}

pub(super) fn compiler_input<'records, 'proof, 'lease>(
    static_fixture: &'records StaticFixture,
    live: &'records Fixture,
    proof: &'proof VerifiedModelLicenseControl<'lease>,
    selected_lease: &'lease crate::VerifiedManagedOllamaModelPackageLease,
    production: &'records ProductionModelLicenseApprovalPolicy,
) -> GenerationQualificationLicenseAssessmentInput<'records, 'proof, 'lease> {
    GenerationQualificationLicenseAssessmentInput {
        operation_policy: &static_fixture.operation_policy,
        operation_policy_relations: static_fixture.operation_relations(live),
        operation_policy_input: &static_fixture.operation_policy_input,
        request_projection: &static_fixture.projection,
        request_projection_relations: GenerationQualificationRequestProjectionV1Relations {
            operation_policy: &static_fixture.operation_policy,
            qualification_plan: &static_fixture.plan,
            suite: &static_fixture.qualification.suite,
            planned_attempts: &static_fixture.attempts,
        },
        request_projection_entry_inputs: &static_fixture.projection_inputs,
        target_generation_system: static_fixture.target(),
        target_generation_system_relations: static_fixture.target_relations(live),
        assessment_policy: &static_fixture.assessment_policy,
        model_license_control: proof,
        selected_model_package_lease: selected_lease,
        production_approval_policy: production,
    }
}

pub(super) fn structural_control(
    lease: &crate::VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
) -> VerifiedModelLicenseControl<'_> {
    let review = ModelLicenseControlCompiler::reviewer_json_for_test(lease, permission);
    let compiled =
        ModelLicenseControlCompiler::compile(lease, permission, &review, &CancellationToken::new())
            .expect("compile structural control");
    ModelLicenseControlVerifier::verify_structural(
        compiled.canonical_bytes(),
        lease,
        permission,
        &CancellationToken::new(),
    )
    .expect("verify structural control")
}

pub(super) fn assessment_policy(
    control_id: &ModelLicenseControlId,
    approved: bool,
) -> VerifiedGenerationQualificationLicenseAssessmentPolicy {
    let bytes = serde_json::to_vec(&LicensePolicy {
        authority: Authority::None,
        policy: LicensePolicyKind::GenerationQualificationLicenseAssessment,
        decision_rule: LicenseDecisionRule::ExactControlAndProductionApproval,
        model_license_control_id: control_id.digest(),
        permission: LicensePermission::LocalGeneration,
        procedure_id: crate::GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_ID,
        procedure_version: crate::GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_VERSION,
        schema_version: crate::GENERATION_QUALIFICATION_ASSESSMENT_POLICY_SCHEMA_VERSION,
    })
    .expect("assessment policy JSON");
    let denied = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &bytes,
        control_id,
        &ProductionGenerationQualificationLicenseAssessmentPolicySource::new(),
    )
    .expect("verified denied assessment policy");
    if approved {
        let source =
            ProductionGenerationQualificationLicenseAssessmentPolicySource::exact_test_source(
                denied.policy_id().clone(),
            );
        GenerationQualificationAssessmentPolicyVerifier::verify_license(&bytes, control_id, &source)
            .expect("verified approved assessment policy")
    } else {
        denied
    }
}

fn operation_policy_input(
    license_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    output: CandidateOutputCeilingsV1,
    variant: &str,
) -> GenerationQualificationOperationPolicyV1Input {
    GenerationQualificationOperationPolicyV1Input {
        limits: GenerationQualificationOperationLimitsV1::new(
            4_096,
            4_096,
            2_048,
            256,
            output.maximum_envelope_bytes(),
            output.candidate_count(),
            output.maximum_candidate_bytes(),
            output.maximum_aggregate_candidate_bytes(),
            2,
            1,
            60_000,
        )
        .expect("operation limits"),
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id:
            GenerationQualificationPlatformAssessmentPolicyId::from_canonical_policy_bytes(
                format!(r#"{{"policy":"platform {variant}"}}"#).as_bytes(),
            )
            .expect("platform policy ID"),
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id: license_policy_id,
        attempt_ledger_policy_digest: digest(&format!("attempt ledger {variant}")),
        repeatability_policy_digest: digest(&format!("repeatability {variant}")),
        resource_policy_digest: digest(&format!("resource {variant}")),
        human_adjudication_policy_digest: digest(&format!("adjudication {variant}")),
    }
}

fn operation_relations<'a>(
    live: &'a Fixture,
    qualification: &'a QualificationFixture,
    systems: &'a [GenerationSystemRecordV1],
    attempts: &'a [PlannedCandidateAttemptV1],
    plan: &'a GenerationQualificationPlanV1,
) -> GenerationQualificationOperationPolicyV1Relations<'a> {
    GenerationQualificationOperationPolicyV1Relations {
        suite: &qualification.suite,
        plan,
        repetitions: std::slice::from_ref(&qualification.repetition),
        planned_attempts: attempts,
        target_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &systems[0],
            relations: system_relations(live, qualification),
        },
        baseline_system: GenerationQualificationOperationSystemRelationsV1 {
            generation_system: &systems[1],
            relations: system_relations(live, qualification),
        },
    }
}

fn system_relations<'a>(
    live: &'a Fixture,
    qualification: &'a QualificationFixture,
) -> GenerationSystemRecordV1Relations<'a> {
    let model = live.model_lease.private_view();
    GenerationSystemRecordV1Relations {
        runtime_package_manifest: &live.runtime.runtime_manifest,
        runtime_build: &live.runtime.runtime_build,
        effective_runtime_state: &qualification.expected_runtime_state,
        model_artifact_set: model.artifact_set_manifest(),
        model_package_manifest: model.model_package_manifest(),
        effective_package_evidence_v2: qualification.characterized_package.evidence(),
    }
}

fn projection_inputs(
    attempts: &[PlannedCandidateAttemptV1],
) -> Vec<GenerationQualificationRequestProjectionEntryV1Input> {
    attempts
        .iter()
        .enumerate()
        .map(
            |(index, attempt)| GenerationQualificationRequestProjectionEntryV1Input {
                structured_completion_request_binding_id:
                    StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                        "structured request {index}"
                    ))),
                complete_input_byte_count: attempt.source_byte_count(),
                context_token_limit: 2_048,
                output_token_limit: 256,
            },
        )
        .collect()
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Authority {
    None,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum LicensePolicyKind {
    GenerationQualificationLicenseAssessment,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum LicenseDecisionRule {
    ExactControlAndProductionApproval,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum LicensePermission {
    LocalGeneration,
}

#[derive(Serialize)]
struct LicensePolicy<'a> {
    authority: Authority,
    policy: LicensePolicyKind,
    decision_rule: LicenseDecisionRule,
    model_license_control_id: &'a Digest,
    permission: LicensePermission,
    procedure_id: &'static str,
    procedure_version: u32,
    schema_version: u32,
}
