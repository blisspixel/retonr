use std::fmt;

use rewrite_model::GenerationQualificationContractError;
use thiserror::Error;

use crate::{ManagedOllamaModelAuthorityError, PackageAttestationError};

use super::ManagedJudgePrecursorRelationship;

/// Failure to compile or revalidate one exact managed-judge precursor.
#[derive(Error)]
pub enum ManagedJudgePrecursorCompilationError {
    /// Work was cancelled outside a retained-package operation.
    #[error("managed-judge precursor compilation was cancelled")]
    Cancelled,
    /// Current retained runtime bytes or managed-tree identity changed.
    #[error("managed-judge runtime package revalidation failed")]
    RuntimeRevalidation(#[source] Box<PackageAttestationError>),
    /// Current specialized model foundation or license authority changed.
    #[error("managed-judge model authority revalidation failed")]
    ModelRevalidation(#[source] Box<ManagedOllamaModelAuthorityError>),
    /// One closed relationship was substituted.
    #[error("managed-judge precursor relationship is invalid: {0:?}")]
    Relationship(ManagedJudgePrecursorRelationship),
    /// A portable judge record rejected internally rederived values.
    #[error("managed-judge precursor portable contract rejected the input")]
    PortableContract(#[source] GenerationQualificationContractError),
}

impl fmt::Debug for ManagedJudgePrecursorCompilationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedJudgePrecursorCompilationError");
        match self {
            Self::Cancelled => value.field("kind", &"cancelled"),
            Self::RuntimeRevalidation(_) => value.field("kind", &"runtime_revalidation"),
            Self::ModelRevalidation(_) => value.field("kind", &"model_revalidation"),
            Self::Relationship(relationship) => value
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::PortableContract(_) => value.field("kind", &"portable_contract"),
        };
        value.finish_non_exhaustive()
    }
}
