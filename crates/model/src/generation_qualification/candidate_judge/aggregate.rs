use std::{collections::HashSet, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::response::Wire as ResponseEntryWire;
use super::{
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgePlanV1, CandidateJudgeResponseV1,
    CandidateJudgeScheduleV1, MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES,
};
use crate::generation_qualification::codec::{
    append_count, append_digest, validate_canonical_json,
};
use crate::generation_qualification::{
    CANDIDATE_JUDGE_REQUEST_AGGREGATE_ID_DOMAIN, CANDIDATE_JUDGE_RESPONSE_AGGREGATE_ID_DOMAIN,
    CandidateJudgePlanId, CandidateJudgeRequestAggregateId, CandidateJudgeResponseAggregateId,
    CandidateJudgeScheduleId, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GenerationQualificationContractError, OllamaRetainedSessionResponseId,
    StructuredCompletionRequestBindingId,
};
use rewrite_types::Digest;

/// Content-free ordered identities for every scheduled structured request.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeRequestAggregateV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    structured_request_binding_ids: Vec<StructuredCompletionRequestBindingId>,
    #[serde(skip)]
    id: CandidateJudgeRequestAggregateId,
}

impl CandidateJudgeRequestAggregateV1 {
    /// Creates a complete schedule-order request aggregate.
    ///
    /// # Errors
    ///
    /// Returns an error unless the derived schedule-indexed response records form
    /// a unique complete array. Supplied provider transport IDs may repeat.
    pub fn new(
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        ids: Vec<StructuredCompletionRequestBindingId>,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(CANDIDATE_JUDGE_SCHEMA_VERSION, plan, schedule, ids, None)
    }

    fn build(
        schema_version: u32,
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        ids: Vec<StructuredCompletionRequestBindingId>,
        expected: Option<&RequestWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_common(schema_version, plan, schedule, &ids)?;
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: plan.candidate_judge_plan_id().clone(),
            candidate_judge_schedule_id: schedule.candidate_judge_schedule_id().clone(),
            structured_request_binding_ids: ids,
            id: CandidateJudgeRequestAggregateId(Digest::sha256(
                b"uninitialized judge request aggregate",
            )),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
            );
        }
        value.id = CandidateJudgeRequestAggregateId(aggregate_digest(
            CANDIDATE_JUDGE_REQUEST_AGGREGATE_ID_DOMAIN,
            value.candidate_judge_plan_id.digest(),
            value.candidate_judge_schedule_id.digest(),
            None,
            value
                .structured_request_binding_ids
                .iter()
                .map(StructuredCompletionRequestBindingId::digest),
        )?);
        validate_json_size(&value, MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES)?;
        Ok(value)
    }

    /// Parses bounded canonical JSON and reloads its plan and schedule.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: RequestWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let value = Self::build(
            wire.schema_version,
            plan,
            schedule,
            wire.structured_request_binding_ids.clone(),
            Some(&wire),
        )?;
        validate_canonical_json(bytes, &value)?;
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
    /// Returns request identities in exact schedule order.
    #[must_use]
    pub fn structured_request_binding_ids(&self) -> &[StructuredCompletionRequestBindingId] {
        &self.structured_request_binding_ids
    }
    /// Returns the checked request count.
    #[must_use]
    pub fn entry_count(&self) -> u32 {
        bounded_count(self.structured_request_binding_ids.len())
    }
    /// Returns the content-derived request aggregate identity.
    #[must_use]
    pub const fn request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        &self.id
    }
}

impl fmt::Debug for CandidateJudgeRequestAggregateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeRequestAggregateV1")
            .field("schema_version", &self.schema_version)
            .field("request_aggregate_id", &self.id)
            .field("entry_count", &self.structured_request_binding_ids.len())
            .finish_non_exhaustive()
    }
}

/// Content-free ordered identities for every scheduled structured response.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeResponseAggregateV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    candidate_judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    responses: Vec<CandidateJudgeResponseV1>,
    #[serde(skip)]
    retained_session_response_ids: Vec<OllamaRetainedSessionResponseId>,
    #[serde(skip)]
    id: CandidateJudgeResponseAggregateId,
}

impl CandidateJudgeResponseAggregateV1 {
    /// Creates a complete schedule-order response aggregate.
    ///
    /// # Errors
    ///
    /// Returns an error unless the derived schedule-indexed response records form a unique,
    /// complete schedule-order array. Raw provider response IDs may repeat because they are nested
    /// evidence, not the portable schedule association.
    pub fn new(
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        ids: Vec<OllamaRetainedSessionResponseId>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let responses = ids
            .into_iter()
            .enumerate()
            .map(|(index, response_id)| {
                CandidateJudgeResponseV1::new(plan, schedule, requests, index, response_id)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::build(
            CANDIDATE_JUDGE_SCHEMA_VERSION,
            plan,
            schedule,
            requests,
            responses,
            None,
        )
    }

    fn build(
        schema_version: u32,
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        responses: Vec<CandidateJudgeResponseV1>,
        expected: Option<&ResponseWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_response_entries(schema_version, plan, schedule, requests, &responses)?;
        let retained_session_response_ids = responses
            .iter()
            .map(|response| response.retained_session_response_id().clone())
            .collect();
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: plan.candidate_judge_plan_id().clone(),
            candidate_judge_schedule_id: schedule.candidate_judge_schedule_id().clone(),
            candidate_judge_request_aggregate_id: requests.request_aggregate_id().clone(),
            responses,
            retained_session_response_ids,
            id: CandidateJudgeResponseAggregateId(Digest::sha256(
                b"uninitialized judge response aggregate",
            )),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
            );
        }
        value.id = CandidateJudgeResponseAggregateId(aggregate_digest(
            CANDIDATE_JUDGE_RESPONSE_AGGREGATE_ID_DOMAIN,
            value.candidate_judge_plan_id.digest(),
            value.candidate_judge_schedule_id.digest(),
            Some(value.candidate_judge_request_aggregate_id.digest()),
            value
                .responses
                .iter()
                .map(|response| response.candidate_judge_response_id().digest()),
        )?);
        validate_json_size(&value, MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES)?;
        Ok(value)
    }

    /// Parses bounded canonical JSON and reloads its plan, schedule, and request aggregate.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: ResponseWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let responses = wire
            .responses
            .iter()
            .map(|response| CandidateJudgeResponseV1::from_wire(response, plan, schedule, requests))
            .collect::<Result<Vec<_>, _>>()?;
        let value = Self::build(
            wire.schema_version,
            plan,
            schedule,
            requests,
            responses,
            Some(&wire),
        )?;
        validate_canonical_json(bytes, &value)?;
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
    /// Returns the exact request aggregate used to derive every response identity.
    #[must_use]
    pub const fn candidate_judge_request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        &self.candidate_judge_request_aggregate_id
    }
    /// Returns schedule-indexed response records in exact schedule order.
    #[must_use]
    pub fn responses(&self) -> &[CandidateJudgeResponseV1] {
        &self.responses
    }
    /// Returns response identities in exact schedule order.
    #[must_use]
    pub fn retained_session_response_ids(&self) -> &[OllamaRetainedSessionResponseId] {
        &self.retained_session_response_ids
    }
    /// Returns the checked response count.
    #[must_use]
    pub fn entry_count(&self) -> u32 {
        bounded_count(self.responses.len())
    }
    /// Returns the content-derived response aggregate identity.
    #[must_use]
    pub const fn response_aggregate_id(&self) -> &CandidateJudgeResponseAggregateId {
        &self.id
    }
}

impl fmt::Debug for CandidateJudgeResponseAggregateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeResponseAggregateV1")
            .field("schema_version", &self.schema_version)
            .field("response_aggregate_id", &self.id)
            .field("entry_count", &self.responses.len())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestWire {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    structured_request_binding_ids: Vec<StructuredCompletionRequestBindingId>,
}

impl RequestWire {
    fn matches(&self, value: &CandidateJudgeRequestAggregateV1) -> bool {
        self.schema_version == value.schema_version
            && self.candidate_judge_plan_id == value.candidate_judge_plan_id
            && self.candidate_judge_schedule_id == value.candidate_judge_schedule_id
            && self.structured_request_binding_ids == value.structured_request_binding_ids
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseWire {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    candidate_judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    responses: Vec<ResponseEntryWire>,
}

impl ResponseWire {
    fn matches(&self, value: &CandidateJudgeResponseAggregateV1) -> bool {
        self.schema_version == value.schema_version
            && self.candidate_judge_plan_id == value.candidate_judge_plan_id
            && self.candidate_judge_schedule_id == value.candidate_judge_schedule_id
            && self.candidate_judge_request_aggregate_id
                == value.candidate_judge_request_aggregate_id
            && self.responses.len() == value.responses.len()
            && self
                .responses
                .iter()
                .zip(&value.responses)
                .all(|(wire, response)| wire.matches(response))
    }
}

fn validate_response_entries(
    schema_version: u32,
    plan: &CandidateJudgePlanV1,
    schedule: &CandidateJudgeScheduleV1,
    requests: &CandidateJudgeRequestAggregateV1,
    responses: &[CandidateJudgeResponseV1],
) -> Result<(), GenerationQualificationContractError> {
    validate_common(schema_version, plan, schedule, responses)?;
    if requests.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || requests.candidate_judge_schedule_id() != schedule.candidate_judge_schedule_id()
        || requests.entry_count() != schedule.entry_count()
        || responses.iter().enumerate().any(|(index, response)| {
            response.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
                || response.candidate_judge_schedule_id() != schedule.candidate_judge_schedule_id()
                || usize::try_from(response.schedule_index()) != Ok(index)
                || requests.structured_request_binding_ids().get(index)
                    != Some(response.structured_request_binding_id())
        })
    {
        return Err(
            GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
        );
    }
    Ok(())
}

fn validate_common<T: Eq + std::hash::Hash>(
    schema_version: u32,
    plan: &CandidateJudgePlanV1,
    schedule: &CandidateJudgeScheduleV1,
    ids: &[T],
) -> Result<(), GenerationQualificationContractError> {
    if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
        || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
    {
        return Err(GenerationQualificationContractError::UnsupportedSchema(
            schema_version,
        ));
    }
    if schedule.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || ids.len() != schedule.entries().len()
        || ids.is_empty()
        || ids.iter().collect::<HashSet<_>>().len() != ids.len()
    {
        return Err(
            GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch,
        );
    }
    Ok(())
}

fn aggregate_digest<'a>(
    domain: &[u8],
    plan_id: &Digest,
    schedule_id: &Digest,
    relation_id: Option<&Digest>,
    ids: impl Iterator<Item = &'a Digest>,
) -> Result<Digest, GenerationQualificationContractError> {
    let values = ids.collect::<Vec<_>>();
    let mut canonical = domain.to_vec();
    append_digest(&mut canonical, plan_id);
    append_digest(&mut canonical, schedule_id);
    if let Some(relation_id) = relation_id {
        append_digest(&mut canonical, relation_id);
    }
    append_count(&mut canonical, values.len())?;
    for value in values {
        append_digest(&mut canonical, value);
    }
    Ok(Digest::sha256(&canonical))
}

fn validate_json_size(
    value: &impl Serialize,
    maximum: usize,
) -> Result<(), GenerationQualificationContractError> {
    if serde_json::to_vec(value)
        .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
        .len()
        > maximum
    {
        return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
    }
    Ok(())
}

fn bounded_count(value: usize) -> u32 {
    u32::try_from(value).expect("candidate judge aggregates are bounded to 512 entries")
}
