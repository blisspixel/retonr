use std::{ffi::OsString, path::Path};

use rewrite_model_store::{ArtifactStateStore, StoredArtifactInstallation};
use rewrite_types::CancellationToken;

use crate::{
    artifact_inventory::ArtifactInventoryError,
    artifact_storage::{
        ExactArtifactExpectation, ExactArtifactSync, ExactArtifactVerificationError,
        ExistingArtifactStorage, LifecycleLockMode, VerifiedManagedArtifact,
        verify_exact_artifact_for_runtime,
    },
};

/// Caller-owned ceilings for acquiring one managed runtime artifact lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactLeaseLimits {
    /// Maximum bytes accepted for the selected artifact.
    pub maximum_artifact_bytes: u64,
    /// Maximum entries inspected in managed artifact storage.
    pub maximum_storage_entries: usize,
}

/// Shared lifecycle lease retained for the complete managed artifact use lifetime.
///
/// The lease pins the storage boundary and verified artifact handle. Removal takes
/// the exclusive lifecycle lock and therefore fails while any lease remains live.
pub struct RuntimeArtifactLease {
    _artifact: VerifiedManagedArtifact,
    _storage: ExistingArtifactStorage,
    selection: StoredArtifactInstallation,
}

impl RuntimeArtifactLease {
    /// Acquires and verifies one exact installed artifact under the shared lifecycle
    /// lock without exposing a path to the caller.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactInventoryError`] when limits are invalid, storage is busy
    /// or unsafe, cancellation is observed, or the exact managed bytes disagree.
    pub fn acquire(
        root: impl AsRef<Path>,
        store: &ArtifactStateStore,
        selection: StoredArtifactInstallation,
        limits: RuntimeArtifactLeaseLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, ArtifactInventoryError> {
        if limits.maximum_artifact_bytes == 0
            || limits.maximum_storage_entries == 0
            || selection.installed.byte_size > limits.maximum_artifact_bytes
        {
            return Err(ArtifactInventoryError::InvalidLimits);
        }
        selection
            .installed
            .validate()
            .map_err(|_| ArtifactInventoryError::UnsafeStorageLayout)?;
        let expected_key = format!("artifacts/{}", selection.installed.artifact_digest.as_str());
        if selection.installed.storage_key != expected_key {
            return Err(ArtifactInventoryError::UnsafeStorageLayout);
        }
        let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Shared)?;
        let (current, _) = store
            .artifact_removal_state(&selection.installed.artifact_id)
            .map_err(ArtifactInventoryError::State)?;
        if current.as_ref() != Some(&selection) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        let name = OsString::from(selection.installed.artifact_digest.as_str());
        let artifact = verify_exact_artifact_for_runtime(
            storage.artifacts(),
            &name,
            ExactArtifactExpectation {
                byte_size: selection.installed.byte_size,
                digest: &selection.installed.artifact_digest,
                maximum_entries: limits.maximum_storage_entries,
                sync: ExactArtifactSync::Normal,
            },
            cancellation,
            |_| {},
        )
        .map_err(map_verification_error)?;
        storage.validate_layout()?;
        let (current, _) = store
            .artifact_removal_state(&selection.installed.artifact_id)
            .map_err(ArtifactInventoryError::State)?;
        if current.as_ref() != Some(&selection) {
            return Err(ArtifactInventoryError::ConcurrentModification);
        }
        Ok(Self {
            _artifact: artifact,
            _storage: storage,
            selection,
        })
    }

    /// Exact installation generation protected by this live lease.
    #[must_use]
    pub const fn selection(&self) -> &StoredArtifactInstallation {
        &self.selection
    }
}

fn map_verification_error(error: ExactArtifactVerificationError) -> ArtifactInventoryError {
    match error {
        ExactArtifactVerificationError::Boundary(error) => error,
        ExactArtifactVerificationError::Missing
        | ExactArtifactVerificationError::SizeMismatch
        | ExactArtifactVerificationError::DigestMismatch
        | ExactArtifactVerificationError::Aliased => ArtifactInventoryError::ConcurrentModification,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rewrite_model::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, InstalledArtifact, LicenseRecord,
    };
    use rewrite_model_store::ArtifactStateStore;
    use rewrite_types::Digest;

    use super::*;
    use crate::{ArtifactRemovalError, ArtifactRemovalLimits, ArtifactRemovalService};

    #[test]
    fn live_runtime_lease_blocks_exclusive_removal() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("managed");
        fs::create_dir(&root).expect("create root");
        fs::create_dir(root.join("artifacts")).expect("create artifacts");
        fs::write(root.join(".artifact-import.lock"), []).expect("create lock");
        let bytes = b"artifact";
        let digest = Digest::sha256(bytes);
        let manifest = ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            source: ArtifactSource {
                origin: "fixture/model".to_owned(),
                revision: "revision".to_owned(),
            },
            artifact_digest: digest.clone(),
            byte_size: u64::try_from(bytes.len()).expect("size"),
            format: "gguf".to_owned(),
            family: "fixture".to_owned(),
            architecture: None,
            quantization: None,
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: Some(8_192),
            },
        };
        let installed = InstalledArtifact {
            artifact_id: manifest.artifact_id.clone(),
            artifact_digest: digest.clone(),
            byte_size: manifest.byte_size,
            storage_key: format!("artifacts/{}", digest.as_str()),
        };
        fs::write(root.join("artifacts").join(digest.as_str()), bytes).expect("write artifact");
        #[cfg(windows)]
        {
            let canonical = root.join("artifacts").join(digest.as_str());
            let mut permissions = fs::metadata(&canonical)
                .expect("artifact metadata")
                .permissions();
            permissions.set_readonly(true);
            fs::set_permissions(&canonical, permissions).expect("make artifact read-only");
        }
        let mut store =
            ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("open state");
        let selection = store
            .put_installation(&manifest, &installed)
            .expect("register")
            .installation;
        let lease = RuntimeArtifactLease::acquire(
            &root,
            &store,
            selection,
            RuntimeArtifactLeaseLimits {
                maximum_artifact_bytes: 1024,
                maximum_storage_entries: 8,
            },
            &CancellationToken::new(),
        )
        .expect("acquire lease");
        assert!(matches!(
            ArtifactRemovalService::open_existing(
                &root,
                &mut store,
                ArtifactRemovalLimits {
                    maximum_artifact_bytes: 1024,
                    maximum_storage_entries: 8,
                }
            ),
            Err(ArtifactRemovalError::StorageInUse)
        ));
        drop(lease);
        ArtifactRemovalService::open_existing(
            &root,
            &mut store,
            ArtifactRemovalLimits {
                maximum_artifact_bytes: 1024,
                maximum_storage_entries: 8,
            },
        )
        .expect("removal opens after lease drop");
    }

    #[test]
    fn stale_and_prepared_selections_cannot_be_leased() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("managed");
        fs::create_dir(&root).expect("create root");
        fs::create_dir(root.join("artifacts")).expect("create artifacts");
        fs::write(root.join(".artifact-import.lock"), []).expect("create lock");
        let bytes = b"artifact";
        let digest = Digest::sha256(bytes);
        let manifest = ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            source: ArtifactSource {
                origin: "fixture/model".to_owned(),
                revision: "revision".to_owned(),
            },
            artifact_digest: digest.clone(),
            byte_size: u64::try_from(bytes.len()).expect("size"),
            format: "gguf".to_owned(),
            family: "fixture".to_owned(),
            architecture: None,
            quantization: None,
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: Some(8_192),
            },
        };
        let installed = InstalledArtifact {
            artifact_id: manifest.artifact_id.clone(),
            artifact_digest: digest.clone(),
            byte_size: manifest.byte_size,
            storage_key: format!("artifacts/{}", digest.as_str()),
        };
        fs::write(root.join("artifacts").join(digest.as_str()), bytes).expect("write artifact");
        let mut store =
            ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("open state");
        let first = store
            .put_installation(&manifest, &installed)
            .expect("register")
            .installation;
        store
            .prepare_artifact_removal(&first)
            .expect("prepare removal");
        let lease_limits = RuntimeArtifactLeaseLimits {
            maximum_artifact_bytes: 1024,
            maximum_storage_entries: 8,
        };
        assert!(matches!(
            RuntimeArtifactLease::acquire(
                &root,
                &store,
                first.clone(),
                lease_limits,
                &CancellationToken::new()
            ),
            Err(ArtifactInventoryError::ConcurrentModification)
        ));
        store
            .complete_artifact_removal(&first)
            .expect("complete removal state");
        let second = store
            .put_installation(&manifest, &installed)
            .expect("reinstall")
            .installation;
        assert!(matches!(
            RuntimeArtifactLease::acquire(
                &root,
                &store,
                first,
                lease_limits,
                &CancellationToken::new()
            ),
            Err(ArtifactInventoryError::ConcurrentModification)
        ));
        RuntimeArtifactLease::acquire(
            &root,
            &store,
            second,
            lease_limits,
            &CancellationToken::new(),
        )
        .expect("current reinstalled generation leases");
    }

    #[cfg(unix)]
    #[test]
    fn runtime_lease_requires_only_read_access() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("managed");
        fs::create_dir(&root).expect("create root");
        fs::create_dir(root.join("artifacts")).expect("create artifacts");
        fs::write(root.join(".artifact-import.lock"), []).expect("create lock");
        let bytes = b"artifact";
        let digest = Digest::sha256(bytes);
        let manifest = ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            source: ArtifactSource {
                origin: "fixture/model".to_owned(),
                revision: "revision".to_owned(),
            },
            artifact_digest: digest.clone(),
            byte_size: 8,
            format: "gguf".to_owned(),
            family: "fixture".to_owned(),
            architecture: None,
            quantization: None,
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: Some(8_192),
            },
        };
        let installed = InstalledArtifact {
            artifact_id: manifest.artifact_id.clone(),
            artifact_digest: digest.clone(),
            byte_size: 8,
            storage_key: format!("artifacts/{}", digest.as_str()),
        };
        let canonical = root.join("artifacts").join(digest.as_str());
        fs::write(&canonical, bytes).expect("write artifact");
        fs::set_permissions(&canonical, fs::Permissions::from_mode(0o400)).expect("make read-only");
        let mut store =
            ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("open state");
        let selection = store
            .put_installation(&manifest, &installed)
            .expect("register")
            .installation;
        RuntimeArtifactLease::acquire(
            &root,
            &store,
            selection,
            RuntimeArtifactLeaseLimits {
                maximum_artifact_bytes: 1024,
                maximum_storage_entries: 8,
            },
            &CancellationToken::new(),
        )
        .expect("read-only artifact leases");
    }
}
