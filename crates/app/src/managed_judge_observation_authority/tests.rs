use rewrite_types::{CancellationToken, Digest};

use crate::effective_runtime_state_observation::{
    CompletedSequenceMutation, completed_sequence_for_observation_authority,
    completed_sequence_with_unsealed_preflight_binding, mutate_completed_sequence,
};
use crate::managed_judge_precursor::portable_judge_plan_schedules;

use super::*;

#[test]
fn exact_schedule_derives_frozen_aggregates_and_content_free_authority() {
    assert_eq!(
        MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN,
        b"retonr:managed-local-judge-preflight-observer-binding:v1\0"
    );
    let (plan, schedule, _, _) = portable_judge_plan_schedules();
    let sequence = completed_sequence_for_observation_authority(
        schedule.candidate_judge_schedule_id(),
        schedule.entry_count(),
    );
    let authority = compile(plan, schedule, sequence).expect("exact observation authority");

    assert_eq!(authority.attempt_count(), 2);
    assert_eq!(authority.completed_sequence().bindings().len(), 2);
    assert_eq!(
        authority.retained_session_preflight_digest(),
        authority.completed_sequence().bindings()[0].retained_preflight_digest()
    );
    assert!(
        authority.completed_sequence().bindings()[0]
            .effective_runtime_state()
            .is_none()
    );
    assert_eq!(
        authority.judge_plan_id(),
        authority.judge_plan().candidate_judge_plan_id()
    );
    assert_eq!(
        authority.judge_schedule_id(),
        authority.judge_schedule().candidate_judge_schedule_id()
    );
    assert_eq!(
        authority.residency_receipt_aggregate_digest().as_str(),
        "c2ffa35894ea01f2f5c1cc25669cc54f76eac660bd930e7a8b81d654b8bd0fcf"
    );
    assert_eq!(
        authority.preflight_observer_binding_digest().as_str(),
        "07bb1c56659db32302d047936bdce55126d2d572eae3a39887040ba6d5e357d6"
    );
    assert_eq!(
        authority.process_observation_aggregate_digest().as_str(),
        "31937b1b7db4f5e209d052267a093a3cc63991300b678ba13b9d42862e0e9ab3"
    );
    assert_eq!(
        authority
            .native_load_observation_aggregate_digest()
            .as_str(),
        "30aa35bd9a87812a05c213ff743f42c020a39eca0a7eae84ff663786ebb3e87f"
    );
    assert_eq!(
        authority.connection_observation_aggregate_digest().as_str(),
        "fae48512cebcf815c84475525fea25ab5407dbc139966f6c312b1cbbeab4bf53"
    );
    assert_eq!(
        authority
            .effective_runtime_state_observation_aggregate_digest()
            .as_str(),
        "de46f941b1a9f49327df6c0dade7e20045be4e97d70373a886982d682aa37795"
    );
    assert_eq!(
        authority
            .effective_runtime_state_join_id()
            .digest()
            .as_str(),
        "edca8345f8401f3943969fdc04783a4aba6b4582af597b4e88104b4988554d24"
    );
    let debug = format!("{authority:?}");
    assert!(debug.contains("attempt_count: 2"));
    assert!(!debug.contains("aggregate-authority"));
}

#[test]
fn retained_session_preflight_digest_is_owner_derived_and_revalidated() {
    let mut authority = exact_authority();
    authority.retained_session_preflight_digest = digest("substituted retained preflight");

    assert!(matches!(
        authority.revalidate_retained_bindings(&CancellationToken::new()),
        Err(ManagedJudgeObservationAuthorityError::Relationship(
            ManagedJudgeObservationAuthorityRelationship::DerivedAggregate
        ))
    ));
}

#[test]
fn cancellation_and_portable_or_sequence_substitutions_fail_closed() {
    let (plan, schedule, alternate_plan, alternate_schedule) = portable_judge_plan_schedules();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        ManagedJudgeObservationAuthorityCompiler::compile(
            ManagedJudgeObservationAuthorityInput {
                completed_sequence: completed_sequence_for_observation_authority(
                    schedule.candidate_judge_schedule_id(),
                    schedule.entry_count(),
                ),
                judge_plan: plan.clone(),
                judge_schedule: schedule.clone(),
            },
            &cancelled,
        ),
        Err(ManagedJudgeObservationAuthorityError::Cancelled)
    ));

    assert_relationship(
        compile(
            alternate_plan,
            schedule.clone(),
            completed_sequence_for_observation_authority(
                schedule.candidate_judge_schedule_id(),
                schedule.entry_count(),
            ),
        ),
        ManagedJudgeObservationAuthorityRelationship::PlanSchedule,
    );
    assert_relationship(
        compile(
            plan.clone(),
            alternate_schedule.clone(),
            completed_sequence_for_observation_authority(
                alternate_schedule.candidate_judge_schedule_id(),
                alternate_schedule.entry_count(),
            ),
        ),
        ManagedJudgeObservationAuthorityRelationship::PlanSchedule,
    );
    assert_relationship(
        compile(
            plan,
            schedule.clone(),
            completed_sequence_for_observation_authority(
                alternate_schedule.candidate_judge_schedule_id(),
                schedule.entry_count(),
            ),
        ),
        ManagedJudgeObservationAuthorityRelationship::SequenceSchedule,
    );
}

#[test]
fn count_order_cursor_span_and_shared_preflight_are_revalidated() {
    check_mutation(
        CompletedSequenceMutation::RemoveLast,
        ManagedJudgeObservationAuthorityRelationship::AttemptCount,
    );
    check_mutation(
        CompletedSequenceMutation::SwapFirstTwo,
        ManagedJudgeObservationAuthorityRelationship::ScheduleCursor,
    );
    check_mutation(
        CompletedSequenceMutation::Cursor { index: 1, value: 0 },
        ManagedJudgeObservationAuthorityRelationship::ScheduleCursor,
    );
    check_mutation(
        CompletedSequenceMutation::FirstResponse {
            index: 1,
            value: 18,
        },
        ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
    );
    check_mutation(
        CompletedSequenceMutation::LastResponse {
            index: 0,
            value: 15,
        },
        ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
    );
    check_mutation(
        CompletedSequenceMutation::FirstResidency { index: 0 },
        ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
    );
    check_mutation(
        CompletedSequenceMutation::LastResidency { index: 1 },
        ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
    );
    check_mutation(
        CompletedSequenceMutation::ExecutionFirstResponse { index: 0 },
        ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
    );
    check_mutation(
        CompletedSequenceMutation::ExecutionLastResponse { index: 1 },
        ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
    );
    check_mutation(
        CompletedSequenceMutation::RetainedPreflight { index: 1 },
        ManagedJudgeObservationAuthorityRelationship::RetainedPreflight,
    );
    check_mutation(
        CompletedSequenceMutation::ReceiptBinding { index: 1 },
        ManagedJudgeObservationAuthorityRelationship::ResidencyReceipt,
    );
}

#[test]
fn every_independent_leaf_changes_only_its_own_aggregate_and_join_tracks_effective_state() {
    let baseline = exact_authority();
    let cases = [
        (
            CompletedSequenceMutation::Receipt { index: 1 },
            Leaf::Residency,
        ),
        (
            CompletedSequenceMutation::Process {
                index: 1,
                value: digest("other process"),
            },
            Leaf::Process,
        ),
        (
            CompletedSequenceMutation::NativeLoad {
                index: 1,
                value: digest("other native load"),
            },
            Leaf::NativeLoad,
        ),
        (
            CompletedSequenceMutation::Connection {
                index: 1,
                value: digest("other connection"),
            },
            Leaf::Connection,
        ),
        (
            CompletedSequenceMutation::EffectiveState {
                index: 1,
                value: digest("other effective state"),
            },
            Leaf::EffectiveState,
        ),
    ];
    for (mutation, expected) in cases {
        let changed = authority_with_mutation(mutation);
        assert_leaf_change(&baseline, &changed, expected);
    }
}

#[test]
fn every_preflight_observer_field_changes_only_its_dedicated_provenance_binding() {
    let baseline = exact_authority();
    for mutation in [
        CompletedSequenceMutation::PreflightInitialProcess,
        CompletedSequenceMutation::PreflightPostProcess,
        CompletedSequenceMutation::PreflightFinalProcess,
        CompletedSequenceMutation::PreflightNativeLoad,
        CompletedSequenceMutation::PreflightConnection,
    ] {
        let changed = authority_with_mutation(mutation);
        assert_ne!(
            baseline.preflight_observer_binding_digest(),
            changed.preflight_observer_binding_digest()
        );
        assert_eq!(
            baseline.residency_receipt_aggregate_digest(),
            changed.residency_receipt_aggregate_digest()
        );
        assert_eq!(
            baseline.process_observation_aggregate_digest(),
            changed.process_observation_aggregate_digest()
        );
        assert_eq!(
            baseline.native_load_observation_aggregate_digest(),
            changed.native_load_observation_aggregate_digest()
        );
        assert_eq!(
            baseline.connection_observation_aggregate_digest(),
            changed.connection_observation_aggregate_digest()
        );
        assert_eq!(
            baseline.effective_runtime_state_observation_aggregate_digest(),
            changed.effective_runtime_state_observation_aggregate_digest()
        );
    }
}

#[test]
fn unsealed_and_retained_preflight_bindings_agree_and_schedule_identity_is_significant() {
    let (plan, schedule, alternate_plan, alternate_schedule) = portable_judge_plan_schedules();
    let (sequence, unsealed_binding) = completed_sequence_with_unsealed_preflight_binding(
        schedule.candidate_judge_schedule_id().clone(),
        schedule.candidate_judge_schedule_id(),
        schedule.entry_count(),
    );
    let authority = compile(plan, schedule.clone(), sequence).expect("exact authority");
    assert_eq!(
        authority.preflight_observer_binding_digest(),
        &unsealed_binding
    );

    let (alternate_sequence, alternate_unsealed_binding) =
        completed_sequence_with_unsealed_preflight_binding(
            alternate_schedule.candidate_judge_schedule_id().clone(),
            alternate_schedule.candidate_judge_schedule_id(),
            alternate_schedule.entry_count(),
        );
    let alternate_authority = compile(alternate_plan, alternate_schedule, alternate_sequence)
        .expect("alternate exact authority");
    assert_eq!(
        alternate_authority.preflight_observer_binding_digest(),
        &alternate_unsealed_binding
    );
    assert_ne!(unsealed_binding, alternate_unsealed_binding);
    assert_ne!(
        authority.residency_receipt_aggregate_digest(),
        alternate_authority.residency_receipt_aggregate_digest()
    );
    assert_ne!(
        authority.process_observation_aggregate_digest(),
        alternate_authority.process_observation_aggregate_digest()
    );
    assert_ne!(
        authority.native_load_observation_aggregate_digest(),
        alternate_authority.native_load_observation_aggregate_digest()
    );
    assert_ne!(
        authority.connection_observation_aggregate_digest(),
        alternate_authority.connection_observation_aggregate_digest()
    );
    assert_ne!(
        authority.effective_runtime_state_observation_aggregate_digest(),
        alternate_authority.effective_runtime_state_observation_aggregate_digest()
    );
    assert_ne!(
        authority.effective_runtime_state_join_id(),
        alternate_authority.effective_runtime_state_join_id()
    );

    let (_, foreign_binding) = completed_sequence_with_unsealed_preflight_binding(
        schedule.candidate_judge_schedule_id().clone(),
        alternate_authority.judge_schedule_id(),
        schedule.entry_count(),
    );
    assert_ne!(
        authority.preflight_observer_binding_digest(),
        &foreign_binding
    );
}

#[test]
fn equal_owner_derived_leaves_are_valid_and_error_debug_is_redacted() {
    let authority = exact_authority();
    let bindings = authority.completed_sequence().bindings();
    assert_eq!(
        bindings[0].process_binding_digest(),
        bindings[1].process_binding_digest()
    );
    assert_eq!(
        bindings[0].effective_state_binding_digest(),
        bindings[1].effective_state_binding_digest()
    );

    let mut sequence = completed_sequence(&authority);
    mutate_completed_sequence(&mut sequence, CompletedSequenceMutation::RemoveLast);
    let result = compile(
        authority.judge_plan().clone(),
        authority.judge_schedule().clone(),
        sequence,
    );
    let Err(error) = result else {
        panic!("invalid count unexpectedly compiled");
    };
    let debug = format!("{error:?}");
    assert!(debug.contains("AttemptCount"));
    assert!(!debug.contains(authority.judge_schedule_id().digest().as_str()));
    assert!(
        format!("{:?}", ManagedJudgeObservationAuthorityError::Cancelled).contains("cancelled")
    );
    let portable = ManagedJudgeObservationAuthorityError::PortableContract(
        GenerationQualificationContractError::EncodingOverflow,
    );
    let portable_debug = format!("{portable:?}");
    assert!(portable_debug.contains("portable_contract"));
    assert!(!portable_debug.contains("EncodingOverflow"));
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Leaf {
    Residency,
    Process,
    NativeLoad,
    Connection,
    EffectiveState,
}

fn compile(
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    completed_sequence: CompletedManagedJudgeObservationSequence,
) -> Result<VerifiedManagedJudgeObservationAuthority, ManagedJudgeObservationAuthorityError> {
    ManagedJudgeObservationAuthorityCompiler::compile(
        ManagedJudgeObservationAuthorityInput {
            judge_plan,
            judge_schedule,
            completed_sequence,
        },
        &CancellationToken::new(),
    )
}

fn exact_authority() -> VerifiedManagedJudgeObservationAuthority {
    let (plan, schedule, _, _) = portable_judge_plan_schedules();
    let sequence = completed_sequence_for_observation_authority(
        schedule.candidate_judge_schedule_id(),
        schedule.entry_count(),
    );
    compile(plan, schedule, sequence).expect("exact authority")
}

fn completed_sequence(
    authority: &VerifiedManagedJudgeObservationAuthority,
) -> CompletedManagedJudgeObservationSequence {
    completed_sequence_for_observation_authority(
        authority.judge_schedule_id(),
        u32::try_from(authority.attempt_count()).expect("attempt count"),
    )
}

fn authority_with_mutation(
    mutation: CompletedSequenceMutation,
) -> VerifiedManagedJudgeObservationAuthority {
    let (plan, schedule, _, _) = portable_judge_plan_schedules();
    let mut sequence = completed_sequence_for_observation_authority(
        schedule.candidate_judge_schedule_id(),
        schedule.entry_count(),
    );
    mutate_completed_sequence(&mut sequence, mutation);
    compile(plan, schedule, sequence).expect("valid changed leaf")
}

fn check_mutation(
    mutation: CompletedSequenceMutation,
    expected: ManagedJudgeObservationAuthorityRelationship,
) {
    let (plan, schedule, _, _) = portable_judge_plan_schedules();
    let mut sequence = completed_sequence_for_observation_authority(
        schedule.candidate_judge_schedule_id(),
        schedule.entry_count(),
    );
    mutate_completed_sequence(&mut sequence, mutation);
    assert_relationship(compile(plan, schedule, sequence), expected);
}

fn assert_relationship<T>(
    result: Result<T, ManagedJudgeObservationAuthorityError>,
    expected: ManagedJudgeObservationAuthorityRelationship,
) {
    match result {
        Err(ManagedJudgeObservationAuthorityError::Relationship(actual)) => {
            assert_eq!(actual, expected);
        }
        Err(error) => panic!("expected {expected:?}, got {error:?}"),
        Ok(_) => panic!("expected {expected:?}, got success"),
    }
}

fn assert_leaf_change(
    baseline: &VerifiedManagedJudgeObservationAuthority,
    changed: &VerifiedManagedJudgeObservationAuthority,
    expected: Leaf,
) {
    assert_eq!(
        baseline.residency_receipt_aggregate_digest()
            != changed.residency_receipt_aggregate_digest(),
        expected == Leaf::Residency
    );
    assert_eq!(
        baseline.process_observation_aggregate_digest()
            != changed.process_observation_aggregate_digest(),
        expected == Leaf::Process
    );
    assert_eq!(
        baseline.native_load_observation_aggregate_digest()
            != changed.native_load_observation_aggregate_digest(),
        expected == Leaf::NativeLoad
    );
    assert_eq!(
        baseline.connection_observation_aggregate_digest()
            != changed.connection_observation_aggregate_digest(),
        expected == Leaf::Connection
    );
    assert_eq!(
        baseline.effective_runtime_state_observation_aggregate_digest()
            != changed.effective_runtime_state_observation_aggregate_digest(),
        expected == Leaf::EffectiveState
    );
    assert_eq!(
        baseline.effective_runtime_state_join_id() != changed.effective_runtime_state_join_id(),
        expected == Leaf::EffectiveState
    );
}

fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}
