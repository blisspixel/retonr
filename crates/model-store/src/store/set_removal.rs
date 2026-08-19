use rusqlite::{TransactionBehavior, params};

use rewrite_model::ArtifactSetId;

use super::ArtifactStateStore;
use crate::{
    ArtifactRemovalPhase, ExclusiveArtifactLifecycleLock, RemovalCompletionDisposition,
    RemovalPreparationDisposition, StoreError, StoreResult, StoredArtifactSetInstallation,
    StoredArtifactSetRemoval,
    artifact_set_installation::load_artifact_set_manifest,
    artifact_set_removal::{
        decode_set_removal_with_manifest, load_set_removal_state, validate_set_removal_state,
        write_set_removal,
    },
};

impl ArtifactStateStore {
    /// Returns every durably prepared set removal in deterministic identity order.
    ///
    /// This bounded read validates every retained journal against its manifest and
    /// installation generation, then returns only prepared selections. It reads no
    /// managed set bytes and returns no completed removal history.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the caller limit is invalid or exceeded, or when
    /// persisted set-removal state is missing, malformed, or inconsistent.
    pub fn pending_artifact_set_removals(
        &self,
        maximum_entries: usize,
    ) -> StoreResult<Vec<StoredArtifactSetInstallation>> {
        if maximum_entries == 0 {
            return Err(StoreError::InvalidLimit);
        }
        let query_limit = maximum_entries
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(StoreError::InvalidLimit)?;
        let transaction = self.connection.unchecked_transaction()?;
        let mut statement = transaction.prepare(
            "SELECT artifact_set_id, installation_epoch, phase, record_json
             FROM artifact_set_removals
             ORDER BY artifact_set_id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([query_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut pending = Vec::new();
        for (inspected, row) in rows.enumerate() {
            if inspected == maximum_entries {
                return Err(StoreError::InventoryLimitExceeded);
            }
            let (indexed_id, indexed_epoch, indexed_phase, encoded) = row?;
            let id: ArtifactSetId =
                serde_json::from_value(serde_json::Value::String(indexed_id.clone()))
                    .map_err(|_| StoreError::CorruptRecord)?;
            let manifest =
                load_artifact_set_manifest(&transaction, &id)?.ok_or(StoreError::CorruptRecord)?;
            let removal = decode_set_removal_with_manifest(
                &indexed_id,
                indexed_epoch,
                &indexed_phase,
                &encoded,
                &manifest,
            )?;
            let (installation, current) = load_set_removal_state(&transaction, &id)?;
            if current.as_ref() != Some(&removal) {
                return Err(StoreError::CorruptRecord);
            }
            validate_set_removal_state(
                &indexed_id,
                Some(&manifest),
                installation.as_ref(),
                Some(&removal),
            )?;
            if removal.phase == ArtifactRemovalPhase::Prepared {
                if installation.is_some() {
                    return Err(StoreError::CorruptRecord);
                }
                pending.push(removal.selection);
            }
        }
        drop(statement);
        transaction.commit()?;
        Ok(pending)
    }

    /// Loads the integrity-validated current set installation and latest journal.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when persisted state is malformed or inconsistent.
    pub fn artifact_set_removal_state(
        &self,
        artifact_set_id: &ArtifactSetId,
    ) -> StoreResult<(
        Option<StoredArtifactSetInstallation>,
        Option<StoredArtifactSetRemoval>,
    )> {
        let transaction = self.connection.unchecked_transaction()?;
        let state = load_set_removal_state(&transaction, artifact_set_id)?;
        transaction.commit()?;
        Ok(state)
    }

    /// Atomically records a pending set removal and revokes installed-set authority.
    ///
    /// The caller must verify the current managed tree before this transaction.
    /// Exact retries are idempotent. No managed bytes are changed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the selection is stale, corrupt, or the
    /// transaction cannot commit in full.
    pub fn prepare_artifact_set_removal(
        &mut self,
        _lifecycle_lock: &ExclusiveArtifactLifecycleLock,
        selection: &StoredArtifactSetInstallation,
    ) -> StoreResult<RemovalPreparationDisposition> {
        let artifact_set_id = selection.installed.artifact_set_id();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let manifest = load_artifact_set_manifest(&transaction, artifact_set_id)?;
        selection
            .installed
            .validate_against(manifest.as_ref().ok_or(StoreError::MissingRecord)?)
            .map_err(StoreError::InvalidArtifactSetInstallation)?;
        let (current, removal) = load_set_removal_state(&transaction, artifact_set_id)?;
        if current
            .as_ref()
            .is_some_and(|installed| installed != selection)
        {
            return Err(StoreError::StaleInstallation);
        }
        if let Some(removal) = removal.as_ref()
            && removal.selection == *selection
        {
            if removal.phase == ArtifactRemovalPhase::Completed {
                return Ok(RemovalPreparationDisposition::AlreadyCompleted);
            }
            return Ok(RemovalPreparationDisposition::AlreadyPrepared);
        }
        if current.as_ref() != Some(selection) {
            return Err(StoreError::StaleInstallation);
        }
        let pending =
            StoredArtifactSetRemoval::new(selection.clone(), ArtifactRemovalPhase::Prepared);
        write_set_removal(&transaction, &pending)?;
        let changed = transaction.execute(
            "DELETE FROM installed_artifact_sets
             WHERE artifact_set_id = ?1 AND installation_epoch = ?2",
            params![
                artifact_set_id.digest().as_str(),
                selection.epoch.database_value()?
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::StaleInstallation);
        }
        transaction.commit()?;
        Ok(RemovalPreparationDisposition::Prepared)
    }

    /// Marks an exact prepared set removal completed after tree absence is proven.
    ///
    /// This method does not prove tree absence. Callers must confirm the canonical
    /// set root is gone, synchronize the sets directory, and revalidate the held
    /// storage boundary before completing the journal.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the exact pending journal is absent, conflicts,
    /// is corrupt, or cannot be updated atomically.
    pub fn complete_artifact_set_removal(
        &mut self,
        _lifecycle_lock: &ExclusiveArtifactLifecycleLock,
        selection: &StoredArtifactSetInstallation,
    ) -> StoreResult<RemovalCompletionDisposition> {
        let artifact_set_id = selection.installed.artifact_set_id();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (current, removal) = load_set_removal_state(&transaction, artifact_set_id)?;
        if current
            .as_ref()
            .is_some_and(|installed| installed != selection)
        {
            return Err(StoreError::StaleInstallation);
        }
        let removal = removal.ok_or(StoreError::MissingRecord)?;
        if removal.selection != *selection {
            return Err(StoreError::StaleInstallation);
        }
        if removal.phase == ArtifactRemovalPhase::Completed {
            return Ok(RemovalCompletionDisposition::AlreadyCompleted);
        }
        if current.is_some() {
            return Err(StoreError::CorruptRecord);
        }
        write_set_removal(
            &transaction,
            &StoredArtifactSetRemoval::new(selection.clone(), ArtifactRemovalPhase::Completed),
        )?;
        transaction.commit()?;
        Ok(RemovalCompletionDisposition::Completed)
    }
}
