use rusqlite::{Connection, OptionalExtension as _, params};

use rewrite_model::{ArtifactSetId, ArtifactSetManifest, InstalledArtifactSet};

use crate::{
    ArtifactRemovalPhase, StoreError, StoreResult,
    artifact_set_installation::{
        ArtifactSetInstallationEpoch, StoredArtifactSetInstallation,
        load_artifact_set_installation, load_artifact_set_manifest,
    },
};

const SET_REMOVAL_RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_SET_REMOVAL_RECORD_BYTES: usize = 1_024;

/// Integrity-validated latest removal journal state for one artifact set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifactSetRemoval {
    /// Exact installation generation selected by the operation.
    pub selection: StoredArtifactSetInstallation,
    /// Current durable removal phase.
    pub phase: ArtifactRemovalPhase,
}

impl StoredArtifactSetRemoval {
    pub(crate) fn new(
        selection: StoredArtifactSetInstallation,
        phase: ArtifactRemovalPhase,
    ) -> Self {
        Self { selection, phase }
    }

    pub(crate) fn encode(&self) -> StoreResult<String> {
        let encoded = format!(
            "{{\"installation_epoch\":{},\"installed\":{},\"phase\":\"{}\",\"schema_version\":{}}}",
            self.selection.epoch.get(),
            self.selection.installed.canonical_json(),
            self.phase.key(),
            SET_REMOVAL_RECORD_SCHEMA_VERSION
        );
        if encoded.len() > MAX_SET_REMOVAL_RECORD_BYTES {
            Err(StoreError::RecordTooLarge)
        } else {
            Ok(encoded)
        }
    }
}

pub(crate) fn decode_set_removal_record(
    indexed_id: &str,
    indexed_epoch: i64,
    indexed_phase: &str,
    encoded: &str,
    manifest: Option<&ArtifactSetManifest>,
) -> StoreResult<StoredArtifactSetRemoval> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        installation_epoch: u64,
        installed: serde_json::Value,
        phase: ArtifactRemovalPhase,
        schema_version: u32,
    }

    if encoded.len() > MAX_SET_REMOVAL_RECORD_BYTES {
        return Err(StoreError::RecordTooLarge);
    }
    let wire: Wire = serde_json::from_str(encoded).map_err(|_| StoreError::CorruptRecord)?;
    let epoch = ArtifactSetInstallationEpoch::from_database(indexed_epoch)?;
    if wire.schema_version != SET_REMOVAL_RECORD_SCHEMA_VERSION
        || wire.installation_epoch != epoch.get()
        || wire.phase.key() != indexed_phase
    {
        return Err(StoreError::CorruptRecord);
    }
    let installed_json = serde_json::to_vec(&wire.installed).map_err(StoreError::Serialization)?;
    let installed = match manifest {
        Some(manifest) => InstalledArtifactSet::from_json_bytes(&installed_json, manifest)
            .map_err(|_| StoreError::CorruptRecord)?,
        None => return Err(StoreError::CorruptRecord),
    };
    if installed.artifact_set_id().digest().as_str() != indexed_id {
        return Err(StoreError::CorruptRecord);
    }
    let stored = StoredArtifactSetRemoval {
        selection: StoredArtifactSetInstallation { installed, epoch },
        phase: wire.phase,
    };
    if stored.encode()? != encoded {
        return Err(StoreError::CorruptRecord);
    }
    Ok(stored)
}

pub(crate) fn decode_set_removal_with_manifest(
    indexed_id: &str,
    indexed_epoch: i64,
    indexed_phase: &str,
    encoded: &str,
    manifest: &ArtifactSetManifest,
) -> StoreResult<StoredArtifactSetRemoval> {
    decode_set_removal_record(
        indexed_id,
        indexed_epoch,
        indexed_phase,
        encoded,
        Some(manifest),
    )
}

pub(crate) fn write_set_removal(
    connection: &Connection,
    removal: &StoredArtifactSetRemoval,
) -> StoreResult<()> {
    connection.execute(
        "INSERT INTO artifact_set_removals
             (artifact_set_id, installation_epoch, phase, record_json)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(artifact_set_id) DO UPDATE SET
             installation_epoch = excluded.installation_epoch,
             phase = excluded.phase,
             record_json = excluded.record_json",
        params![
            removal
                .selection
                .installed
                .artifact_set_id()
                .digest()
                .as_str(),
            removal.selection.epoch.database_value()?,
            removal.phase.key(),
            removal.encode()?
        ],
    )?;
    Ok(())
}

pub(crate) fn load_set_removal_state(
    connection: &Connection,
    id: &ArtifactSetId,
) -> StoreResult<(
    Option<StoredArtifactSetInstallation>,
    Option<StoredArtifactSetRemoval>,
)> {
    let manifest = load_artifact_set_manifest(connection, id)?;
    let installation = load_artifact_set_installation(connection, id)?;
    let removal = load_set_removal_row(connection, id, manifest.as_ref())?;
    validate_set_removal_state(
        id.digest().as_str(),
        manifest.as_ref(),
        installation.as_ref(),
        removal.as_ref(),
    )?;
    Ok((installation, removal))
}

fn load_set_removal_row(
    connection: &Connection,
    id: &ArtifactSetId,
    manifest: Option<&ArtifactSetManifest>,
) -> StoreResult<Option<StoredArtifactSetRemoval>> {
    connection
        .query_row(
            "SELECT installation_epoch, phase, record_json
             FROM artifact_set_removals WHERE artifact_set_id = ?1",
            [id.digest().as_str()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .map(|(indexed_epoch, indexed_phase, encoded)| {
            let manifest = manifest.ok_or(StoreError::CorruptRecord)?;
            decode_set_removal_with_manifest(
                id.digest().as_str(),
                indexed_epoch,
                &indexed_phase,
                &encoded,
                manifest,
            )
        })
        .transpose()
}

pub(crate) fn validate_set_removal_state(
    indexed_id: &str,
    manifest: Option<&ArtifactSetManifest>,
    installation: Option<&StoredArtifactSetInstallation>,
    removal: Option<&StoredArtifactSetRemoval>,
) -> StoreResult<()> {
    if let Some(installation) = installation {
        let manifest = manifest.ok_or(StoreError::CorruptRecord)?;
        if installation.installed.artifact_set_id().digest().as_str() != indexed_id
            || manifest.artifact_set_id().digest().as_str() != indexed_id
        {
            return Err(StoreError::CorruptRecord);
        }
        installation
            .installed
            .validate_against(manifest)
            .map_err(|_| StoreError::CorruptRecord)?;
    } else if manifest.is_some_and(|value| value.artifact_set_id().digest().as_str() != indexed_id)
    {
        return Err(StoreError::CorruptRecord);
    }
    if let Some(removal) = removal {
        let manifest = manifest.ok_or(StoreError::CorruptRecord)?;
        removal
            .selection
            .installed
            .validate_against(manifest)
            .map_err(|_| StoreError::CorruptRecord)?;
        if removal
            .selection
            .installed
            .artifact_set_id()
            .digest()
            .as_str()
            != indexed_id
        {
            return Err(StoreError::CorruptRecord);
        }
        if removal.phase == ArtifactRemovalPhase::Prepared && installation.is_some() {
            return Err(StoreError::CorruptRecord);
        }
        if let Some(installation) = installation
            && (removal.phase != ArtifactRemovalPhase::Completed
                || removal.selection.epoch.next()? != installation.epoch)
        {
            return Err(StoreError::CorruptRecord);
        }
    } else if installation.is_some_and(|value| value.epoch != ArtifactSetInstallationEpoch::first())
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(())
}
