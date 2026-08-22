use std::{io, path::PathBuf};

use rewrite_model::{ArtifactSetManifest, RuntimePackageManifest, RuntimePackageManifestId};
use rewrite_model_store::StoreError;
use rewrite_ollama_package::{RuntimeLayoutLimits, RuntimeReconstructionError};
use thiserror::Error;

use crate::{
    ArtifactRepositoryErrorKind, ArtifactSetImportDisposition, ArtifactSetImportError,
    ArtifactSetImportLimits, ArtifactSetInstallationKey, PackageManifestWriteDisposition,
};

mod source;

pub(crate) use source::PinnedReviewedOllamaRuntime;

/// Validated selection of one reviewed runtime layout and its member tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewedOllamaRuntimeSource {
    layout_path: PathBuf,
    member_root: PathBuf,
}

impl ReviewedOllamaRuntimeSource {
    /// Forms an absolute layout and member-tree selection without opening them.
    ///
    /// Filesystem type, link, and replacement checks occur when the import pins
    /// the source. The layout file is not discovered from the member tree.
    ///
    /// # Errors
    ///
    /// Returns [`OllamaRuntimeImportError::InvalidSource`] when a path cannot be
    /// made absolute, or [`OllamaRuntimeImportError::UnsafeSource`] when the
    /// layout file sits inside the member tree.
    pub fn new(
        layout_path: impl Into<PathBuf>,
        member_root: impl Into<PathBuf>,
    ) -> Result<Self, OllamaRuntimeImportError> {
        let layout_path = std::path::absolute(layout_path.into())
            .map_err(OllamaRuntimeImportError::InvalidSource)?;
        let member_root = std::path::absolute(member_root.into())
            .map_err(OllamaRuntimeImportError::InvalidSource)?;
        if overlapping_source_paths(&layout_path, &member_root) {
            return Err(OllamaRuntimeImportError::UnsafeSource);
        }
        Ok(Self {
            layout_path,
            member_root,
        })
    }

    /// Returns the selected absolute reviewed layout path.
    #[must_use]
    pub fn layout_path(&self) -> &std::path::Path {
        &self.layout_path
    }

    /// Returns the selected absolute member-tree root.
    #[must_use]
    pub fn member_root(&self) -> &std::path::Path {
        &self.member_root
    }
}

/// Caller-owned ceilings for one reviewed Ollama runtime import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaRuntimeImportLimits {
    /// Parser and member-byte ceilings, which may only lower hard limits.
    pub reconstruction: RuntimeLayoutLimits,
    /// Managed artifact-set publication and storage ceilings.
    pub artifact_set: ArtifactSetImportLimits,
}

/// Exact structural evidence reconstructed and read back after persistence.
///
/// This evidence is inert. It does not qualify, activate, lease, execute, or
/// admit a runtime to the production cloud-disable allowlist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaRuntimeImportEvidence {
    artifact_set: ArtifactSetManifest,
    runtime_package: RuntimePackageManifest,
}

impl OllamaRuntimeImportEvidence {
    pub(crate) const fn new(
        artifact_set: ArtifactSetManifest,
        runtime_package: RuntimePackageManifest,
    ) -> Self {
        Self {
            artifact_set,
            runtime_package,
        }
    }

    /// Returns the exact canonical runtime artifact-set manifest.
    #[must_use]
    pub const fn artifact_set(&self) -> &ArtifactSetManifest {
        &self.artifact_set
    }

    /// Returns the exact persisted semantic runtime-package manifest.
    #[must_use]
    pub const fn runtime_package(&self) -> &RuntimePackageManifest {
        &self.runtime_package
    }
}

/// Successful inert import and durable readback result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaRuntimeImportResult {
    /// Exact store-issued artifact-set installation generation.
    pub artifact_set_key: ArtifactSetInstallationKey,
    /// Managed byte publication disposition.
    pub artifact_set_disposition: ArtifactSetImportDisposition,
    /// Immutable semantic package-manifest write disposition.
    pub runtime_package_disposition: PackageManifestWriteDisposition,
    /// Reconstructed and durably read-back structural evidence.
    pub evidence: OllamaRuntimeImportEvidence,
}

impl OllamaRuntimeImportResult {
    /// Returns the exact semantic runtime-package identity.
    #[must_use]
    pub fn runtime_package_manifest_id(&self) -> RuntimePackageManifestId {
        self.evidence.runtime_package.runtime_package_manifest_id()
    }
}

/// Failure from the reviewed Ollama runtime import boundary.
#[derive(Debug, Error)]
pub enum OllamaRuntimeImportError {
    /// A selected layout or member-tree path could not be made absolute.
    #[error("reviewed Ollama runtime source path is invalid")]
    InvalidSource(#[source] io::Error),
    /// A source path could not be opened or read.
    #[error("reviewed Ollama runtime source could not be read")]
    SourceIo(#[source] io::Error),
    /// A selected source boundary is indirect, special, or multiply linked.
    #[error("reviewed Ollama runtime source is unsafe")]
    UnsafeSource,
    /// A pinned source path or file changed during import.
    #[error("reviewed Ollama runtime source changed during import")]
    SourceChanged,
    /// Cooperative cancellation was observed before inert registration completed.
    #[error("reviewed Ollama runtime import was cancelled")]
    Cancelled,
    /// Strict layout, member, or runtime-contract reconstruction failed.
    #[error(transparent)]
    Reconstruction(#[from] RuntimeReconstructionError),
    /// Application-owned staging or exact artifact-set registration failed.
    #[error(transparent)]
    ArtifactSet(#[from] ArtifactSetImportError),
    /// Semantic runtime-package persistence or readback failed.
    #[error("reviewed Ollama runtime package state operation failed")]
    State(#[from] StoreError),
    /// A successful write could not be read back exactly.
    #[error("reviewed Ollama runtime package readback disagreed with the imported package")]
    ReadbackConflict,
}

impl OllamaRuntimeImportError {
    pub(crate) fn kind(&self) -> ArtifactRepositoryErrorKind {
        use ArtifactRepositoryErrorKind as Kind;
        match self {
            Self::InvalidSource(_) => Kind::InvalidInput,
            Self::SourceIo(_) => Kind::Operational,
            Self::SourceChanged => Kind::ConcurrentModification,
            Self::Cancelled | Self::Reconstruction(RuntimeReconstructionError::Cancelled) => {
                Kind::Cancelled
            }
            Self::Reconstruction(
                RuntimeReconstructionError::LayoutTooLarge
                | RuntimeReconstructionError::LimitExceeded,
            ) => Kind::ResourceLimit,
            Self::UnsafeSource | Self::ReadbackConflict | Self::Reconstruction(_) => Kind::Conflict,
            Self::ArtifactSet(error) => crate::artifact_repository::set_import_error_kind(error),
            Self::State(error) => crate::artifact_repository::store_error_kind(error),
        }
    }
}

fn overlapping_source_paths(layout_path: &std::path::Path, member_root: &std::path::Path) -> bool {
    path_is_within(layout_path, member_root) || path_is_within(member_root, layout_path)
}

fn path_is_within(path: &std::path::Path, ancestor: &std::path::Path) -> bool {
    let path = path.components().collect::<Vec<_>>();
    let ancestor = ancestor.components().collect::<Vec<_>>();
    path.len() >= ancestor.len()
        && path
            .iter()
            .zip(&ancestor)
            .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
}

#[cfg(test)]
mod tests;
