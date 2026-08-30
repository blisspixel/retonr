use rewrite_model::{
    ArtifactId, CandidateJudgeCaseV1, CandidateJudgeLimitsV1, CandidateJudgeOrderPolicyV1,
    CandidateJudgePlanV1, CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations,
    CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1, CandidateOutputCeilingsV1,
    CandidateReceiptPairSetId, CandidateSelectionPolicyV1, GenerationCaseManifestV1,
    GenerationCaseManifestV1Input, GenerationClusterRecordV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationRepetitionRecordV1, GenerationSuiteManifestV1, GenerationSystemRecordV1,
    PlannedCandidateAttemptV1, PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
    StructuredCompletionRequestBindingId,
};
use rewrite_types::Digest;

use crate::candidate_attempt_precursor::tests::support::{
    Fixture, bindings_with_strategy, characterized, launch, system, verified_policy,
};
use crate::{
    GenerationSystemPolicyBindingsV1, GenerationSystemPolicyPermission,
    GenerationSystemPolicyPurpose, ManagedJudgePrecursorCompilationInput,
    ReleasedGenerationEffectivePackageV2, StaticModelInterpretationV1,
    VerifiedGenerationSystemPolicy,
};

pub(crate) struct JudgeFixture {
    pub(crate) live: Fixture,
    pub(crate) characterized: ReleasedGenerationEffectivePackageV2,
    pub(crate) static_model: StaticModelInterpretationV1,
    pub(super) policy_bindings: GenerationSystemPolicyBindingsV1,
    pub(super) policy: VerifiedGenerationSystemPolicy,
    pub(crate) judge_system: GenerationSystemRecordV1,
    candidate_systems: Vec<GenerationSystemRecordV1>,
    suite: GenerationSuiteManifestV1,
    repetition: GenerationRepetitionRecordV1,
    selection: CandidateSelectionPolicyV1,
    planned: Vec<PlannedCandidateAttemptV1>,
    qualification_plan: GenerationQualificationPlanV1,
    pub(crate) judge_plan: CandidateJudgePlanV1,
    pub(crate) schedule: CandidateJudgeScheduleV1,
    pub(crate) requests: CandidateJudgeRequestAggregateV1,
}

impl JudgeFixture {
    pub(crate) fn new() -> Self {
        let SystemFixture {
            live,
            characterized,
            static_model,
            policy_bindings,
            policy,
            judge_system,
            candidate_systems: systems,
        } = system_fixture();
        let QualificationMaterial {
            cluster,
            case,
            suite,
            repetition,
        } = qualification_material(&policy_bindings);
        let planned = systems
            .iter()
            .enumerate()
            .map(|(index, value)| attempt(&suite, &case, &cluster, &repetition, value, index))
            .collect::<Vec<_>>();
        let selection = CandidateSelectionPolicyV1::new(&suite, &[0]).expect("selection policy");
        let qualification_plan = GenerationQualificationPlanV1::new(
            &suite,
            std::slice::from_ref(&repetition),
            &systems,
            &planned,
            GenerationQualificationPlanV1Input {
                limits: GenerationQualificationPlanLimitsV1::new(1, 2, 4_096, 16, 256, 1_048_576)
                    .expect("qualification limits"),
                selection_policy_digest: selection.selection_policy_id().digest().clone(),
                failure_policy_digest: digest("judge failure policy"),
            },
        )
        .expect("qualification plan");
        let relations = CandidateJudgePlanV1Relations {
            qualification_plan: &qualification_plan,
            suite: &suite,
            repetition: &repetition,
            selection_policy: &selection,
            planned_attempts: &planned,
            candidate_a_system: &systems[0],
            candidate_b_system: &systems[1],
            judge_system: &judge_system,
        };
        let judge_plan = CandidateJudgePlanV1::new(
            relations,
            CandidateJudgePlanV1Input {
                case_material_set_digest: digest("judge case material"),
                rubric_digest: digest("judge rubric"),
                cases: vec![
                    CandidateJudgeCaseV1::new(case.case_id().clone(), vec!["meaning".to_owned()])
                        .expect("judge case contract"),
                ],
                order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
                presentation_seed: 42,
                attempts_per_order: 1,
                limits: CandidateJudgeLimitsV1::new(
                    1, 1_024, 1_024, 4_096, 4_096, 256, 4_096, 10_000,
                )
                .expect("judge limits"),
                prompt_contract_digest: judge_system.prompt_digest().clone(),
                output_schema_digest: judge_system.output_schema_digest().clone(),
            },
        )
        .expect("judge plan");
        let pair_set = pair_set_id("candidate receipt pair");
        let schedule =
            CandidateJudgeScheduleV1::new(&judge_plan, &pair_set).expect("judge schedule");
        let request_ids = (0..schedule.entries().len())
            .map(|index| {
                StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                    "judge request {index}"
                )))
            })
            .collect();
        let requests = CandidateJudgeRequestAggregateV1::new(&judge_plan, &schedule, request_ids)
            .expect("judge requests");
        Self {
            live,
            characterized,
            static_model,
            policy_bindings,
            policy,
            judge_system,
            candidate_systems: systems,
            suite,
            repetition,
            selection,
            planned,
            qualification_plan,
            judge_plan,
            schedule,
            requests,
        }
    }

    pub(crate) fn plan_with_presentation_seed(
        &self,
        presentation_seed: u64,
    ) -> CandidateJudgePlanV1 {
        CandidateJudgePlanV1::new(
            CandidateJudgePlanV1Relations {
                qualification_plan: &self.qualification_plan,
                suite: &self.suite,
                repetition: &self.repetition,
                selection_policy: &self.selection,
                planned_attempts: &self.planned,
                candidate_a_system: &self.candidate_systems[0],
                candidate_b_system: &self.candidate_systems[1],
                judge_system: &self.judge_system,
            },
            CandidateJudgePlanV1Input {
                case_material_set_digest: self.judge_plan.case_material_set_digest().clone(),
                rubric_digest: self.judge_plan.rubric_digest().clone(),
                cases: self.judge_plan.cases().to_vec(),
                order_policy: self.judge_plan.order_policy(),
                presentation_seed,
                attempts_per_order: self.judge_plan.attempts_per_order(),
                limits: self.judge_plan.limits(),
                prompt_contract_digest: self.judge_plan.prompt_contract_digest().clone(),
                output_schema_digest: self.judge_plan.output_schema_digest().clone(),
            },
        )
        .expect("alternate judge plan")
    }
}

struct SystemFixture {
    live: Fixture,
    characterized: ReleasedGenerationEffectivePackageV2,
    static_model: StaticModelInterpretationV1,
    policy_bindings: GenerationSystemPolicyBindingsV1,
    policy: VerifiedGenerationSystemPolicy,
    judge_system: GenerationSystemRecordV1,
    candidate_systems: Vec<GenerationSystemRecordV1>,
}

fn system_fixture() -> SystemFixture {
    let live = Fixture::new();
    let launch_plan = launch(&live.model_lease, "latest");
    let expected_state = live.runtime.runtime_state.clone();
    let characterized = characterized(&live, &launch_plan, &expected_state);
    let static_model =
        StaticModelInterpretationV1::derive(&launch_plan).expect("static judge model");
    let output_schema = digest("judge output schema");
    let bindings_a = bindings_with_strategy(output_schema.clone(), "candidate a");
    let bindings_b = bindings_with_strategy(output_schema.clone(), "candidate b");
    let policy_bindings = bindings_with_strategy(output_schema, "judge");
    let mut candidate_systems = vec![
        system(
            &live,
            &launch_plan,
            &expected_state,
            &characterized,
            &static_model,
            &bindings_a,
        ),
        system(
            &live,
            &launch_plan,
            &expected_state,
            &characterized,
            &static_model,
            &bindings_b,
        ),
    ];
    candidate_systems.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    let judge_system = system(
        &live,
        &launch_plan,
        &expected_state,
        &characterized,
        &static_model,
        &policy_bindings,
    );
    let policy = verified_policy(
        &policy_bindings,
        GenerationSystemPolicyPermission::ConstructGenerationSystem,
        GenerationSystemPolicyPurpose::ManagedJudgeGeneration,
    );
    SystemFixture {
        live,
        characterized,
        static_model,
        policy_bindings,
        policy,
        judge_system,
        candidate_systems,
    }
}

struct QualificationMaterial {
    cluster: GenerationClusterRecordV1,
    case: GenerationCaseManifestV1,
    suite: GenerationSuiteManifestV1,
    repetition: GenerationRepetitionRecordV1,
}

fn qualification_material(
    policy_bindings: &GenerationSystemPolicyBindingsV1,
) -> QualificationMaterial {
    let cluster = GenerationClusterRecordV1::new("judge-case", digest("cluster policy"))
        .expect("judge cluster");
    let source = b"judge source";
    let source_digest = Digest::sha256(source);
    let case = GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: "judge-case".to_owned(),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count: source.len() as u64,
            case_contract_digest: digest("case contract"),
            language_digest: policy_bindings.language_digest().clone(),
            mode_digest: policy_bindings.mode_digest().clone(),
            format_digest: policy_bindings.format_digest().clone(),
        },
    )
    .expect("judge case");
    let suite =
        GenerationSuiteManifestV1::new(digest("judge suite protocol"), std::slice::from_ref(&case))
            .expect("judge suite");
    let repetition = GenerationRepetitionRecordV1::new(&suite, 0, digest("judge repetition"))
        .expect("judge repetition");
    QualificationMaterial {
        cluster,
        case,
        suite,
        repetition,
    }
}

pub(crate) fn portable_judge_plan_schedules() -> (
    CandidateJudgePlanV1,
    CandidateJudgeScheduleV1,
    CandidateJudgePlanV1,
    CandidateJudgeScheduleV1,
) {
    let fixture = JudgeFixture::new();
    let alternate_plan = fixture.plan_with_presentation_seed(43);
    let alternate_schedule = CandidateJudgeScheduleV1::new(
        &alternate_plan,
        fixture.schedule.candidate_receipt_pair_set_id(),
    )
    .expect("alternate judge schedule");
    (
        fixture.judge_plan,
        fixture.schedule,
        alternate_plan,
        alternate_schedule,
    )
}

pub(super) fn compilation_input(
    fixture: &mut JudgeFixture,
) -> ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_> {
    ManagedJudgePrecursorCompilationInput {
        judge_plan: &fixture.judge_plan,
        judge_schedule: &fixture.schedule,
        request_aggregate: &fixture.requests,
        judge_system: &fixture.judge_system,
        runtime_manifest: &fixture.live.runtime.runtime_manifest,
        runtime_package: &mut fixture.live.runtime.runtime_package,
        runtime_build: &fixture.live.runtime.runtime_build,
        admitted_runtime: &fixture.live.runtime.admitted,
        generation_path: &fixture.live.runtime.path,
        frozen_components: &fixture.live.runtime.frozen,
        prepared_isolation: &fixture.live.runtime.prepared_isolation,
        launch_plan: launch(&fixture.live.model_lease, "latest"),
        characterized_package: &fixture.characterized,
        expected_runtime_state: &fixture.live.runtime.runtime_state,
        static_model: &fixture.static_model,
        generation_policy: &fixture.policy,
    }
}

fn attempt(
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    system: &GenerationSystemRecordV1,
    index: usize,
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
            attempt_ordinal: u32::try_from(index).expect("attempt ordinal"),
            declared_seed: u64::try_from(index).expect("attempt seed"),
            grounded_request_digest: digest(&format!("grounded request {index}")),
            generation_request_binding_id:
                rewrite_model::GenerationRequestBindingId::from_derived_digest(digest(&format!(
                    "generation request {index}"
                ))),
            candidate_output_contract_digest: system.output_schema_digest().clone(),
            output_ceilings: CandidateOutputCeilingsV1::new(1, 1_024, 1_024)
                .expect("candidate ceilings"),
        },
    )
    .expect("planned attempt")
}

pub(super) fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}

pub(super) fn pair_set_id(value: &str) -> CandidateReceiptPairSetId {
    serde_json::from_value(serde_json::json!(digest(value))).expect("inert pair-set test identity")
}
