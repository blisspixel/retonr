use rewrite_model::{CandidateGenerationAttemptPrecursorV1, PlannedCandidateAttemptV1};

use super::error::{
    ManagedCandidateAttemptFailureFacts, ManagedCandidateAttemptProgress,
    RetainedBracketCleanupFailures,
};
use super::outcome::{ManagedCandidateAttemptExecutionOutcome, failed_managed_attempt};
use super::{ManagedCandidateAttemptExecutionError, ManagedCandidateAttemptPrimaryFailure};
use crate::local_ollama_managed_preflight::generation::LocalOllamaManagedGenerationError;

pub(super) fn generation_failure_outcome(
    planned_attempt: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    error: LocalOllamaManagedGenerationError,
    progress: &ManagedCandidateAttemptProgress,
    retained_cleanup: Option<RetainedBracketCleanupFailures>,
) -> Result<ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptExecutionError> {
    let facts = progress
        .generation_failure_facts(&error)
        .with_cleanup_failed(retained_cleanup.is_some());
    failed_managed_attempt(
        planned_attempt,
        precursor,
        ManagedCandidateAttemptPrimaryFailure::Generation(error),
        retained_cleanup,
        facts,
    )
    .map_err(ManagedCandidateAttemptExecutionError::from)
}

pub(super) fn override_terminal_category(
    facts: &mut ManagedCandidateAttemptFailureFacts,
    terminal: Option<&LocalOllamaManagedGenerationError>,
    progress: &ManagedCandidateAttemptProgress,
) {
    if let Some(error) = terminal {
        facts.failure_category = progress.generation_failure_facts(error).failure_category;
    }
}
