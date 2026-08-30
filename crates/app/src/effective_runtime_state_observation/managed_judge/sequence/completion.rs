use std::{sync::Arc, time::Instant};

use rewrite_types::CancellationToken;

use super::{
    CompletedManagedJudgeObservationSequence, ManagedJudgeObservationError,
    ManagedJudgeObservationSequence,
};

impl ManagedJudgeObservationSequence {
    /// Finishes only after every schedule entry has one sealed ordered binding.
    ///
    /// # Errors
    ///
    /// Returns an error if the schedule, response sequence, or sealing sequence is
    /// incomplete or cancellation has been requested.
    pub fn finish(
        self,
        cancellation: &CancellationToken,
    ) -> Result<CompletedManagedJudgeObservationSequence, ManagedJudgeObservationError> {
        self.finish_with_deadline(cancellation, None)
    }

    /// Finishes under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, or incomplete-sequence error.
    pub fn finish_until(
        self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<CompletedManagedJudgeObservationSequence, ManagedJudgeObservationError> {
        self.finish_with_deadline(cancellation, Some(operation_deadline))
    }

    fn finish_with_deadline(
        self,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<CompletedManagedJudgeObservationSequence, ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(supplied_deadline);
        super::deadline::ensure_active_until(cancellation, operation_deadline)?;
        let result = self.finish_inner();
        super::deadline::deadline_precedence(result, cancellation, operation_deadline)
    }

    fn finish_inner(
        self,
    ) -> Result<CompletedManagedJudgeObservationSequence, ManagedJudgeObservationError> {
        if self.poisoned
            || self.awaiting_preflight
            || self.awaiting_seal.is_some()
            || self.pending_worker.is_some()
            || self.next_cursor != self.attempt_count
            || self.completed_responses != self.expected_response_count
            || self.bindings.len()
                != usize::try_from(self.attempt_count)
                    .map_err(|_| ManagedJudgeObservationError::InvalidCount)?
            || self.preflight.is_none()
            || !self.response_evidence.is_empty()
            || self.bindings.iter().enumerate().any(|(index, binding)| {
                !Arc::ptr_eq(&self.sequence_token, &binding.sequence_token)
                    || binding.schedule_id != self.schedule_id
                    || usize::try_from(binding.schedule_cursor).ok() != Some(index)
            })
        {
            return Err(ManagedJudgeObservationError::IncompleteSequence);
        }
        let preflight = self
            .preflight
            .ok_or(ManagedJudgeObservationError::IncompleteSequence)?;
        if !Arc::ptr_eq(&self.sequence_token, &preflight.sequence_token) {
            return Err(ManagedJudgeObservationError::IncompleteSequence);
        }
        Ok(CompletedManagedJudgeObservationSequence {
            schedule_id: self.schedule_id,
            preflight,
            bindings: self.bindings,
        })
    }
}
