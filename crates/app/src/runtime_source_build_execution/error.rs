use std::io;

use rewrite_runtime_isolation::IsolationError;
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{ArtifactInventoryError, RuntimeSourceBuildBundleError};

use super::RuntimeSourceBuildFailure;

/// Failure at the composed retained-bundle controlled-build boundary.
#[derive(Debug, Error)]
pub enum RuntimeSourceBuildExecutionError {
    /// The output path could not be made absolute.
    #[error("runtime source-build output path is invalid")]
    InvalidOutput(#[source] io::Error),
    /// The selected output boundary could not be inspected.
    #[error("runtime source-build output could not be inspected")]
    OutputIo(#[source] io::Error),
    /// The selected output boundary is indirect or not a direct directory.
    #[error("runtime source-build output boundary is unsafe")]
    UnsafeOutput,
    /// The selected output directory contained an entry before launch.
    #[error("runtime source-build output directory is not empty")]
    OutputNotEmpty,
    /// The primary and rebuild output selections overlap.
    #[error("runtime source-build attempt output boundaries overlap")]
    OutputOverlap,
    /// The held or selected output directory identity changed.
    #[error("runtime source-build output directory changed")]
    OutputChanged,
    /// The completed output tree exceeded its fixed entry ceiling.
    #[error("runtime source-build output tree exceeded its entry limit")]
    OutputTreeLimitExceeded,
    /// The completed output tree exceeded its aggregate logical-byte ceiling.
    #[error("runtime source-build output tree exceeded its byte limit")]
    OutputByteLimitExceeded,
    /// A retained capability or managed observation diverged from the build plan.
    #[error("runtime source-build execution diverged from its verified plan")]
    PlanMismatch,
    /// The retained build program returned a non-success process status.
    #[error("runtime source-build program did not complete successfully")]
    BuildFailed(RuntimeSourceBuildFailure),
    /// The build program emitted more bytes than the retained stream ceiling.
    #[error("runtime source-build output exceeded its stream limit")]
    OutputLimitExceeded,
    /// Cooperative cancellation was observed.
    #[error("runtime source-build execution was cancelled")]
    Cancelled,
    /// The verified input bundle failed acquisition or revalidation.
    #[error(transparent)]
    Bundle(#[from] RuntimeSourceBuildBundleError),
    /// Managed runtime isolation or build execution failed.
    #[error(transparent)]
    Isolation(#[from] IsolationError),
}

pub(super) fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), RuntimeSourceBuildExecutionError> {
    if cancellation.is_cancelled() {
        Err(RuntimeSourceBuildExecutionError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn map_output(error: ArtifactInventoryError) -> RuntimeSourceBuildExecutionError {
    match error {
        ArtifactInventoryError::StorageIo(error) => {
            RuntimeSourceBuildExecutionError::OutputIo(error)
        }
        ArtifactInventoryError::Cancelled => RuntimeSourceBuildExecutionError::Cancelled,
        ArtifactInventoryError::ConcurrentModification => {
            RuntimeSourceBuildExecutionError::OutputChanged
        }
        ArtifactInventoryError::StorageEntryLimitExceeded => {
            RuntimeSourceBuildExecutionError::OutputNotEmpty
        }
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::StorageInUse
        | ArtifactInventoryError::InvalidLimits
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::State(_) => RuntimeSourceBuildExecutionError::UnsafeOutput,
    }
}

pub(super) fn map_completed_output(
    error: ArtifactInventoryError,
) -> RuntimeSourceBuildExecutionError {
    if matches!(error, ArtifactInventoryError::StorageEntryLimitExceeded) {
        RuntimeSourceBuildExecutionError::OutputTreeLimitExceeded
    } else {
        map_output(error)
    }
}
