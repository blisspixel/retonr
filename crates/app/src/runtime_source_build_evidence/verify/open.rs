use std::fs::File;

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::RuntimeSourceBuildAttempt;
use rewrite_types::CancellationToken;

use super::{INPUT_PREFIX, RuntimeSourceBuildEvidenceBundleLease, attempt_prefix, prefixed_path};
use crate::runtime_source_build_evidence::contract::{
    RuntimeSourceBuildEvidenceBundleError, ensure_active, map_storage,
};

impl RuntimeSourceBuildEvidenceBundleLease {
    pub(crate) fn open_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, RuntimeSourceBuildEvidenceBundleError> {
        self.open_declared(path, cancellation)
    }

    pub(crate) fn open_source_input(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, RuntimeSourceBuildEvidenceBundleError> {
        let path = prefixed_path(INPUT_PREFIX, path)?;
        self.open_declared(&path, cancellation)
    }

    pub(crate) fn open_primary_member(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, RuntimeSourceBuildEvidenceBundleError> {
        let path = prefixed_path(attempt_prefix(RuntimeSourceBuildAttempt::Primary), path)?;
        self.open_declared(&path, cancellation)
    }

    fn open_declared(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, RuntimeSourceBuildEvidenceBundleError> {
        ensure_active(cancellation)?;
        if !self
            .manifest
            .members()
            .iter()
            .any(|member| member.relative_path() == path)
        {
            return Err(RuntimeSourceBuildEvidenceBundleError::TreeMismatch);
        }
        self.pinned.revalidate(cancellation)?;
        let opened = self
            .pinned
            .root
            .open_relative_regular_file(path)
            .map_err(map_storage)?;
        if !opened.fingerprint.has_single_link() {
            return Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary);
        }
        self.pinned.revalidate(cancellation)?;
        Ok(opened.file)
    }
}
