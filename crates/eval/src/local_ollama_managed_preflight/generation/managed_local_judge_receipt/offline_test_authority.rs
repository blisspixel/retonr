use rewrite_model::{CandidateJudgeObservationBatchV1, CandidateJudgeResponseAggregateV1};
use rewrite_types::CancellationToken;

use super::{ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptView};
use crate::CandidateJudgeRunnerHandoff;
use crate::local_ollama_managed_preflight::generation::managed_schedule_runner::{
    ManagedJudgeScheduleAuthorityFailures, ManagedJudgeScheduleExecutionAuthorityError,
};

pub(super) struct ExactOfflineReceiptAuthority<'store> {
    eval: CandidateJudgeRunnerHandoff<'store>,
    response_aggregate: CandidateJudgeResponseAggregateV1,
    observation_batch: CandidateJudgeObservationBatchV1,
}

impl<'store> ExactOfflineReceiptAuthority<'store> {
    pub(super) const fn new(
        eval: CandidateJudgeRunnerHandoff<'store>,
        response_aggregate: CandidateJudgeResponseAggregateV1,
        observation_batch: CandidateJudgeObservationBatchV1,
    ) -> Self {
        Self {
            eval,
            response_aggregate,
            observation_batch,
        }
    }

    pub(super) fn with_revalidated_authorities<T, E>(
        &mut self,
        cancellation: &CancellationToken,
        record: &ManagedLocalJudgeReceiptRecordV1,
        use_authorities: impl FnOnce(ManagedLocalJudgeReceiptView<'_, '_>) -> Result<T, E>,
    ) -> Result<T, ManagedJudgeScheduleExecutionAuthorityError<E>> {
        if let Err(initial) = self.eval.revalidate(cancellation) {
            let initial = ManagedJudgeScheduleAuthorityFailures::eval_for_test(
                initial,
                cancellation.is_cancelled(),
            );
            return match self.eval.revalidate(&CancellationToken::new()) {
                Ok(()) => Err(ManagedJudgeScheduleExecutionAuthorityError::Initial(
                    initial,
                )),
                Err(final_validation) => Err(
                    ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
                        initial,
                        final_validation: ManagedJudgeScheduleAuthorityFailures::eval_for_test(
                            final_validation,
                            false,
                        ),
                    },
                ),
            };
        }
        let primary = use_authorities(ManagedLocalJudgeReceiptView {
            eval: &self.eval,
            judge_system: self.eval.judge_system(),
            response_aggregate: &self.response_aggregate,
            observation_batch: &self.observation_batch,
            record,
        });
        let final_validation = self
            .eval
            .revalidate(&CancellationToken::new())
            .map_err(|source| ManagedJudgeScheduleAuthorityFailures::eval_for_test(source, false));
        match (primary, final_validation) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => {
                Err(ManagedJudgeScheduleExecutionAuthorityError::Callback(error))
            }
            (Ok(_value), Err(failures)) => {
                Err(ManagedJudgeScheduleExecutionAuthorityError::Final(failures))
            }
            (Err(callback), Err(final_validation)) => Err(
                ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
                    callback,
                    final_validation,
                },
            ),
        }
    }
}
