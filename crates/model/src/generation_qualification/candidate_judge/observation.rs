use std::{collections::HashSet, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::response::Wire as ResponseWire;
use super::{
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgePlanV1, CandidateJudgePresentationV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseV1, CandidateJudgeScheduleV1,
    MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES,
};
use crate::generation_qualification::codec::{
    append_count, append_digest, append_text, append_u32, validate_canonical_json,
};
use crate::generation_qualification::{
    CANDIDATE_JUDGE_OBSERVATION_BATCH_ID_DOMAIN, CANDIDATE_JUDGE_OBSERVATION_ID_DOMAIN,
    CandidateJudgeObservationBatchId, CandidateJudgeObservationId, CandidateJudgePlanId,
    CandidateJudgeRequestAggregateId, CandidateJudgeScheduleId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId,
    GenerationQualificationContractError, OllamaRetainedSessionResponseId,
};
use rewrite_types::Digest;

/// Normalized structured choice returned by the local judge.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateJudgeChoiceV1 {
    /// The first presented candidate is preferred.
    First,
    /// The second presented candidate is preferred.
    Second,
    /// The presented candidates are equivalent.
    Tie,
    /// The judge cannot make the bounded choice.
    Abstain,
}

impl CandidateJudgeChoiceV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::First => 0,
            Self::Second => 1,
            Self::Tie => 2,
            Self::Abstain => 3,
        }
    }
}

/// One content-free normalized observation bound to an exact schedule entry.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeObservationV1 {
    schema_version: u32,
    case_id: GenerationCaseId,
    presentation: CandidateJudgePresentationV1,
    attempt_ordinal: u32,
    candidate_judge_response: CandidateJudgeResponseV1,
    choice: CandidateJudgeChoiceV1,
    cited_rubric_clause_ids: Vec<String>,
    #[serde(skip)]
    id: CandidateJudgeObservationId,
}

impl CandidateJudgeObservationV1 {
    /// Creates one normalized observation after checking its schedule and rubric closure.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign schedule entry or invalid clause closure.
    pub fn new(
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        schedule_index: usize,
        response_id: OllamaRetainedSessionResponseId,
        choice: CandidateJudgeChoiceV1,
        cited_rubric_clause_ids: Vec<String>,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            CANDIDATE_JUDGE_SCHEMA_VERSION,
            plan,
            schedule,
            requests,
            schedule_index,
            response_id,
            choice,
            cited_rubric_clause_ids,
            None,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "wire reconstruction uses the frozen fields"
    )]
    fn build(
        schema_version: u32,
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        schedule_index: usize,
        response_id: OllamaRetainedSessionResponseId,
        choice: CandidateJudgeChoiceV1,
        cited_rubric_clause_ids: Vec<String>,
        expected: Option<&ObservationWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
            || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
        {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        let entry = schedule.entries().get(schedule_index).ok_or(
            GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
        )?;
        let case = plan
            .cases()
            .iter()
            .find(|case| case.case_id() == entry.case_id())
            .ok_or(
                GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
            )?;
        if schedule.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
            || cited_rubric_clause_ids.is_empty()
            || cited_rubric_clause_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || cited_rubric_clause_ids
                .iter()
                .any(|clause| !case.rubric_clause_ids().contains(clause))
        {
            return Err(
                GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
            );
        }
        let candidate_judge_response = match expected {
            Some(wire) => CandidateJudgeResponseV1::from_wire(
                &wire.candidate_judge_response,
                plan,
                schedule,
                requests,
            )
            .map_err(map_response_error)?,
            None => CandidateJudgeResponseV1::new(
                plan,
                schedule,
                requests,
                schedule_index,
                response_id,
            )?,
        };
        let mut value = Self {
            schema_version,
            case_id: entry.case_id().clone(),
            presentation: entry.presentation(),
            attempt_ordinal: entry.attempt_ordinal(),
            candidate_judge_response,
            choice,
            cited_rubric_clause_ids,
            id: CandidateJudgeObservationId(Digest::sha256(b"uninitialized judge observation")),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
            );
        }
        value.id = CandidateJudgeObservationId(Digest::sha256(&value.canonical_bytes()?));
        Ok(value)
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let mut output = CANDIDATE_JUDGE_OBSERVATION_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.case_id.digest());
        output.push(self.presentation.tag());
        append_u32(&mut output, self.attempt_ordinal);
        append_digest(
            &mut output,
            self.candidate_judge_response
                .candidate_judge_response_id()
                .digest(),
        );
        output.push(self.choice.tag());
        append_count(&mut output, self.cited_rubric_clause_ids.len())?;
        for clause in &self.cited_rubric_clause_ids {
            append_text(&mut output, clause);
        }
        Ok(output)
    }

    /// Returns the exact case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }
    /// Returns the scheduled presentation.
    #[must_use]
    pub const fn presentation(&self) -> CandidateJudgePresentationV1 {
        self.presentation
    }
    /// Returns the scheduled attempt ordinal.
    #[must_use]
    pub const fn attempt_ordinal(&self) -> u32 {
        self.attempt_ordinal
    }
    /// Returns the exact retained response identity.
    #[must_use]
    pub const fn retained_session_response_id(&self) -> &OllamaRetainedSessionResponseId {
        self.candidate_judge_response.retained_session_response_id()
    }
    /// Returns the exact schedule-indexed response record.
    #[must_use]
    pub const fn candidate_judge_response(&self) -> &CandidateJudgeResponseV1 {
        &self.candidate_judge_response
    }
    /// Returns the normalized choice.
    #[must_use]
    pub const fn choice(&self) -> CandidateJudgeChoiceV1 {
        self.choice
    }
    /// Returns strictly ascending cited clause identifiers.
    #[must_use]
    pub fn cited_rubric_clause_ids(&self) -> &[String] {
        &self.cited_rubric_clause_ids
    }
    /// Returns the content-derived observation identity.
    #[must_use]
    pub const fn observation_id(&self) -> &CandidateJudgeObservationId {
        &self.id
    }
}

fn map_response_error(
    error: GenerationQualificationContractError,
) -> GenerationQualificationContractError {
    match error {
        GenerationQualificationContractError::UnsupportedSchema(version) => {
            GenerationQualificationContractError::UnsupportedSchema(version)
        }
        _ => GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
    }
}

impl fmt::Debug for CandidateJudgeObservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeObservationV1")
            .field("observation_id", &self.id)
            .field("case_id", &self.case_id)
            .field("choice", &self.choice)
            .field("cited_clause_count", &self.cited_rubric_clause_ids.len())
            .finish_non_exhaustive()
    }
}

/// Complete exact schedule-order normalized observation batch.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeObservationBatchV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    candidate_judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    observations: Vec<CandidateJudgeObservationV1>,
    #[serde(skip)]
    id: CandidateJudgeObservationBatchId,
}

impl CandidateJudgeObservationBatchV1 {
    /// Creates a complete schedule-order observation batch.
    ///
    /// # Errors
    ///
    /// Returns an error unless every unique observation matches its schedule entry.
    pub fn new(
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        observations: Vec<CandidateJudgeObservationV1>,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            CANDIDATE_JUDGE_SCHEMA_VERSION,
            plan,
            schedule,
            requests,
            observations,
            None,
        )
    }

    fn build(
        schema_version: u32,
        plan: &CandidateJudgePlanV1,
        schedule: &CandidateJudgeScheduleV1,
        requests: &CandidateJudgeRequestAggregateV1,
        observations: Vec<CandidateJudgeObservationV1>,
        expected: Option<&BatchWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
            || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
        {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if schedule.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
            || requests.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
            || requests.candidate_judge_schedule_id() != schedule.candidate_judge_schedule_id()
            || observations.len() != schedule.entries().len()
            || observations.is_empty()
            || observations
                .iter()
                .map(CandidateJudgeObservationV1::observation_id)
                .collect::<HashSet<_>>()
                .len()
                != observations.len()
            || observations.iter().enumerate().zip(schedule.entries()).any(
                |((index, value), entry)| {
                    value.case_id() != entry.case_id()
                        || value.presentation() != entry.presentation()
                        || value.attempt_ordinal() != entry.attempt_ordinal()
                        || value.candidate_judge_response().candidate_judge_plan_id()
                            != plan.candidate_judge_plan_id()
                        || value
                            .candidate_judge_response()
                            .candidate_judge_schedule_id()
                            != schedule.candidate_judge_schedule_id()
                        || usize::try_from(value.candidate_judge_response().schedule_index())
                            != Ok(index)
                        || requests.structured_request_binding_ids().get(index)
                            != Some(
                                value
                                    .candidate_judge_response()
                                    .structured_request_binding_id(),
                            )
                },
            )
        {
            return Err(
                GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
            );
        }
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: plan.candidate_judge_plan_id().clone(),
            candidate_judge_schedule_id: schedule.candidate_judge_schedule_id().clone(),
            candidate_judge_request_aggregate_id: requests.request_aggregate_id().clone(),
            observations,
            id: CandidateJudgeObservationBatchId(Digest::sha256(
                b"uninitialized judge observation batch",
            )),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
            );
        }
        let mut canonical = CANDIDATE_JUDGE_OBSERVATION_BATCH_ID_DOMAIN.to_vec();
        append_digest(&mut canonical, value.candidate_judge_plan_id.digest());
        append_digest(&mut canonical, value.candidate_judge_schedule_id.digest());
        append_digest(
            &mut canonical,
            value.candidate_judge_request_aggregate_id.digest(),
        );
        append_count(&mut canonical, value.observations.len())?;
        for observation in &value.observations {
            append_digest(&mut canonical, observation.observation_id().digest());
        }
        value.id = CandidateJudgeObservationBatchId(Digest::sha256(&canonical));
        if serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            .len()
            > MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES
        {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        Ok(value)
    }

    /// Parses bounded canonical JSON and rederives every observation.
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
        if bytes.len() > MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: BatchWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.observations.len() != schedule.entries().len() {
            return Err(
                GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch,
            );
        }
        let observations = wire
            .observations
            .iter()
            .enumerate()
            .map(|(index, observation)| {
                CandidateJudgeObservationV1::build(
                    observation.schema_version,
                    plan,
                    schedule,
                    requests,
                    index,
                    observation
                        .candidate_judge_response
                        .ollama_retained_session_response_id
                        .clone(),
                    observation.choice,
                    observation.cited_rubric_clause_ids.clone(),
                    Some(observation),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value = Self::build(
            wire.schema_version,
            plan,
            schedule,
            requests,
            observations,
            Some(&wire),
        )?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    /// Returns observations in exact schedule order.
    #[must_use]
    pub fn observations(&self) -> &[CandidateJudgeObservationV1] {
        &self.observations
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
    /// Returns the exact request aggregate used by every nested response.
    #[must_use]
    pub const fn candidate_judge_request_aggregate_id(&self) -> &CandidateJudgeRequestAggregateId {
        &self.candidate_judge_request_aggregate_id
    }
    /// Returns the checked batch count.
    #[must_use]
    pub fn entry_count(&self) -> u32 {
        u32::try_from(self.observations.len()).unwrap_or(u32::MAX)
    }
    /// Returns the content-derived observation-batch identity.
    #[must_use]
    pub const fn observation_batch_id(&self) -> &CandidateJudgeObservationBatchId {
        &self.id
    }
}

impl fmt::Debug for CandidateJudgeObservationBatchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeObservationBatchV1")
            .field("observation_batch_id", &self.id)
            .field("entry_count", &self.observations.len())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationWire {
    schema_version: u32,
    case_id: GenerationCaseId,
    presentation: CandidateJudgePresentationV1,
    attempt_ordinal: u32,
    candidate_judge_response: ResponseWire,
    choice: CandidateJudgeChoiceV1,
    cited_rubric_clause_ids: Vec<String>,
}

impl ObservationWire {
    fn matches(&self, value: &CandidateJudgeObservationV1) -> bool {
        self.schema_version == value.schema_version
            && self.case_id == value.case_id
            && self.presentation == value.presentation
            && self.attempt_ordinal == value.attempt_ordinal
            && self
                .candidate_judge_response
                .matches(&value.candidate_judge_response)
            && self.choice == value.choice
            && self.cited_rubric_clause_ids == value.cited_rubric_clause_ids
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchWire {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_judge_schedule_id: CandidateJudgeScheduleId,
    candidate_judge_request_aggregate_id: CandidateJudgeRequestAggregateId,
    observations: Vec<ObservationWire>,
}

impl BatchWire {
    fn matches(&self, value: &CandidateJudgeObservationBatchV1) -> bool {
        self.schema_version == value.schema_version
            && self.candidate_judge_plan_id == value.candidate_judge_plan_id
            && self.candidate_judge_schedule_id == value.candidate_judge_schedule_id
            && self.candidate_judge_request_aggregate_id
                == value.candidate_judge_request_aggregate_id
            && self.observations.len() == value.observations.len()
    }
}
