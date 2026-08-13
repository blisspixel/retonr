use rusqlite::{TransactionBehavior, params};

use rewrite_model::{ArtifactId, ArtifactManifest, InstalledArtifact};

use super::{ArtifactStateStore, validate_binding};
use crate::{
    ArtifactRemovalPhase, StoreError, StoreResult, StoredArtifactInstallation,
    StoredArtifactRemoval,
    binding::load_active_bindings,
    record::{load_record, validate_existing_installation},
    removal::{load_installation, load_removal, write_removal},
};

/// Outcome of durably preparing an exact installation generation for removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemovalPreparationDisposition {
    /// Installed-state authority was removed and a pending journal was committed.
    Prepared,
    /// The exact generation was already durably prepared.
    AlreadyPrepared,
    /// The exact generation was already durably completed.
    AlreadyCompleted,
}

/// Outcome of durably completing an exact installation removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemovalCompletionDisposition {
    /// The pending journal was marked completed.
    Completed,
    /// The exact generation was already marked completed.
    AlreadyCompleted,
}

impl ArtifactStateStore {
    /// Loads the integrity-validated current installation selection and latest
    /// removal journal for one artifact.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when persisted state is malformed or inconsistent.
    pub fn artifact_removal_state(
        &self,
        artifact_id: &ArtifactId,
    ) -> StoreResult<(
        Option<StoredArtifactInstallation>,
        Option<StoredArtifactRemoval>,
    )> {
        let transaction = self.connection.unchecked_transaction()?;
        let manifest = load_record::<ArtifactManifest>(
            &transaction,
            "artifact_manifests",
            "artifact_id",
            artifact_id.digest().as_str(),
        )?;
        let installation = load_installation(&transaction, artifact_id)?;
        let removal = load_removal(&transaction, artifact_id)?;
        validate_removal_state(
            artifact_id.digest().as_str(),
            manifest.as_ref(),
            installation.as_ref(),
            removal.as_ref(),
        )?;
        transaction.commit()?;
        Ok((installation, removal))
    }

    /// Atomically records a pending removal and revokes installed-state authority.
    ///
    /// The caller must verify current managed bytes before this transaction. Exact
    /// retries are idempotent. No managed bytes are changed by this operation.
    ///
    /// Low-level durable transition for an application-owned removal protocol.
    ///
    /// This method does not inspect or delete filesystem bytes. Callers must first
    /// hold the product lifecycle lock exclusively and verify the exact managed
    /// artifact through the application boundary. This unpublished workspace crate
    /// does not expose a standalone managed-byte removal API.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the selection is stale, active, corrupt, or the
    /// transaction cannot commit in full.
    pub fn prepare_artifact_removal(
        &mut self,
        selection: &StoredArtifactInstallation,
    ) -> StoreResult<RemovalPreparationDisposition> {
        selection
            .installed
            .validate()
            .map_err(StoreError::InvalidInstallation)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let indexed_id = selection.installed.artifact_id.digest().as_str();
        let manifest = load_record::<ArtifactManifest>(
            &transaction,
            "artifact_manifests",
            "artifact_id",
            indexed_id,
        )?;
        let current = load_installation(&transaction, &selection.installed.artifact_id)?;
        let removal = load_removal(&transaction, &selection.installed.artifact_id)?;
        validate_removal_state(
            indexed_id,
            manifest.as_ref(),
            current.as_ref(),
            removal.as_ref(),
        )?;
        if let Some(removal) = removal.as_ref()
            && removal.selection == *selection
        {
            if removal.phase == ArtifactRemovalPhase::Completed {
                return Ok(RemovalPreparationDisposition::AlreadyCompleted);
            }
            validate_no_active_binding(&transaction, selection)?;
            return Ok(RemovalPreparationDisposition::AlreadyPrepared);
        }
        if current.as_ref() != Some(selection) {
            return Err(StoreError::StaleInstallation);
        }
        validate_no_active_binding(&transaction, selection)?;
        let pending = StoredArtifactRemoval::new(selection.clone(), ArtifactRemovalPhase::Prepared);
        write_removal(&transaction, &pending)?;
        let changed = transaction.execute(
            "DELETE FROM installed_artifacts
             WHERE artifact_id = ?1 AND installation_epoch = ?2",
            params![
                selection.installed.artifact_id.digest().as_str(),
                selection.epoch.database_value()?
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::StaleInstallation);
        }
        transaction.commit()?;
        Ok(RemovalPreparationDisposition::Prepared)
    }

    /// Marks an exact prepared removal completed after canonical byte absence and
    /// directory durability have been proven by the application boundary.
    ///
    /// Low-level durable completion for an application-owned removal protocol.
    ///
    /// This method does not prove byte absence. Callers must confirm exact canonical
    /// absence, synchronize the artifact directory, and revalidate the held storage
    /// boundary before completing the journal.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the exact pending journal is absent, conflicts,
    /// is corrupt, or cannot be updated atomically.
    pub fn complete_artifact_removal(
        &mut self,
        selection: &StoredArtifactInstallation,
    ) -> StoreResult<RemovalCompletionDisposition> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let indexed_id = selection.installed.artifact_id.digest().as_str();
        let manifest = load_record::<ArtifactManifest>(
            &transaction,
            "artifact_manifests",
            "artifact_id",
            indexed_id,
        )?;
        let current = load_installation(&transaction, &selection.installed.artifact_id)?;
        let removal = load_removal(&transaction, &selection.installed.artifact_id)?
            .ok_or(StoreError::MissingRecord)?;
        validate_removal_state(
            indexed_id,
            manifest.as_ref(),
            current.as_ref(),
            Some(&removal),
        )?;
        if removal.selection != *selection {
            return Err(StoreError::StaleInstallation);
        }
        if removal.phase == ArtifactRemovalPhase::Completed {
            return Ok(RemovalCompletionDisposition::AlreadyCompleted);
        }
        if current.is_some() {
            return Err(StoreError::CorruptRecord);
        }
        validate_no_active_binding(&transaction, selection)?;
        write_removal(
            &transaction,
            &StoredArtifactRemoval::new(selection.clone(), ArtifactRemovalPhase::Completed),
        )?;
        transaction.commit()?;
        Ok(RemovalCompletionDisposition::Completed)
    }
}

pub(super) fn validate_removal_state(
    indexed_id: &str,
    manifest: Option<&ArtifactManifest>,
    installation: Option<&StoredArtifactInstallation>,
    removal: Option<&StoredArtifactRemoval>,
) -> StoreResult<()> {
    validate_existing_installation(
        indexed_id,
        manifest,
        installation.map(|value| &value.installed),
    )?;
    if let Some(removal) = removal {
        validate_existing_installation(indexed_id, manifest, Some(&removal.selection.installed))?;
        if removal.phase == ArtifactRemovalPhase::Prepared && installation.is_some() {
            return Err(StoreError::CorruptRecord);
        }
        if let Some(installation) = installation
            && (removal.phase != ArtifactRemovalPhase::Completed
                || removal.selection.epoch.next()? != installation.epoch)
        {
            return Err(StoreError::CorruptRecord);
        }
    } else if installation
        .is_some_and(|value| value.epoch != crate::ArtifactInstallationEpoch::first())
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}

pub(super) fn validate_installation_input(
    manifest: &ArtifactManifest,
    installed: &InstalledArtifact,
) -> StoreResult<()> {
    manifest.validate().map_err(StoreError::InvalidManifest)?;
    installed
        .validate()
        .map_err(StoreError::InvalidInstallation)?;
    if manifest.artifact_id != installed.artifact_id
        || manifest.artifact_digest != installed.artifact_digest
        || manifest.byte_size != installed.byte_size
    {
        Err(StoreError::ImmutableConflict)
    } else {
        Ok(())
    }
}

pub(super) fn require_record(
    transaction: &rusqlite::Transaction<'_>,
    table: &str,
    key_column: &str,
    key: &str,
) -> StoreResult<()> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {key_column} = ?1)");
    let exists: bool = transaction.query_row(&sql, [key], |row| row.get(0))?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::MissingRecord)
    }
}

fn validate_no_active_binding(
    connection: &rusqlite::Connection,
    selection: &StoredArtifactInstallation,
) -> StoreResult<()> {
    validate_no_active_binding_for_id(connection, &selection.installed.artifact_id)
}

fn validate_no_active_binding_for_id(
    connection: &rusqlite::Connection,
    artifact_id: &ArtifactId,
) -> StoreResult<()> {
    for binding in load_active_bindings(connection)? {
        validate_binding(connection, &binding, &mut |_| true)?;
        if binding.artifact_id == *artifact_id {
            return Err(StoreError::ActiveArtifact);
        }
    }
    Ok(())
}
