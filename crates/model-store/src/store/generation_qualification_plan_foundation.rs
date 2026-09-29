//! Atomic inert persistence for one complete generation qualification plan.

use std::{error::Error, fmt};

use rewrite_model::{
    CandidateSelectionPolicyV1, GenerationCaseManifestV1, GenerationClusterRecordV1,
    GenerationDeterministicCaseContractV1, GenerationQualificationPlanId,
    GenerationQualificationPlanV1, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

mod codec;
pub(crate) mod read;
mod write;

use codec::canonical_input;
pub(crate) use read::load_foundation;
use write::insert_foundation;

/// Exact typed records needed to persist one portable plan foundation.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPlanFoundationV1Input<'a> {
    /// Complete unique cluster set referenced by the cases.
    pub clusters: &'a [GenerationClusterRecordV1],
    /// Exact deterministic contracts in semantic case order.
    pub deterministic_case_contracts: &'a [GenerationDeterministicCaseContractV1],
    /// Exact cases in semantic suite order.
    pub cases: &'a [GenerationCaseManifestV1],
    /// Exact suite over `cases`.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact preregistered repetitions in plan order.
    pub repetitions: &'a [GenerationRepetitionRecordV1],
    /// Exact digest-sorted generation-system set.
    pub generation_systems: &'a [GenerationSystemRecordV1],
    /// Complete planned attempts in execution order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// Exact pre-output candidate selection policy.
    pub candidate_selection_policy: &'a CandidateSelectionPolicyV1,
    /// Exact plan over every supplied dependency.
    pub plan: &'a GenerationQualificationPlanV1,
}

/// Inert records borrowed only while the staged transaction readback is valid.
pub struct GenerationQualificationPlanFoundationV1Readback<'a> {
    foundation: &'a GenerationQualificationPlanFoundationV1,
}

impl GenerationQualificationPlanFoundationV1Readback<'_> {
    /// Returns the complete recursively revalidated foundation.
    #[must_use]
    pub const fn foundation(&self) -> &GenerationQualificationPlanFoundationV1 {
        self.foundation
    }
}

/// Owned, inert, recursively revalidated portable plan foundation.
///
/// Reading this value grants no execution, qualification, or live-use authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationQualificationPlanFoundationV1 {
    pub(super) clusters: Vec<GenerationClusterRecordV1>,
    pub(super) deterministic_case_contracts: Vec<GenerationDeterministicCaseContractV1>,
    pub(super) cases: Vec<GenerationCaseManifestV1>,
    pub(super) suite: GenerationSuiteManifestV1,
    pub(super) repetitions: Vec<GenerationRepetitionRecordV1>,
    pub(super) generation_systems: Vec<GenerationSystemRecordV1>,
    pub(super) planned_attempts: Vec<PlannedCandidateAttemptV1>,
    pub(super) candidate_selection_policy: CandidateSelectionPolicyV1,
    pub(super) plan: GenerationQualificationPlanV1,
}

impl GenerationQualificationPlanFoundationV1 {
    /// Returns the complete unique cluster set in first semantic use order.
    #[must_use]
    pub fn clusters(&self) -> &[GenerationClusterRecordV1] {
        &self.clusters
    }

    /// Returns deterministic contracts in exact semantic case order.
    #[must_use]
    pub fn deterministic_case_contracts(&self) -> &[GenerationDeterministicCaseContractV1] {
        &self.deterministic_case_contracts
    }

    /// Returns exact cases in semantic suite order.
    #[must_use]
    pub fn cases(&self) -> &[GenerationCaseManifestV1] {
        &self.cases
    }

    /// Returns the exact suite manifest.
    #[must_use]
    pub const fn suite(&self) -> &GenerationSuiteManifestV1 {
        &self.suite
    }

    /// Returns repetitions in exact plan order.
    #[must_use]
    pub fn repetitions(&self) -> &[GenerationRepetitionRecordV1] {
        &self.repetitions
    }

    /// Returns the digest-sorted generation-system set.
    #[must_use]
    pub fn generation_systems(&self) -> &[GenerationSystemRecordV1] {
        &self.generation_systems
    }

    /// Returns planned attempts in exact execution order.
    #[must_use]
    pub fn planned_attempts(&self) -> &[PlannedCandidateAttemptV1] {
        &self.planned_attempts
    }

    /// Returns the exact pre-output candidate selection policy.
    #[must_use]
    pub const fn candidate_selection_policy(&self) -> &CandidateSelectionPolicyV1 {
        &self.candidate_selection_policy
    }

    /// Returns the exact qualification plan.
    #[must_use]
    pub const fn plan(&self) -> &GenerationQualificationPlanV1 {
        &self.plan
    }
}

/// Outcome of one atomic plan-foundation transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationPlanFoundationV1WriteDisposition {
    /// Root plan write result. Every dependency and ordering row is also exact.
    pub plan: WriteDisposition,
}

/// Failure of the atomic plan-foundation transaction.
pub enum GenerationQualificationPlanFoundationV1TransactionError<E> {
    /// Durable storage or typed validation failed.
    Store(StoreError),
    /// The caller rejected the typed staged readback.
    Validation(E),
}

impl<E> fmt::Debug for GenerationQualificationPlanFoundationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationQualificationPlanFoundationV1TransactionError::Store",
            Self::Validation(_) => {
                "GenerationQualificationPlanFoundationV1TransactionError::Validation"
            }
        })
    }
}

impl<E> fmt::Display for GenerationQualificationPlanFoundationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation qualification plan foundation storage failed",
            Self::Validation(_) => {
                "generation qualification plan foundation readback validation failed"
            }
        })
    }
}

impl<E> Error for GenerationQualificationPlanFoundationV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically creates or verifies one complete portable plan foundation.
    ///
    /// Every generation-system foundation must already exist. All other records,
    /// exact ordering rows, and the root plan are staged and recursively decoded
    /// inside one immediate transaction before commit.
    ///
    /// # Errors
    ///
    /// Returns a typed transaction error for invalid input, missing or corrupt
    /// generation systems, immutable conflict, callback rejection, or commit failure.
    pub fn transact_generation_qualification_plan_foundation_v1<T, E>(
        &mut self,
        input: GenerationQualificationPlanFoundationV1Input<'_>,
        validate: impl for<'readback> FnOnce(
            GenerationQualificationPlanFoundationV1Readback<'readback>,
        ) -> Result<T, E>,
    ) -> Result<
        (T, GenerationQualificationPlanFoundationV1WriteDisposition),
        GenerationQualificationPlanFoundationV1TransactionError<E>,
    > {
        let encoded = canonical_input(input)
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?;
        if let Some(stored) = load_foundation(&transaction, input.plan.qualification_plan_id())
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?
        {
            if !codec::matches_input(&stored, input) {
                return Err(
                    GenerationQualificationPlanFoundationV1TransactionError::Store(
                        StoreError::ImmutableConflict,
                    ),
                );
            }
            let output = validate(GenerationQualificationPlanFoundationV1Readback {
                foundation: &stored,
            })
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Validation)?;
            transaction
                .commit()
                .map_err(StoreError::from)
                .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?;
            return Ok((
                output,
                GenerationQualificationPlanFoundationV1WriteDisposition {
                    plan: WriteDisposition::AlreadyPresent,
                },
            ));
        }
        let plan_disposition = insert_foundation(&transaction, input, &encoded)
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?;
        let stored = load_foundation(&transaction, input.plan.qualification_plan_id())
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?
            .ok_or(StoreError::CorruptRecord)
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?;
        if !codec::matches_input(&stored, input) {
            return Err(
                GenerationQualificationPlanFoundationV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ),
            );
        }
        let output = validate(GenerationQualificationPlanFoundationV1Readback {
            foundation: &stored,
        })
        .map_err(GenerationQualificationPlanFoundationV1TransactionError::Validation)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationQualificationPlanFoundationV1TransactionError::Store)?;
        Ok((
            output,
            GenerationQualificationPlanFoundationV1WriteDisposition {
                plan: plan_disposition,
            },
        ))
    }

    /// Reads and recursively revalidates one complete portable plan foundation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for any missing dependency, malformed or oversized
    /// record, index substitution, association gap, or recursive mismatch.
    pub fn generation_qualification_plan_foundation_v1(
        &self,
        plan_id: &GenerationQualificationPlanId,
    ) -> StoreResult<Option<GenerationQualificationPlanFoundationV1>> {
        let transaction = self.connection.unchecked_transaction()?;
        let stored = load_foundation(&transaction, plan_id)?;
        transaction.commit()?;
        Ok(stored)
    }
}

#[cfg(test)]
#[path = "generation_qualification_plan_foundation/tests.rs"]
mod tests;
