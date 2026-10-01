//! Private deterministic settlement projection over retained live handoff inputs.

use super::CandidateJudgeRunnerHandoff;
use crate::candidate_deterministic_compiler::{
    CompiledCandidateDeterministicEvidence, compile_candidate_deterministic_evidence,
};
use crate::{
    CandidateDeterministicCompilerError, VerifiedCandidateBatchSet, VerifiedGenerationCaseMaterial,
};
use rewrite_model::CandidateJudgePlanV1Input;
use rewrite_types::CancellationToken;

impl CandidateJudgeRunnerHandoff<'_> {
    pub(crate) fn settlement_candidates(
        &self,
    ) -> (
        &VerifiedCandidateBatchSet,
        &VerifiedCandidateBatchSet,
        &VerifiedGenerationCaseMaterial<'_>,
    ) {
        (&self.candidate_a, &self.candidate_b, &self.case_material)
    }

    pub(crate) fn compile_settlement_evidence(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<CompiledCandidateDeterministicEvidence, CandidateDeterministicCompilerError> {
        compile_candidate_deterministic_evidence(
            &self.candidate_a,
            &self.candidate_b,
            &self.case_material,
            cancellation,
        )
    }

    // These declarations were reloaded and checked against live material, rubric,
    // requests and full candidate scope before the handoff was constructed.
    pub(crate) fn settlement_plan_input(&self) -> CandidateJudgePlanV1Input {
        CandidateJudgePlanV1Input {
            case_material_set_digest: self.case_material.case_material_set_digest().clone(),
            rubric_digest: self.judge_plan.rubric_digest().clone(),
            cases: self.judge_plan.cases().to_vec(),
            order_policy: self.judge_plan.order_policy(),
            presentation_seed: self.judge_plan.presentation_seed(),
            attempts_per_order: self.judge_plan.attempts_per_order(),
            limits: self.judge_plan.limits(),
            prompt_contract_digest: crate::local_judge_prompt_contract_digest(),
            output_schema_digest: rewrite_inference::local_judge_attempt_output_contract()
                .schema_digest,
        }
    }
}
