use std::{sync::Arc, time::Instant};

use rewrite_model::{CandidateJudgeScheduleId, CandidateJudgeScheduleV1};
use rewrite_ollama::{OllamaResponseObservation, OllamaResponseObservationPhase};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, NativeManagedLinuxProcessLease, RetainedTcpConnection,
    RetainedTcpConnectionEvidence,
};
use rewrite_types::{CancellationToken, Digest};

#[cfg(test)]
use super::contract::ManagedJudgeAttemptObservation;
use super::contract::{
    CompletedManagedJudgeObservationSequence, MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT,
    MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT, ManagedJudgeAttemptObserverBinding,
    ManagedJudgePreflightObserverBinding,
};
use super::error::ManagedJudgeObservationError;
use super::framing::connection_span_digest;
use super::process::ManagedJudgeProcessAuthority;
#[cfg(test)]
use super::state::EffectiveStateObservationSubject;
#[cfg(test)]
use super::worker::ManagedJudgeWorkerAuthority;
use super::worker::PendingManagedJudgeWorkerObservation;

#[cfg(any(target_os = "linux", test))]
const MAXIMUM_JUDGE_ATTEMPTS: u32 = 512;

macro_rules! fail {
    ($sequence:expr, $error:expr) => {{
        $sequence.poisoned = true;
        return Err($error);
    }};
}

/// Retained app-owned observer for one exact complete managed judge schedule.
///
/// The sequence owns the concrete managed process lease. Callers provide only
/// live response callbacks and the retained native package objects required by
/// the native observer. All process, connection, and native-load bindings are
/// derived internally.
pub struct ManagedJudgeObservationSequence {
    process: Box<dyn ManagedJudgeProcessAuthority>,
    initial_process: AttachedProcessEvidence,
    sequence_token: Arc<()>,
    schedule_id: CandidateJudgeScheduleId,
    attempt_count: u32,
    expected_response_count: u64,
    completed_responses: u64,
    next_cursor: u32,
    active_connection: Option<RetainedTcpConnection>,
    initial_connection: Option<RetainedTcpConnectionEvidence>,
    response_evidence: Vec<RetainedTcpConnectionEvidence>,
    awaiting_preflight: bool,
    preflight: Option<ManagedJudgePreflightObserverBinding>,
    pending_worker: Option<PendingManagedJudgeWorkerObservation>,
    awaiting_seal: Option<u32>,
    bindings: Vec<ManagedJudgeAttemptObserverBinding>,
    poisoned: bool,
    operation_deadline: Option<Instant>,
}

impl ManagedJudgeObservationSequence {
    /// Returns whether the concrete managed observer is available on this target.
    #[must_use]
    pub const fn supported_on_current_platform() -> bool {
        cfg!(target_os = "linux")
    }

    /// Starts observation for one exact schedule by consuming the process lease.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedJudgeObservationError::UnsupportedPlatform`] outside Linux,
    /// or a count or cancellation error before retaining authority.
    pub fn new(
        process: NativeManagedLinuxProcessLease,
        schedule: &CandidateJudgeScheduleV1,
        cancellation: &CancellationToken,
    ) -> Result<Self, ManagedJudgeObservationError> {
        Self::new_with_deadline(process, schedule, cancellation, None)
    }

    /// Starts observation under one retained absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedJudgeObservationError::DeadlineExceeded`] at or after
    /// `operation_deadline`, or the same errors as [`Self::new`].
    pub fn new_until(
        process: NativeManagedLinuxProcessLease,
        schedule: &CandidateJudgeScheduleV1,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<Self, ManagedJudgeObservationError> {
        Self::new_with_deadline(process, schedule, cancellation, Some(operation_deadline))
    }

    fn new_with_deadline(
        process: NativeManagedLinuxProcessLease,
        schedule: &CandidateJudgeScheduleV1,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<Self, ManagedJudgeObservationError> {
        #[cfg(not(target_os = "linux"))]
        {
            drop(process);
            let _ = schedule;
            deadline::deadline_precedence(
                Err(ManagedJudgeObservationError::UnsupportedPlatform),
                cancellation,
                operation_deadline,
            )
        }
        #[cfg(target_os = "linux")]
        {
            Self::from_authority(
                Box::new(process),
                schedule.candidate_judge_schedule_id().clone(),
                schedule.entry_count(),
                cancellation,
                operation_deadline,
            )
        }
    }

    #[cfg(any(target_os = "linux", test))]
    fn from_authority(
        process: Box<dyn ManagedJudgeProcessAuthority>,
        schedule_id: CandidateJudgeScheduleId,
        attempt_count: u32,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<Self, ManagedJudgeObservationError> {
        let result = (|| {
            deadline::ensure_active_until(cancellation, operation_deadline)?;
            if attempt_count == 0 || attempt_count > MAXIMUM_JUDGE_ATTEMPTS {
                return Err(ManagedJudgeObservationError::InvalidCount);
            }
            let expected_response_count = u64::from(attempt_count)
                .checked_mul(u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT))
                .and_then(|count| {
                    count.checked_add(u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT))
                })
                .ok_or(ManagedJudgeObservationError::InvalidCount)?;
            let response_capacity = usize::try_from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT)
                .map_err(|_| ManagedJudgeObservationError::InvalidCount)?;
            let binding_capacity = usize::try_from(attempt_count)
                .map_err(|_| ManagedJudgeObservationError::InvalidCount)?;
            let initial_process = process.initial_evidence().clone();
            Ok(Self {
                process,
                initial_process,
                sequence_token: Arc::new(()),
                schedule_id,
                attempt_count,
                expected_response_count,
                completed_responses: 0,
                next_cursor: 0,
                active_connection: None,
                initial_connection: None,
                response_evidence: Vec::with_capacity(response_capacity),
                awaiting_preflight: false,
                preflight: None,
                pending_worker: None,
                awaiting_seal: None,
                bindings: Vec::with_capacity(binding_capacity),
                poisoned: false,
                operation_deadline,
            })
        })();
        deadline::deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Observes one exact retained response-sequence boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, reordered callbacks, wrong ordinals,
    /// failed response attempts, overlong spans, or native observer failure.
    pub fn observe_response(
        &mut self,
        observation: OllamaResponseObservation,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.observe_response_with_deadline(observation, cancellation, None)
    }

    /// Observes one response boundary under a caller deadline capped by the retained deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, sequence, connection, or process error.
    pub fn observe_response_until(
        &mut self,
        observation: OllamaResponseObservation,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.observe_response_with_deadline(observation, cancellation, Some(operation_deadline))
    }

    fn observe_response_with_deadline(
        &mut self,
        observation: OllamaResponseObservation,
        cancellation: &CancellationToken,
        supplied_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(supplied_deadline);
        deadline::ensure_active_until(cancellation, operation_deadline)?;
        let addresses = observation.addresses();
        let connection = match RetainedTcpConnection::new(addresses.client(), addresses.server()) {
            Ok(connection) => connection,
            Err(error) => {
                let error = deadline::deadline_precedence::<()>(
                    Err(ManagedJudgeObservationError::Process(error)),
                    cancellation,
                    operation_deadline,
                )
                .expect_err("connection construction failed");
                fail!(self, error);
            }
        };
        self.observe_phase_with_deadline(
            observation.phase(),
            connection,
            cancellation,
            operation_deadline,
        )
    }

    #[cfg(test)]
    fn observe_phase(
        &mut self,
        phase: OllamaResponseObservationPhase,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeObservationError> {
        let operation_deadline = self.deadline(None);
        self.observe_phase_with_deadline(phase, connection, cancellation, operation_deadline)
    }

    fn observe_phase_with_deadline(
        &mut self,
        phase: OllamaResponseObservationPhase,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        let result = self.observe_phase_inner(phase, connection, cancellation, operation_deadline);
        let result = deadline::deadline_precedence(result, cancellation, operation_deadline);
        if let Err(error) = result {
            fail!(self, error);
        }
        result
    }

    fn observe_phase_inner(
        &mut self,
        phase: OllamaResponseObservationPhase,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        self.ensure_sequence_active(cancellation, operation_deadline)?;
        if self.awaiting_preflight {
            fail!(self, ManagedJudgeObservationError::AwaitingPreflightSeal);
        }
        if self.awaiting_seal.is_some() {
            fail!(self, ManagedJudgeObservationError::AwaitingSeal);
        }
        match phase {
            OllamaResponseObservationPhase::BeforeResponses => {
                if self.completed_responses != 0 || self.initial_connection.is_some() {
                    fail!(self, ManagedJudgeObservationError::InvalidResponseSequence);
                }
                let evidence = match operation_deadline {
                    Some(deadline) => {
                        self.process
                            .observe_connection_until(connection, cancellation, deadline)
                    }
                    None => self.process.observe_connection(connection, cancellation),
                }
                .map_err(ManagedJudgeObservationError::Process)?;
                self.active_connection = Some(connection);
                self.initial_connection = Some(evidence);
                Ok(())
            }
            OllamaResponseObservationPhase::AfterResponse { ordinal } => {
                let expected = self
                    .completed_responses
                    .checked_add(1)
                    .ok_or(ManagedJudgeObservationError::InvalidCount)?;
                let ordinal = u64::try_from(ordinal)
                    .map_err(|_| ManagedJudgeObservationError::InvalidCount)?;
                let boundary = self.current_span_boundary()?;
                if self.worker_observation_was_skipped(ordinal)? {
                    fail!(
                        self,
                        ManagedJudgeObservationError::IncompleteWorkerObservation
                    );
                }
                if ordinal != expected || ordinal > boundary || connection != self.connection()? {
                    fail!(self, ManagedJudgeObservationError::InvalidResponseSequence);
                }
                let initial = self.initial_connection()?.clone();
                let evidence = match operation_deadline {
                    Some(deadline) => self.process.reobserve_connection_until(
                        connection,
                        &initial,
                        cancellation,
                        deadline,
                    ),
                    None => self
                        .process
                        .reobserve_connection(connection, &initial, cancellation),
                }
                .map_err(ManagedJudgeObservationError::Process)?;
                self.response_evidence.push(evidence);
                self.completed_responses = ordinal;
                Ok(())
            }
            OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses,
            } => {
                let observed = u64::try_from(completed_responses)
                    .map_err(|_| ManagedJudgeObservationError::InvalidCount)?;
                if observed != self.completed_responses || connection != self.connection()? {
                    fail!(self, ManagedJudgeObservationError::InvalidResponseSequence);
                }
                let initial = self.initial_connection()?.clone();
                match operation_deadline {
                    Some(deadline) => self.process.reobserve_connection_until(
                        connection,
                        &initial,
                        cancellation,
                        deadline,
                    ),
                    None => self
                        .process
                        .reobserve_connection(connection, &initial, cancellation),
                }
                .map_err(ManagedJudgeObservationError::Process)?;
                fail!(self, ManagedJudgeObservationError::FailedResponseAttempt);
            }
        }
    }

    fn current_span_boundary(&self) -> Result<u64, ManagedJudgeObservationError> {
        if self.preflight.is_none() {
            return Ok(u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT));
        }
        u64::from(self.next_cursor)
            .checked_add(1)
            .and_then(|count| count.checked_mul(u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT)))
            .and_then(|count| count.checked_add(u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT)))
            .ok_or(ManagedJudgeObservationError::InvalidCount)
    }

    fn worker_observation_was_skipped(
        &self,
        ordinal: u64,
    ) -> Result<bool, ManagedJudgeObservationError> {
        if self.preflight.is_none() {
            return Ok(false);
        }
        let worker_ordinal = u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT)
            .checked_add(
                u64::from(self.next_cursor)
                    .checked_mul(u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT))
                    .ok_or(ManagedJudgeObservationError::InvalidCount)?,
            )
            .and_then(|value| {
                value.checked_add(u64::from(
                    super::contract::MANAGED_JUDGE_WORKER_RESPONSE_OFFSET,
                ))
            })
            .ok_or(ManagedJudgeObservationError::InvalidCount)?;
        Ok(ordinal > worker_ordinal && self.pending_worker.is_none())
    }

    fn span_digest(
        &self,
        span_kind: u8,
        schedule_cursor: u32,
        first: u64,
        last: u64,
    ) -> Result<Digest, ManagedJudgeObservationError> {
        let initial = self.initial_connection()?;
        Ok(connection_span_digest(
            &self.schedule_id,
            span_kind,
            schedule_cursor,
            first,
            last,
            initial,
            &self.response_evidence,
        ))
    }

    fn connection(&self) -> Result<RetainedTcpConnection, ManagedJudgeObservationError> {
        self.active_connection
            .ok_or(ManagedJudgeObservationError::InvalidResponseSequence)
    }

    fn initial_connection(
        &self,
    ) -> Result<&RetainedTcpConnectionEvidence, ManagedJudgeObservationError> {
        self.initial_connection
            .as_ref()
            .ok_or(ManagedJudgeObservationError::InvalidResponseSequence)
    }

    fn ensure_sequence_active(
        &self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<(), ManagedJudgeObservationError> {
        deadline::ensure_active_until(cancellation, operation_deadline)?;
        if self.poisoned {
            return Err(ManagedJudgeObservationError::InvalidResponseSequence);
        }
        Ok(())
    }

    fn deadline(&self, supplied: Option<Instant>) -> Option<Instant> {
        deadline::earliest_deadline(self.operation_deadline, supplied)
    }

    #[cfg(test)]
    pub(super) fn for_test(
        process: Box<dyn ManagedJudgeProcessAuthority>,
        schedule_id: CandidateJudgeScheduleId,
        attempt_count: u32,
        cancellation: &CancellationToken,
    ) -> Result<Self, ManagedJudgeObservationError> {
        Self::from_authority(process, schedule_id, attempt_count, cancellation, None)
    }

    #[cfg(test)]
    pub(super) fn for_test_until(
        process: Box<dyn ManagedJudgeProcessAuthority>,
        schedule_id: CandidateJudgeScheduleId,
        attempt_count: u32,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<Self, ManagedJudgeObservationError> {
        Self::from_authority(
            process,
            schedule_id,
            attempt_count,
            cancellation,
            Some(operation_deadline),
        )
    }
}

mod completion;
mod deadline;
mod debug;
mod protocol;
mod seal;

#[cfg(test)]
pub(crate) mod tests;
