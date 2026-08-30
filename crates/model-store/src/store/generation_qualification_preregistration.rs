//! Atomic durable preregistration for a qualification operation and request projection.

use std::{error::Error, fmt};

use rewrite_model::{
    ArtifactSetManifest, GenerationQualificationOperationPolicyId,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionId, GenerationQualificationRequestProjectionV1,
    GenerationQualificationRequestProjectionV1Relations,
    MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES,
    MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES,
};
use rusqlite::{Connection, TransactionBehavior, params};

use super::generation_qualification_plan_foundation::{
    GenerationQualificationPlanFoundationV1, read::load_foundation as load_plan_foundation,
};
use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

use super::generation_qualification_plan_foundation::GenerationQualificationPlanFoundationV1Input;

/// Complete inert foundation material required before one operation is preregistered.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPreregistrationFoundationV1Input<'a> {
    /// Exact runtime artifact sets required by the plan's generation systems.
    pub runtime_artifact_sets: &'a [ArtifactSetManifest],
    /// Complete portable plan foundation.
    pub plan_foundation: GenerationQualificationPlanFoundationV1Input<'a>,
}

/// Exact typed inputs required to preregister one policy and its sole projection.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPreregistrationV1Input<'a> {
    /// Exact inert operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete independently retained relationship closure for the policy.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'a>,
    /// Exact independently retained non-derived policy inputs.
    pub operation_policy_input: &'a GenerationQualificationOperationPolicyV1Input,
    /// Exact inert request projection.
    pub request_projection: &'a GenerationQualificationRequestProjectionV1,
    /// Complete independently derived projection entries in plan order.
    pub request_projection_entry_inputs:
        &'a [GenerationQualificationRequestProjectionEntryV1Input],
}

/// Independently trusted inputs required to read one durable preregistration cohort.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPreregistrationReadInput<'a> {
    /// Expected operation-policy identity.
    pub operation_policy_id: &'a GenerationQualificationOperationPolicyId,
    /// Expected request-projection identity.
    pub request_projection_id: &'a GenerationQualificationRequestProjectionId,
    /// Complete independently retained relationship closure for the policy.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'a>,
    /// Exact independently retained non-derived policy inputs.
    pub operation_policy_input: &'a GenerationQualificationOperationPolicyV1Input,
    /// Complete independently derived projection entries in plan order.
    pub request_projection_entry_inputs:
        &'a [GenerationQualificationRequestProjectionEntryV1Input],
}

/// Inert, typed records borrowed only for the duration of transaction validation.
pub struct GenerationQualificationPreregistrationReadback<'a> {
    plan_foundation: &'a GenerationQualificationPlanFoundationV1,
    operation_policy: &'a GenerationQualificationOperationPolicyV1,
    request_projection: &'a GenerationQualificationRequestProjectionV1,
}

impl GenerationQualificationPreregistrationReadback<'_> {
    /// Returns the complete recursively revalidated portable plan foundation.
    #[must_use]
    pub const fn plan_foundation(&self) -> &GenerationQualificationPlanFoundationV1 {
        self.plan_foundation
    }

    /// Returns the exact revalidated operation policy.
    #[must_use]
    pub const fn operation_policy(&self) -> &GenerationQualificationOperationPolicyV1 {
        self.operation_policy
    }

    /// Returns the exact revalidated request projection.
    #[must_use]
    pub const fn request_projection(&self) -> &GenerationQualificationRequestProjectionV1 {
        self.request_projection
    }
}

/// Owned inert records read from one complete durable preregistration cohort.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredGenerationQualificationPreregistration {
    plan_foundation: GenerationQualificationPlanFoundationV1,
    operation_policy: GenerationQualificationOperationPolicyV1,
    request_projection: GenerationQualificationRequestProjectionV1,
}

impl StoredGenerationQualificationPreregistration {
    /// Returns the complete recursively revalidated portable plan foundation.
    #[must_use]
    pub const fn plan_foundation(&self) -> &GenerationQualificationPlanFoundationV1 {
        &self.plan_foundation
    }

    /// Returns the exact revalidated operation policy.
    #[must_use]
    pub const fn operation_policy(&self) -> &GenerationQualificationOperationPolicyV1 {
        &self.operation_policy
    }

    /// Returns the exact revalidated request projection.
    #[must_use]
    pub const fn request_projection(&self) -> &GenerationQualificationRequestProjectionV1 {
        &self.request_projection
    }
}

/// Per-record outcome of one atomic preregistration transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationPreregistrationWriteDisposition {
    /// Operation-policy write outcome.
    pub operation_policy: WriteDisposition,
    /// Request-projection write outcome.
    pub request_projection: WriteDisposition,
}

/// Failure of the atomic preregistration transaction.
pub enum GenerationQualificationPreregistrationTransactionError<E> {
    /// Durable storage or typed record validation failed.
    Store(StoreError),
    /// The caller's cancellation or deadline gate rejected the operation.
    Gate(E),
    /// The caller rejected the typed staged readback.
    Validation(E),
}

impl<E> fmt::Debug for GenerationQualificationPreregistrationTransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationQualificationPreregistrationTransactionError::Store",
            Self::Gate(_) => "GenerationQualificationPreregistrationTransactionError::Gate",
            Self::Validation(_) => {
                "GenerationQualificationPreregistrationTransactionError::Validation"
            }
        })
    }
}

impl<E> fmt::Display for GenerationQualificationPreregistrationTransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation qualification preregistration storage failed",
            Self::Gate(_) => "generation qualification preregistration gate rejected the commit",
            Self::Validation(_) => {
                "generation qualification preregistration readback validation failed"
            }
        })
    }
}

impl<E> Error for GenerationQualificationPreregistrationTransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically stores and revalidates one operation policy and its sole projection.
    ///
    /// The exact portable plan foundation must already exist. It is cold-read and
    /// recursively validated under the same write lock before either preregistration
    /// record is written.
    ///
    /// The gate runs before lock acquisition, after lock acquisition, and immediately
    /// before commit. The readback callback cannot return a borrow of transaction-local
    /// records. Neither callback error is exposed by this type's debug or display output.
    ///
    /// # Errors
    ///
    /// Returns a typed transaction error when a gate rejects, the callback rejects,
    /// the foundation is missing or invalid, either record is invalid or conflicting,
    /// or the transaction cannot commit.
    pub fn transact_generation_qualification_preregistration<T, E>(
        &mut self,
        input: GenerationQualificationPreregistrationV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
        validate: impl for<'readback> FnOnce(
            GenerationQualificationPreregistrationReadback<'readback>,
        ) -> Result<T, E>,
    ) -> Result<
        (T, GenerationQualificationPreregistrationWriteDisposition),
        GenerationQualificationPreregistrationTransactionError<E>,
    > {
        gate().map_err(GenerationQualificationPreregistrationTransactionError::Gate)?;
        let encoded = canonical_input(&input)
            .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;
        gate().map_err(GenerationQualificationPreregistrationTransactionError::Gate)?;

        let plan_foundation =
            require_plan_foundation(&transaction, input.operation_policy_relations)
                .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;

        let policy_disposition = insert_policy(&transaction, input.operation_policy, &encoded.0)
            .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;
        let projection_disposition =
            insert_projection(&transaction, input.request_projection, &encoded.1)
                .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;
        let stored = load_pair(
            &transaction,
            GenerationQualificationPreregistrationReadInput {
                operation_policy_id: input.operation_policy.operation_policy_id(),
                request_projection_id: input.request_projection.request_projection_id(),
                operation_policy_relations: input.operation_policy_relations,
                operation_policy_input: input.operation_policy_input,
                request_projection_entry_inputs: input.request_projection_entry_inputs,
            },
            plan_foundation,
        )
        .map_err(GenerationQualificationPreregistrationTransactionError::Store)?
        .ok_or(StoreError::CorruptRecord)
        .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;
        if stored.operation_policy != *input.operation_policy
            || stored.request_projection != *input.request_projection
        {
            return Err(
                GenerationQualificationPreregistrationTransactionError::Store(
                    StoreError::ImmutableConflict,
                ),
            );
        }
        let output = validate(GenerationQualificationPreregistrationReadback {
            plan_foundation: &stored.plan_foundation,
            operation_policy: &stored.operation_policy,
            request_projection: &stored.request_projection,
        })
        .map_err(GenerationQualificationPreregistrationTransactionError::Validation)?;
        gate().map_err(GenerationQualificationPreregistrationTransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationQualificationPreregistrationTransactionError::Store)?;
        Ok((
            output,
            GenerationQualificationPreregistrationWriteDisposition {
                operation_policy: policy_disposition,
                request_projection: projection_disposition,
            },
        ))
    }

    /// Reads one complete preregistration cohort using independent typed dependencies.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for a missing foundation, partial preregistration,
    /// corruption, substitution, malformed canonical bytes, or an inconsistent
    /// relationship closure.
    pub fn generation_qualification_preregistration(
        &self,
        input: GenerationQualificationPreregistrationReadInput<'_>,
    ) -> StoreResult<Option<StoredGenerationQualificationPreregistration>> {
        let transaction = self.connection.unchecked_transaction()?;
        let stored = load_preregistration(&transaction, input)?;
        transaction.commit()?;
        Ok(stored)
    }
}

pub(crate) fn load_preregistration(
    connection: &Connection,
    input: GenerationQualificationPreregistrationReadInput<'_>,
) -> StoreResult<Option<StoredGenerationQualificationPreregistration>> {
    let plan_foundation = require_plan_foundation(connection, input.operation_policy_relations)?;
    load_pair(connection, input, plan_foundation)
}

fn require_plan_foundation(
    connection: &Connection,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
) -> StoreResult<GenerationQualificationPlanFoundationV1> {
    let stored = load_plan_foundation(connection, relations.plan.qualification_plan_id())?
        .ok_or(StoreError::MissingRecord)?;
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
    if stored.plan() != relations.plan
        || stored.suite() != relations.suite
        || stored.repetitions() != relations.repetitions
        || stored.planned_attempts() != relations.planned_attempts
        || stored.generation_systems() != expected_systems
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(stored)
}

fn canonical_input(
    input: &GenerationQualificationPreregistrationV1Input<'_>,
) -> StoreResult<(Vec<u8>, Vec<u8>)> {
    let policy = serde_json::to_vec(input.operation_policy)?;
    require_blob_bound(
        policy.len(),
        MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES,
    )?;
    let decoded_policy = GenerationQualificationOperationPolicyV1::from_json_bytes(
        &policy,
        input.operation_policy_relations,
        input.operation_policy_input,
    )
    .map_err(StoreError::InvalidGenerationQualificationPreregistration)?;
    if decoded_policy != *input.operation_policy || serde_json::to_vec(&decoded_policy)? != policy {
        return Err(StoreError::CorruptRecord);
    }
    let projection = serde_json::to_vec(input.request_projection)?;
    require_blob_bound(
        projection.len(),
        MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES,
    )?;
    let decoded_projection = GenerationQualificationRequestProjectionV1::from_json_bytes(
        &projection,
        projection_relations(input.operation_policy, input.operation_policy_relations),
        input.request_projection_entry_inputs,
    )
    .map_err(StoreError::InvalidGenerationQualificationPreregistration)?;
    if decoded_projection != *input.request_projection
        || serde_json::to_vec(&decoded_projection)? != projection
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok((policy, projection))
}

fn projection_relations<'a>(
    policy: &'a GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'a>,
) -> GenerationQualificationRequestProjectionV1Relations<'a> {
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy: policy,
        qualification_plan: relations.plan,
        suite: relations.suite,
        planned_attempts: relations.planned_attempts,
    }
}

fn require_blob_bound(length: usize, maximum: usize) -> StoreResult<()> {
    if length == 0 {
        Err(StoreError::CorruptRecord)
    } else if length > maximum {
        Err(StoreError::RecordTooLarge)
    } else {
        Ok(())
    }
}

fn insert_policy(
    connection: &Connection,
    policy: &GenerationQualificationOperationPolicyV1,
    encoded: &[u8],
) -> StoreResult<WriteDisposition> {
    let changed = connection.execute(
        "INSERT INTO generation_qualification_operation_policies (
             operation_policy_id, generation_qualification_plan_id, suite_manifest_id,
             target_generation_system_id, baseline_generation_system_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT DO NOTHING",
        params![
            policy.operation_policy_id().digest().as_str(),
            policy.generation_qualification_plan_id().digest().as_str(),
            policy.suite_manifest_id().digest().as_str(),
            policy.target_generation_system_id().digest().as_str(),
            policy.baseline_generation_system_id().digest().as_str(),
            encoded,
        ],
    )?;
    let disposition = disposition(changed)?;
    if disposition == WriteDisposition::AlreadyPresent {
        read::require_exact_policy_row(
            connection,
            &read::PolicyRow {
                record_id: policy.operation_policy_id().digest().as_str().to_owned(),
                plan_id: policy
                    .generation_qualification_plan_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                suite_id: policy.suite_manifest_id().digest().as_str().to_owned(),
                target_system_id: policy
                    .target_generation_system_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                baseline_system_id: policy
                    .baseline_generation_system_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                canonical_json: encoded.to_vec(),
            },
        )?;
    }
    Ok(disposition)
}

fn insert_projection(
    connection: &Connection,
    projection: &GenerationQualificationRequestProjectionV1,
    encoded: &[u8],
) -> StoreResult<WriteDisposition> {
    let entry_count =
        i64::try_from(projection.entry_count()).map_err(|_| StoreError::InvalidLimit)?;
    let changed = connection.execute(
        "INSERT INTO generation_qualification_request_projections (
             request_projection_id, operation_policy_id, generation_qualification_plan_id,
             suite_manifest_id, target_generation_system_id, baseline_generation_system_id,
             entry_count, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT DO NOTHING",
        params![
            projection.request_projection_id().digest().as_str(),
            projection.operation_policy_id().digest().as_str(),
            projection
                .generation_qualification_plan_id()
                .digest()
                .as_str(),
            projection.suite_manifest_id().digest().as_str(),
            projection.target_generation_system_id().digest().as_str(),
            projection.baseline_generation_system_id().digest().as_str(),
            entry_count,
            encoded,
        ],
    )?;
    let disposition = disposition(changed)?;
    if disposition == WriteDisposition::AlreadyPresent {
        read::require_exact_projection_row(
            connection,
            &read::ProjectionRow {
                record_id: projection
                    .request_projection_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                operation_policy_id: projection
                    .operation_policy_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                plan_id: projection
                    .generation_qualification_plan_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                suite_id: projection.suite_manifest_id().digest().as_str().to_owned(),
                target_system_id: projection
                    .target_generation_system_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                baseline_system_id: projection
                    .baseline_generation_system_id()
                    .digest()
                    .as_str()
                    .to_owned(),
                entry_count,
                canonical_json: encoded.to_vec(),
            },
        )?;
    }
    Ok(disposition)
}

fn disposition(changed: usize) -> StoreResult<WriteDisposition> {
    match changed {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

#[path = "generation_qualification_preregistration/read.rs"]
mod read;
use read::load_pair;

#[cfg(test)]
#[path = "generation_qualification_preregistration/tests.rs"]
pub(crate) mod tests;
