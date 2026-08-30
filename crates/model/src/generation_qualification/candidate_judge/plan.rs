use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::{CANDIDATE_JUDGE_SCHEMA_VERSION, MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES};
use crate::generation_qualification::codec::{
    append_u32, valid_machine_key, validate_canonical_json,
};
use crate::generation_qualification::{
    CandidateJudgePlanId, CandidateSelectionPolicyId, CandidateSelectionPolicyV1, GenerationCaseId,
    GenerationQualificationContractError, GenerationQualificationPlanId,
    GenerationQualificationPlanV1, GenerationRepetitionId, GenerationRepetitionRecordV1,
    GenerationSuiteManifestId, GenerationSuiteManifestV1, GenerationSystemId,
    GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};

const MAX_JUDGE_CASES: u32 = 256;
const MAX_RUBRIC_CLAUSES_PER_CASE: usize = 32;
const MAX_RUBRIC_CLAUSE_ID_BYTES: usize = 64;
const MAX_SOURCE_BYTES: u32 = 1_024 * 1_024;
const MAX_CANDIDATE_BYTES: u32 = 1_024 * 1_024;
const MAX_INPUT_BYTES: u32 = 4 * 1_024 * 1_024;
const MAX_CONTEXT_TOKENS: u32 = 131_072;
const MAX_OUTPUT_TOKENS: u32 = 8_192;
const MIN_RESPONSE_BYTES: u32 = 256;
const MAX_RESPONSE_BYTES: u32 = 64 * 1_024;
const MAX_ELAPSED_MILLIS: u32 = 60 * 60 * 1_000;

mod accessors;
mod validation;

use validation::validate_relations;

/// One exact pre-output case admitted to candidate judging.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeCaseV1 {
    case_id: GenerationCaseId,
    rubric_clause_ids: Vec<String>,
}

impl CandidateJudgeCaseV1 {
    /// Creates one typed case binding with a strict nonempty clause set.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for empty, excessive,
    /// invalid, duplicated, or reordered rubric clause identifiers.
    pub fn new(
        case_id: GenerationCaseId,
        rubric_clause_ids: Vec<String>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let value = Self {
            case_id,
            rubric_clause_ids,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns the exact typed generation-case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }

    /// Returns strictly ascending admitted rubric clause identifiers.
    #[must_use]
    pub fn rubric_clause_ids(&self) -> &[String] {
        &self.rubric_clause_ids
    }

    fn validate(&self) -> Result<(), GenerationQualificationContractError> {
        if self.rubric_clause_ids.is_empty()
            || self.rubric_clause_ids.len() > MAX_RUBRIC_CLAUSES_PER_CASE
            || self
                .rubric_clause_ids
                .iter()
                .any(|value| !valid_machine_key(value, MAX_RUBRIC_CLAUSE_ID_BYTES))
            || self
                .rubric_clause_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(
                GenerationQualificationContractError::CandidateJudgePlanRelationshipMismatch,
            );
        }
        Ok(())
    }
}

impl fmt::Debug for CandidateJudgeCaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeCaseV1")
            .field("case_id", &self.case_id)
            .field("rubric_clause_count", &self.rubric_clause_ids.len())
            .finish_non_exhaustive()
    }
}

/// The only admitted candidate presentation policy in version 1.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateJudgeOrderPolicyV1 {
    /// Present each eligible pair once in each candidate order.
    BothOrders,
}

/// Per-run limits that may only lower the reviewed adapter ceilings.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the maximum prefix is part of the frozen public JSON field names"
)]
pub struct CandidateJudgeLimitsV1 {
    maximum_judge_cases: u32,
    maximum_source_bytes: u32,
    maximum_candidate_bytes: u32,
    maximum_complete_input_bytes: u32,
    maximum_context_tokens: u32,
    maximum_output_tokens: u32,
    maximum_response_bytes: u32,
    maximum_elapsed_milliseconds: u32,
}

impl CandidateJudgeLimitsV1 {
    /// Creates one bounded exact local-judge execution envelope.
    ///
    /// # Errors
    ///
    /// Returns an error when any limit is zero, inconsistent, or above its ceiling.
    #[expect(
        clippy::too_many_arguments,
        reason = "the eight values are the frozen protocol field order"
    )]
    pub const fn new(
        maximum_judge_cases: u32,
        maximum_source_bytes: u32,
        maximum_candidate_bytes: u32,
        maximum_complete_input_bytes: u32,
        maximum_context_tokens: u32,
        maximum_output_tokens: u32,
        maximum_response_bytes: u32,
        maximum_elapsed_milliseconds: u32,
    ) -> Result<Self, GenerationQualificationContractError> {
        let value = Self {
            maximum_judge_cases,
            maximum_source_bytes,
            maximum_candidate_bytes,
            maximum_complete_input_bytes,
            maximum_context_tokens,
            maximum_output_tokens,
            maximum_response_bytes,
            maximum_elapsed_milliseconds,
        };
        if value.valid() {
            Ok(value)
        } else {
            Err(GenerationQualificationContractError::InvalidLimits)
        }
    }

    const fn valid(self) -> bool {
        self.maximum_judge_cases > 0
            && self.maximum_judge_cases <= MAX_JUDGE_CASES
            && self.maximum_source_bytes > 0
            && self.maximum_source_bytes <= MAX_SOURCE_BYTES
            && self.maximum_candidate_bytes > 0
            && self.maximum_candidate_bytes <= MAX_CANDIDATE_BYTES
            && self.maximum_complete_input_bytes > 0
            && self.maximum_complete_input_bytes <= MAX_INPUT_BYTES
            && self.maximum_complete_input_bytes >= self.maximum_source_bytes
            && self.maximum_complete_input_bytes >= self.maximum_candidate_bytes
            && self.maximum_context_tokens > 0
            && self.maximum_context_tokens <= MAX_CONTEXT_TOKENS
            && self.maximum_output_tokens > 0
            && self.maximum_output_tokens <= MAX_OUTPUT_TOKENS
            && self.maximum_response_bytes >= MIN_RESPONSE_BYTES
            && self.maximum_response_bytes <= MAX_RESPONSE_BYTES
            && self.maximum_elapsed_milliseconds > 0
            && self.maximum_elapsed_milliseconds <= MAX_ELAPSED_MILLIS
    }

    /// Returns the maximum number of eligible judge cases.
    #[must_use]
    pub const fn maximum_judge_cases(self) -> u32 {
        self.maximum_judge_cases
    }
    /// Returns the maximum exact source bytes per attempt.
    #[must_use]
    pub const fn maximum_source_bytes(self) -> u32 {
        self.maximum_source_bytes
    }
    /// Returns the maximum bytes in either candidate per attempt.
    #[must_use]
    pub const fn maximum_candidate_bytes(self) -> u32 {
        self.maximum_candidate_bytes
    }
    /// Returns the maximum complete canonical prompt bytes per attempt.
    #[must_use]
    pub const fn maximum_complete_input_bytes(self) -> u32 {
        self.maximum_complete_input_bytes
    }
    /// Returns the maximum requested context tokens.
    #[must_use]
    pub const fn maximum_context_tokens(self) -> u32 {
        self.maximum_context_tokens
    }
    /// Returns the maximum requested output tokens.
    #[must_use]
    pub const fn maximum_output_tokens(self) -> u32 {
        self.maximum_output_tokens
    }
    /// Returns the maximum response bytes per attempt.
    #[must_use]
    pub const fn maximum_response_bytes(self) -> u32 {
        self.maximum_response_bytes
    }
    /// Returns the maximum elapsed milliseconds for the joined run.
    #[must_use]
    pub const fn maximum_elapsed_milliseconds(self) -> u32 {
        self.maximum_elapsed_milliseconds
    }

    fn append_to(self, output: &mut Vec<u8>) {
        for value in [
            self.maximum_judge_cases,
            self.maximum_source_bytes,
            self.maximum_candidate_bytes,
            self.maximum_complete_input_bytes,
            self.maximum_context_tokens,
            self.maximum_output_tokens,
            self.maximum_response_bytes,
            self.maximum_elapsed_milliseconds,
        ] {
            append_u32(output, value);
        }
    }
}

/// Exact records required to validate one inert pre-output judge plan.
#[derive(Clone, Copy)]
pub struct CandidateJudgePlanV1Relations<'a> {
    /// Candidate generation qualification plan containing only A and B systems.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Complete semantic-order generation suite.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact repetition selected for both candidate systems.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Exact pre-output candidate selection policy.
    pub selection_policy: &'a CandidateSelectionPolicyV1,
    /// Every planned candidate attempt in exact qualification-plan order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// Candidate A generation system.
    pub candidate_a_system: &'a GenerationSystemRecordV1,
    /// Candidate B generation system.
    pub candidate_b_system: &'a GenerationSystemRecordV1,
    /// Separately reloaded judge generation system.
    pub judge_system: &'a GenerationSystemRecordV1,
}

/// Exact caller-supplied values for one pre-output judge plan.
pub struct CandidateJudgePlanV1Input {
    /// Digest of the complete verified case-material set.
    pub case_material_set_digest: Digest,
    /// Digest of the exact canonical local-judge rubric.
    pub rubric_digest: Digest,
    /// Exact eligible semantic-order case subset.
    pub cases: Vec<CandidateJudgeCaseV1>,
    /// Frozen presentation-order policy.
    pub order_policy: CandidateJudgeOrderPolicyV1,
    /// Frozen presentation selector seed.
    pub presentation_seed: u64,
    /// Attempts per presentation order, fixed to one in V1.
    pub attempts_per_order: u32,
    /// Exact lowerable execution limits.
    pub limits: CandidateJudgeLimitsV1,
    /// Digest of the exact neutral prompt contract.
    pub prompt_contract_digest: Digest,
    /// Digest of the exact neutral structured output schema.
    pub output_schema_digest: Digest,
}

/// Inert pre-output plan for exact candidate judging.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgePlanV1 {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    selection_policy_id: CandidateSelectionPolicyId,
    candidate_a_generation_system_id: GenerationSystemId,
    candidate_b_generation_system_id: GenerationSystemId,
    judge_generation_system_id: GenerationSystemId,
    case_material_set_digest: Digest,
    rubric_digest: Digest,
    cases: Vec<CandidateJudgeCaseV1>,
    order_policy: CandidateJudgeOrderPolicyV1,
    presentation_seed: u64,
    attempts_per_order: u32,
    limits: CandidateJudgeLimitsV1,
    prompt_contract_digest: Digest,
    output_schema_digest: Digest,
    #[serde(skip)]
    id: CandidateJudgePlanId,
}

impl CandidateJudgePlanV1 {
    /// Creates one bounded inert plan from exact portable relationships.
    ///
    /// Case eligibility and rubric semantics remain eval-owned. This constructor
    /// validates only their bounded typed framing and semantic suite subsequence.
    ///
    /// # Errors
    ///
    /// Returns an error for any invalid limit, case, or reloaded relationship.
    pub fn new(
        relations: CandidateJudgePlanV1Relations<'_>,
        input: CandidateJudgePlanV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(CANDIDATE_JUDGE_SCHEMA_VERSION, relations, input, None)
    }

    fn build(
        schema_version: u32,
        relations: CandidateJudgePlanV1Relations<'_>,
        input: CandidateJudgePlanV1Input,
        expected: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_relations(schema_version, relations, &input)?;
        let mut value = Self {
            schema_version,
            qualification_plan_id: relations.qualification_plan.qualification_plan_id().clone(),
            suite_manifest_id: relations.suite.suite_manifest_id().clone(),
            repetition_id: relations.repetition.repetition_id().clone(),
            selection_policy_id: relations.selection_policy.selection_policy_id().clone(),
            candidate_a_generation_system_id: relations
                .candidate_a_system
                .generation_system_id()
                .clone(),
            candidate_b_generation_system_id: relations
                .candidate_b_system
                .generation_system_id()
                .clone(),
            judge_generation_system_id: relations.judge_system.generation_system_id().clone(),
            case_material_set_digest: input.case_material_set_digest,
            rubric_digest: input.rubric_digest,
            cases: input.cases,
            order_policy: input.order_policy,
            presentation_seed: input.presentation_seed,
            attempts_per_order: input.attempts_per_order,
            limits: input.limits,
            prompt_contract_digest: input.prompt_contract_digest,
            output_schema_digest: input.output_schema_digest,
            id: CandidateJudgePlanId(Digest::sha256(b"uninitialized judge plan")),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgePlanRelationshipMismatch,
            );
        }
        let canonical = value.canonical_bytes()?;
        if canonical.len() > MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        value.id = CandidateJudgePlanId(Digest::sha256(&canonical));
        if serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            .len()
            > MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES
        {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        Ok(value)
    }

    /// Parses bounded canonical JSON and reloads all portable dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: CandidateJudgePlanV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let input = wire.input();
        let value = Self::build(wire.schema_version, relations, input, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    selection_policy_id: CandidateSelectionPolicyId,
    candidate_a_generation_system_id: GenerationSystemId,
    candidate_b_generation_system_id: GenerationSystemId,
    judge_generation_system_id: GenerationSystemId,
    case_material_set_digest: Digest,
    rubric_digest: Digest,
    cases: Vec<CandidateJudgeCaseWire>,
    order_policy: CandidateJudgeOrderPolicyV1,
    presentation_seed: u64,
    attempts_per_order: u32,
    limits: CandidateJudgeLimitsV1,
    prompt_contract_digest: Digest,
    output_schema_digest: Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateJudgeCaseWire {
    case_id: GenerationCaseId,
    rubric_clause_ids: Vec<String>,
}

impl Wire {
    fn input(&self) -> CandidateJudgePlanV1Input {
        CandidateJudgePlanV1Input {
            case_material_set_digest: self.case_material_set_digest.clone(),
            rubric_digest: self.rubric_digest.clone(),
            cases: self
                .cases
                .iter()
                .map(|value| CandidateJudgeCaseV1 {
                    case_id: value.case_id.clone(),
                    rubric_clause_ids: value.rubric_clause_ids.clone(),
                })
                .collect(),
            order_policy: self.order_policy,
            presentation_seed: self.presentation_seed,
            attempts_per_order: self.attempts_per_order,
            limits: self.limits,
            prompt_contract_digest: self.prompt_contract_digest.clone(),
            output_schema_digest: self.output_schema_digest.clone(),
        }
    }

    fn matches(&self, value: &CandidateJudgePlanV1) -> bool {
        self.schema_version == value.schema_version
            && self.qualification_plan_id == value.qualification_plan_id
            && self.suite_manifest_id == value.suite_manifest_id
            && self.repetition_id == value.repetition_id
            && self.selection_policy_id == value.selection_policy_id
            && self.candidate_a_generation_system_id == value.candidate_a_generation_system_id
            && self.candidate_b_generation_system_id == value.candidate_b_generation_system_id
            && self.judge_generation_system_id == value.judge_generation_system_id
            && self.case_material_set_digest == value.case_material_set_digest
            && self.rubric_digest == value.rubric_digest
            && self.order_policy == value.order_policy
            && self.presentation_seed == value.presentation_seed
            && self.attempts_per_order == value.attempts_per_order
            && self.limits == value.limits
            && self.prompt_contract_digest == value.prompt_contract_digest
            && self.output_schema_digest == value.output_schema_digest
            && self.cases.len() == value.cases.len()
            && self.cases.iter().zip(&value.cases).all(|(left, right)| {
                left.case_id == right.case_id && left.rubric_clause_ids == right.rubric_clause_ids
            })
    }
}
