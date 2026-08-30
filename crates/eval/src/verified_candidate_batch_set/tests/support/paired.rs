use std::{cell::RefCell, rc::Rc};

use rewrite_model::{
    CandidateSelectionPolicyV1, EffectivePackageEvidenceV2, GenerationCaseManifestV1,
    GenerationClusterRecordV1, GenerationQualificationDecisionRuleV1,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationOperationLimitsV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationOperationSystemRelationsV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationQualificationPlatformAssessmentPolicyId, GenerationRepetitionRecordV1,
    GenerationSuiteManifestV1, GenerationSystemRecordV1, MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES, MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
    MAX_GENERATION_RETAINED_INPUT_BYTES, PlannedCandidateAttemptV1,
    generation_qualification_plan_failure_policy_digest,
};
use rewrite_types::Digest;

use super::{Scenario, digest, limits, offline_batch, planned_attempt, system_fixture};
use crate::VerifiedCandidateBatchSetInput;

pub(super) fn selected_candidate_text(case_key: &str, suffix: &str, index: usize) -> String {
    match case_key {
        "eligible-rewrite" if suffix.contains("equal-selected") => {
            "Acme, 42 needs polish!".to_owned()
        }
        "eligible-rewrite" if suffix.ends_with("-a") => "Acme, 42 needs polish!".to_owned(),
        "eligible-rewrite" => "Acme 42 needs polish?".to_owned(),
        "eligible-zeta" if suffix.contains("equal-selected") => "Zeta, 7 needs polish!".to_owned(),
        "eligible-zeta" if suffix.ends_with("-a") => "Zeta, 7 needs polish!".to_owned(),
        "eligible-zeta" => "Zeta 7 needs polish?".to_owned(),
        "protected-literal" if suffix.contains("hard-gate-failed") => {
            "Retain Acme 42 exactly.".to_owned()
        }
        "zeta-literal" if suffix.contains("hard-gate-failed") => {
            "Keep Zeta 7 unchanged.".to_owned()
        }
        _ => format!("selected candidate {suffix} {index}"),
    }
}

pub(in crate::verified_candidate_batch_set::tests) struct PairedScenario {
    pub(in crate::verified_candidate_batch_set::tests) candidate_a: Scenario,
    pub(in crate::verified_candidate_batch_set::tests) candidate_b: Scenario,
    pub(in crate::verified_candidate_batch_set::tests) operation_policy:
        Option<GenerationQualificationOperationPolicyV1>,
}

#[expect(
    clippy::too_many_lines,
    reason = "the test fixture keeps the optional qualification closure explicit"
)]
pub(in crate::verified_candidate_batch_set::tests) fn paired_scenario(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    suffix: &str,
) -> PairedScenario {
    let qualification_closure = suffix.contains("qualification-closure");
    let cluster = GenerationClusterRecordV1::new("literal", digest("cluster policy"))
        .expect("material cluster");
    assert!(
        cases
            .iter()
            .all(|case| case.cluster_id() == cluster.cluster_id())
    );
    let repetition =
        GenerationRepetitionRecordV1::new(suite, 0, digest(&format!("repetition {suffix}")))
            .expect("repetition");
    let fixture_a = system_fixture();
    let fixture_b = system_fixture();
    let system_a = fixture_a.record_named(&format!("{suffix} candidate a"));
    let system_b = fixture_b.record_named(&format!("{suffix} candidate b"));
    let mut systems = vec![system_a.clone(), system_b.clone()];
    systems.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    let selected_ordinal = u8::from(!qualification_closure);
    let selection_policy =
        CandidateSelectionPolicyV1::new(suite, &vec![selected_ordinal; cases.len()])
            .expect("selection policy");
    let planned_attempts = build_attempts(suite, cases, &cluster, &repetition, &systems, suffix);
    let operation_input =
        qualification_closure.then(|| qualification_operation_input(&planned_attempts));
    let failure_policy_digest = operation_input.as_ref().map_or_else(
        || digest(&format!("failure policy {suffix}")),
        |input| {
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
        },
    );
    let qualification_plan = GenerationQualificationPlanV1::new(
        suite,
        std::slice::from_ref(&repetition),
        &systems,
        &planned_attempts,
        GenerationQualificationPlanV1Input {
            limits: if qualification_closure {
                qualification_plan_limits(
                    u32::try_from(planned_attempts.len()).expect("attempt count"),
                )
            } else {
                limits(u32::try_from(planned_attempts.len()).expect("attempt count"))
            },
            selection_policy_digest: selection_policy.selection_policy_id().digest().clone(),
            failure_policy_digest,
        },
    )
    .expect("paired plan");
    let operation_policy = operation_input.map(|input| {
        GenerationQualificationOperationPolicyV1::new(
            GenerationQualificationOperationPolicyV1Relations {
                suite,
                plan: &qualification_plan,
                repetitions: std::slice::from_ref(&repetition),
                planned_attempts: &planned_attempts,
                target_system: GenerationQualificationOperationSystemRelationsV1 {
                    generation_system: &system_a,
                    relations: fixture_a.relations(),
                },
                baseline_system: GenerationQualificationOperationSystemRelationsV1 {
                    generation_system: &system_b,
                    relations: fixture_b.relations(),
                },
            },
            input,
        )
        .expect("qualification closure operation policy")
    });
    let candidate_a = paired_candidate(
        suite,
        cases,
        &cluster,
        &repetition,
        &system_a,
        &fixture_a.effective_package,
        &selection_policy,
        &planned_attempts,
        &qualification_plan,
        &format!("{suffix}-a"),
        operation_policy.as_ref(),
    );
    let candidate_b = paired_candidate(
        suite,
        cases,
        &cluster,
        &repetition,
        &system_b,
        &fixture_b.effective_package,
        &selection_policy,
        &planned_attempts,
        &qualification_plan,
        &format!("{suffix}-b"),
        None,
    );
    PairedScenario {
        candidate_a,
        candidate_b,
        operation_policy,
    }
}

fn qualification_operation_input(
    attempts: &[PlannedCandidateAttemptV1],
) -> GenerationQualificationOperationPolicyV1Input {
    let attempt_count = u32::try_from(attempts.len()).expect("attempt count");
    let envelope_bytes = attempts[0].output_ceilings().maximum_envelope_bytes();
    GenerationQualificationOperationPolicyV1Input {
        limits: GenerationQualificationOperationLimitsV1::new(
            1_024,
            4_096,
            4_096,
            1_024,
            envelope_bytes,
            1,
            1_024,
            1_024,
            attempt_count,
            1,
            60_000,
        )
        .expect("qualification operation limits"),
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id:
            GenerationQualificationPlatformAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"qualification-platform-v1"}"#,
            )
            .expect("platform policy ID"),
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id:
            GenerationQualificationLicenseAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"qualification-license-v1"}"#,
            )
            .expect("license policy ID"),
        attempt_ledger_policy_digest: digest("qualification attempt ledger policy"),
        repeatability_policy_digest: digest("qualification repeatability policy"),
        resource_policy_digest: Digest::from_sha256_hex(
            "01ed4005f8c5001b4b5513d1e174e040e7cd8a41e43002c583aff02f77c9d310",
        )
        .expect("canonical qualification resource policy digest"),
        human_adjudication_policy_digest: digest("qualification adjudication policy"),
    }
}

fn qualification_plan_limits(attempt_count: u32) -> GenerationQualificationPlanLimitsV1 {
    GenerationQualificationPlanLimitsV1::new(
        1,
        attempt_count,
        MAX_GENERATION_RETAINED_INPUT_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES,
        MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    )
    .expect("qualification plan limits")
}

fn build_attempts(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    systems: &[GenerationSystemRecordV1],
    suffix: &str,
) -> Vec<PlannedCandidateAttemptV1> {
    let mut attempts = Vec::with_capacity(cases.len() * systems.len());
    for system in systems {
        for case in cases {
            let ordinal = attempts.len();
            attempts.push(planned_attempt(
                suite,
                case,
                cluster,
                repetition,
                system,
                ordinal,
                &format!("{suffix}-{ordinal}"),
            ));
        }
    }
    attempts
}

#[expect(
    clippy::too_many_arguments,
    reason = "the paired fixture keeps every exact common-plan relationship explicit"
)]
fn paired_candidate(
    suite: &GenerationSuiteManifestV1,
    cases: &[GenerationCaseManifestV1],
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    system: &GenerationSystemRecordV1,
    effective_package: &EffectivePackageEvidenceV2,
    selection_policy: &CandidateSelectionPolicyV1,
    planned_attempts: &[PlannedCandidateAttemptV1],
    qualification_plan: &GenerationQualificationPlanV1,
    suffix: &str,
    operation_policy: Option<&GenerationQualificationOperationPolicyV1>,
) -> Scenario {
    let log = Rc::new(RefCell::new(Vec::new()));
    let batches = cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            let planned = planned_attempts
                .iter()
                .find(|attempt| {
                    attempt.case_id() == case.case_id()
                        && attempt.generation_system_id() == system.generation_system_id()
                })
                .expect("one planned attempt per system case");
            offline_batch(
                qualification_plan,
                suite,
                case,
                cluster,
                repetition,
                planned,
                system,
                effective_package,
                index,
                Rc::clone(&log),
                suffix,
                operation_policy,
            )
        })
        .collect();
    Scenario {
        input: VerifiedCandidateBatchSetInput {
            qualification_plan: qualification_plan.clone(),
            suite: suite.clone(),
            repetition: repetition.clone(),
            generation_system: system.clone(),
            selection_policy: selection_policy.clone(),
            planned_attempts: planned_attempts.to_vec(),
        },
        batches,
        log,
    }
}
