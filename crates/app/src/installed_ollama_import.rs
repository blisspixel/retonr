use std::{io, path::PathBuf};

use rewrite_model::{ArtifactSetManifest, ModelPackageManifest, ModelPackageManifestId};
use rewrite_model_store::StoreError;
use rewrite_ollama_package::{
    ReconstructionError, ReconstructionLimits, RootfsDescriptorComparison,
};
use thiserror::Error;

use crate::{
    ArtifactRepositoryErrorKind, ArtifactSetImportDisposition, ArtifactSetImportError,
    ArtifactSetImportLimits, ArtifactSetInstallationKey,
};

mod source;

pub(crate) use source::PinnedInstalledOllamaModel;

/// A validated local Ollama model reference.
///
/// Components are stored separately so they can only be used as direct path
/// names under the fixed Ollama `manifests` hierarchy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaModelReference {
    registry: String,
    namespace: String,
    model: String,
    tag: String,
}

impl OllamaModelReference {
    /// Validates one registry, namespace, model, and tag reference.
    ///
    /// # Errors
    ///
    /// Returns [`OllamaModelImportError::InvalidReference`] for empty,
    /// noncanonical, oversized, or path-capable components.
    pub fn new(
        registry: impl Into<String>,
        namespace: impl Into<String>,
        model: impl Into<String>,
        tag: impl Into<String>,
    ) -> Result<Self, OllamaModelImportError> {
        let value = Self {
            registry: registry.into(),
            namespace: namespace.into(),
            model: model.into(),
            tag: tag.into(),
        };
        if !valid_component(&value.registry, 253)
            || !valid_component(&value.namespace, 128)
            || !valid_component(&value.model, 128)
            || !valid_component(&value.tag, 128)
        {
            return Err(OllamaModelImportError::InvalidReference);
        }
        Ok(value)
    }

    /// Returns the registry path component.
    #[must_use]
    pub fn registry(&self) -> &str {
        &self.registry
    }

    /// Returns the namespace path component.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the model path component.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Returns the tag path component.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Returns the canonical shortest Ollama API reference for these exact parts.
    ///
    /// The default registry and namespace are omitted using Ollama's stable
    /// display rules. Nondefault registries and namespaces remain explicit.
    #[must_use]
    pub fn runtime_reference(&self) -> String {
        const DEFAULT_REGISTRY: &str = "registry.ollama.ai";
        const DEFAULT_NAMESPACE: &str = "library";

        if self.registry == DEFAULT_REGISTRY {
            if self.namespace == DEFAULT_NAMESPACE {
                format!("{}:{}", self.model, self.tag)
            } else {
                format!("{}/{}:{}", self.namespace, self.model, self.tag)
            }
        } else {
            format!(
                "{}/{}/{}:{}",
                self.registry, self.namespace, self.model, self.tag
            )
        }
    }

    pub(crate) fn source_locator(&self) -> String {
        format!("{}/{}/{}", self.registry, self.namespace, self.model)
    }
}

/// Validated selection of one installed Ollama models root and model reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledOllamaModelSource {
    models_root: PathBuf,
    reference: OllamaModelReference,
}

impl InstalledOllamaModelSource {
    /// Forms an absolute models-root selection without opening it.
    ///
    /// Filesystem type, link, and replacement checks occur when the import pins
    /// the source. No manifest or blob path is accepted from the caller.
    ///
    /// # Errors
    ///
    /// Returns [`OllamaModelImportError::InvalidModelsRoot`] when the path cannot
    /// be made absolute.
    pub fn new(
        models_root: impl Into<PathBuf>,
        reference: OllamaModelReference,
    ) -> Result<Self, OllamaModelImportError> {
        let models_root = std::path::absolute(models_root.into())
            .map_err(OllamaModelImportError::InvalidModelsRoot)?;
        Ok(Self {
            models_root,
            reference,
        })
    }

    /// Returns the selected absolute Ollama models root.
    #[must_use]
    pub fn models_root(&self) -> &std::path::Path {
        &self.models_root
    }

    /// Returns the validated installed-model reference.
    #[must_use]
    pub const fn reference(&self) -> &OllamaModelReference {
        &self.reference
    }
}

/// Caller-owned ceilings for one installed Ollama model import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaModelImportLimits {
    /// Parser and descriptor byte ceilings, which may only lower hard limits.
    pub reconstruction: ReconstructionLimits,
    /// Managed artifact-set publication and storage ceilings.
    pub artifact_set: ArtifactSetImportLimits,
}

/// Persistence outcome for the inert semantic package manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageManifestWriteDisposition {
    /// The exact immutable package manifest was inserted.
    Inserted,
    /// The exact immutable package manifest was already present.
    AlreadyPresent,
}

/// Exact structural evidence reconstructed and read back after persistence.
///
/// This evidence is inert. It does not qualify, activate, lease, or execute the
/// imported model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaModelImportEvidence {
    artifact_set: ArtifactSetManifest,
    model_package: ModelPackageManifest,
    rootfs_comparison: RootfsDescriptorComparison,
}

impl OllamaModelImportEvidence {
    pub(crate) const fn new(
        artifact_set: ArtifactSetManifest,
        model_package: ModelPackageManifest,
        rootfs_comparison: RootfsDescriptorComparison,
    ) -> Self {
        Self {
            artifact_set,
            model_package,
            rootfs_comparison,
        }
    }

    /// Returns the exact canonical six-member byte manifest.
    #[must_use]
    pub const fn artifact_set(&self) -> &ArtifactSetManifest {
        &self.artifact_set
    }

    /// Returns the exact persisted semantic model-package manifest.
    #[must_use]
    pub const fn model_package(&self) -> &ModelPackageManifest {
        &self.model_package
    }

    /// Returns the informational config `rootfs.diff_ids` comparison.
    #[must_use]
    pub const fn rootfs_comparison(&self) -> &RootfsDescriptorComparison {
        &self.rootfs_comparison
    }
}

/// Successful inert import and durable readback result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaModelImportResult {
    /// Exact store-issued artifact-set installation generation.
    pub artifact_set_key: ArtifactSetInstallationKey,
    /// Managed byte publication disposition.
    pub artifact_set_disposition: ArtifactSetImportDisposition,
    /// Immutable semantic package-manifest write disposition.
    pub model_package_disposition: PackageManifestWriteDisposition,
    /// Reconstructed and durably read-back structural evidence.
    pub evidence: OllamaModelImportEvidence,
}

impl OllamaModelImportResult {
    /// Returns the exact semantic package-manifest identity.
    #[must_use]
    pub fn model_package_manifest_id(&self) -> ModelPackageManifestId {
        self.evidence.model_package.model_package_manifest_id()
    }
}

/// Failure from the installed Ollama model import boundary.
#[derive(Debug, Error)]
pub enum OllamaModelImportError {
    /// A reference component is not one bounded canonical direct path name.
    #[error("installed Ollama model reference is invalid")]
    InvalidReference,
    /// The selected models root could not be made absolute.
    #[error("installed Ollama models root is invalid")]
    InvalidModelsRoot(#[source] io::Error),
    /// A source path could not be opened or read.
    #[error("installed Ollama model source could not be read")]
    SourceIo(#[source] io::Error),
    /// A selected source boundary is indirect, special, or multiply linked.
    #[error("installed Ollama model source is unsafe")]
    UnsafeSource,
    /// A pinned source path or file changed during import.
    #[error("installed Ollama model source changed during import")]
    SourceChanged,
    /// Cooperative cancellation was observed before inert registration completed.
    #[error("installed Ollama model import was cancelled")]
    Cancelled,
    /// Strict manifest, descriptor, GGUF, or model-contract reconstruction failed.
    #[error(transparent)]
    Reconstruction(#[from] ReconstructionError),
    /// Application-owned staging or exact artifact-set registration failed.
    #[error(transparent)]
    ArtifactSet(#[from] ArtifactSetImportError),
    /// Semantic model-package persistence or readback failed.
    #[error("installed Ollama model package state operation failed")]
    State(#[from] StoreError),
    /// A successful write could not be read back exactly.
    #[error("installed Ollama model package readback disagreed with the imported package")]
    ReadbackConflict,
}

impl OllamaModelImportError {
    pub(crate) fn kind(&self) -> ArtifactRepositoryErrorKind {
        use ArtifactRepositoryErrorKind as Kind;
        match self {
            Self::InvalidReference | Self::InvalidModelsRoot(_) => Kind::InvalidInput,
            Self::SourceIo(_) => Kind::Operational,
            Self::SourceChanged => Kind::ConcurrentModification,
            Self::Cancelled | Self::Reconstruction(ReconstructionError::Cancelled) => {
                Kind::Cancelled
            }
            Self::Reconstruction(
                ReconstructionError::ManifestTooLarge | ReconstructionError::LimitExceeded,
            ) => Kind::ResourceLimit,
            Self::UnsafeSource | Self::ReadbackConflict | Self::Reconstruction(_) => Kind::Conflict,
            Self::ArtifactSet(error) => crate::artifact_repository::set_import_error_kind(error),
            Self::State(error) => crate::artifact_repository::store_error_kind(error),
        }
    }
}

fn valid_component(value: &str, maximum: usize) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= maximum
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && rewrite_model::ArtifactSetRelativePath::new(value.to_owned())
            .is_ok_and(|path| !path.as_str().contains('/'))
}

#[cfg(test)]
mod tests;
