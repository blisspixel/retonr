use std::cell::RefCell;

use rewrite_model::{
    CandidateJudgePlanId, CandidateJudgeRequestAggregateId, CandidateJudgeRequestAggregateV1,
    CandidateJudgeScheduleId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId,
    GenerationSystemRecordV1, ManagedOllamaEffectiveRuntimeStateJoinId,
    StructuredCompletionRequestBindingId,
};
use rewrite_types::{CancellationToken, Digest};

use crate::candidate_attempt_precursor::tests::support::launch;
use crate::effective_runtime_state_observation::completed_sequence_for_effective_package;
use crate::managed_judge_precursor::JudgeFixture;
use crate::{
    ManagedJudgeObservationAuthorityCompiler, ManagedJudgeObservationAuthorityInput,
    PackageAttestationService,
};

use super::release::run_finalizers;
use super::*;

#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/finalization.rs"]
mod finalization;

#[test]
fn every_finalizer_runs_and_preserves_each_failure() {
    for mask in 0_u8..16 {
        let order = RefCell::new(Vec::new());
        let (cleanup, model, runtime, evidence) = run_finalizers(
            || {
                order.borrow_mut().push("cleanup");
                (mask & 1 == 0).then_some(()).ok_or("cleanup")
            },
            || {
                order.borrow_mut().push("model");
                (mask & 2 == 0).then_some(()).ok_or("model")
            },
            || {
                order.borrow_mut().push("runtime");
                (mask & 4 == 0).then_some(()).ok_or("runtime")
            },
            || {
                order.borrow_mut().push("evidence");
                (mask & 8 == 0).then_some(()).ok_or("evidence")
            },
        );
        assert_eq!(*order.borrow(), ["cleanup", "model", "runtime", "evidence"]);
        assert_eq!(cleanup.is_some(), mask & 1 != 0);
        assert_eq!(model.is_some(), mask & 2 != 0);
        assert_eq!(runtime.is_some(), mask & 4 != 0);
        assert_eq!(evidence.is_some(), mask & 8 != 0);
    }
}

#[test]
fn complete_all_attempt_batch_releases_distinct_schedule_capability() {
    let mut fixture = JudgeFixture::new();
    let managed = managed_bracket(
        &fixture.live.model_lease,
        &fixture.live.runtime.runtime_manifest,
        &fixture.live.runtime.runtime_package,
        &fixture.live.runtime.admitted,
        &fixture.live.runtime.prepared_isolation,
    );
    let (authority, requests) = authority_and_requests(&fixture, &managed);
    let runtime_generation = fixture
        .live
        .runtime
        .runtime_package
        .installation_key()
        .installation_generation();
    let model_generation = fixture
        .live
        .model_lease
        .private_view()
        .installation_generation();
    let expected = ExpectedRelease::new(
        &authority,
        &requests,
        &fixture,
        runtime_generation,
        model_generation,
    );
    let plan = VerifiedManagedJudgeEffectivePackagePlan::prepare(
        ManagedJudgeEffectivePackagePlanInput {
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
            prepared_isolation: &fixture.live.runtime.prepared_isolation,
            model_package: &fixture.live.model_lease,
            static_model: &fixture.static_model,
            characterized_package: &fixture.characterized,
            runtime_installation_generation: runtime_generation,
            model_installation_generation: model_generation,
            managed_ollama: managed,
        },
        &CancellationToken::new(),
    )
    .expect("batch effective-package plan");
    let plan_debug = format!("{plan:?}");
    assert!(plan_debug.contains("attempt_count"));
    assert!(!plan_debug.contains("managed judge input"));
    let released = plan.release().expect("cleanup-gated batch release");
    assert_release(released, &expected);
}

struct ExpectedRelease {
    plan: CandidateJudgePlanId,
    schedule: CandidateJudgeScheduleId,
    requests: CandidateJudgeRequestAggregateId,
    system: GenerationSystemRecordV1,
    evidence: EffectivePackageEvidenceV2Id,
    state: EffectiveRuntimeStateId,
    preflight: Digest,
    retained_preflight: Digest,
    residency: Digest,
    process: Digest,
    native: Digest,
    connection: Digest,
    effective: Digest,
    join: ManagedOllamaEffectiveRuntimeStateJoinId,
    foundation: ModelPackageFoundationId,
    license: ModelLicenseControlId,
    runtime_generation: u64,
    model_generation: u64,
}

impl ExpectedRelease {
    fn new(
        authority: &VerifiedManagedJudgeObservationAuthority,
        requests: &CandidateJudgeRequestAggregateV1,
        fixture: &JudgeFixture,
        runtime_generation: u64,
        model_generation: u64,
    ) -> Self {
        Self {
            plan: authority.judge_plan_id().clone(),
            schedule: authority.judge_schedule_id().clone(),
            requests: requests.request_aggregate_id().clone(),
            system: fixture.judge_system.clone(),
            evidence: fixture
                .characterized
                .evidence()
                .effective_package_evidence_v2_id(),
            state: fixture
                .live
                .runtime
                .runtime_state
                .effective_runtime_state_id(),
            preflight: authority.preflight_observer_binding_digest().clone(),
            retained_preflight: authority.retained_session_preflight_digest().clone(),
            residency: authority.residency_receipt_aggregate_digest().clone(),
            process: authority.process_observation_aggregate_digest().clone(),
            native: authority.native_load_observation_aggregate_digest().clone(),
            connection: authority.connection_observation_aggregate_digest().clone(),
            effective: authority
                .effective_runtime_state_observation_aggregate_digest()
                .clone(),
            join: authority.effective_runtime_state_join_id().clone(),
            foundation: fixture.live.model_lease.foundation_id().clone(),
            license: fixture.characterized.license_control_id().clone(),
            runtime_generation,
            model_generation,
        }
    }
}

fn assert_release(
    mut released: ReleasedManagedJudgeEffectivePackageV2,
    expected: &ExpectedRelease,
) {
    assert_release_accessors(&mut released, expected);
    let parts = released.into_receipt_parts();
    assert_eq!(
        parts.evidence.effective_package_evidence_v2_id(),
        expected.evidence
    );
    assert_eq!(
        parts.artifact_set.artifact_set_id(),
        parts.evidence.artifact_set_id().clone()
    );
    assert_eq!(
        &parts.runtime_build.runtime_build_id(),
        parts.evidence.runtime_build_id()
    );
    assert_eq!(parts.observation_authority.attempt_count(), 2);
    assert_eq!(
        parts.request_aggregate.request_aggregate_id(),
        &expected.requests
    );
    assert_eq!(parts.judge_system, expected.system);
    assert_eq!(
        parts.expected_runtime_state.effective_runtime_state_id(),
        expected.state
    );
    assert_eq!(
        parts.runtime_installation_generation,
        expected.runtime_generation
    );
    assert_eq!(
        parts.model_installation_generation,
        expected.model_generation
    );
    assert_eq!(parts.foundation_id, expected.foundation);
    assert_eq!(parts.license_control_id, expected.license);
    assert_eq!(parts.first_response_ordinal, 8);
    assert_eq!(parts.last_response_ordinal, 25);
}

fn assert_release_accessors(
    released: &mut ReleasedManagedJudgeEffectivePackageV2,
    expected: &ExpectedRelease,
) {
    released
        .revalidate_retained_authority(&CancellationToken::new())
        .expect("released authority revalidation");
    assert_eq!(released.judge_plan_id(), &expected.plan);
    assert_eq!(released.judge_schedule_id(), &expected.schedule);
    assert_eq!(released.request_aggregate_id(), &expected.requests);
    assert_eq!(
        released.judge_generation_system_id(),
        expected.system.generation_system_id()
    );
    assert_eq!(
        released.effective_package_evidence_v2_id(),
        expected.evidence
    );
    assert_eq!(released.attempt_count(), 2);
    assert_eq!(released.first_response_ordinal(), 8);
    assert_eq!(released.last_response_ordinal(), 25);
    assert_eq!(released.judge_system(), &expected.system);
    assert_eq!(released.effective_runtime_state_id(), expected.state);
    assert_eq!(
        released.preflight_observer_binding_digest(),
        &expected.preflight
    );
    assert_eq!(
        released.retained_session_preflight_digest(),
        &expected.retained_preflight
    );
    assert_eq!(
        released.residency_receipt_aggregate_digest(),
        &expected.residency
    );
    assert_eq!(
        released.process_observation_aggregate_digest(),
        &expected.process
    );
    assert_eq!(
        released.native_load_observation_aggregate_digest(),
        &expected.native
    );
    assert_eq!(
        released.connection_observation_aggregate_digest(),
        &expected.connection
    );
    assert_eq!(
        released.effective_runtime_state_observation_aggregate_digest(),
        &expected.effective
    );
    assert_eq!(released.effective_runtime_state_join_id(), &expected.join);
    assert_eq!(
        released.runtime_installation_generation(),
        expected.runtime_generation
    );
    assert_eq!(
        released.model_installation_generation(),
        expected.model_generation
    );
    assert_eq!(released.foundation_id(), &expected.foundation);
    assert_eq!(released.license_control_id(), &expected.license);
    assert!(!format!("{released:?}").contains("managed judge input"));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        released.revalidate_retained_authority(&cancelled),
        Err(ManagedJudgeEffectivePackageFinalValidationError::Authority(
            _
        ))
    ));
    released.runtime_installation_generation = 0;
    assert!(matches!(
        released.revalidate_retained_authority(&CancellationToken::new()),
        Err(ManagedJudgeEffectivePackageFinalValidationError::Relationship)
    ));
    released.runtime_installation_generation = expected.runtime_generation;
}

fn managed_bracket<'model>(
    model: &'model VerifiedManagedOllamaModelPackageLease,
    runtime_manifest: &RuntimePackageManifest,
    runtime_package: &RuntimePackageLease,
    admitted_runtime: &VerifiedAdmittedRuntime,
    isolation: &PreparedIsolation,
) -> ManagedOllamaIsolationLease<'model> {
    PackageAttestationService::managed_ollama_test_fixture(
        runtime_manifest,
        runtime_package,
        admitted_runtime,
        isolation,
        launch(model, "latest"),
    )
    .expect("sealed managed bracket")
}

fn authority_and_requests(
    fixture: &JudgeFixture,
    managed: &ManagedOllamaIsolationLease<'_>,
) -> (
    VerifiedManagedJudgeObservationAuthority,
    CandidateJudgeRequestAggregateV1,
) {
    let sequence = completed_sequence_for_effective_package(
        fixture.schedule.candidate_judge_schedule_id(),
        fixture.schedule.entry_count(),
        &fixture.live.runtime.runtime_state,
        &fixture.live.runtime.admitted,
        &fixture.live.runtime.path,
        &fixture.live.runtime.runtime_package,
        managed,
    );
    let request_ids = sequence
        .bindings()
        .iter()
        .map(|binding| {
            StructuredCompletionRequestBindingId::from_derived_digest(
                binding.request_binding_digest().clone(),
            )
        })
        .collect();
    let requests =
        CandidateJudgeRequestAggregateV1::new(&fixture.judge_plan, &fixture.schedule, request_ids)
            .expect("exact batch requests");
    let authority = ManagedJudgeObservationAuthorityCompiler::compile(
        ManagedJudgeObservationAuthorityInput {
            judge_plan: fixture.judge_plan.clone(),
            judge_schedule: fixture.schedule.clone(),
            completed_sequence: sequence,
        },
        &CancellationToken::new(),
    )
    .expect("batch observation authority");
    (authority, requests)
}
