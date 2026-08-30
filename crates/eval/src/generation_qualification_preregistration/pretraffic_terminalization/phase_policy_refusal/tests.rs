use std::time::{Duration, Instant};

use rewrite_app::GenerationQualificationPhasePolicySourceDisposition;
use rewrite_model::{
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations, GenerationPhasePolicyDenialReasonV1,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseEvidenceError,
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    GenerationResourcePolicyDenialRecordV1Relations,
};
use rewrite_types::CancellationToken;

use super::evidence::{
    DeniedPolicySelection, checked_elapsed_nanoseconds, denied_policy_selection,
};
use super::*;
use crate::generation_qualification_preregistration::pretraffic_terminalization::relations::phase_scope;

#[path = "tests/support.rs"]
mod support;

use support::{with_prepared, with_prepared_and_foreign_human};

const RESOURCE_POLICY_JSON: &[u8] = concat!(
    "{\"authority\":\"none\",",
    "\"decision_rule\":\"all_complete_target_observations_within_declared_limits\",",
    "\"procedure_id\":\"retonr:generation-qualification-resource-policy:procedure\",",
    "\"procedure_version\":1,",
    "\"measurement_profile\":\"managed_local_generation_v1\",",
    "\"maximum_attempt_elapsed_nanoseconds\":30000000000,",
    "\"maximum_first_response_nanoseconds\":5000000000,",
    "\"maximum_cleanup_nanoseconds\":2000000000,",
    "\"maximum_worker_high_water_resident_bytes\":17179869184,",
    "\"maximum_installed_footprint_bytes\":34359738368,",
    "\"required_provider_observations\":[\"prompt_token_count\",",
    "\"generated_token_count\",\"total_duration_nanoseconds\",",
    "\"load_duration_nanoseconds\",\"prompt_evaluation_duration_nanoseconds\",",
    "\"evaluation_duration_nanoseconds\"],\"schema_version\":1}"
)
.as_bytes();
const HUMAN_POLICY_JSON: &[u8] = concat!(
    "{\"authority\":\"none\",",
    "\"decision_rule\":\"two_independent_blinded_reviews_then_role_separated_adjudication\",",
    "\"procedure_id\":\"retonr:generation-qualification-human-adjudication-policy:procedure\",",
    "\"procedure_version\":1,",
    "\"presentation_rule\":\"deterministic_blinded_candidate_pair_v1\",",
    "\"presentation_seed\":7627468237172680543,",
    "\"eligible_case_rule\":\"all_cases_in_all_passed_repetitions\",",
    "\"primary_reviewer_count\":2,\"require_distinct_primary_reviewers\":true,",
    "\"require_role_separated_adjudicator\":true,",
    "\"adjudication_trigger\":\"disagreement_tie_or_abstention\",",
    "\"allowed_outcomes\":[\"acceptable\",\"unacceptable\",\"abstain\"],",
    "\"schema_version\":1}"
)
.as_bytes();
const RESOURCE_POLICY_DIGEST: &str =
    "01ed4005f8c5001b4b5513d1e174e040e7cd8a41e43002c583aff02f77c9d310";
const HUMAN_POLICY_DIGEST: &str =
    "79d434e04a9e1753a31b8b60db540d33f22c7491d38ca63a8e9ece6ec6368821";

#[test]
fn both_denied_sources_close_exact_failed_inert_terminal() {
    with_prepared(None, |prepared, resource_policy, human_policy| {
        let mut finalized = prepared
            .finalize_phase_policy_refused_pretraffic(
                resource_policy,
                human_policy,
                &CancellationToken::new(),
            )
            .expect("source-denied terminal");
        assert_exact_refusal(&finalized);
        finalized
            .revalidate(&CancellationToken::new())
            .expect("fresh refusal revalidation");
    });
}

#[test]
fn one_denied_policy_is_sufficient_in_either_position() {
    let denied = GenerationQualificationPhasePolicySourceDisposition::Denied;
    let approved = GenerationQualificationPhasePolicySourceDisposition::Approved;
    assert_eq!(
        denied_policy_selection(denied, approved),
        Ok(DeniedPolicySelection {
            resource: true,
            human: false,
        })
    );
    assert_eq!(
        denied_policy_selection(approved, denied),
        Ok(DeniedPolicySelection {
            resource: false,
            human: true,
        })
    );
}

#[test]
fn two_approved_policies_fail_closed_in_the_pure_disposition_kernel() {
    let approved = GenerationQualificationPhasePolicySourceDisposition::Approved;
    assert_eq!(
        denied_policy_selection(approved, approved),
        Err(GenerationQualificationPhasePolicyRefusalError::BothPoliciesApproved)
    );
}

#[test]
fn substituted_human_policy_authority_is_refused() {
    with_prepared_and_foreign_human(|prepared, resource_policy, foreign_human_policy| {
        assert!(matches!(
            prepared.finalize_phase_policy_refused_pretraffic(
                resource_policy,
                foreign_human_policy,
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationPhasePolicyRefusalError::HumanPolicyMismatch)
        ));
    });
}

#[test]
fn denial_records_refuse_policy_and_scope_substitution() {
    with_prepared(None, |prepared, resource_policy, human_policy| {
        let mut finalized = prepared
            .finalize_phase_policy_refused_pretraffic(
                resource_policy,
                human_policy,
                &CancellationToken::new(),
            )
            .expect("source-denied terminal");
        let FinalizedPhasePolicyRefusedPretrafficGenerationQualification {
            prepared,
            resource_policy,
            human_policy,
            evidence,
        } = &mut finalized;
        prepared
            .with_validated_view(&CancellationToken::new(), |view| {
                let scope = phase_scope(&view);
                let resource = evidence
                    .resource_denial_record
                    .as_ref()
                    .expect("resource denial");
                assert_eq!(
                    resource.validate_against(GenerationResourcePolicyDenialRecordV1Relations {
                        scope,
                        phase_policy_digest: human_policy.policy_digest(),
                    }),
                    Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
                );
                let substituted_scope = GenerationQualificationPhaseScopeV1 {
                    generation_system: view
                        .operation_policy_relations
                        .baseline_system
                        .generation_system,
                    qualification_plan: scope.qualification_plan,
                    suite: scope.suite,
                };
                let human = evidence.human_denial_record.as_ref().expect("human denial");
                assert_eq!(
                    human.validate_against(
                        GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
                            scope: substituted_scope,
                            phase_policy_digest: human_policy.policy_digest(),
                        },
                    ),
                    Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
                );
                resource_policy
                    .revalidate_operation_policy(view.operation_policy)
                    .expect("retained resource policy");
                Ok::<_, ()>(())
            })
            .expect("outer Prepared validation");
    });
}

#[test]
fn cancellation_is_aggregated_across_the_prepared_bracket() {
    with_prepared(None, |prepared, resource_policy, human_policy| {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            prepared.finalize_phase_policy_refused_pretraffic(
                resource_policy,
                human_policy,
                &cancellation,
            ),
            Err(GenerationQualificationPhasePolicyRefusalError::ValidationAggregation)
        ));
    });
}

#[test]
fn elapsed_gate_preserves_deadline_before_cancellation() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let started = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .expect("representable test instant");
    assert_eq!(
        checked_elapsed_nanoseconds(started, Instant::now(), &cancellation),
        Err(GenerationQualificationPhasePolicyRefusalError::DeadlineExceeded)
    );
}

#[test]
fn expired_prepared_refusal_aggregates_deadline_before_cancellation() {
    with_prepared(Some(2_000), |prepared, resource_policy, human_policy| {
        std::thread::sleep(Duration::from_millis(2_250));
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            prepared.finalize_phase_policy_refused_pretraffic(
                resource_policy,
                human_policy,
                &cancellation,
            ),
            Err(GenerationQualificationPhasePolicyRefusalError::ValidationAggregation)
        ));
    });
}

#[test]
fn checked_elapsed_refuses_reversed_monotonic_time() {
    let cancellation = CancellationToken::new();
    let started = Instant::now() + Duration::from_secs(1);
    assert_eq!(
        checked_elapsed_nanoseconds(started, started + Duration::from_secs(1), &cancellation),
        Err(GenerationQualificationPhasePolicyRefusalError::ElapsedUnavailable)
    );
}

#[test]
fn debug_errors_and_denial_records_are_content_redacted() {
    with_prepared(None, |prepared, resource_policy, human_policy| {
        let finalized = prepared
            .finalize_phase_policy_refused_pretraffic(
                resource_policy,
                human_policy,
                &CancellationToken::new(),
            )
            .expect("source-denied terminal");
        let debug = format!("{finalized:?}");
        assert!(!debug.contains(RESOURCE_POLICY_DIGEST));
        assert!(!debug.contains(HUMAN_POLICY_DIGEST));
        for error in [
            GenerationQualificationPhasePolicyRefusalError::ResourcePolicyMismatch,
            GenerationQualificationPhasePolicyRefusalError::DenialRecordMismatch,
            GenerationQualificationPhasePolicyRefusalError::ReceiptMismatch,
        ] {
            let display = error.to_string();
            assert!(!display.contains(RESOURCE_POLICY_DIGEST));
            assert!(!display.contains(HUMAN_POLICY_DIGEST));
        }
    });
}

#[test]
fn source_has_no_runtime_call_or_qualification_record_construction() {
    let source = include_str!("../phase_policy_refusal.rs");
    let evidence_source = include_str!("evidence.rs");
    for forbidden in [
        "GenerationQualificationRecordV1::new",
        ".acquire(",
        ".launch(",
        ".generate(",
        ".traffic(",
        "ManagedGeneration",
        "VerifiedManaged",
        "CandidateGeneration",
    ] {
        assert!(!source.contains(forbidden), "forbidden source: {forbidden}");
        assert!(
            !evidence_source.contains(forbidden),
            "forbidden evidence source: {forbidden}"
        );
    }
}

fn assert_exact_refusal(
    finalized: &FinalizedPhasePolicyRefusedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
) {
    let resource_denial = finalized
        .resource_policy_denial_record()
        .expect("resource source disposition record");
    let human_denial = finalized
        .human_policy_denial_record()
        .expect("human source disposition record");
    assert_eq!(
        resource_denial.reason(),
        GenerationPhasePolicyDenialReasonV1::PolicySourceDenied
    );
    assert_eq!(
        human_denial.reason(),
        GenerationPhasePolicyDenialReasonV1::PolicySourceDenied
    );
    for manifest in [
        serde_json::to_vec(finalized.attempt_ledger_manifest()).expect("ledger JSON"),
        serde_json::to_vec(finalized.repeatability_manifest()).expect("repeatability JSON"),
        serde_json::to_vec(finalized.resource_manifest()).expect("resource JSON"),
        serde_json::to_vec(finalized.human_adjudication_manifest()).expect("human JSON"),
    ] {
        assert!(!manifest.windows(64).any(|window| {
            window
                == resource_denial
                    .resource_policy_denial_record_id()
                    .digest()
                    .as_str()
                    .as_bytes()
                || window
                    == human_denial
                        .human_adjudication_policy_denial_record_id()
                        .digest()
                        .as_str()
                        .as_bytes()
        }));
    }
    for (count, status) in [
        (
            finalized.attempt_ledger_manifest().evidence_item_count(),
            finalized.attempt_ledger_manifest().status(),
        ),
        (
            finalized.repeatability_manifest().evidence_item_count(),
            finalized.repeatability_manifest().status(),
        ),
        (
            finalized.resource_manifest().evidence_item_count(),
            finalized.resource_manifest().status(),
        ),
        (
            finalized
                .human_adjudication_manifest()
                .evidence_item_count(),
            finalized.human_adjudication_manifest().status(),
        ),
    ] {
        assert_eq!(count, 0);
        assert_eq!(status, GenerationQualificationPhaseStatusV1::Skipped);
    }
    assert_eq!(finalized.receipt().peak_concurrent_attempts(), 0);
    let maximum_elapsed_nanoseconds = u64::from(
        finalized
            .prepared
            .operation_policy()
            .limits()
            .maximum_elapsed_milliseconds(),
    ) * 1_000_000;
    assert!(finalized.receipt().elapsed_nanoseconds() < maximum_elapsed_nanoseconds);
    assert_eq!(
        finalized.receipt().terminal_status(),
        GenerationQualificationOperationTerminalStatusV1::Failed
    );
    assert_eq!(
        finalized.receipt().finalization_status(),
        GenerationQualificationOperationFinalizationStatusV1::NotRequired
    );
}
