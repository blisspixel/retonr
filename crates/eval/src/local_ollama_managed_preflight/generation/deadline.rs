use std::time::Instant;

use rewrite_inference::{InferenceError, InferenceErrorKind, OperationContext};
use rewrite_types::CancellationToken;

use super::LocalOllamaManagedGenerationError;

const DEADLINE_CODE: &str = "candidate_operation_deadline_exceeded";
const CANCELLED_CODE: &str = "candidate_operation_cancelled";

#[derive(Clone, Copy)]
pub(super) struct CandidateOperationDeadline {
    deadline: Option<Instant>,
}

impl CandidateOperationDeadline {
    pub(super) const fn compatibility() -> Self {
        Self { deadline: None }
    }

    pub(super) const fn until(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
        }
    }

    pub(super) const fn instant(self) -> Option<Instant> {
        self.deadline
    }

    pub(super) const fn context(self, cancellation: &CancellationToken) -> OperationContext<'_> {
        OperationContext::new(cancellation, self.deadline)
    }

    pub(super) fn ensure_active(
        self,
        cancellation: &CancellationToken,
    ) -> Result<(), LocalOllamaManagedGenerationError> {
        self.precedence(Ok(()), cancellation)
    }

    pub(super) fn precedence<T>(
        self,
        result: Result<T, LocalOllamaManagedGenerationError>,
        cancellation: &CancellationToken,
    ) -> Result<T, LocalOllamaManagedGenerationError> {
        candidate_operation_precedence_at(
            result,
            cancellation.is_cancelled(),
            self.deadline,
            Instant::now(),
        )
    }

    pub(super) fn terminal_override(
        self,
        cancellation: &CancellationToken,
    ) -> Option<LocalOllamaManagedGenerationError> {
        self.ensure_active(cancellation).err()
    }

    pub(super) fn finalize_failure<C>(
        self,
        operation: LocalOllamaManagedGenerationError,
        cancellation: &CancellationToken,
        finalizer: impl FnOnce() -> C,
    ) -> (LocalOllamaManagedGenerationError, C) {
        let cleanup = finalizer();
        let operation = candidate_operation_precedence_at::<()>(
            Err(operation),
            cancellation.is_cancelled(),
            self.deadline,
            Instant::now(),
        )
        .expect_err("a failed operation remains failed after mandatory finalization");
        (operation, cleanup)
    }
}

fn candidate_operation_precedence_at<T>(
    result: Result<T, LocalOllamaManagedGenerationError>,
    cancelled: bool,
    deadline: Option<Instant>,
    now: Instant,
) -> Result<T, LocalOllamaManagedGenerationError> {
    let Some(deadline) = deadline else {
        return result;
    };
    if now >= deadline {
        return Err(gate_error(InferenceErrorKind::Deadline, DEADLINE_CODE));
    }
    if cancelled {
        return Err(gate_error(InferenceErrorKind::Cancelled, CANCELLED_CODE));
    }
    result
}

fn gate_error(kind: InferenceErrorKind, code: &'static str) -> LocalOllamaManagedGenerationError {
    LocalOllamaManagedGenerationError::Session(InferenceError::new(kind, code))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    use rewrite_inference::{InferenceError, InferenceErrorKind};
    use rewrite_model::{
        CandidateGenerationAttemptCleanupDispositionV1,
        CandidateGenerationAttemptFailureCategoryV1, CandidateGenerationAttemptFailurePhaseV1,
    };

    use super::{CandidateOperationDeadline, candidate_operation_precedence_at};
    use crate::local_ollama_managed_preflight::generation::LocalOllamaManagedGenerationError;

    fn underlying_error() -> LocalOllamaManagedGenerationError {
        LocalOllamaManagedGenerationError::Session(InferenceError::new(
            InferenceErrorKind::Permanent,
            "underlying",
        ))
    }

    fn kind(error: &LocalOllamaManagedGenerationError) -> InferenceErrorKind {
        match error {
            LocalOllamaManagedGenerationError::Session(error) => error.kind,
            _ => panic!("deadline gate returned a non-session error"),
        }
    }

    #[test]
    fn equality_and_expiry_beat_cancellation_and_underlying_error() {
        let now = Instant::now();
        for deadline in [
            now,
            now.checked_sub(Duration::from_nanos(1))
                .expect("the monotonic instant supports a one-nanosecond earlier fixture"),
        ] {
            let error = candidate_operation_precedence_at::<()>(
                Err(underlying_error()),
                true,
                Some(deadline),
                now,
            )
            .expect_err("deadline must win");
            assert_eq!(kind(&error), InferenceErrorKind::Deadline);
        }
    }

    #[test]
    fn cancellation_beats_underlying_error_before_deadline() {
        let now = Instant::now();
        let error = candidate_operation_precedence_at::<()>(
            Err(underlying_error()),
            true,
            Some(now + Duration::from_secs(1)),
            now,
        )
        .expect_err("cancellation must win");
        assert_eq!(kind(&error), InferenceErrorKind::Cancelled);
    }

    #[test]
    fn underlying_result_survives_when_strict_gate_is_active() {
        let now = Instant::now();
        let error = candidate_operation_precedence_at::<()>(
            Err(underlying_error()),
            false,
            Some(now + Duration::from_secs(1)),
            now,
        )
        .expect_err("underlying error must be retained");
        assert_eq!(kind(&error), InferenceErrorKind::Permanent);
    }

    #[test]
    fn compatibility_gate_does_not_reclassify_cancellation() {
        let now = Instant::now();
        let error = candidate_operation_precedence_at::<()>(
            Err(underlying_error()),
            true,
            CandidateOperationDeadline::compatibility().instant(),
            now,
        )
        .expect_err("compatibility error must be retained");
        assert_eq!(kind(&error), InferenceErrorKind::Permanent);
    }

    #[test]
    fn runtime_gate_methods_cover_active_cancelled_expired_and_compatibility_paths() {
        let active = rewrite_types::CancellationToken::new();
        let future = CandidateOperationDeadline::until(Instant::now() + Duration::from_mins(1));
        assert!(future.ensure_active(&active).is_ok());
        assert_eq!(
            future.precedence(Ok(7_u8), &active).expect("active result"),
            7
        );
        assert!(future.terminal_override(&active).is_none());

        let cancelled = rewrite_types::CancellationToken::new();
        cancelled.cancel();
        let error = future
            .precedence(Err::<(), _>(underlying_error()), &cancelled)
            .expect_err("cancellation must override the underlying error");
        assert_eq!(kind(&error), InferenceErrorKind::Cancelled);

        let expired = CandidateOperationDeadline::until(Instant::now());
        let error = expired
            .terminal_override(&cancelled)
            .expect("deadline must override simultaneous cancellation");
        assert_eq!(kind(&error), InferenceErrorKind::Deadline);

        assert!(
            CandidateOperationDeadline::compatibility()
                .terminal_override(&cancelled)
                .is_none()
        );
    }

    #[test]
    fn mandatory_finalizer_runs_before_terminal_deadline_classification() {
        let finalized = Cell::new(false);
        let cancellation = rewrite_types::CancellationToken::new();
        cancellation.cancel();
        let (error, cleanup) = CandidateOperationDeadline::until(Instant::now()).finalize_failure(
            underlying_error(),
            &cancellation,
            || {
                finalized.set(true);
                "retained cleanup evidence"
            },
        );
        assert!(finalized.get());
        assert_eq!(cleanup, "retained cleanup evidence");
        assert_eq!(kind(&error), InferenceErrorKind::Deadline);
    }

    #[test]
    fn context_and_failure_facts_retain_the_original_deadline_and_stage() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let cancellation = rewrite_types::CancellationToken::new();
        let gate = CandidateOperationDeadline::until(deadline);
        assert_eq!(gate.context(&cancellation).deadline(), Some(deadline));

        for phase in [
            CandidateGenerationAttemptFailurePhaseV1::Launch,
            CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation,
            CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic,
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            CandidateGenerationAttemptFailurePhaseV1::FinalObservation,
            CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
            CandidateGenerationAttemptFailurePhaseV1::Cleanup,
        ] {
            let error = super::gate_error(InferenceErrorKind::Deadline, "stage_test");
            let (observed_phase, category, cleanup) =
                super::super::managed_attempt::failure_facts_for_test(phase, &error);
            assert_eq!(observed_phase, phase);
            assert_eq!(
                category,
                CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded
            );
            assert_eq!(
                cleanup,
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded
            );
        }
    }
}
