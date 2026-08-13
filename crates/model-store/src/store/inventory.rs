use rusqlite::Connection;

use rewrite_model::{ActiveArtifactBinding, ArtifactManifest, InstalledArtifact};

use super::{ArtifactStateStore, validate_binding};
use crate::{StoreError, StoreResult, binding::load_active_bindings, record::decode_record};

/// One integrity-validated manifest with its optional installation and active use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifactState {
    /// Immutable artifact facts and source metadata.
    pub manifest: ArtifactManifest,
    /// Integrity-validated persisted installation record, when present.
    ///
    /// Current filesystem bytes are not verified by this store operation.
    pub installed: Option<InstalledArtifact>,
    /// Durable bindings validated against the same persisted-state snapshot.
    ///
    /// Callers must verify current artifact bytes before treating a binding as
    /// runtime authority.
    pub active_bindings: Vec<ActiveArtifactBinding>,
}

impl ArtifactStateStore {
    /// Returns every artifact manifest in deterministic content-identity order.
    ///
    /// Manifests, optional installed records, and active bindings are decoded and
    /// validated as one read transaction before any result is returned.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] with no partial result when the caller limit is
    /// invalid or exceeded, or persisted state is missing, malformed, or
    /// inconsistent.
    pub fn artifact_inventory(
        &self,
        maximum_entries: usize,
    ) -> StoreResult<Vec<StoredArtifactState>> {
        if maximum_entries == 0 {
            return Err(StoreError::InvalidLimit);
        }
        let query_limit = maximum_entries
            .checked_add(1)
            .and_then(|value| i64::try_from(value).ok())
            .ok_or(StoreError::InvalidLimit)?;
        let transaction = self.connection.unchecked_transaction()?;
        let mut states = load_artifact_states(&transaction, maximum_entries, query_limit)?;
        let mut validated_bindings = Vec::new();
        for binding in load_active_bindings(&transaction)? {
            let verified = validate_binding(&transaction, &binding, &mut |candidate| {
                states
                    .iter()
                    .any(|item| item.installed.as_ref() == Some(candidate))
            })?;
            validated_bindings.push(verified);
        }
        for binding in validated_bindings {
            let state = states
                .iter_mut()
                .find(|item| item.manifest.artifact_id == binding.artifact_id)
                .ok_or(StoreError::CorruptRecord)?;
            if state.installed.is_none() {
                return Err(StoreError::CorruptRecord);
            }
            state.active_bindings.push(binding);
        }
        for state in &mut states {
            state.active_bindings.sort_by_key(|binding| binding.role);
        }
        transaction.commit()?;
        Ok(states)
    }
}

fn load_artifact_states(
    connection: &Connection,
    maximum_entries: usize,
    query_limit: i64,
) -> StoreResult<Vec<StoredArtifactState>> {
    let mut statement = connection.prepare(
        "SELECT manifests.artifact_id, manifests.record_json, installed.record_json
         FROM artifact_manifests AS manifests
         LEFT JOIN installed_artifacts AS installed
           ON installed.artifact_id = manifests.artifact_id
         ORDER BY manifests.artifact_id ASC
         LIMIT ?1",
    )?;
    let rows = statement.query_map([query_limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut states = Vec::new();
    for row in rows {
        if states.len() == maximum_entries {
            return Err(StoreError::InventoryLimitExceeded);
        }
        let (stored_id, manifest_record, installed_record) = row?;
        let manifest = decode_record::<ArtifactManifest>(&manifest_record)?;
        manifest.validate().map_err(StoreError::InvalidManifest)?;
        if manifest.artifact_id.digest().as_str() != stored_id {
            return Err(StoreError::CorruptRecord);
        }
        let installed = installed_record
            .map(|record| {
                let installed = decode_record::<InstalledArtifact>(&record)?;
                installed
                    .validate()
                    .map_err(StoreError::InvalidInstallation)?;
                if manifest.artifact_id != installed.artifact_id
                    || manifest.artifact_digest != installed.artifact_digest
                    || manifest.byte_size != installed.byte_size
                {
                    return Err(StoreError::CorruptRecord);
                }
                Ok(installed)
            })
            .transpose()?;
        states.push(StoredArtifactState {
            manifest,
            installed,
            active_bindings: Vec::new(),
        });
    }
    reject_installed_without_manifest(connection)?;
    Ok(states)
}

fn reject_installed_without_manifest(connection: &Connection) -> StoreResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM installed_artifacts AS installed
             LEFT JOIN artifact_manifests AS manifests
               ON manifests.artifact_id = installed.artifact_id
             WHERE manifests.artifact_id IS NULL
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
