use rewrite_types::Digest;

use super::*;
use crate::ArtifactId;
use crate::generation_qualification::generation_system::test_support::{self, SystemFixture};
use crate::generation_qualification::{
    CandidateOutputCeilingsV1, GenerationCaseManifestV1, GenerationCaseManifestV1Input,
    GenerationClusterRecordV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1Input, GenerationRequestBindingId, PlannedCandidateAttemptV1Input,
    PlannedCandidateAttemptV1Relations,
};

pub(crate) const CANDIDATE_BYTES: u64 = 1_024;
pub(crate) const OUTPUT_ENVELOPE_BYTES: u64 = 6_428;
pub(crate) const ATTEMPT_COUNT: u32 = 8;

#[expect(
    clippy::struct_field_names,
    reason = "system_fixture distinguishes the full typed closure from system records"
)]
pub(crate) struct Fixture {
    pub cluster: GenerationClusterRecordV1,
    pub cases: Vec<GenerationCaseManifestV1>,
    pub suite: GenerationSuiteManifestV1,
    pub repetitions: Vec<GenerationRepetitionRecordV1>,
    pub system_fixture: SystemFixture,
    pub systems: Vec<GenerationSystemRecordV1>,
    pub attempts: Vec<PlannedCandidateAttemptV1>,
    pub plan: GenerationQualificationPlanV1,
    pub policy_input: GenerationQualificationOperationPolicyV1Input,
    pub policy: GenerationQualificationOperationPolicyV1,
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
                relations: self.system_fixture.relations(),
            },
            baseline_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: &self.systems[1],
                relations: self.system_fixture.relations(),
            },
        }
    }
}

pub(crate) fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

pub(crate) fn operation_limits(attempt_count: u32) -> GenerationQualificationOperationLimitsV1 {
    GenerationQualificationOperationLimitsV1::new(
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
    .expect("operation limits")
}

pub(crate) fn policy_input(attempt_count: u32) -> GenerationQualificationOperationPolicyV1Input {
    GenerationQualificationOperationPolicyV1Input {
        limits: operation_limits(attempt_count),
        decision_rule: GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses,
        platform_assessment_policy_id:
            GenerationQualificationPlatformAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"platform-v1"}"#,
            )
            .expect("platform policy ID"),
        required_license_permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
        license_assessment_policy_id:
            GenerationQualificationLicenseAssessmentPolicyId::from_canonical_policy_bytes(
                br#"{"policy":"license-v1"}"#,
            )
            .expect("license policy ID"),
        attempt_ledger_policy_digest: digest("attempt ledger policy"),
        repeatability_policy_digest: digest("repeatability policy"),
        resource_policy_digest: digest("resource policy"),
        human_adjudication_policy_digest: digest("human adjudication policy"),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one fixture builds the complete plan-wide typed relationship closure"
)]
pub(crate) fn fixture() -> Fixture {
    let cluster = GenerationClusterRecordV1::new("operation-policy", digest("cluster policy"))
        .expect("cluster");
    let cases = vec![
        case(&cluster, "case-a", "source a"),
        case(&cluster, "case-b", "source b"),
    ];
    let suite =
        GenerationSuiteManifestV1::new(digest("suite protocol"), &cases).expect("suite manifest");
    let repetitions = (0..2)
        .map(|ordinal| {
            GenerationRepetitionRecordV1::new(
                &suite,
                ordinal,
                digest(&format!("repetition policy {ordinal}")),
            )
            .expect("repetition")
        })
        .collect::<Vec<_>>();

    let system_fixture = test_support::fixture(false);
    let mut systems = (0..2)
        .map(|index| {
            let mut input = system_fixture.input();
            input.strategy_digest = digest(&format!("system strategy {index}"));
            GenerationSystemRecordV1::new(system_fixture.relations(), input)
                .expect("generation system")
        })
        .collect::<Vec<_>>();
    systems.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });

    let output_ceilings = CandidateOutputCeilingsV1::new(1, CANDIDATE_BYTES, CANDIDATE_BYTES)
        .expect("candidate output ceilings");
    assert_eq!(
        output_ceilings.maximum_envelope_bytes(),
        OUTPUT_ENVELOPE_BYTES
    );
    let mut attempts = Vec::with_capacity(ATTEMPT_COUNT as usize);
    for repetition in &repetitions {
        for case in &cases {
            for system in &systems {
                let ordinal = u32::try_from(attempts.len()).expect("attempt ordinal");
                attempts.push(
                    PlannedCandidateAttemptV1::new(
                        PlannedCandidateAttemptV1Relations {
                            suite: &suite,
                            case,
                            cluster: &cluster,
                            repetition,
                            generation_system: system,
                        },
                        PlannedCandidateAttemptV1Input {
                            attempt_ordinal: ordinal,
                            declared_seed: u64::from(ordinal) + 10,
                            grounded_request_digest: digest(&format!("grounded request {ordinal}")),
                            generation_request_binding_id:
                                GenerationRequestBindingId::from_derived_digest(digest(&format!(
                                    "request binding {ordinal}"
                                ))),
                            candidate_output_contract_digest: digest("single candidate contract"),
                            output_ceilings,
                        },
                    )
                    .expect("planned attempt"),
                );
            }
        }
    }

    let policy_input = policy_input(ATTEMPT_COUNT);
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
        &suite,
        &repetitions,
        &systems,
        &attempts,
        GenerationQualificationPlanV1Input {
            limits: GenerationQualificationPlanLimitsV1::new(
                1,
                ATTEMPT_COUNT,
                4_096,
                128,
                256,
                OUTPUT_ENVELOPE_BYTES,
            )
            .expect("plan limits"),
            selection_policy_digest: digest("selection policy"),
            failure_policy_digest,
        },
    )
    .expect("qualification plan");
    let policy = GenerationQualificationOperationPolicyV1::new(
        GenerationQualificationOperationPolicyV1Relations {
            suite: &suite,
            plan: &plan,
            repetitions: &repetitions,
            planned_attempts: &attempts,
            target_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: &systems[0],
                relations: system_fixture.relations(),
            },
            baseline_system: GenerationQualificationOperationSystemRelationsV1 {
                generation_system: &systems[1],
                relations: system_fixture.relations(),
            },
        },
        policy_input.clone(),
    )
    .expect("operation policy");

    Fixture {
        cluster,
        cases,
        suite,
        repetitions,
        system_fixture,
        systems,
        attempts,
        plan,
        policy_input,
        policy,
    }
}

fn case(
    cluster: &GenerationClusterRecordV1,
    case_key: &str,
    source: &str,
) -> GenerationCaseManifestV1 {
    let source_digest = digest(source);
    GenerationCaseManifestV1::new(
        cluster,
        GenerationCaseManifestV1Input {
            case_key: case_key.to_owned(),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count: u64::try_from(source.len()).expect("source count"),
            case_contract_digest: digest("case contract"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("case")
}
