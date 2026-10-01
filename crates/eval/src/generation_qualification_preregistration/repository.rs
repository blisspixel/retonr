//! Durable repository facade for qualification preregistration.

use std::{fmt, path::Path, time::Instant};

use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionV1,
};
use rewrite_model_store::{
    ArtifactStateStore, GenerationQualificationPlanFoundationV1Input,
    GenerationQualificationPlanFoundationV1TransactionError,
    GenerationQualificationPreregistrationFoundationV1Input,
    GenerationQualificationPreregistrationReadback,
    GenerationQualificationPreregistrationTransactionError,
    GenerationQualificationPreregistrationV1Input,
    GenerationQualificationPreregistrationWriteDisposition, GenerationSystemFoundationV1Input,
    GenerationSystemFoundationV1TransactionError, StoreError,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{GenerationQualificationPreparationError, check_gate};

mod activation_admission;
mod candidate_execution;
mod deterministic;
mod judge;
mod receipt_set;
mod repeatability;

/// Failure to open the durable qualification preregistration repository.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPreregistrationOpenError {
    /// The exact current durable store could not be opened or initialized.
    #[error("generation qualification preregistration repository is unavailable")]
    Unavailable,
}

/// Concrete eval-owned facade over the exact current artifact state store.
///
/// The public surface exposes no raw connection or mutation method. Foundation writes
/// use the schema 8 and schema 9 typed transactions, and preregistration uses the
/// schema 7 typed transaction preserved by the current schema.
pub struct GenerationQualificationPreregistrationRepository {
    store: ArtifactStateStore,
}

impl GenerationQualificationPreregistrationRepository {
    /// Opens or creates one exact-current durable preregistration database.
    ///
    /// Existing older schemas require the model store's explicit verified-backup
    /// migration session and are not migrated by this method.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error when the database cannot be opened,
    /// initialized, or validated at the exact current schema.
    pub fn open(path: &Path) -> Result<Self, GenerationQualificationPreregistrationOpenError> {
        ArtifactStateStore::open(path)
            .map(|store| Self { store })
            .map_err(|_| GenerationQualificationPreregistrationOpenError::Unavailable)
    }

    pub(crate) fn persist_foundation(
        &mut self,
        foundation: GenerationQualificationPreregistrationFoundationV1Input<'_>,
        operation_relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationPreparationError> {
        validate_foundation_input(foundation.plan_foundation, operation_relations)?;
        for system in [
            operation_relations.target_system,
            operation_relations.baseline_system,
        ] {
            check_gate(deadline, cancellation)?;
            let runtime_artifact_set = foundation
                .runtime_artifact_sets
                .iter()
                .find(|artifact_set| {
                    &artifact_set.artifact_set_id()
                        == system.relations.runtime_package_manifest.artifact_set_id()
                })
                .ok_or(GenerationQualificationPreparationError::ReadbackMismatch)?;
            self.store
                .put_artifact_set_manifest(runtime_artifact_set)
                .map_err(|error| map_store_error(&error))?;
            self.store
                .put_runtime_package_manifest(system.relations.runtime_package_manifest)
                .map_err(|error| map_store_error(&error))?;
            self.store
                .put_runtime_build_identity(system.relations.runtime_build)
                .map_err(|error| map_store_error(&error))?;
            self.store
                .put_effective_runtime_state(system.relations.effective_runtime_state)
                .map_err(|error| map_store_error(&error))?;
            self.store
                .put_artifact_set_manifest(system.relations.model_artifact_set)
                .map_err(|error| map_store_error(&error))?;
            self.store
                .put_model_package_manifest(system.relations.model_package_manifest)
                .map_err(|error| map_store_error(&error))?;
            check_gate(deadline, cancellation)?;
            self.store
                .transact_generation_system_foundation_v1(
                    GenerationSystemFoundationV1Input {
                        generation_system: system.generation_system,
                        relations: system.relations,
                    },
                    |_| check_gate(deadline, cancellation),
                )
                .map_err(map_system_foundation_error)?;
        }
        check_gate(deadline, cancellation)?;
        self.store
            .transact_generation_qualification_plan_foundation_v1(
                foundation.plan_foundation,
                |readback| {
                    check_gate(deadline, cancellation)?;
                    if readback.foundation().plan() == foundation.plan_foundation.plan {
                        Ok(())
                    } else {
                        Err(GenerationQualificationPreparationError::ReadbackMismatch)
                    }
                },
            )
            .map_err(map_plan_foundation_error)?;
        check_gate(deadline, cancellation)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the schema-7 transaction requires one complete typed relationship closure"
    )]
    pub(crate) fn transact<T>(
        &mut self,
        operation_policy: &GenerationQualificationOperationPolicyV1,
        operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        operation_policy_input: &GenerationQualificationOperationPolicyV1Input,
        request_projection: &GenerationQualificationRequestProjectionV1,
        request_projection_entry_inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
        deadline: Instant,
        cancellation: &CancellationToken,
        validate: impl for<'readback> FnOnce(
            GenerationQualificationPreregistrationReadback<'readback>,
        )
            -> Result<T, GenerationQualificationPreparationError>,
    ) -> Result<
        (T, GenerationQualificationPreregistrationWriteDisposition),
        GenerationQualificationPreparationError,
    > {
        self.store
            .transact_generation_qualification_preregistration(
                GenerationQualificationPreregistrationV1Input {
                    operation_policy,
                    operation_policy_relations,
                    operation_policy_input,
                    request_projection,
                    request_projection_entry_inputs,
                },
                || check_gate(deadline, cancellation),
                validate,
            )
            .map_err(map_transaction_error)
    }
}

impl fmt::Debug for GenerationQualificationPreregistrationRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationPreregistrationRepository")
            .field("backend", &"sqlite")
            .finish_non_exhaustive()
    }
}

fn map_transaction_error(
    error: GenerationQualificationPreregistrationTransactionError<
        GenerationQualificationPreparationError,
    >,
) -> GenerationQualificationPreparationError {
    match error {
        GenerationQualificationPreregistrationTransactionError::Store(error) => {
            map_store_error(&error)
        }
        GenerationQualificationPreregistrationTransactionError::Gate(error)
        | GenerationQualificationPreregistrationTransactionError::Validation(error) => error,
    }
}

fn validate_foundation_input(
    input: GenerationQualificationPlanFoundationV1Input<'_>,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
) -> Result<(), GenerationQualificationPreparationError> {
    let mut expected_systems = vec![
        relations.target_system.generation_system.clone(),
        relations.baseline_system.generation_system.clone(),
    ];
    expected_systems.sort_unstable_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    if input.plan == relations.plan
        && input.suite == relations.suite
        && input.repetitions == relations.repetitions
        && input.planned_attempts == relations.planned_attempts
        && input.generation_systems == expected_systems
    {
        Ok(())
    } else {
        Err(GenerationQualificationPreparationError::ReadbackMismatch)
    }
}

fn map_system_foundation_error(
    error: GenerationSystemFoundationV1TransactionError<GenerationQualificationPreparationError>,
) -> GenerationQualificationPreparationError {
    match error {
        GenerationSystemFoundationV1TransactionError::Store(error) => map_store_error(&error),
        GenerationSystemFoundationV1TransactionError::Validation(error) => error,
    }
}

fn map_plan_foundation_error(
    error: GenerationQualificationPlanFoundationV1TransactionError<
        GenerationQualificationPreparationError,
    >,
) -> GenerationQualificationPreparationError {
    match error {
        GenerationQualificationPlanFoundationV1TransactionError::Store(error) => {
            map_store_error(&error)
        }
        GenerationQualificationPlanFoundationV1TransactionError::Validation(error) => error,
    }
}

fn map_store_error(error: &StoreError) -> GenerationQualificationPreparationError {
    match error {
        StoreError::Serialization(_) => GenerationQualificationPreparationError::RepositoryEncoding,
        StoreError::RecordTooLarge => GenerationQualificationPreparationError::RepositoryLimit,
        StoreError::ImmutableConflict => {
            GenerationQualificationPreparationError::RepositoryConflict
        }
        StoreError::InvalidGenerationQualificationPreregistration(_)
        | StoreError::InvalidGenerationQualificationPlan(_)
        | StoreError::InvalidCandidateGenerationAttemptPrecursor(_)
        | StoreError::InvalidCandidateGenerationExecution(_)
        | StoreError::InvalidGenerationDeterministicCaseContract(_)
        | StoreError::CorruptRecord
        | StoreError::MissingRecord => GenerationQualificationPreparationError::ReadbackMismatch,
        _ => GenerationQualificationPreparationError::RepositoryUnavailable,
    }
}

#[cfg(test)]
#[path = "repository/tests.rs"]
mod tests;
