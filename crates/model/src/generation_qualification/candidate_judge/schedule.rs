use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};

use super::{
    CANDIDATE_JUDGE_ATTEMPT_SEED_DOMAIN, CANDIDATE_JUDGE_PRESENTATION_ORDER_DOMAIN,
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgePlanV1, MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES,
};
use crate::generation_qualification::codec::{
    append_count, append_digest, append_u32, append_u64, validate_canonical_json,
};
use crate::generation_qualification::{
    CANDIDATE_JUDGE_SCHEDULE_ID_DOMAIN, CandidateJudgePlanId, CandidateJudgeScheduleId,
    CandidateReceiptPairSetId, GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId,
    GenerationQualificationContractError,
};
use rewrite_types::Digest;

/// Candidate presentation relative to the identity-significant A and B positions.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CandidateJudgePresentationV1 {
    /// Candidate A is presented first and candidate B second.
    CandidateAFirst,
    /// Candidate B is presented first and candidate A second.
    CandidateBFirst,
}

impl CandidateJudgePresentationV1 {
    pub(super) const fn tag(self) -> u8 {
        match self {
            Self::CandidateAFirst => 0,
            Self::CandidateBFirst => 1,
        }
    }
}

/// One exact deterministic candidate-judge attempt.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeScheduleEntryV1 {
    case_id: GenerationCaseId,
    presentation: CandidateJudgePresentationV1,
    attempt_ordinal: u32,
    seed: u64,
}

impl CandidateJudgeScheduleEntryV1 {
    /// Returns the exact generation case.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }

    /// Returns the identity-significant candidate presentation.
    #[must_use]
    pub const fn presentation(&self) -> CandidateJudgePresentationV1 {
        self.presentation
    }

    /// Returns the per-order attempt ordinal, fixed to zero in V1.
    #[must_use]
    pub const fn attempt_ordinal(&self) -> u32 {
        self.attempt_ordinal
    }

    /// Returns the exact structured-request seed.
    #[must_use]
    pub const fn seed(&self) -> u64 {
        self.seed
    }
}

impl fmt::Debug for CandidateJudgeScheduleEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeScheduleEntryV1")
            .field("case_id", &self.case_id)
            .field("presentation", &self.presentation)
            .field("attempt_ordinal", &self.attempt_ordinal)
            .finish_non_exhaustive()
    }
}

/// Complete exact semantic-order two-presentation schedule.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateJudgeScheduleV1 {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_receipt_pair_set_id: CandidateReceiptPairSetId,
    presentation_seed: u64,
    entries: Vec<CandidateJudgeScheduleEntryV1>,
    #[serde(skip)]
    id: CandidateJudgeScheduleId,
}

impl CandidateJudgeScheduleV1 {
    /// Derives the complete exact V1 schedule without caller-selected entries.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for checked arithmetic or
    /// fixed-size failures.
    pub fn new(
        plan: &CandidateJudgePlanV1,
        pair_set_id: &CandidateReceiptPairSetId,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            CANDIDATE_JUDGE_SCHEMA_VERSION,
            plan,
            pair_set_id,
            plan.presentation_seed(),
            None,
        )
    }

    fn build(
        schema_version: u32,
        plan: &CandidateJudgePlanV1,
        pair_set_id: &CandidateReceiptPairSetId,
        presentation_seed: u64,
        expected: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
            || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
        {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if presentation_seed != plan.presentation_seed() {
            return Err(
                GenerationQualificationContractError::CandidateJudgeScheduleRelationshipMismatch,
            );
        }
        let capacity = plan
            .cases()
            .len()
            .checked_mul(2)
            .ok_or(GenerationQualificationContractError::EncodingOverflow)?;
        let mut entries = Vec::with_capacity(capacity);
        for case in plan.cases() {
            let [first, second] = presentations(plan, pair_set_id, case.case_id());
            for presentation in [first, second] {
                entries.push(CandidateJudgeScheduleEntryV1 {
                    case_id: case.case_id().clone(),
                    presentation,
                    attempt_ordinal: 0,
                    seed: attempt_seed(plan, pair_set_id, case.case_id(), presentation, 0),
                });
            }
        }
        let mut value = Self {
            schema_version,
            candidate_judge_plan_id: plan.candidate_judge_plan_id().clone(),
            candidate_receipt_pair_set_id: pair_set_id.clone(),
            presentation_seed,
            entries,
            id: CandidateJudgeScheduleId(Digest::sha256(b"uninitialized judge schedule")),
        };
        if expected.is_some_and(|wire| !wire.matches(&value)) {
            return Err(
                GenerationQualificationContractError::CandidateJudgeScheduleRelationshipMismatch,
            );
        }
        value.id = CandidateJudgeScheduleId(Digest::sha256(&value.canonical_bytes()?));
        if serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            .len()
            > MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES
        {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        Ok(value)
    }

    /// Parses bounded canonical JSON and rederives every schedule entry.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        plan: &CandidateJudgePlanV1,
        pair_set_id: &CandidateReceiptPairSetId,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let value = Self::build(
            wire.schema_version,
            plan,
            pair_set_id,
            wire.presentation_seed,
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

    /// Returns the identity-significant ordered receipt pair.
    #[must_use]
    pub const fn candidate_receipt_pair_set_id(&self) -> &CandidateReceiptPairSetId {
        &self.candidate_receipt_pair_set_id
    }

    /// Returns the frozen presentation selector seed.
    #[must_use]
    pub const fn presentation_seed(&self) -> u64 {
        self.presentation_seed
    }

    /// Returns complete entries in eligible semantic-case order.
    #[must_use]
    pub fn entries(&self) -> &[CandidateJudgeScheduleEntryV1] {
        &self.entries
    }

    /// Returns the checked exact entry count.
    #[must_use]
    pub fn entry_count(&self) -> u32 {
        u32::try_from(self.entries.len()).unwrap_or(u32::MAX)
    }

    /// Returns the content-derived exact schedule identity.
    #[must_use]
    pub const fn candidate_judge_schedule_id(&self) -> &CandidateJudgeScheduleId {
        &self.id
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let mut output = CANDIDATE_JUDGE_SCHEDULE_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.candidate_judge_plan_id.digest());
        append_digest(&mut output, self.candidate_receipt_pair_set_id.digest());
        append_u64(&mut output, self.presentation_seed);
        append_count(&mut output, self.entries.len())?;
        for entry in &self.entries {
            append_digest(&mut output, entry.case_id().digest());
            output.push(entry.presentation().tag());
            append_u32(&mut output, entry.attempt_ordinal());
            append_u64(&mut output, entry.seed());
        }
        Ok(output)
    }
}

impl fmt::Debug for CandidateJudgeScheduleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeScheduleV1")
            .field("schema_version", &self.schema_version)
            .field("candidate_judge_schedule_id", &self.id)
            .field("entry_count", &self.entries.len())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    candidate_judge_plan_id: CandidateJudgePlanId,
    candidate_receipt_pair_set_id: CandidateReceiptPairSetId,
    presentation_seed: u64,
    entries: Vec<EntryWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryWire {
    case_id: GenerationCaseId,
    presentation: CandidateJudgePresentationV1,
    attempt_ordinal: u32,
    seed: u64,
}

impl Wire {
    fn matches(&self, value: &CandidateJudgeScheduleV1) -> bool {
        self.schema_version == value.schema_version
            && self.candidate_judge_plan_id == value.candidate_judge_plan_id
            && self.candidate_receipt_pair_set_id == value.candidate_receipt_pair_set_id
            && self.presentation_seed == value.presentation_seed
            && self.entries.len() == value.entries.len()
            && self
                .entries
                .iter()
                .zip(&value.entries)
                .all(|(left, right)| {
                    left.case_id == right.case_id
                        && left.presentation == right.presentation
                        && left.attempt_ordinal == right.attempt_ordinal
                        && left.seed == right.seed
                })
    }
}

fn presentations(
    plan: &CandidateJudgePlanV1,
    pair_set_id: &CandidateReceiptPairSetId,
    case_id: &GenerationCaseId,
) -> [CandidateJudgePresentationV1; 2] {
    let selector = derived_u64(
        CANDIDATE_JUDGE_PRESENTATION_ORDER_DOMAIN,
        plan,
        pair_set_id,
        case_id,
        None,
    );
    if selector & 1 == 0 {
        [
            CandidateJudgePresentationV1::CandidateAFirst,
            CandidateJudgePresentationV1::CandidateBFirst,
        ]
    } else {
        [
            CandidateJudgePresentationV1::CandidateBFirst,
            CandidateJudgePresentationV1::CandidateAFirst,
        ]
    }
}

fn attempt_seed(
    plan: &CandidateJudgePlanV1,
    pair_set_id: &CandidateReceiptPairSetId,
    case_id: &GenerationCaseId,
    presentation: CandidateJudgePresentationV1,
    attempt_ordinal: u32,
) -> u64 {
    derived_u64(
        CANDIDATE_JUDGE_ATTEMPT_SEED_DOMAIN,
        plan,
        pair_set_id,
        case_id,
        Some((presentation, attempt_ordinal)),
    )
}

fn derived_u64(
    domain: &[u8],
    plan: &CandidateJudgePlanV1,
    pair_set_id: &CandidateReceiptPairSetId,
    case_id: &GenerationCaseId,
    attempt: Option<(CandidateJudgePresentationV1, u32)>,
) -> u64 {
    let mut material = Vec::with_capacity(domain.len() + 64 * 3 + 13);
    material.extend_from_slice(domain);
    append_digest(&mut material, plan.candidate_judge_plan_id().digest());
    append_digest(&mut material, pair_set_id.digest());
    append_u64(&mut material, plan.presentation_seed());
    append_digest(&mut material, case_id.digest());
    if let Some((presentation, ordinal)) = attempt {
        material.push(presentation.tag());
        append_u32(&mut material, ordinal);
    }
    let digest = Sha256::digest(&material);
    u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 prefix is eight bytes"),
    )
}
