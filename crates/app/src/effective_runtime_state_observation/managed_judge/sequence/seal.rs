use std::{sync::Arc, time::Instant};

use rewrite_inference::StructuredCompletionRequest;
use rewrite_ollama::OllamaResidentSessionExecutionReceipt;
use rewrite_types::{CancellationToken, Digest};

use super::super::super::ManagedOllamaEffectiveRuntimeState;
use super::super::super::authority::ManagedOllamaManagedJudgeAttemptFacts;
use super::super::contract::{
    MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT, MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET,
    ManagedJudgeAttemptObservation, ManagedJudgeAttemptObserverBinding,
    RetainedManagedJudgeEffectiveState,
};
use super::super::error::ManagedJudgeObservationError;
#[cfg(test)]
use super::super::state::EffectiveStateObservationSubject;
use super::super::state::retain_effective_state;
use super::ManagedJudgeObservationSequence;

struct ValidatedAttemptClosure {
    request: Digest,
    response: Digest,
    receipt_binding: Digest,
    preflight: Digest,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
    first_residency_ordinal: u64,
    last_residency_ordinal: u64,
    receipt: OllamaResidentSessionExecutionReceipt,
}

impl ManagedJudgeObservationSequence {
    /// Seals one observation, exact request, complete receipt, and live state.
    ///
    /// The raw request is consumed and discarded after its complete binding is
    /// checked. The complete content-free receipt is retained privately.
    ///
    /// # Errors
    ///
    /// Returns an error for sequence substitution; request, response, preflight,
    /// receipt, or ordinal mismatch; cancellation; or effective-state mismatch.
    pub fn seal_attempt(
        &mut self,
        observation: ManagedJudgeAttemptObservation,
        request: StructuredCompletionRequest,
        receipt: OllamaResidentSessionExecutionReceipt,
        state: ManagedOllamaEffectiveRuntimeState,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.seal_attempt_retaining(
            observation,
            request,
            receipt,
            |facts| retain_effective_state(state, facts),
            cancellation,
            None,
        )
    }

    /// Seals one attempt under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, substitution, closure, or state error.
    pub fn seal_attempt_until(
        &mut self,
        observation: ManagedJudgeAttemptObservation,
        request: StructuredCompletionRequest,
        receipt: OllamaResidentSessionExecutionReceipt,
        state: ManagedOllamaEffectiveRuntimeState,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.seal_attempt_retaining(
            observation,
            request,
            receipt,
            |facts| retain_effective_state(state, facts),
            cancellation,
            Some(operation_deadline),
        )
    }

    #[cfg(test)]
    pub(super) fn seal_attempt_with_subject<S: EffectiveStateObservationSubject>(
        &mut self,
        observation: ManagedJudgeAttemptObservation,
        request: StructuredCompletionRequest,
        receipt: OllamaResidentSessionExecutionReceipt,
        state: S,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.seal_attempt_retaining(
            observation,
            request,
            receipt,
            |facts| {
                state
                    .binds_observations(facts)
                    .then(|| state.into_retained())
            },
            cancellation,
            None,
        )
    }

    #[cfg(test)]
    pub(super) fn seal_attempt_with_subject_until<S: EffectiveStateObservationSubject>(
        &mut self,
        observation: ManagedJudgeAttemptObservation,
        request: StructuredCompletionRequest,
        receipt: OllamaResidentSessionExecutionReceipt,
        state: S,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.seal_attempt_retaining(
            observation,
            request,
            receipt,
            |facts| {
                state
                    .binds_observations(facts)
                    .then(|| state.into_retained())
            },
            cancellation,
            Some(operation_deadline),
        )
    }

    fn seal_attempt_retaining<F>(
        &mut self,
        observation: ManagedJudgeAttemptObservation,
        request: StructuredCompletionRequest,
        receipt: OllamaResidentSessionExecutionReceipt,
        retain_state: F,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError>
    where
        F: FnOnce(
            &ManagedOllamaManagedJudgeAttemptFacts<'_>,
        ) -> Option<RetainedManagedJudgeEffectiveState>,
    {
        let operation_deadline = self.deadline(supplied_deadline);
        let result = (|| {
            self.validate_observation_authority(&observation, cancellation, operation_deadline)?;
            let closure = self.validate_attempt_closure(&observation, request, receipt)?;
            let state_facts = attempt_facts(&observation, &closure);
            let retained = retain_state(&state_facts)
                .ok_or(ManagedJudgeObservationError::EffectiveStateMismatch)?;
            self.retain_sealed_attempt(observation, closure, retained)
        })();
        let result = super::deadline::deadline_precedence(result, cancellation, operation_deadline);
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        result
    }

    fn validate_observation_authority(
        &self,
        observation: &ManagedJudgeAttemptObservation,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.ensure_sequence_active(cancellation, operation_deadline)?;
        if !Arc::ptr_eq(&self.sequence_token, &observation.sequence_token)
            || observation.schedule_id != self.schedule_id
            || self.awaiting_seal != Some(observation.schedule_cursor)
            || observation.schedule_cursor != self.next_cursor
        {
            return Err(ManagedJudgeObservationError::ObservationSubstitution);
        }
        Ok(())
    }

    fn validate_attempt_closure(
        &self,
        observation: &ManagedJudgeAttemptObservation,
        request: StructuredCompletionRequest,
        receipt: OllamaResidentSessionExecutionReceipt,
    ) -> Result<ValidatedAttemptClosure, ManagedJudgeObservationError> {
        let execution = receipt.execution();
        let request_binding = request.binding_digest();
        let first_response = ordinal(execution.first_response_ordinal())?;
        let last_response = ordinal(execution.last_response_ordinal())?;
        let first_residency = ordinal(receipt.first_residency_ordinal())?;
        let last_residency = ordinal(receipt.last_residency_ordinal())?;
        let expected_residency_first = observation
            .first_response_ordinal
            .checked_add(u64::from(MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET))
            .ok_or(ManagedJudgeObservationError::InvalidCount)?;
        let response_count = last_response
            .checked_sub(first_response)
            .and_then(|span| span.checked_add(1))
            .ok_or(ManagedJudgeObservationError::AttemptClosureMismatch)?;
        if request.validate().is_err()
            || request_binding != *execution.request_digest()
            || first_response != observation.first_response_ordinal
            || last_response != observation.last_response_ordinal
            || response_count != u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT)
            || first_residency != expected_residency_first
            || last_residency != observation.last_response_ordinal
            || self.bindings.first().is_some_and(|binding| {
                binding.retained_preflight_digest() != execution.preflight_digest()
            })
        {
            return Err(ManagedJudgeObservationError::AttemptClosureMismatch);
        }
        drop(request);
        let receipt_binding = receipt.complete_binding_digest();
        Ok(ValidatedAttemptClosure {
            request: request_binding,
            response: execution.response_digest().clone(),
            receipt_binding,
            preflight: execution.preflight_digest().clone(),
            first_response_ordinal: first_response,
            last_response_ordinal: last_response,
            first_residency_ordinal: first_residency,
            last_residency_ordinal: last_residency,
            receipt,
        })
    }

    fn retain_sealed_attempt(
        &mut self,
        observation: ManagedJudgeAttemptObservation,
        closure: ValidatedAttemptClosure,
        effective_state: RetainedManagedJudgeEffectiveState,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.bindings.push(ManagedJudgeAttemptObserverBinding {
            sequence_token: Arc::clone(&self.sequence_token),
            schedule_id: self.schedule_id.clone(),
            schedule_cursor: observation.schedule_cursor,
            first_response_ordinal: observation.first_response_ordinal,
            last_response_ordinal: observation.last_response_ordinal,
            process_binding_digest: observation.process.evidence_digest().clone(),
            native_load_binding_digest: observation
                .native_load
                .native_load_observation_id()
                .digest()
                .clone(),
            connection_binding_digest: observation.connection_binding_digest,
            receipt_binding_digest: closure.receipt_binding,
            receipt: closure.receipt,
            effective_state,
        });
        self.awaiting_seal = None;
        self.next_cursor = self
            .next_cursor
            .checked_add(1)
            .ok_or(ManagedJudgeObservationError::InvalidCount)?;
        Ok(())
    }
}

fn attempt_facts<'a>(
    observation: &'a ManagedJudgeAttemptObservation,
    closure: &'a ValidatedAttemptClosure,
) -> ManagedOllamaManagedJudgeAttemptFacts<'a> {
    ManagedOllamaManagedJudgeAttemptFacts {
        server_process: &observation.process,
        server_native_load: &observation.native_load,
        initial_worker: &observation.initial_worker,
        final_worker: &observation.final_worker,
        worker_native_load: &observation.worker_native_load,
        model_mapping: &observation.model_mapping,
        request: &closure.request,
        response: &closure.response,
        receipt: &closure.receipt_binding,
        preflight: &closure.preflight,
        first_response_ordinal: closure.first_response_ordinal,
        last_response_ordinal: closure.last_response_ordinal,
        first_residency_ordinal: closure.first_residency_ordinal,
        last_residency_ordinal: closure.last_residency_ordinal,
    }
}

fn ordinal(value: usize) -> Result<u64, ManagedJudgeObservationError> {
    u64::try_from(value).map_err(|_error| ManagedJudgeObservationError::InvalidCount)
}
