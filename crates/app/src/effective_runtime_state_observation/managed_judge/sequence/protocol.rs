use std::{sync::Arc, time::Instant};

use rewrite_runtime_attestor::{
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerObservationRequest,
    NativeLoadObservationRequest,
};
use rewrite_types::CancellationToken;

use super::super::contract::{
    MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT, MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT,
    MANAGED_JUDGE_WORKER_RESPONSE_OFFSET, ManagedJudgeAttemptObservation,
    ManagedJudgePreflightEvidence, ManagedJudgePreflightObservation,
    ManagedJudgePreflightObserverBinding,
};
use super::super::error::ManagedJudgeObservationError;
use super::super::worker::PendingManagedJudgeWorkerObservation;
use super::ManagedJudgeObservationSequence;

impl ManagedJudgeObservationSequence {
    /// Derives the complete typed preflight observation after exact ordinal 7.
    ///
    /// The returned token exposes the evidence required by the existing managed
    /// preflight report builder. It must be consumed by [`Self::seal_preflight`]
    /// before response ordinal 8 can be observed.
    ///
    /// # Errors
    ///
    /// Returns an error for an incomplete or repeated preflight, cancellation,
    /// process/native observation failure, or changed retained evidence.
    pub fn complete_preflight_observation(
        &mut self,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ManagedJudgePreflightObservation, ManagedJudgeObservationError> {
        self.complete_preflight_observation_with_deadline(native_request, cancellation, None)
    }

    /// Completes preflight under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, span, process, or native-load error.
    pub fn complete_preflight_observation_until(
        &mut self,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedJudgePreflightObservation, ManagedJudgeObservationError> {
        self.complete_preflight_observation_with_deadline(
            native_request,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn complete_preflight_observation_with_deadline(
        &mut self,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<ManagedJudgePreflightObservation, ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(supplied_deadline);
        let result = self.complete_preflight_observation_inner(
            native_request,
            cancellation,
            operation_deadline,
        );
        let result = super::deadline::deadline_precedence(result, cancellation, operation_deadline);
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        result
    }

    fn complete_preflight_observation_inner(
        &mut self,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedJudgePreflightObservation, ManagedJudgeObservationError> {
        self.ensure_sequence_active(cancellation, operation_deadline)?;
        let preflight_count = usize::try_from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT)
            .map_err(|_| ManagedJudgeObservationError::InvalidCount)?;
        if self.preflight.is_some()
            || self.awaiting_preflight
            || self.completed_responses != u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT)
            || self.response_evidence.len() != preflight_count
            || self.awaiting_seal.is_some()
            || self.pending_worker.is_some()
        {
            return Err(ManagedJudgeObservationError::InvalidPreflightSpan);
        }
        let post_preflight_process = match operation_deadline {
            Some(deadline) => self.process.reobserve_until(cancellation, deadline),
            None => self.process.reobserve(cancellation),
        }
        .map_err(ManagedJudgeObservationError::Process)?;
        let native_load = match operation_deadline {
            Some(deadline) => {
                self.process
                    .observe_native_load_until(native_request, cancellation, deadline)
            }
            None => self
                .process
                .observe_native_load(native_request, cancellation),
        }
        .map_err(ManagedJudgeObservationError::NativeLoad)?;
        let final_process = match operation_deadline {
            Some(deadline) => self.process.reobserve_until(cancellation, deadline),
            None => self.process.reobserve(cancellation),
        }
        .map_err(ManagedJudgeObservationError::Process)?;
        if self.initial_process != post_preflight_process
            || self.initial_process != final_process
            || native_load.process_evidence_digest() != self.initial_process.evidence_digest()
        {
            return Err(ManagedJudgeObservationError::ObservationSubstitution);
        }
        let connection_binding_digest =
            self.span_digest(0, 0, 1, u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT))?;
        let initial_connection = self.initial_connection()?.clone();
        let connection_witness = self
            .response_evidence
            .last()
            .cloned()
            .ok_or(ManagedJudgeObservationError::InvalidPreflightSpan)?;
        let mut connection_observations = Vec::with_capacity(preflight_count.saturating_add(1));
        connection_observations.push(initial_connection);
        connection_observations.append(&mut self.response_evidence);
        self.awaiting_preflight = true;
        Ok(ManagedJudgePreflightObservation {
            sequence_token: Arc::clone(&self.sequence_token),
            evidence: ManagedJudgePreflightEvidence {
                initial_process: self.initial_process.clone(),
                post_preflight_process,
                final_process,
                native_load,
                connection_witness,
                connection_observations,
                connection_binding_digest,
            },
        })
    }

    /// Seals the exact preflight observation into this schedule authority.
    ///
    /// # Errors
    ///
    /// Returns an error if the token was substituted or preflight is not awaiting
    /// sealing. Every error permanently poisons the sequence.
    pub fn seal_preflight(
        &mut self,
        observation: ManagedJudgePreflightObservation,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.seal_preflight_with_deadline(observation, cancellation, None)
    }

    /// Seals preflight under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, or substitution error.
    pub fn seal_preflight_until(
        &mut self,
        observation: ManagedJudgePreflightObservation,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.seal_preflight_with_deadline(observation, cancellation, Some(operation_deadline))
    }

    fn seal_preflight_with_deadline(
        &mut self,
        observation: ManagedJudgePreflightObservation,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(supplied_deadline);
        let result = self.seal_preflight_inner(observation, cancellation, operation_deadline);
        let result = super::deadline::deadline_precedence(result, cancellation, operation_deadline);
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        result
    }

    fn seal_preflight_inner(
        &mut self,
        observation: ManagedJudgePreflightObservation,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.ensure_sequence_active(cancellation, operation_deadline)?;
        if !self.awaiting_preflight
            || self.preflight.is_some()
            || !Arc::ptr_eq(&self.sequence_token, &observation.sequence_token)
        {
            return Err(ManagedJudgeObservationError::ObservationSubstitution);
        }
        self.preflight = Some(ManagedJudgePreflightObserverBinding {
            sequence_token: observation.sequence_token,
            evidence: observation.evidence,
        });
        self.awaiting_preflight = false;
        Ok(())
    }

    /// Starts the exact cursor-bound worker bracket at attempt response offset 4.
    ///
    /// Discovery, native-load observation, and model-mapping observation all run
    /// while the app retains the opaque worker lease. The request values are only
    /// borrowed for this call.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong phase or cursor, cancellation, substitution,
    /// or worker observer failure. Every error permanently poisons the sequence.
    pub fn begin_attempt_worker_observation(
        &mut self,
        schedule_cursor: u32,
        worker_request: &ManagedGenerationWorkerObservationRequest<'_>,
        native_request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.begin_attempt_worker_observation_with_deadline(
            schedule_cursor,
            worker_request,
            native_request,
            cancellation,
            None,
        )
    }

    /// Starts the worker bracket under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, phase, substitution, or worker error.
    pub fn begin_attempt_worker_observation_until(
        &mut self,
        schedule_cursor: u32,
        worker_request: &ManagedGenerationWorkerObservationRequest<'_>,
        native_request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.begin_attempt_worker_observation_with_deadline(
            schedule_cursor,
            worker_request,
            native_request,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn begin_attempt_worker_observation_with_deadline(
        &mut self,
        schedule_cursor: u32,
        worker_request: &ManagedGenerationWorkerObservationRequest<'_>,
        native_request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(supplied_deadline);
        let result = self.begin_attempt_worker_observation_inner(
            schedule_cursor,
            worker_request,
            native_request,
            cancellation,
            operation_deadline,
        );
        let result = super::deadline::deadline_precedence(result, cancellation, operation_deadline);
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        result
    }

    fn begin_attempt_worker_observation_inner(
        &mut self,
        schedule_cursor: u32,
        worker_request: &ManagedGenerationWorkerObservationRequest<'_>,
        native_request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.ensure_sequence_active(cancellation, operation_deadline)?;
        if schedule_cursor != self.next_cursor || schedule_cursor >= self.attempt_count {
            return Err(ManagedJudgeObservationError::InvalidScheduleCursor);
        }
        if self.preflight.is_none()
            || self.awaiting_preflight
            || self.awaiting_seal.is_some()
            || self.pending_worker.is_some()
        {
            return Err(ManagedJudgeObservationError::InvalidWorkerObservationPhase);
        }
        let worker_ordinal = u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT)
            .checked_add(
                u64::from(schedule_cursor)
                    .checked_mul(u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT))
                    .ok_or(ManagedJudgeObservationError::InvalidCount)?,
            )
            .and_then(|value| value.checked_add(u64::from(MANAGED_JUDGE_WORKER_RESPONSE_OFFSET)))
            .ok_or(ManagedJudgeObservationError::InvalidCount)?;
        if self.completed_responses != worker_ordinal
            || self.response_evidence.len()
                != usize::try_from(MANAGED_JUDGE_WORKER_RESPONSE_OFFSET)
                    .map_err(|_| ManagedJudgeObservationError::InvalidCount)?
        {
            return Err(ManagedJudgeObservationError::InvalidWorkerObservationPhase);
        }
        let mut lease = match operation_deadline {
            Some(deadline) => {
                self.process
                    .observe_generation_worker_until(worker_request, cancellation, deadline)
            }
            None => self
                .process
                .observe_generation_worker(worker_request, cancellation),
        }
        .map_err(ManagedJudgeObservationError::Worker)?;
        let initial = lease.initial_evidence().clone();
        let native_load = match operation_deadline {
            Some(deadline) => {
                lease.observe_native_load_until(native_request, cancellation, deadline)
            }
            None => lease.observe_native_load(native_request, cancellation),
        }
        .map_err(ManagedJudgeObservationError::Worker)?;
        let model_mapping = match operation_deadline {
            Some(deadline) => lease.observe_model_mapping_until(cancellation, deadline),
            None => lease.observe_model_mapping(cancellation),
        }
        .map_err(ManagedJudgeObservationError::Worker)?;
        if initial.worker_artifact_id() != worker_request.retained_worker.artifact_id()
            || initial.model_artifact_id() != worker_request.retained_model_weight.artifact_id()
            || model_mapping.model_artifact_id()
                != worker_request.retained_model_weight.artifact_id()
            || model_mapping.mapping_region_count() == 0
        {
            return Err(ManagedJudgeObservationError::ObservationSubstitution);
        }
        self.pending_worker = Some(PendingManagedJudgeWorkerObservation {
            schedule_cursor,
            lease,
            initial,
            native_load,
            model_mapping,
        });
        Ok(())
    }

    /// Closes one exact nine-response span and reobserves all retained authorities.
    ///
    /// # Errors
    ///
    /// Returns an error for a wrong cursor, incomplete span, missing worker bracket,
    /// cancellation, substitution, or process/native/worker observer failure.
    pub fn complete_attempt_observation(
        &mut self,
        schedule_cursor: u32,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ManagedJudgeAttemptObservation, ManagedJudgeObservationError> {
        self.complete_attempt_observation_with_deadline(
            schedule_cursor,
            native_request,
            cancellation,
            None,
        )
    }

    /// Completes an attempt under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, span, process, native-load, or worker error.
    pub fn complete_attempt_observation_until(
        &mut self,
        schedule_cursor: u32,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedJudgeAttemptObservation, ManagedJudgeObservationError> {
        self.complete_attempt_observation_with_deadline(
            schedule_cursor,
            native_request,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn complete_attempt_observation_with_deadline(
        &mut self,
        schedule_cursor: u32,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<ManagedJudgeAttemptObservation, ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(supplied_deadline);
        let result = self.complete_attempt_observation_inner(
            schedule_cursor,
            native_request,
            cancellation,
            operation_deadline,
        );
        let result = super::deadline::deadline_precedence(result, cancellation, operation_deadline);
        if let Err(error) = result {
            self.poisoned = true;
            return Err(error);
        }
        result
    }

    fn complete_attempt_observation_inner(
        &mut self,
        schedule_cursor: u32,
        native_request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedJudgeAttemptObservation, ManagedJudgeObservationError> {
        self.ensure_sequence_active(cancellation, operation_deadline)?;
        if self.awaiting_seal.is_some() {
            return Err(ManagedJudgeObservationError::AwaitingSeal);
        }
        if schedule_cursor != self.next_cursor || schedule_cursor >= self.attempt_count {
            return Err(ManagedJudgeObservationError::InvalidScheduleCursor);
        }
        if self.preflight.is_none() || self.awaiting_preflight {
            return Err(ManagedJudgeObservationError::InvalidPreflightSpan);
        }
        let first = u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT)
            + u64::from(schedule_cursor) * u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT)
            + 1;
        let last = first + u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT) - 1;
        if self.completed_responses != last
            || self.response_evidence.len()
                != usize::try_from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT)
                    .map_err(|_| ManagedJudgeObservationError::InvalidCount)?
        {
            return Err(ManagedJudgeObservationError::InvalidAttemptSpan);
        }
        let mut worker = self
            .pending_worker
            .take()
            .filter(|pending| pending.schedule_cursor == schedule_cursor)
            .ok_or(ManagedJudgeObservationError::IncompleteWorkerObservation)?;
        let final_worker = match operation_deadline {
            Some(deadline) => worker.lease.reobserve_until(cancellation, deadline),
            None => worker.lease.reobserve(cancellation),
        }
        .map_err(ManagedJudgeObservationError::Worker)?;
        if final_worker != worker.initial {
            return Err(ManagedJudgeObservationError::ObservationSubstitution);
        }
        let post_response_process = match operation_deadline {
            Some(deadline) => self.process.reobserve_until(cancellation, deadline),
            None => self.process.reobserve(cancellation),
        }
        .map_err(ManagedJudgeObservationError::Process)?;
        let native_load = match operation_deadline {
            Some(deadline) => {
                self.process
                    .observe_native_load_until(native_request, cancellation, deadline)
            }
            None => self
                .process
                .observe_native_load(native_request, cancellation),
        }
        .map_err(ManagedJudgeObservationError::NativeLoad)?;
        let final_process = match operation_deadline {
            Some(deadline) => self.process.reobserve_until(cancellation, deadline),
            None => self.process.reobserve(cancellation),
        }
        .map_err(ManagedJudgeObservationError::Process)?;
        if post_response_process != final_process
            || native_load.process_evidence_digest() != final_process.evidence_digest()
        {
            return Err(ManagedJudgeObservationError::ObservationSubstitution);
        }
        let connection_binding_digest = self.span_digest(1, schedule_cursor, first, last)?;
        self.response_evidence.clear();
        self.awaiting_seal = Some(schedule_cursor);
        Ok(ManagedJudgeAttemptObservation {
            sequence_token: Arc::clone(&self.sequence_token),
            schedule_id: self.schedule_id.clone(),
            schedule_cursor,
            first_response_ordinal: first,
            last_response_ordinal: last,
            process: final_process,
            native_load,
            initial_worker: worker.initial,
            final_worker,
            worker_native_load: worker.native_load,
            model_mapping: worker.model_mapping,
            connection_binding_digest,
        })
    }
}
