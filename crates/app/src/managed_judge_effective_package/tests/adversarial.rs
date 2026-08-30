use rewrite_model::{
    CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1, EffectiveRuntimeState,
    EffectiveRuntimeStateInput,
};
use rewrite_runtime_isolation::{IsolationPolicy, PreparedIsolation};
use rewrite_types::{CancellationToken, Digest};

use crate::candidate_attempt_precursor::tests::support::{characterized, launch};
use crate::effective_runtime_state_observation::CompletedSequenceMutation;
use crate::managed_judge_precursor::JudgeFixture;

use super::super::*;
use super::{authority_and_requests, managed_bracket};

#[derive(Clone, Copy)]
enum Fault {
    None,
    PortableLineage,
    RuntimeGeneration,
    ExactIsolationSubject,
    ModelGeneration,
    ModelDrift,
    RuntimeDrift,
    ReversedRequests,
    Sequence(SequenceFault),
    Evidence,
}

#[derive(Clone, Copy)]
enum SequenceFault {
    RemoveLast,
    Swap,
    Cursor,
    FirstResponse,
    LastResponse,
    ExecutionFirst,
    ExecutionLast,
    FirstResidency,
    LastResidency,
    RequestAndReceipt,
    Response,
    ReceiptBinding,
    Preflight,
    OfflineFirstState,
    ProcessLeaf,
}

#[derive(Clone, Copy)]
enum CancellationPoint {
    None,
    BeforePreparation,
    AfterDerivation,
}

#[test]
fn every_relationship_category_fails_closed() {
    let cases = [
        (
            Fault::PortableLineage,
            ManagedJudgeEffectivePackageRelationship::PortableLineage,
        ),
        (
            Fault::RuntimeGeneration,
            ManagedJudgeEffectivePackageRelationship::Runtime,
        ),
        (
            Fault::ExactIsolationSubject,
            ManagedJudgeEffectivePackageRelationship::Isolation,
        ),
        (
            Fault::ModelGeneration,
            ManagedJudgeEffectivePackageRelationship::Model,
        ),
        (
            Fault::ReversedRequests,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            Fault::Sequence(SequenceFault::OfflineFirstState),
            ManagedJudgeEffectivePackageRelationship::RealEffectiveState,
        ),
        (
            Fault::Sequence(SequenceFault::ProcessLeaf),
            ManagedJudgeEffectivePackageRelationship::EffectiveState,
        ),
        (
            Fault::Evidence,
            ManagedJudgeEffectivePackageRelationship::Evidence,
        ),
    ];
    for (fault, expected) in cases {
        assert_relationship(run(fault, CancellationPoint::None), expected);
    }
}

#[test]
fn every_attempt_cursor_request_response_receipt_ordinal_and_state_mutation_fails() {
    let cases = [
        (
            SequenceFault::RemoveLast,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::Swap,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::Cursor,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::FirstResponse,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::LastResponse,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::ExecutionFirst,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::ExecutionLast,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::FirstResidency,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::LastResidency,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::RequestAndReceipt,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::Response,
            ManagedJudgeEffectivePackageRelationship::EffectiveState,
        ),
        (
            SequenceFault::ReceiptBinding,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::Preflight,
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ),
        (
            SequenceFault::OfflineFirstState,
            ManagedJudgeEffectivePackageRelationship::RealEffectiveState,
        ),
        (
            SequenceFault::ProcessLeaf,
            ManagedJudgeEffectivePackageRelationship::EffectiveState,
        ),
    ];
    for (fault, expected) in cases {
        assert_relationship(
            run(Fault::Sequence(fault), CancellationPoint::None),
            expected,
        );
    }
}

#[test]
fn first_attempt_loss_rejects_a_final_attempt_only_surrogate() {
    assert_relationship(
        run(
            Fault::Sequence(SequenceFault::OfflineFirstState),
            CancellationPoint::None,
        ),
        ManagedJudgeEffectivePackageRelationship::RealEffectiveState,
    );
}

#[test]
fn digest_equal_distinct_prepared_isolation_is_rejected_by_subject_identity() {
    let first = prepared_isolation();
    let second = prepared_isolation();
    assert_eq!(first.policy_digest(), second.policy_digest());
    assert!(!first.subject_token().binds_exact(&second));
    assert_relationship(
        run(Fault::ExactIsolationSubject, CancellationPoint::None),
        ManagedJudgeEffectivePackageRelationship::Isolation,
    );
}

#[test]
fn cancellation_before_and_after_derivation_closes_without_releasing() {
    for point in [
        CancellationPoint::BeforePreparation,
        CancellationPoint::AfterDerivation,
    ] {
        let error = run(Fault::None, point).expect_err("cancelled preparation");
        assert!(matches!(
            error.primary(),
            ManagedJudgeEffectivePackageDerivationError::Cancelled
        ));
        assert!(error.finalization().is_none());
        let debug = format!("{error:?}");
        assert!(debug.contains("cancelled"));
        assert!(!debug.contains("batch-effective-package"));
    }
}

#[test]
fn initial_model_and_runtime_drift_are_typed_and_redacted() {
    let model = run(Fault::ModelDrift, CancellationPoint::None).expect_err("model drift");
    assert!(matches!(
        model.primary(),
        ManagedJudgeEffectivePackageDerivationError::ModelRevalidation(_)
    ));
    assert!(model.finalization().is_some());
    assert!(!format!("{model:?}").contains("unexpected-member.bin"));

    let runtime = run(Fault::RuntimeDrift, CancellationPoint::None).expect_err("runtime drift");
    assert!(matches!(
        runtime.primary(),
        ManagedJudgeEffectivePackageDerivationError::RuntimeRevalidation(_)
    ));
    assert!(runtime.finalization().is_some());
    assert!(!format!("{runtime:?}").contains("unexpected.bin"));
}

fn run(
    fault: Fault,
    cancellation_point: CancellationPoint,
) -> Result<ReleasedManagedJudgeEffectivePackageV2, ManagedJudgeEffectivePackagePlanError> {
    let mut fixture = JudgeFixture::new();
    let managed = managed_bracket(
        &fixture.live.model_lease,
        &fixture.live.runtime.runtime_manifest,
        &fixture.live.runtime.runtime_package,
        &fixture.live.runtime.admitted,
        &fixture.live.runtime.prepared_isolation,
    );
    let (mut authority, requests) = authority_and_requests(&fixture, &managed);
    if let Fault::Sequence(sequence_fault) = fault {
        authority.test_support_mutate_sequence(mutation(sequence_fault));
    }
    let requests = substitute_requests(fault, &fixture, requests);
    let foreign_isolation = prepared_isolation();
    let selected_isolation = if matches!(fault, Fault::ExactIsolationSubject) {
        &foreign_isolation
    } else {
        &fixture.live.runtime.prepared_isolation
    };
    let alternate_launch = launch(&fixture.live.model_lease, "latest");
    let alternate_characterized = characterized(
        &fixture.live,
        &alternate_launch,
        &changed_runtime_state(&fixture),
    );
    let selected_characterized = if matches!(fault, Fault::Evidence) {
        &alternate_characterized
    } else {
        &fixture.characterized
    };
    let runtime_generation = fixture
        .live
        .runtime
        .runtime_package
        .installation_key()
        .installation_generation()
        + u64::from(matches!(fault, Fault::RuntimeGeneration));
    let model_generation = fixture
        .live
        .model_lease
        .private_view()
        .installation_generation()
        + u64::from(matches!(fault, Fault::ModelGeneration));
    if matches!(fault, Fault::ModelDrift) {
        fixture.live.add_model_member();
    }
    if matches!(fault, Fault::RuntimeDrift) {
        fixture.live.add_runtime_member();
    }
    let cancellation = CancellationToken::new();
    if matches!(cancellation_point, CancellationPoint::BeforePreparation) {
        cancellation.cancel();
    }
    let input = ManagedJudgeEffectivePackagePlanInput {
        observation_authority: authority,
        request_aggregate: requests,
        judge_system: fixture.judge_system.clone(),
        expected_runtime_state: fixture.live.runtime.runtime_state.clone(),
        runtime_manifest: &fixture.live.runtime.runtime_manifest,
        runtime_package: &mut fixture.live.runtime.runtime_package,
        runtime_build: &fixture.live.runtime.runtime_build,
        admitted_runtime: &fixture.live.runtime.admitted,
        generation_path: &fixture.live.runtime.path,
        frozen_components: &fixture.live.runtime.frozen,
        prepared_isolation: selected_isolation,
        model_package: &fixture.live.model_lease,
        static_model: &fixture.static_model,
        characterized_package: selected_characterized,
        runtime_installation_generation: runtime_generation,
        model_installation_generation: model_generation,
        managed_ollama: managed,
    };
    let plan = if matches!(cancellation_point, CancellationPoint::AfterDerivation) {
        VerifiedManagedJudgeEffectivePackagePlan::prepare_with_post_derivation(
            input,
            &cancellation,
            || cancellation.cancel(),
        )
    } else {
        VerifiedManagedJudgeEffectivePackagePlan::prepare(input, &cancellation)
    }?;
    plan.release()
        .map_err(|release| ManagedJudgeEffectivePackagePlanError {
            primary: ManagedJudgeEffectivePackageDerivationError::Relationship(
                ManagedJudgeEffectivePackageRelationship::Evidence,
            ),
            finalization: Some(release),
        })
}

fn substitute_requests(
    fault: Fault,
    fixture: &JudgeFixture,
    requests: CandidateJudgeRequestAggregateV1,
) -> CandidateJudgeRequestAggregateV1 {
    let mut request_ids = requests.structured_request_binding_ids().to_vec();
    if matches!(fault, Fault::ReversedRequests) {
        request_ids.reverse();
        return CandidateJudgeRequestAggregateV1::new(
            &fixture.judge_plan,
            &fixture.schedule,
            request_ids,
        )
        .expect("reversed request aggregate");
    }
    if matches!(fault, Fault::PortableLineage) {
        let other_plan = fixture.plan_with_presentation_seed(7);
        let other_schedule = CandidateJudgeScheduleV1::new(
            &other_plan,
            fixture.schedule.candidate_receipt_pair_set_id(),
        )
        .expect("foreign schedule");
        return CandidateJudgeRequestAggregateV1::new(&other_plan, &other_schedule, request_ids)
            .expect("foreign request aggregate");
    }
    requests
}

fn mutation(fault: SequenceFault) -> CompletedSequenceMutation {
    match fault {
        SequenceFault::RemoveLast => CompletedSequenceMutation::RemoveLast,
        SequenceFault::Swap => CompletedSequenceMutation::SwapFirstTwo,
        SequenceFault::Cursor => CompletedSequenceMutation::Cursor { index: 0, value: 1 },
        SequenceFault::FirstResponse => {
            CompletedSequenceMutation::FirstResponse { index: 0, value: 9 }
        }
        SequenceFault::LastResponse => CompletedSequenceMutation::LastResponse {
            index: 0,
            value: 15,
        },
        SequenceFault::ExecutionFirst => {
            CompletedSequenceMutation::ExecutionFirstResponse { index: 0 }
        }
        SequenceFault::ExecutionLast => {
            CompletedSequenceMutation::ExecutionLastResponse { index: 0 }
        }
        SequenceFault::FirstResidency => CompletedSequenceMutation::FirstResidency { index: 0 },
        SequenceFault::LastResidency => CompletedSequenceMutation::LastResidency { index: 0 },
        SequenceFault::RequestAndReceipt => CompletedSequenceMutation::Receipt { index: 0 },
        SequenceFault::Response => CompletedSequenceMutation::Response { index: 0 },
        SequenceFault::ReceiptBinding => CompletedSequenceMutation::ReceiptBinding { index: 0 },
        SequenceFault::Preflight => CompletedSequenceMutation::RetainedPreflight { index: 0 },
        SequenceFault::OfflineFirstState => CompletedSequenceMutation::EffectiveState {
            index: 0,
            value: Digest::sha256(b"offline final-attempt surrogate"),
        },
        SequenceFault::ProcessLeaf => CompletedSequenceMutation::Process {
            index: 0,
            value: Digest::sha256(b"changed process leaf"),
        },
    }
}

fn prepared_isolation() -> PreparedIsolation {
    PreparedIsolation::test_support_from_policy(
        IsolationPolicy::new(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(5),
            32,
            32,
            4_096,
            256,
            64,
        )
        .expect("isolation policy"),
    )
}

fn changed_runtime_state(fixture: &JudgeFixture) -> EffectiveRuntimeState {
    EffectiveRuntimeState::new(
        &fixture.live.runtime.runtime_build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "candidate-precursor-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: Digest::sha256(b"different characterized snapshot"),
            launch_policy_digest: Digest::sha256(b"launch policy"),
            loaded_components_digest: Digest::sha256(b"loaded components"),
            effective_configuration_digest: Digest::sha256(b"effective configuration"),
            platform_digest: Digest::sha256(b"platform"),
            execution_class_digest: Digest::sha256(b"runtime execution class"),
            isolation_policy_digest: fixture.live.runtime.prepared_isolation.policy_digest(),
            effective_context_tokens: 8_192,
            compute_backend: rewrite_model::ComputeBackend::NativeCpu,
            placement: rewrite_model::ExecutionPlacement::CpuOnly,
        },
    )
    .expect("changed runtime state")
}

fn assert_relationship(
    result: Result<ReleasedManagedJudgeEffectivePackageV2, ManagedJudgeEffectivePackagePlanError>,
    expected: ManagedJudgeEffectivePackageRelationship,
) {
    let error = result.expect_err("substitution must fail closed");
    assert!(matches!(
        error.primary(),
        ManagedJudgeEffectivePackageDerivationError::Relationship(actual) if *actual == expected
    ));
    assert!(format!("{error:?}").contains("relationship"));
    assert_eq!(
        error.to_string(),
        "managed judge effective-package preparation failed"
    );
    assert!(error.source().is_some());
}
use std::error::Error as _;
