use rewrite_model::{CandidateJudgePlanV1, CandidateJudgeScheduleV1};
use rewrite_types::CancellationToken;

use crate::effective_runtime_state_observation::{
    CompletedManagedJudgeObservationSequence, MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT,
    MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET, MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT,
};

use super::{
    ManagedJudgeObservationAuthorityError, ManagedJudgeObservationAuthorityInput,
    ManagedJudgeObservationAuthorityRelationship, ensure_active,
};

pub(super) fn validate(
    input: &ManagedJudgeObservationAuthorityInput,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeObservationAuthorityError> {
    validate_retained(
        &input.judge_plan,
        &input.judge_schedule,
        &input.completed_sequence,
        cancellation,
    )
}

pub(super) fn validate_retained(
    judge_plan: &CandidateJudgePlanV1,
    judge_schedule: &CandidateJudgeScheduleV1,
    completed_sequence: &CompletedManagedJudgeObservationSequence,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeObservationAuthorityError> {
    validate_schedule(judge_plan, judge_schedule, completed_sequence)?;
    validate_bindings(judge_schedule, completed_sequence, cancellation)
}

fn validate_schedule(
    judge_plan: &CandidateJudgePlanV1,
    judge_schedule: &CandidateJudgeScheduleV1,
    completed_sequence: &CompletedManagedJudgeObservationSequence,
) -> Result<(), ManagedJudgeObservationAuthorityError> {
    if judge_schedule.candidate_judge_plan_id() != judge_plan.candidate_judge_plan_id() {
        return Err(relationship(
            ManagedJudgeObservationAuthorityRelationship::PlanSchedule,
        ));
    }
    let expected =
        CandidateJudgeScheduleV1::new(judge_plan, judge_schedule.candidate_receipt_pair_set_id())
            .map_err(ManagedJudgeObservationAuthorityError::PortableContract)?;
    if &expected != judge_schedule {
        return Err(relationship(
            ManagedJudgeObservationAuthorityRelationship::PlanSchedule,
        ));
    }
    if completed_sequence.schedule_id() != judge_schedule.candidate_judge_schedule_id() {
        return Err(relationship(
            ManagedJudgeObservationAuthorityRelationship::SequenceSchedule,
        ));
    }
    Ok(())
}

fn validate_bindings(
    judge_schedule: &CandidateJudgeScheduleV1,
    completed_sequence: &CompletedManagedJudgeObservationSequence,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeObservationAuthorityError> {
    let bindings = completed_sequence.bindings();
    if bindings.len() != judge_schedule.entries().len() {
        return Err(relationship(
            ManagedJudgeObservationAuthorityRelationship::AttemptCount,
        ));
    }
    let mut expected_first = u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT) + 1;
    let mut retained_preflight = None;
    for (index, binding) in bindings.iter().enumerate() {
        ensure_active(cancellation)?;
        let expected_cursor =
            u32::try_from(index).expect("an exact V1 judge schedule has at most 512 entries");
        if binding.schedule_cursor() != expected_cursor {
            return Err(relationship(
                ManagedJudgeObservationAuthorityRelationship::ScheduleCursor,
            ));
        }
        let expected_last = checked_ordinal_add(
            expected_first,
            u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT) - 1,
        )?;
        let first_residency = ordinal(binding.first_residency_ordinal())?;
        let last_residency = ordinal(binding.last_residency_ordinal())?;
        let execution_first = ordinal(binding.receipt().execution().first_response_ordinal())?;
        let execution_last = ordinal(binding.receipt().execution().last_response_ordinal())?;
        let expected_first_residency = checked_ordinal_add(
            expected_first,
            u64::from(MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET),
        )?;
        if binding.first_response_ordinal() != expected_first
            || binding.last_response_ordinal() != expected_last
            || execution_first != binding.first_response_ordinal()
            || execution_last != binding.last_response_ordinal()
            || first_residency != expected_first_residency
            || last_residency != expected_last
        {
            return Err(relationship(
                ManagedJudgeObservationAuthorityRelationship::ResponseSpan,
            ));
        }
        if binding.complete_receipt_binding_digest() != &binding.receipt().complete_binding_digest()
        {
            return Err(relationship(
                ManagedJudgeObservationAuthorityRelationship::ResidencyReceipt,
            ));
        }
        match retained_preflight {
            None => retained_preflight = Some(binding.retained_preflight_digest()),
            Some(expected) if expected == binding.retained_preflight_digest() => {}
            Some(_) => {
                return Err(relationship(
                    ManagedJudgeObservationAuthorityRelationship::RetainedPreflight,
                ));
            }
        }
        expected_first = checked_ordinal_add(expected_last, 1)?;
    }
    Ok(())
}

fn ordinal(value: usize) -> Result<u64, ManagedJudgeObservationAuthorityError> {
    u64::try_from(value)
        .map_err(|_| relationship(ManagedJudgeObservationAuthorityRelationship::ResponseSpan))
}

fn checked_ordinal_add(
    value: u64,
    increment: u64,
) -> Result<u64, ManagedJudgeObservationAuthorityError> {
    value
        .checked_add(increment)
        .ok_or_else(|| relationship(ManagedJudgeObservationAuthorityRelationship::ResponseSpan))
}

const fn relationship(
    value: ManagedJudgeObservationAuthorityRelationship,
) -> ManagedJudgeObservationAuthorityError {
    ManagedJudgeObservationAuthorityError::Relationship(value)
}
