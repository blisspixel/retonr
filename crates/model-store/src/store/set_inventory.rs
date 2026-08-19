use rusqlite::Connection;

use rewrite_model::{ArtifactSetId, ArtifactSetManifest};

use super::ArtifactStateStore;
use crate::artifact_set_installation::{
    StoredArtifactSetInstallation, load_artifact_set_installation, load_artifact_set_manifest,
};
use crate::{StoreError, StoreResult};

/// One integrity-validated artifact-set manifest with optional installation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifactSetState {
    /// Immutable artifact-set membership facts.
    pub manifest: ArtifactSetManifest,
    /// Integrity-validated persisted set-root record, when present.
    ///
    /// Current filesystem bytes are not verified by this store operation.
    pub installed: Option<StoredArtifactSetInstallation>,
}

impl ArtifactStateStore {
    /// Returns every artifact-set manifest in deterministic identity order.
    ///
    /// Manifests and optional installed-set records are decoded and validated as
    /// one read transaction before any result is returned. This listing is
    /// structural evidence only. It does not prove that managed bytes exist,
    /// grant a lease, qualify a package, or authorize a role.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] with no partial result when the caller limit is
    /// invalid or exceeded, or persisted state is missing, malformed, or
    /// inconsistent.
    pub fn artifact_set_inventory(
        &self,
        maximum_entries: usize,
    ) -> StoreResult<Vec<StoredArtifactSetState>> {
        if maximum_entries == 0 {
            return Err(StoreError::InvalidLimit);
        }
        let query_limit = maximum_entries
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(StoreError::InvalidLimit)?;
        let transaction = self.connection.unchecked_transaction()?;
        let states = load_artifact_set_states(&transaction, maximum_entries, query_limit)?;
        reject_installed_set_without_manifest(&transaction)?;
        transaction.commit()?;
        Ok(states)
    }
}

fn load_artifact_set_states(
    connection: &Connection,
    maximum_entries: usize,
    query_limit: i64,
) -> StoreResult<Vec<StoredArtifactSetState>> {
    let mut statement = connection.prepare(
        "SELECT artifact_set_id FROM artifact_set_manifests
         ORDER BY artifact_set_id ASC
         LIMIT ?1",
    )?;
    let rows = statement.query_map([query_limit], |row| row.get::<_, String>(0))?;
    let mut states = Vec::new();
    for row in rows {
        if states.len() == maximum_entries {
            return Err(StoreError::InventoryLimitExceeded);
        }
        let stored_id = row?;
        let id: ArtifactSetId = serde_json::from_value(serde_json::Value::String(stored_id))
            .map_err(|_| StoreError::CorruptRecord)?;
        let manifest =
            load_artifact_set_manifest(connection, &id)?.ok_or(StoreError::CorruptRecord)?;
        let installed = load_artifact_set_installation(connection, &id)?;
        states.push(StoredArtifactSetState {
            manifest,
            installed,
        });
    }
    Ok(states)
}

fn reject_installed_set_without_manifest(connection: &Connection) -> StoreResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM installed_artifact_sets AS installed
             LEFT JOIN artifact_set_manifests AS manifests
               ON manifests.artifact_set_id = installed.artifact_set_id
             WHERE manifests.artifact_set_id IS NULL
         )",
        [],
        |row| row.get(0),
    )?;
    if exists {
        Err(StoreError::MissingRecord)
    } else {
        Ok(())
    }
}
