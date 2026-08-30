use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgePlanV1, CandidateJudgeRequestAggregateV1,
    CandidateJudgeScheduleV1,
};
use crate::generation_qualification::codec::{append_digest, append_u32};
use crate::generation_qualification::{
    CANDIDATE_JUDGE_RESPONSE_ID_DOMAIN, CandidateJudgePlanId, CandidateJudgeResponseId,
    CandidateJudgeScheduleId, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GenerationQualificationContractError, OllamaRetainedSessionResponseId,
    StructuredCompletionRequestBindingId,
};
use rewrite_types::Digest;

/// One inert provider response identity bound to its derived judge-schedule position.
#[derive(Clone, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeResponseV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    schedule_index: u32,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    ollama_retained_session_response_id: OllamaRetainedSessionResponseId,
    #[serde(skip)]
    id: CandidateJudgeResponseId,
}

impl CandidateJudgeResponseV1 {
    pub(super) fn new(
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        schedule_index: usize,
        response_id: OllamaRetainedSessionResponseId,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            CANDIDATE_JUDGE_SCHEMA_VERSION,
            plan,
            schedule,
            requests,
            schedule_index,
            response_id,
            None,
        )
    }

    pub(super) fn from_wire(
        wire: &Wire,
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        let schedule_index = usize::try_from(wire.schedule_index)
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        Self::build(
            wire.schema_version,
            plan,
            schedule,
            requests,
            schedule_index,
            wire.ollama_retained_session_response_id.clone(),
            Some(wire),
        )
    }

    fn build(
        schema_version: u32,
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        schedule_index: usize,
        response_id: OllamaRetainedSessionResponseId,
        expected: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
            || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
        {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        let index = u32::try_from(schedule_index)
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        let request_id = requests
            .structured_request_binding_ids()
            .get(schedule_index)
            .ok_or(
                GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
            )?;
        if schedule.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
            || requests.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
            || requests.candidate_judge_schedule_id() != schedule.candidate_judge_schedule_id()
            || schedule.entries().get(schedule_index).is_none()
        {
            return Err(
                GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
            );
        }
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: plan.candidate_judge_plan_id().clone(),
            candidate_judge_schedule_id: schedule.candidate_judge_schedule_id().clone(),
            schedule_index: index,
            structured_request_binding_id: request_id.clone(),
            ollama_retained_session_response_id: response_id,
            id: CandidateJudgeResponseId(Digest::sha256(b"uninitialized judge response")),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
            );
        }
        let mut canonical = CANDIDATE_JUDGE_RESPONSE_ID_DOMAIN.to_vec();
        append_u32(&mut canonical, value.schema_version);
        append_digest(&mut canonical, value.candidate_judge_plan_id.digest());
        append_digest(&mut canonical, value.candidate_judge_schedule_id.digest());
        append_u32(&mut canonical, value.schedule_index);
        append_digest(&mut canonical, value.structured_request_binding_id.digest());
        append_digest(
            &mut canonical,
            value.ollama_retained_session_response_id.digest(),
        );
        value.id = CandidateJudgeResponseId(Digest::sha256(&canonical));
        Ok(value)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact judge-plan identity.
    #[must_use]
    pub const fn candidate_judge_plan_id(&self) -> &CandidateJudgePlanId {
        &self.candidate_judge_plan_id
    }

    /// Returns the exact judge-schedule identity.
    #[must_use]
    pub const fn candidate_judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        &self.candidate_judge_schedule_id
    }

    /// Returns the exact zero-based schedule position.
    #[must_use]
    pub const fn schedule_index(&self) -> u32 {
        self.schedule_index
    }

    /// Returns the exact structured request bound at this schedule position.
    #[must_use]
    pub const fn structured_request_binding_id(&self) -> &StructuredCompletionRequestBindingId {
        &self.structured_request_binding_id
    }

    /// Returns the nested provider transport response identity.
    #[must_use]
    pub const fn retained_session_response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.ollama_retained_session_response_id
    }

    /// Returns the schedule-indexed portable response identity.
    #[must_use]
    pub const fn candidate_judge_response_id(&self) -> &CandidateJudgeResponseId {
        &self.id
    }
}

impl fmt::Debug for CandidateJudgeResponseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeResponseV1")
            .field("candidate_judge_response_id", &self.id)
            .field("schedule_index", &self.schedule_index)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Wire {
    pub(super) schema_version: u32,
    pub(super) candidate_judge_plan_id: CandidateJudgePlanId,
    pub(super) candidate_judge_schedule_id: CandidateJudgeScheduleId,
    pub(super) schedule_index: u32,
    pub(super) structured_request_binding_id: StructuredCompletionRequestBindingId,
    pub(super) ollama_retained_session_response_id: OllamaRetainedSessionResponseId,
}

impl Wire {
    pub(super) fn matches(&self, value: &CandidateJudgeResponseV1) -> bool {
        self.schema_version == value.schema_version
            && self.candidate_judge_plan_id == value.candidate_judge_plan_id
            && self.candidate_judge_schedule_id == value.candidate_judge_schedule_id
            && self.schedule_index == value.schedule_index
            && self.structured_request_binding_id == value.structured_request_binding_id
            && self.ollama_retained_session_response_id == value.ollama_retained_session_response_id
    }
}
