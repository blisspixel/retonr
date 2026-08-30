use crate::effective_runtime_state_observation::{
    CompletedSequenceMutation, mutate_completed_sequence,
};

use super::VerifiedManagedJudgeObservationAuthority;

impl VerifiedManagedJudgeObservationAuthority {
    pub(crate) fn test_support_mutate_sequence(&mut self, mutation: CompletedSequenceMutation) {
        mutate_completed_sequence(&mut self.completed_sequence, mutation);
    }
}
