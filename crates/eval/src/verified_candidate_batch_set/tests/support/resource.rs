use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1, GenerationCaseManifestV1,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPlanV1, GenerationRepetitionRecordV1,
    GenerationResourceAttemptResultRecordV1, GenerationResourceAttemptResultRecordV1Input,
    GenerationResourceAttemptResultRecordV1Relations, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the resource fixture keeps every exact result relationship explicit"
)]
pub(super) fn qualification_resource_result(
    plan: &GenerationQualificationPlanV1,
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    repetition: &GenerationRepetitionRecordV1,
    planned_attempt: &PlannedCandidateAttemptV1,
    system: &GenerationSystemRecordV1,
    operation_policy: &GenerationQualificationOperationPolicyV1,
    attempt_record: &CandidateGenerationAttemptRecordV1,
    receipt: &CandidateGenerationReceiptV1,
) -> GenerationResourceAttemptResultRecordV1 {
    GenerationResourceAttemptResultRecordV1::new(
        GenerationResourceAttemptResultRecordV1Relations {
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: system,
                qualification_plan: plan,
                suite,
            },
            operation_policy,
            case,
            repetition,
            planned_attempt,
            attempt_record,
            candidate_generation_receipt: receipt,
        },
        GenerationResourceAttemptResultRecordV1Input {
            prompt_token_count: 3,
            generated_token_count: 5,
            total_duration_nanoseconds: 12_000,
            load_duration_nanoseconds: 1_000,
            prompt_evaluation_duration_nanoseconds: 2_000,
            evaluation_duration_nanoseconds: 8_000,
            attempt_elapsed_nanoseconds: 20_000,
            first_response_elapsed_nanoseconds: 5_000,
            cleanup_elapsed_nanoseconds: 1_000,
            worker_high_water_resident_bytes: 8_192,
            runtime_installed_payload_bytes: 100,
            model_installed_payload_bytes: 200,
            installed_footprint_bytes: 300,
            exceeded_limits: Vec::new(),
        },
    )
    .expect("qualification resource result")
}

impl super::Scenario {
    pub(crate) fn attach_resource_observations(
        &mut self,
        foundation: rewrite_model_store::GenerationQualificationPlanFoundationV1Input<'_>,
        system: &GenerationSystemRecordV1,
        operation_policy: &GenerationQualificationOperationPolicyV1,
        exceeded: bool,
    ) {
        for batch in &mut self.batches {
            let planned = foundation
                .planned_attempts
                .iter()
                .find(|attempt| attempt.planned_attempt_id() == batch.receipt.planned_attempt_id())
                .expect("exact retained planned attempt");
            let case = foundation
                .cases
                .iter()
                .find(|case| case.case_id() == planned.case_id())
                .expect("exact resource case");
            let repetition = foundation
                .repetitions
                .iter()
                .find(|rep| rep.repetition_id() == planned.repetition_id())
                .expect("exact resource repetition");
            let record = qualification_resource_result(
                foundation.plan,
                foundation.suite,
                case,
                repetition,
                planned,
                system,
                operation_policy,
                &batch.attempt_record,
                &batch.receipt,
            );
            let mut input = GenerationResourceAttemptResultRecordV1Input {
                prompt_token_count: record.prompt_token_count(),
                generated_token_count: record.generated_token_count(),
                total_duration_nanoseconds: record.total_duration_nanoseconds(),
                load_duration_nanoseconds: record.load_duration_nanoseconds(),
                prompt_evaluation_duration_nanoseconds: record
                    .prompt_evaluation_duration_nanoseconds(),
                evaluation_duration_nanoseconds: record.evaluation_duration_nanoseconds(),
                attempt_elapsed_nanoseconds: record.attempt_elapsed_nanoseconds(),
                first_response_elapsed_nanoseconds: record.first_response_elapsed_nanoseconds(),
                cleanup_elapsed_nanoseconds: record.cleanup_elapsed_nanoseconds(),
                worker_high_water_resident_bytes: record.worker_high_water_resident_bytes(),
                runtime_installed_payload_bytes: record.runtime_installed_payload_bytes(),
                model_installed_payload_bytes: record.model_installed_payload_bytes(),
                installed_footprint_bytes: record.installed_footprint_bytes(),
                exceeded_limits: record.exceeded_limits().to_vec(),
            };
            if exceeded {
                input.attempt_elapsed_nanoseconds = 30_000_000_001;
                input.exceeded_limits =
                    vec![rewrite_model::GenerationResourceExceededLimitV1::AttemptElapsed];
            }
            batch.resource_result = Some(
                GenerationResourceAttemptResultRecordV1::new(
                    GenerationResourceAttemptResultRecordV1Relations {
                        scope: GenerationQualificationPhaseScopeV1 {
                            generation_system: system,
                            qualification_plan: foundation.plan,
                            suite: foundation.suite,
                        },
                        operation_policy,
                        case,
                        repetition,
                        planned_attempt: planned,
                        attempt_record: &batch.attempt_record,
                        candidate_generation_receipt: &batch.receipt,
                    },
                    input,
                )
                .expect("synthetic exact resource observations"),
            );
        }
    }
}
