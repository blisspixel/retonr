use rusqlite::{TransactionBehavior, params};

use rewrite_model::{ArtifactSetId, ArtifactSetManifest, InstalledArtifactSet};

use super::{ArtifactStateStore, WriteDisposition};
use crate::artifact_set_installation::{
    ArtifactSetInstallationEpoch, ArtifactSetInstallationWriteDisposition,
    StoredArtifactSetInstallation, load_artifact_set_installation,
    load_artifact_set_installation_by_storage_key, load_artifact_set_manifest,
};
use crate::artifact_set_removal::{load_set_removal_state, validate_set_removal_state};
use crate::{ArtifactRemovalPhase, StoreError, StoreResult, record::insert_immutable};

impl ArtifactStateStore {
    /// Atomically stores an exact artifact-set manifest and its inert set-root state.
    ///
    /// This operation records installed state only. It does not verify filesystem
    /// bytes or grant activation, qualification, runtime, lease, or semantic authority.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when input is invalid, existing state is corrupt or
    /// conflicting, or the transaction cannot commit in full.
    pub fn put_artifact_set_installation(
        &mut self,
        manifest: &ArtifactSetManifest,
        installed: &InstalledArtifactSet,
    ) -> StoreResult<ArtifactSetInstallationWriteDisposition> {
        manifest
            .validate()
            .map_err(StoreError::InvalidArtifactSet)?;
        installed
            .validate_against(manifest)
            .map_err(StoreError::InvalidArtifactSetInstallation)?;
        let manifest_id = manifest.artifact_set_id();
        let manifest_key = manifest_id.digest().as_str();
        let manifest_record = manifest.canonical_json();

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing_manifest = load_artifact_set_manifest(&transaction, &manifest_id)?;
        let existing_installation = load_artifact_set_installation(&transaction, &manifest_id)?;
        let (_, removal) = load_set_removal_state(&transaction, &manifest_id)?;
        validate_set_removal_state(
            manifest_key,
            existing_manifest.as_ref(),
            existing_installation.as_ref(),
            removal.as_ref(),
        )?;
        if removal
            .as_ref()
            .is_some_and(|record| record.phase == ArtifactRemovalPhase::Prepared)
        {
            if existing_installation.is_some() {
                return Err(StoreError::CorruptRecord);
            }
            return Err(StoreError::RemovalPending);
        }
        if let (Some(existing), Some(removal)) = (&existing_installation, &removal)
            && existing.epoch <= removal.selection.epoch
        {
            return Err(StoreError::CorruptRecord);
        }
        let storage_owner =
            load_artifact_set_installation_by_storage_key(&transaction, installed.storage_key())?;
        if existing_manifest
            .as_ref()
            .is_some_and(|existing| existing != manifest)
            || existing_installation
                .as_ref()
                .is_some_and(|existing| &existing.installed != installed)
        {
            return Err(StoreError::ImmutableConflict);
        }
        if storage_owner
            .as_ref()
            .is_some_and(|owner| owner.installed.artifact_set_id() != &manifest_id)
        {
            return Err(StoreError::ImmutableConflict);
        }

        let manifest_disposition = if existing_manifest.is_some() {
            WriteDisposition::AlreadyPresent
        } else {
            insert_immutable(
                &transaction,
                "artifact_set_manifests",
                "artifact_set_id",
                manifest_key,
                &manifest_record,
            )?
        };
        let (installed_disposition, installation) = if let Some(existing) = existing_installation {
            (WriteDisposition::AlreadyPresent, existing)
        } else {
            let epoch = removal
                .map(|record| record.selection.epoch.next())
                .transpose()?
                .unwrap_or_else(ArtifactSetInstallationEpoch::first);
            let installation = StoredArtifactSetInstallation {
                installed: installed.clone(),
                epoch,
            };
            transaction.execute(
                "INSERT INTO installed_artifact_sets
                         (artifact_set_id, storage_key, installation_epoch, record_json)
                     VALUES (?1, ?2, ?3, ?4)",
                params![
                    manifest_key,
                    installed.storage_key(),
                    installation.epoch.database_value()?,
                    installation.encode()?
                ],
            )?;
            (WriteDisposition::Inserted, installation)
        };
        let reloaded = load_artifact_set_installation(&transaction, &manifest_id)?
            .ok_or(StoreError::CorruptRecord)?;
        if reloaded != installation {
            return Err(StoreError::CorruptRecord);
        }
        transaction.commit()?;
        Ok(ArtifactSetInstallationWriteDisposition {
            manifest: manifest_disposition,
            installed: installed_disposition,
            installation: reloaded,
        })
    }

    /// Loads exact artifact-set installation state after recursive validation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when durable manifest, index, epoch, or record state
    /// is malformed, noncanonical, missing, or inconsistent.
    pub fn artifact_set_installation(
        &self,
        id: &ArtifactSetId,
    ) -> StoreResult<Option<StoredArtifactSetInstallation>> {
        load_artifact_set_installation(&self.connection, id)
    }
}
