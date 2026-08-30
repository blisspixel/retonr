use rewrite_model::{
    CandidateJudgePlanV1, CandidateJudgeScheduleV1,
    MANAGED_LOCAL_JUDGE_CONNECTION_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_EFFECTIVE_RUNTIME_STATE_JOIN_DOMAIN,
    MANAGED_LOCAL_JUDGE_EFFECTIVE_STATE_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_NATIVE_LOAD_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_PROCESS_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_RESIDENCY_AGGREGATE_DOMAIN, ManagedOllamaEffectiveRuntimeStateJoinId,
};
use rewrite_types::{CancellationToken, Digest};

use crate::effective_runtime_state_observation::{
    CompletedManagedJudgeObservationSequence, ManagedJudgeAttemptObserverBinding,
};

use super::{
    MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN, ManagedJudgeObservationAuthorityError,
    ManagedJudgeObservationAuthorityInput, ensure_active,
};

pub(super) struct DerivedAggregates {
    pub(super) attempt_count: u64,
    pub(super) preflight_observer: Digest,
    pub(super) retained_preflight: Digest,
    pub(super) residency: Digest,
    pub(super) process: Digest,
    pub(super) native_load: Digest,
    pub(super) connection: Digest,
    pub(super) effective_state: Digest,
    pub(super) effective_state_join: ManagedOllamaEffectiveRuntimeStateJoinId,
}

pub(super) fn derive(
    input: &ManagedJudgeObservationAuthorityInput,
    cancellation: &CancellationToken,
) -> Result<DerivedAggregates, ManagedJudgeObservationAuthorityError> {
    derive_retained(
        &input.judge_plan,
        &input.judge_schedule,
        &input.completed_sequence,
        cancellation,
    )
}

pub(super) fn derive_retained(
    judge_plan: &CandidateJudgePlanV1,
    judge_schedule: &CandidateJudgeScheduleV1,
    completed_sequence: &CompletedManagedJudgeObservationSequence,
    cancellation: &CancellationToken,
) -> Result<DerivedAggregates, ManagedJudgeObservationAuthorityError> {
    let bindings = completed_sequence.bindings();
    let attempt_count =
        u64::try_from(bindings.len()).expect("an exact V1 judge schedule has at most 512 entries");
    let preflight_observer = derive_preflight_observer(judge_schedule, completed_sequence);
    let retained_preflight = bindings
        .first()
        .expect("validated managed judge schedule is nonempty")
        .retained_preflight_digest()
        .clone();
    let residency = aggregate(
        MANAGED_LOCAL_JUDGE_RESIDENCY_AGGREGATE_DOMAIN,
        judge_plan,
        judge_schedule,
        completed_sequence,
        LeafKind::Residency,
        cancellation,
    )?;
    let process = aggregate(
        MANAGED_LOCAL_JUDGE_PROCESS_OBSERVATION_AGGREGATE_DOMAIN,
        judge_plan,
        judge_schedule,
        completed_sequence,
        LeafKind::Process,
        cancellation,
    )?;
    let native_load = aggregate(
        MANAGED_LOCAL_JUDGE_NATIVE_LOAD_OBSERVATION_AGGREGATE_DOMAIN,
        judge_plan,
        judge_schedule,
        completed_sequence,
        LeafKind::NativeLoad,
        cancellation,
    )?;
    let connection = aggregate(
        MANAGED_LOCAL_JUDGE_CONNECTION_OBSERVATION_AGGREGATE_DOMAIN,
        judge_plan,
        judge_schedule,
        completed_sequence,
        LeafKind::Connection,
        cancellation,
    )?;
    let effective_state = aggregate(
        MANAGED_LOCAL_JUDGE_EFFECTIVE_STATE_OBSERVATION_AGGREGATE_DOMAIN,
        judge_plan,
        judge_schedule,
        completed_sequence,
        LeafKind::EffectiveState,
        cancellation,
    )?;
    ensure_active(cancellation)?;
    let mut join =
        Vec::with_capacity(MANAGED_LOCAL_JUDGE_EFFECTIVE_RUNTIME_STATE_JOIN_DOMAIN.len() + 64 * 3);
    join.extend_from_slice(MANAGED_LOCAL_JUDGE_EFFECTIVE_RUNTIME_STATE_JOIN_DOMAIN);
    join.extend_from_slice(
        judge_plan
            .candidate_judge_plan_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    join.extend_from_slice(
        judge_schedule
            .candidate_judge_schedule_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    join.extend_from_slice(effective_state.as_str().as_bytes());
    let effective_state_join =
        ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(Digest::sha256(&join));
    Ok(DerivedAggregates {
        attempt_count,
        preflight_observer,
        retained_preflight,
        residency,
        process,
        native_load,
        connection,
        effective_state,
        effective_state_join,
    })
}

fn derive_preflight_observer(
    judge_schedule: &CandidateJudgeScheduleV1,
    completed_sequence: &CompletedManagedJudgeObservationSequence,
) -> Digest {
    let preflight = completed_sequence.preflight();
    preflight_observer_binding(
        judge_schedule.candidate_judge_schedule_id(),
        preflight.initial_process().evidence_digest(),
        preflight.post_preflight_process().evidence_digest(),
        preflight.final_process().evidence_digest(),
        preflight
            .native_load()
            .native_load_observation_id()
            .digest(),
        preflight.connection_binding_digest(),
    )
}

pub(super) fn preflight_observer_binding(
    schedule_id: &rewrite_model::CandidateJudgeScheduleId,
    initial_process: &Digest,
    post_preflight_process: &Digest,
    final_process: &Digest,
    native_load: &Digest,
    connection: &Digest,
) -> Digest {
    let mut material =
        Vec::with_capacity(MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN.len() + 64 * 6);
    material.extend_from_slice(MANAGED_JUDGE_PREFLIGHT_OBSERVER_BINDING_DOMAIN);
    material.extend_from_slice(schedule_id.digest().as_str().as_bytes());
    material.extend_from_slice(initial_process.as_str().as_bytes());
    material.extend_from_slice(post_preflight_process.as_str().as_bytes());
    material.extend_from_slice(final_process.as_str().as_bytes());
    material.extend_from_slice(native_load.as_str().as_bytes());
    material.extend_from_slice(connection.as_str().as_bytes());
    Digest::sha256(&material)
}

#[derive(Clone, Copy)]
enum LeafKind {
    Residency,
    Process,
    NativeLoad,
    Connection,
    EffectiveState,
}

fn aggregate(
    domain: &[u8],
    judge_plan: &CandidateJudgePlanV1,
    judge_schedule: &CandidateJudgeScheduleV1,
    completed_sequence: &CompletedManagedJudgeObservationSequence,
    leaf_kind: LeafKind,
    cancellation: &CancellationToken,
) -> Result<Digest, ManagedJudgeObservationAuthorityError> {
    let bindings = completed_sequence.bindings();
    let count =
        u64::try_from(bindings.len()).expect("an exact V1 judge schedule has at most 512 entries");
    let mut material = Vec::with_capacity(domain.len() + 64 * 2 + 8 + bindings.len() * 68);
    material.extend_from_slice(domain);
    material.extend_from_slice(
        judge_plan
            .candidate_judge_plan_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    material.extend_from_slice(
        judge_schedule
            .candidate_judge_schedule_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    material.extend_from_slice(&count.to_be_bytes());
    for (index, binding) in bindings.iter().enumerate() {
        ensure_active(cancellation)?;
        let index =
            u32::try_from(index).expect("an exact V1 judge schedule has at most 512 entries");
        material.extend_from_slice(&index.to_be_bytes());
        material.extend_from_slice(leaf(leaf_kind, binding).as_str().as_bytes());
    }
    Ok(Digest::sha256(&material))
}

fn leaf(kind: LeafKind, binding: &ManagedJudgeAttemptObserverBinding) -> &Digest {
    match kind {
        LeafKind::Residency => binding.complete_receipt_binding_digest(),
        LeafKind::Process => binding.process_binding_digest(),
        LeafKind::NativeLoad => binding.native_load_binding_digest(),
        LeafKind::Connection => binding.connection_binding_digest(),
        LeafKind::EffectiveState => binding.effective_state_binding_digest(),
    }
}
