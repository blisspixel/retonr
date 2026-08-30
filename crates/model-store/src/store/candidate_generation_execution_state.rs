//! Bounded terminal-state lookup for one candidate-generation attempt.

use rewrite_model::{GenerationQualificationPlanId, PlannedCandidateAttemptId};
use rusqlite::{OptionalExtension as _, params};

use super::ArtifactStateStore;
use crate::{StoreError, StoreResult};

/// Durable terminal state of one planned candidate-generation attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateGenerationExecutionV1State {
    /// No terminal attempt record exists.
    Missing,
    /// The attempt has one terminal failed record.
    Failed,
    /// The attempt has one terminal completed record.
    Completed,
}

impl ArtifactStateStore {
    /// Reads only the terminal outcome index for one planned attempt.
    ///
    /// This bounded lookup does not read or decode any canonical record bytes.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::CorruptRecord`] if the indexed outcome is not one of
    /// the two schema-defined values, or another [`StoreError`] if the query fails.
    pub fn candidate_generation_execution_v1_state(
        &self,
        plan_id: &GenerationQualificationPlanId,
        attempt_id: &PlannedCandidateAttemptId,
    ) -> StoreResult<CandidateGenerationExecutionV1State> {
        let state = self
            .connection
            .query_row(
                "SELECT CASE outcome
                     WHEN 'failed' THEN 1
                     WHEN 'completed' THEN 2
                     ELSE 0
                 END
                 FROM candidate_generation_attempt_records
                 WHERE generation_qualification_plan_id = ?1
                   AND planned_candidate_attempt_id = ?2
                 LIMIT 1",
                params![plan_id.digest().as_str(), attempt_id.digest().as_str()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        match state {
            None => Ok(CandidateGenerationExecutionV1State::Missing),
            Some(1) => Ok(CandidateGenerationExecutionV1State::Failed),
            Some(2) => Ok(CandidateGenerationExecutionV1State::Completed),
            Some(_) => Err(StoreError::CorruptRecord),
        }
    }
}

#[cfg(test)]
#[path = "candidate_generation_execution_state/tests.rs"]
mod tests;
