use std::{
    ffi::{OsStr, OsString},
    fs::File,
    path::PathBuf,
};

use rewrite_model::{ArtifactId, ArtifactSetRelativePath};
use rewrite_types::CancellationToken;

use crate::artifact_storage::{PinnedDirectory, StableMetadataFingerprint};

use super::{CandidateGenerationEvidenceBundleError, ensure_active, map_storage};

pub(super) struct RetainedMember {
    pub(super) path: ArtifactSetRelativePath,
    pub(super) artifact_id: ArtifactId,
    pub(super) byte_size: u64,
    pub(super) file: File,
    pub(super) baseline: StableMetadataFingerprint,
}

pub(super) struct PinnedBundleRoot {
    location: PinnedBundleLocation,
    pub(super) root: PinnedDirectory,
    pub(super) baseline: StableMetadataFingerprint,
}

enum PinnedBundleLocation {
    Absolute(PathBuf),
    Relative {
        parent: PinnedDirectory,
        name: OsString,
    },
}

impl PinnedBundleRoot {
    pub(super) fn new_absolute(
        path: PathBuf,
        root: PinnedDirectory,
        cancellation: &CancellationToken,
    ) -> Result<Self, CandidateGenerationEvidenceBundleError> {
        ensure_active(cancellation)?;
        let baseline = root.fingerprint().map_err(map_storage)?.stable();
        let value = Self {
            location: PinnedBundleLocation::Absolute(path),
            root,
            baseline,
        };
        value.revalidate(cancellation)?;
        Ok(value)
    }

    pub(super) fn new_relative(
        parent: &PinnedDirectory,
        name: &OsStr,
        root: PinnedDirectory,
        cancellation: &CancellationToken,
    ) -> Result<Self, CandidateGenerationEvidenceBundleError> {
        ensure_active(cancellation)?;
        let baseline = root.fingerprint().map_err(map_storage)?.stable();
        let value = Self {
            location: PinnedBundleLocation::Relative {
                parent: parent.duplicate().map_err(map_storage)?,
                name: name.to_owned(),
            },
            root,
            baseline,
        };
        value.revalidate(cancellation)?;
        Ok(value)
    }

    pub(super) fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateGenerationEvidenceBundleError> {
        ensure_active(cancellation)?;
        let held = self.root.fingerprint().map_err(map_storage)?.stable();
        let named = match &self.location {
            PinnedBundleLocation::Absolute(path) => PinnedDirectory::fingerprint_path(path),
            PinnedBundleLocation::Relative { parent, name } => {
                parent.child_directory_fingerprint(name)
            }
        }
        .map_err(map_storage)?
        .stable();
        if held == self.baseline && named == self.baseline {
            Ok(())
        } else {
            Err(CandidateGenerationEvidenceBundleError::Changed)
        }
    }
}
