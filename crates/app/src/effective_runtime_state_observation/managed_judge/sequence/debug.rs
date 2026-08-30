use std::fmt;

use super::ManagedJudgeObservationSequence;

impl fmt::Debug for ManagedJudgeObservationSequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeObservationSequence")
            .field("schedule_id", &self.schedule_id)
            .field("attempt_count", &self.attempt_count)
            .field("next_cursor", &self.next_cursor)
            .field("completed_responses", &self.completed_responses)
            .field("awaiting_seal", &self.awaiting_seal.is_some())
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}
