use std::{io, path::PathBuf};

use rewrite_ollama_package::{
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputLimits, VerifiedRuntimeSourceBuildInputs,
};
use thiserror::Error;

/// Hard ceiling for files plus directories in one selected source-build bundle.
pub const MAX_RUNTIME_SOURCE_BUILD_BUNDLE_TREE_ENTRIES: usize = 262_144;

/// Validated selection of one manifest file and its separate component tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildBundleSource {
    manifest_path: PathBuf,
    component_root: PathBuf,
}

impl RuntimeSourceBuildBundleSource {
    /// Forms an absolute, nonoverlapping bundle selection without opening it.
    ///
    /// Filesystem type, link, identity, and replacement checks occur when the
    /// verifier pins the selection. The manifest must not be stored inside the
    /// component tree.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildBundleError::InvalidSource`] when a path
    /// cannot be made absolute, or [`RuntimeSourceBuildBundleError::UnsafeSource`]
    /// when the selected paths overlap.
    pub fn new(
        manifest_path: impl Into<PathBuf>,
        component_root: impl Into<PathBuf>,
    ) -> Result<Self, RuntimeSourceBuildBundleError> {
        let manifest_path = std::path::absolute(manifest_path.into())
            .map_err(RuntimeSourceBuildBundleError::InvalidSource)?;
        let component_root = std::path::absolute(component_root.into())
            .map_err(RuntimeSourceBuildBundleError::InvalidSource)?;
        if paths_overlap(&manifest_path, &component_root) {
            return Err(RuntimeSourceBuildBundleError::UnsafeSource);
        }
        Ok(Self {
            manifest_path,
            component_root,
        })
    }

    /// Returns the selected absolute canonical manifest path.
    #[must_use]
    pub fn manifest_path(&self) -> &std::path::Path {
        &self.manifest_path
    }

    /// Returns the selected absolute component-tree root.
    #[must_use]
    pub fn component_root(&self) -> &std::path::Path {
        &self.component_root
    }
}

/// Caller-owned ceilings for one offline source-build bundle verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildBundleLimits {
    /// Canonical manifest and declared component byte ceilings.
    pub inputs: RuntimeSourceBuildInputLimits,
    /// Maximum files plus directories accepted in the selected tree.
    pub maximum_tree_entries: usize,
}

impl Default for RuntimeSourceBuildBundleLimits {
    fn default() -> Self {
        Self {
            inputs: RuntimeSourceBuildInputLimits::default(),
            maximum_tree_entries: MAX_RUNTIME_SOURCE_BUILD_BUNDLE_TREE_ENTRIES,
        }
    }
}

impl RuntimeSourceBuildBundleLimits {
    pub(crate) fn validate(self) -> Result<Self, RuntimeSourceBuildBundleError> {
        self.inputs.validate()?;
        if self.maximum_tree_entries == 0
            || self.maximum_tree_entries > MAX_RUNTIME_SOURCE_BUILD_BUNDLE_TREE_ENTRIES
        {
            return Err(RuntimeSourceBuildBundleError::LimitExceeded);
        }
        Ok(self)
    }
}

/// Exact manifest and component identities verified from one selected bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeSourceBuildBundle {
    manifest_bytes: Vec<u8>,
    inputs: VerifiedRuntimeSourceBuildInputs,
}

impl VerifiedRuntimeSourceBuildBundle {
    pub(crate) const fn new(
        manifest_bytes: Vec<u8>,
        inputs: VerifiedRuntimeSourceBuildInputs,
    ) -> Self {
        Self {
            manifest_bytes,
            inputs,
        }
    }

    /// Returns the exact canonical manifest bytes read from the pinned file.
    #[must_use]
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    /// Returns the typed manifest whose every declared component was hashed.
    #[must_use]
    pub const fn inputs(&self) -> &VerifiedRuntimeSourceBuildInputs {
        &self.inputs
    }
}

/// Failure at the caller-selected offline source-build bundle boundary.
#[derive(Debug, Error)]
pub enum RuntimeSourceBuildBundleError {
    /// A selected path could not be made absolute.
    #[error("runtime source-build bundle path is invalid")]
    InvalidSource(#[source] io::Error),
    /// A selected path or file could not be opened or read.
    #[error("runtime source-build bundle could not be read")]
    SourceIo(#[source] io::Error),
    /// A selected boundary is overlapping, indirect, special, or multiply linked.
    #[error("runtime source-build bundle boundary is unsafe")]
    UnsafeSource,
    /// The component tree contains missing, extra, or incorrectly typed entries.
    #[error("runtime source-build component tree does not match the manifest")]
    SourceTreeMismatch,
    /// A pinned source boundary or entry changed during verification.
    #[error("runtime source-build bundle changed during verification")]
    SourceChanged,
    /// Cooperative cancellation was observed before verification completed.
    #[error("runtime source-build bundle verification was cancelled")]
    Cancelled,
    /// A caller-owned component-tree ceiling is invalid or exceeded.
    #[error("runtime source-build bundle limit was exceeded")]
    LimitExceeded,
    /// The canonical input manifest or one declared component failed validation.
    #[error(transparent)]
    Input(#[from] RuntimeSourceBuildInputError),
}

pub(super) fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    path_is_within(left, right) || path_is_within(right, left)
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
