use std::fmt;

use super::{
    CandidateJudgeCaseV1, CandidateJudgeLimitsV1, CandidateJudgeOrderPolicyV1, CandidateJudgePlanV1,
};
use crate::generation_qualification::codec::{
    append_count, append_digest, append_text, append_u32, append_u64,
};
use crate::generation_qualification::{
    CANDIDATE_JUDGE_PLAN_ID_DOMAIN, CandidateJudgePlanId, CandidateSelectionPolicyId,
    GenerationQualificationContractError, GenerationQualificationPlanId, GenerationRepetitionId,
    GenerationSuiteManifestId, GenerationSystemId,
};
use rewrite_types::Digest;

impl CandidateJudgePlanV1 {
    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the candidate qualification-plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }
    /// Returns the exact suite identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the exact repetition identity.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }
    /// Returns the exact selection-policy identity.
    #[must_use]
    pub const fn selection_policy_id(&self) -> &CandidateSelectionPolicyId {
        &self.selection_policy_id
    }
    /// Returns candidate A's system identity.
    #[must_use]
    pub const fn candidate_a_generation_system_id(&self) -> &GenerationSystemId {
        &self.candidate_a_generation_system_id
    }
    /// Returns candidate B's system identity.
    #[must_use]
    pub const fn candidate_b_generation_system_id(&self) -> &GenerationSystemId {
        &self.candidate_b_generation_system_id
    }
    /// Returns the separately reloaded judge system identity.
    #[must_use]
    pub const fn judge_generation_system_id(&self) -> &GenerationSystemId {
        &self.judge_generation_system_id
    }
    /// Returns the exact case-material set digest.
    #[must_use]
    pub const fn case_material_set_digest(&self) -> &Digest {
        &self.case_material_set_digest
    }
    /// Returns the exact rubric digest.
    #[must_use]
    pub const fn rubric_digest(&self) -> &Digest {
        &self.rubric_digest
    }
    /// Returns the exact eligible semantic-order case subset.
    #[must_use]
    pub fn cases(&self) -> &[CandidateJudgeCaseV1] {
        &self.cases
    }
    /// Returns the fixed order policy.
    #[must_use]
    pub const fn order_policy(&self) -> CandidateJudgeOrderPolicyV1 {
        self.order_policy
    }
    /// Returns the presentation selector seed.
    #[must_use]
    pub const fn presentation_seed(&self) -> u64 {
        self.presentation_seed
    }
    /// Returns attempts per presentation order.
    #[must_use]
    pub const fn attempts_per_order(&self) -> u32 {
        self.attempts_per_order
    }
    /// Returns the exact lowerable limits.
    #[must_use]
    pub const fn limits(&self) -> CandidateJudgeLimitsV1 {
        self.limits
    }
    /// Returns the prompt-contract digest.
    #[must_use]
    pub const fn prompt_contract_digest(&self) -> &Digest {
        &self.prompt_contract_digest
    }
    /// Returns the output-schema digest.
    #[must_use]
    pub const fn output_schema_digest(&self) -> &Digest {
        &self.output_schema_digest
    }
    /// Returns the content-derived plan identity.
    #[must_use]
    pub const fn candidate_judge_plan_id(&self) -> &CandidateJudgePlanId {
        &self.id
    }

    pub(super) fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let mut output = CANDIDATE_JUDGE_PLAN_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.qualification_plan_id.digest(),
            self.suite_manifest_id.digest(),
            self.repetition_id.digest(),
            self.selection_policy_id.digest(),
            self.candidate_a_generation_system_id.digest(),
            self.candidate_b_generation_system_id.digest(),
            self.judge_generation_system_id.digest(),
            &self.case_material_set_digest,
            &self.rubric_digest,
        ] {
            append_digest(&mut output, digest);
        }
        append_count(&mut output, self.cases.len())?;
        for case in &self.cases {
            append_digest(&mut output, case.case_id().digest());
            append_count(&mut output, case.rubric_clause_ids().len())?;
            for clause in case.rubric_clause_ids() {
                append_text(&mut output, clause);
            }
        }
        output.push(0);
        append_u64(&mut output, self.presentation_seed);
        append_u32(&mut output, self.attempts_per_order);
        self.limits.append_to(&mut output);
        append_digest(&mut output, &self.prompt_contract_digest);
        append_digest(&mut output, &self.output_schema_digest);
        Ok(output)
    }
}

impl fmt::Debug for CandidateJudgePlanV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgePlanV1")
            .field("schema_version", &self.schema_version)
            .field("candidate_judge_plan_id", &self.id)
            .field("case_count", &self.cases.len())
            .field("order_policy", &self.order_policy)
            .finish_non_exhaustive()
    }
}
