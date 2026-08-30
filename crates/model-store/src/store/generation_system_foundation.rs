//! Atomic inert persistence for a generation-system foundation.

use std::{error::Error, fmt};

use rewrite_model::{
    EffectivePackageEvidenceV2, GenerationSystemId, GenerationSystemRecordV1,
    GenerationSystemRecordV1Relations,
};
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

mod codec;
pub(crate) mod read;
mod write;

use codec::{canonical_input, require_exact_dependencies};
pub(crate) use read::load_foundation;
use write::{insert_effective_package_evidence_v2, insert_generation_system};

/// Exact typed records needed to persist one inert generation-system foundation.
#[derive(Clone, Copy)]
pub struct GenerationSystemFoundationV1Input<'a> {
    /// Exact generation-system record.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Complete independently retained relationship closure.
    pub relations: GenerationSystemRecordV1Relations<'a>,
}

/// Inert records borrowed only while the transaction readback is valid.
pub struct GenerationSystemFoundationV1Readback<'a> {
    effective_package_evidence_v2: &'a EffectivePackageEvidenceV2,
    generation_system: &'a GenerationSystemRecordV1,
}

impl GenerationSystemFoundationV1Readback<'_> {
    /// Returns the exact revalidated package evidence.
    #[must_use]
    pub const fn effective_package_evidence_v2(&self) -> &EffectivePackageEvidenceV2 {
        self.effective_package_evidence_v2
    }

    /// Returns the exact revalidated generation-system record.
    #[must_use]
    pub const fn generation_system(&self) -> &GenerationSystemRecordV1 {
        self.generation_system
    }
}

/// Owned inert records read from one complete durable foundation.
///
/// These records contain equality evidence only. Reading them grants no runtime,
/// qualification, installation, or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationSystemFoundationV1 {
    effective_package_evidence_v2: EffectivePackageEvidenceV2,
    generation_system: GenerationSystemRecordV1,
}

impl GenerationSystemFoundationV1 {
    /// Returns the exact revalidated package evidence.
    #[must_use]
    pub const fn effective_package_evidence_v2(&self) -> &EffectivePackageEvidenceV2 {
        &self.effective_package_evidence_v2
    }

    /// Returns the exact revalidated generation-system record.
    #[must_use]
    pub const fn generation_system(&self) -> &GenerationSystemRecordV1 {
        &self.generation_system
    }
}

/// Per-record result of one atomic foundation transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationSystemFoundationV1WriteDisposition {
    /// Effective-package evidence write result.
    pub effective_package_evidence_v2: WriteDisposition,
    /// Generation-system write result.
    pub generation_system: WriteDisposition,
}

/// Failure of the atomic generation-system foundation transaction.
pub enum GenerationSystemFoundationV1TransactionError<E> {
    /// Durable storage or typed validation failed.
    Store(StoreError),
    /// The caller rejected the typed staged readback.
    Validation(E),
}

impl<E> fmt::Debug for GenerationSystemFoundationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "GenerationSystemFoundationV1TransactionError::Store",
            Self::Validation(_) => "GenerationSystemFoundationV1TransactionError::Validation",
        })
    }
}

impl<E> fmt::Display for GenerationSystemFoundationV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "generation system foundation storage failed",
            Self::Validation(_) => "generation system foundation readback validation failed",
        })
    }
}

impl<E> Error for GenerationSystemFoundationV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically creates or verifies one inert generation-system foundation.
    ///
    /// Every recursive dependency must already exist as the exact canonical model
    /// record supplied by the caller. Readback is fully decoded before commit.
    ///
    /// # Errors
    ///
    /// Returns a typed transaction error for invalid input, missing or corrupt
    /// dependencies, immutable conflict, callback rejection, or commit failure.
    pub fn transact_generation_system_foundation_v1<T, E>(
        &mut self,
        input: GenerationSystemFoundationV1Input<'_>,
        validate: impl for<'readback> FnOnce(
            GenerationSystemFoundationV1Readback<'readback>,
        ) -> Result<T, E>,
    ) -> Result<
        (T, GenerationSystemFoundationV1WriteDisposition),
        GenerationSystemFoundationV1TransactionError<E>,
    > {
        let encoded =
            canonical_input(input).map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        require_exact_dependencies(&transaction, input.relations)
            .map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        let evidence_disposition = insert_effective_package_evidence_v2(
            &transaction,
            input.relations.effective_package_evidence_v2,
            &encoded.evidence,
        )
        .map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        let system_disposition =
            insert_generation_system(&transaction, input.generation_system, &encoded.system)
                .map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        let stored = load_foundation(&transaction, input.generation_system.generation_system_id())
            .map_err(GenerationSystemFoundationV1TransactionError::Store)?
            .ok_or(StoreError::CorruptRecord)
            .map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        if stored.effective_package_evidence_v2 != *input.relations.effective_package_evidence_v2
            || stored.generation_system != *input.generation_system
        {
            return Err(GenerationSystemFoundationV1TransactionError::Store(
                StoreError::ImmutableConflict,
            ));
        }
        let output = validate(GenerationSystemFoundationV1Readback {
            effective_package_evidence_v2: &stored.effective_package_evidence_v2,
            generation_system: &stored.generation_system,
        })
        .map_err(GenerationSystemFoundationV1TransactionError::Validation)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(GenerationSystemFoundationV1TransactionError::Store)?;
        Ok((
            output,
            GenerationSystemFoundationV1WriteDisposition {
                effective_package_evidence_v2: evidence_disposition,
                generation_system: system_disposition,
            },
        ))
    }

    /// Reads and recursively revalidates one inert generation-system foundation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for a partial foundation, malformed or oversized
    /// bytes, index substitution, or any corrupt recursive dependency.
    pub fn generation_system_foundation_v1(
        &self,
        generation_system_id: &GenerationSystemId,
    ) -> StoreResult<Option<GenerationSystemFoundationV1>> {
        let transaction = self.connection.unchecked_transaction()?;
        let stored = load_foundation(&transaction, generation_system_id)?;
        transaction.commit()?;
        Ok(stored)
    }
}

#[cfg(test)]
#[path = "generation_system_foundation/tests.rs"]
mod tests;
