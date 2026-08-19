use rusqlite::{Connection, OptionalExtension as _};
use serde::Deserialize;

use rewrite_model::{ArtifactSetId, ArtifactSetManifest, InstalledArtifactSet};

use crate::{StoreError, StoreResult, store::WriteDisposition};

const ARTIFACT_SET_INSTALLATION_RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_ARTIFACT_SET_INSTALLATION_RECORD_BYTES: usize = 1_024;

/// Positive generation for one durable artifact-set installation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArtifactSetInstallationEpoch(u64);

impl ArtifactSetInstallationEpoch {
    pub(crate) const fn first() -> Self {
        Self(1)
    }

    pub(crate) fn from_database(value: i64) -> StoreResult<Self> {
        let value = u64::try_from(value).map_err(|_| StoreError::CorruptRecord)?;
        if value == 0 {
            Err(StoreError::CorruptRecord)
        } else {
            Ok(Self(value))
        }
    }

    pub(crate) fn next(self) -> StoreResult<Self> {
        self.0
            .checked_add(1)
            .filter(|value| i64::try_from(*value).is_ok())
            .map(Self)
            .ok_or(StoreError::InstallationEpochExhausted)
    }

    pub(crate) fn database_value(self) -> StoreResult<i64> {
        i64::try_from(self.0).map_err(|_| StoreError::InstallationEpochExhausted)
    }

    /// Returns the positive generation value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) fn for_test(value: u64) -> StoreResult<Self> {
        let database = i64::try_from(value).map_err(|_| StoreError::InstallationEpochExhausted)?;
        Self::from_database(database)
    }
}

/// Integrity-validated durable selection for one artifact-set installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifactSetInstallation {
    /// Structurally validated set-root installation state.
    pub installed: InstalledArtifactSet,
    /// Distinct generation in the artifact-set lifecycle namespace.
    pub epoch: ArtifactSetInstallationEpoch,
}

/// Outcome of atomically storing an artifact-set manifest and installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetInstallationWriteDisposition {
    /// Manifest write outcome.
    pub manifest: WriteDisposition,
    /// Installed set-root write outcome.
    pub installed: WriteDisposition,
    /// Exact durable installation generation selected or inserted.
    pub installation: StoredArtifactSetInstallation,
}

impl StoredArtifactSetInstallation {
    pub(crate) fn encode(&self) -> StoreResult<String> {
        let encoded = format!(
            "{{\"installation_epoch\":{},\"installed\":{},\"schema_version\":{}}}",
            self.epoch.get(),
            self.installed.canonical_json(),
            ARTIFACT_SET_INSTALLATION_RECORD_SCHEMA_VERSION
        );
        if encoded.len() > MAX_ARTIFACT_SET_INSTALLATION_RECORD_BYTES {
            Err(StoreError::RecordTooLarge)
        } else {
            Ok(encoded)
        }
    }
}

pub(crate) fn load_artifact_set_installation(
    connection: &Connection,
    id: &ArtifactSetId,
) -> StoreResult<Option<StoredArtifactSetInstallation>> {
    connection
        .query_row(
            "SELECT installed.storage_key, installed.installation_epoch,
                    installed.record_json, manifests.record_json
             FROM installed_artifact_sets AS installed
             LEFT JOIN artifact_set_manifests AS manifests
                 ON manifests.artifact_set_id = installed.artifact_set_id
             WHERE installed.artifact_set_id = ?1",
            [id.digest().as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?
        .map(
            |(indexed_storage_key, indexed_epoch, encoded, manifest_json)| {
                let manifest_json = manifest_json.ok_or(StoreError::CorruptRecord)?;
                let manifest = ArtifactSetManifest::from_json_bytes(manifest_json.as_bytes())
                    .map_err(|_| StoreError::CorruptRecord)?;
                if manifest.canonical_json() != manifest_json || manifest.artifact_set_id() != *id {
                    return Err(StoreError::CorruptRecord);
                }
                decode_installation(id, &indexed_storage_key, indexed_epoch, &encoded, &manifest)
            },
        )
        .transpose()
}

pub(crate) fn load_artifact_set_installation_by_storage_key(
    connection: &Connection,
    storage_key: &str,
) -> StoreResult<Option<StoredArtifactSetInstallation>> {
    let mut statement = connection.prepare(
        "SELECT artifact_set_id FROM installed_artifact_sets
         WHERE rtrim(storage_key, ' .') = rtrim(?1, ' .') COLLATE NOCASE
         ORDER BY artifact_set_id LIMIT 2",
    )?;
    let mut rows = statement.query([storage_key])?;
    let indexed_id = rows
        .next()?
        .map(|row| row.get::<_, String>(0))
        .transpose()?;
    if rows.next()?.is_some() {
        return Err(StoreError::CorruptRecord);
    }
    indexed_id
        .map(|value| {
            let id: ArtifactSetId = serde_json::from_value(serde_json::Value::String(value))
                .map_err(|_| StoreError::CorruptRecord)?;
            load_artifact_set_installation(connection, &id)?.ok_or(StoreError::CorruptRecord)
        })
        .transpose()
}

pub(crate) fn load_artifact_set_manifest(
    connection: &Connection,
    id: &ArtifactSetId,
) -> StoreResult<Option<ArtifactSetManifest>> {
    connection
        .query_row(
            "SELECT record_json FROM artifact_set_manifests WHERE artifact_set_id = ?1",
            [id.digest().as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|encoded| {
            let manifest = ArtifactSetManifest::from_json_bytes(encoded.as_bytes())
                .map_err(|_| StoreError::CorruptRecord)?;
            if manifest.canonical_json() != encoded || manifest.artifact_set_id() != *id {
                Err(StoreError::CorruptRecord)
            } else {
                Ok(manifest)
            }
        })
        .transpose()
}

fn decode_installation(
    indexed_id: &ArtifactSetId,
    indexed_storage_key: &str,
    indexed_epoch: i64,
    encoded: &str,
    manifest: &ArtifactSetManifest,
) -> StoreResult<StoredArtifactSetInstallation> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        installation_epoch: u64,
        installed: serde_json::Value,
        schema_version: u32,
    }

    if encoded.len() > MAX_ARTIFACT_SET_INSTALLATION_RECORD_BYTES {
        return Err(StoreError::RecordTooLarge);
    }
    let wire: Wire = serde_json::from_str(encoded).map_err(|_| StoreError::CorruptRecord)?;
    let epoch = ArtifactSetInstallationEpoch::from_database(indexed_epoch)?;
    if wire.schema_version != ARTIFACT_SET_INSTALLATION_RECORD_SCHEMA_VERSION
        || wire.installation_epoch != epoch.get()
    {
        return Err(StoreError::CorruptRecord);
    }
    let installed_json = serde_json::to_vec(&wire.installed).map_err(StoreError::Serialization)?;
    let installed = InstalledArtifactSet::from_json_bytes(&installed_json, manifest)
        .map_err(|_| StoreError::CorruptRecord)?;
    if installed.artifact_set_id() != indexed_id || installed.storage_key() != indexed_storage_key {
        return Err(StoreError::CorruptRecord);
    }
    let stored = StoredArtifactSetInstallation { installed, epoch };
    if stored.encode()? != encoded {
        return Err(StoreError::CorruptRecord);
    }
    Ok(stored)
}

#[cfg(test)]
pub(crate) fn encode_for_test(
    installed: InstalledArtifactSet,
    epoch: ArtifactSetInstallationEpoch,
) -> StoreResult<String> {
    StoredArtifactSetInstallation { installed, epoch }.encode()
}
