//! Private scoped persistence projection, never a portable authority constructor.

use super::{VerifiedCandidateJudgeJoin, validate_join_view};
use crate::{VerifiedCandidateBatchSet, VerifiedGenerationCaseMaterial};
use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1Input, ManagedLocalJudgeReceiptRecordV1Input,
};
use rewrite_model_store::{CandidateJudgeExecutionV1Input, CandidateJudgeObservationFactV1};
use rewrite_types::CancellationToken;

pub(crate) struct JudgeSettlementView<'a, 'store> {
    pub(crate) target: &'a VerifiedCandidateBatchSet,
    pub(crate) baseline: &'a VerifiedCandidateBatchSet,
    pub(crate) material: &'a VerifiedGenerationCaseMaterial<'store>,
    pub(crate) input: CandidateJudgeExecutionV1Input<'a>,
}

impl VerifiedCandidateJudgeJoin<'_, '_, '_, '_> {
    pub(crate) fn with_settlement_view<T, E>(
        &mut self,
        cancellation: &CancellationToken,
        use_view: impl FnOnce(JudgeSettlementView<'_, '_>) -> Result<T, E>,
    ) -> Result<Result<T, E>, ()> {
        let triage = &self.triage;
        let join = &self.record;
        self.receipt
            .with_revalidated_authorities(cancellation, |receipt| {
                let view = super::VerifiedCandidateJudgeJoinView {
                    eval: receipt.eval,
                    judge_system: receipt.judge_system,
                    response_aggregate: receipt.response_aggregate,
                    observation_batch: receipt.observation_batch,
                    managed_receipt: receipt.record,
                    triage,
                    record: join,
                };
                validate_join_view(&view, cancellation).map_err(|_| ())?;
                let evidence = receipt
                    .eval
                    .compile_settlement_evidence(cancellation)
                    .map_err(|_| ())?;
                if &evidence.record != receipt.eval.deterministic_evaluation() {
                    return Err(());
                }
                let plan_input = receipt.eval.settlement_plan_input();
                let response_ids = receipt
                    .response_aggregate
                    .responses()
                    .iter()
                    .map(|response| response.retained_session_response_id().clone())
                    .collect::<Vec<_>>();
                let facts = receipt
                    .observation_batch
                    .observations()
                    .iter()
                    .map(|observation| CandidateJudgeObservationFactV1 {
                        retained_session_response_id: observation
                            .candidate_judge_response()
                            .retained_session_response_id(),
                        choice: observation.choice(),
                        cited_rubric_clause_ids: observation.cited_rubric_clause_ids(),
                    })
                    .collect::<Vec<_>>();
                let (target, baseline, material) = receipt.eval.settlement_candidates();
                let target_receipts = target.receipt_set(cancellation).map_err(|_| ())?;
                let baseline_receipts = baseline.receipt_set(cancellation).map_err(|_| ())?;
                let managed_input: &ManagedLocalJudgeReceiptRecordV1Input = &receipt.input;
                Ok(use_view(JudgeSettlementView {
                    target,
                    baseline,
                    material,
                    input: CandidateJudgeExecutionV1Input {
                        qualification_plan_id: target_receipts.qualification_plan_id(),
                        repetition_id: target_receipts.repetition_id(),
                        candidate_a_generation_system_id: target_receipts.generation_system_id(),
                        candidate_b_generation_system_id: baseline_receipts.generation_system_id(),
                        judge_generation_system_id: receipt.judge_system.generation_system_id(),
                        plan: receipt.eval.judge_plan(),
                        plan_input: &plan_input,
                        candidate_a_receipt_set: target_receipts,
                        candidate_b_receipt_set: baseline_receipts,
                        deterministic_evaluation: &evidence.record,
                        deterministic_input: CandidateDeterministicEvaluationRecordV1Input {
                            case_material_set_digest: material.case_material_set_digest(),
                            suite_pair_digest: evidence.record.suite_pair_digest(),
                            report_relationship: &evidence.reports,
                        },
                        request_binding_ids: receipt
                            .eval
                            .request_aggregate()
                            .structured_request_binding_ids(),
                        retained_session_response_ids: &response_ids,
                        observation_facts: &facts,
                        schedule: receipt.eval.judge_schedule(),
                        request_aggregate: receipt.eval.request_aggregate(),
                        response_aggregate: receipt.response_aggregate,
                        observation_batch: receipt.observation_batch,
                        managed_receipt: receipt.record,
                        managed_receipt_input: managed_input,
                        triage_report: triage.canonical_json(),
                        join,
                    },
                }))
            })
            .map_err(|_| ())
    }
}

#[cfg(test)]
pub(crate) fn synthetic_join(
    handoff: crate::CandidateJudgeRunnerHandoff<'_>,
) -> VerifiedCandidateJudgeJoin<'_, 'static, 'static, 'static> {
    use super::super::managed_local_judge_receipt::ManagedLocalJudgeReceipt;
    use super::test_support::{managed_receipt, observation_batch, portable_outputs};
    let (responses, _) = portable_outputs(&handoff);
    let observations = observation_batch(
        &handoff,
        &responses,
        rewrite_model::CandidateJudgeChoiceV1::Tie,
    );
    let record = managed_receipt(&handoff, &responses, &observations);
    let receipt = ManagedLocalJudgeReceipt::from_portable_closure_for_test(
        handoff,
        responses,
        observations,
        record,
    );
    super::VerifiedCandidateJudgeJoinCompiler::compile(receipt, &CancellationToken::new())
        .expect("synthetic opaque join over real eval compiler")
}
