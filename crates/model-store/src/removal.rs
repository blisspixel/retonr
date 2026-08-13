use rusqlite::{Connection, OptionalExtension as _, params};
use serde::{Deserialize, Serialize};

use rewrite_model::{ArtifactId, InstalledArtifact};

use crate::{StoreError, StoreResult, record::decode_record};

const REMOVAL_RECORD_SCHEMA_VERSION: u32 = 1;

/// Positive, monotonically increasing identity for one artifact installation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArtifactInstallationEpoch(u64);

impl ArtifactInstallationEpoch {
    pub(super) fn first() -> Self {
        Self(1)
    }

    pub(super) fn from_database(value: i64) -> StoreResult<Self> {
        let value = u64::try_from(value).map_err(|_| StoreError::CorruptRecord)?;
        if value == 0 {
            Err(StoreError::CorruptRecord)
        } else {
            Ok(Self(value))
        }
    }

    pub(super) fn next(self) -> StoreResult<Self> {
        self.0
            .checked_add(1)
            .filter(|value| i64::try_from(*value).is_ok())
            .map(Self)
            .ok_or(StoreError::InstallationEpochExhausted)
    }

    pub(super) fn database_value(self) -> StoreResult<i64> {
        i64::try_from(self.0).map_err(|_| StoreError::InstallationEpochExhausted)
    }

    #[cfg(test)]
    pub(crate) fn for_test(value: u64) -> StoreResult<Self> {
        let value = i64::try_from(value).map_err(|_| StoreError::InstallationEpochExhausted)?;
        Self::from_database(value)
    }

    /// Returns the positive epoch value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Exact durable selection for one installed artifact generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifactInstallation {
    /// Integrity-validated installation record.
    pub installed: InstalledArtifact,
    /// Generation that prevents an old removal retry from deleting a reinstall.
    pub epoch: ArtifactInstallationEpoch,
}

/// Durable phase of one selected managed-artifact removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRemovalPhase {
    /// Installed-state authority was removed before managed-byte deletion.
    Prepared,
    /// Canonical byte absence and directory durability were confirmed.
    Completed,
}

impl ArtifactRemovalPhase {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRemovalRecord {
    schema_version: u32,
    installed: InstalledArtifact,
    installation_epoch: u64,
    phase: ArtifactRemovalPhase,
}

/// Integrity-validated latest removal journal state for one artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredArtifactRemoval {
    /// Exact installation generation selected by the operation.
    pub selection: StoredArtifactInstallation,
    /// Current durable removal phase.
    pub phase: ArtifactRemovalPhase,
}

impl StoredArtifactRemoval {
    pub(super) fn new(selection: StoredArtifactInstallation, phase: ArtifactRemovalPhase) -> Self {
        Self { selection, phase }
    }

    pub(super) fn encode(&self) -> StoreResult<String> {
        crate::record::encode_record(&ArtifactRemovalRecord {
            schema_version: REMOVAL_RECORD_SCHEMA_VERSION,
            installed: self.selection.installed.clone(),
            installation_epoch: self.selection.epoch.get(),
            phase: self.phase,
        })
    }
}

pub(super) fn load_removal(
    connection: &Connection,
    artifact_id: &ArtifactId,
) -> StoreResult<Option<StoredArtifactRemoval>> {
    connection
        .query_row(
            "SELECT installation_epoch, phase, record_json
             FROM artifact_removals WHERE artifact_id = ?1",
            [artifact_id.digest().as_str()],
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
            decode_removal(
                artifact_id.digest().as_str(),
                indexed_epoch,
                &indexed_phase,
                &encoded,
            )
        })
        .transpose()
}

pub(super) fn load_installation(
    connection: &Connection,
    artifact_id: &ArtifactId,
) -> StoreResult<Option<StoredArtifactInstallation>> {
    connection
        .query_row(
            "SELECT installation_epoch, record_json
             FROM installed_artifacts WHERE artifact_id = ?1",
            [artifact_id.digest().as_str()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(indexed_epoch, encoded)| {
            let installed = decode_record::<InstalledArtifact>(&encoded)?;
            installed
                .validate()
                .map_err(|_| StoreError::CorruptRecord)?;
            if installed.artifact_id != *artifact_id {
                return Err(StoreError::CorruptRecord);
            }
            Ok(StoredArtifactInstallation {
                installed,
                epoch: ArtifactInstallationEpoch::from_database(indexed_epoch)?,
            })
        })
        .transpose()
}

pub(super) fn decode_removal(
    indexed_id: &str,
    indexed_epoch: i64,
    indexed_phase: &str,
    encoded: &str,
) -> StoreResult<StoredArtifactRemoval> {
    let record = decode_record::<ArtifactRemovalRecord>(encoded)?;
    let epoch = ArtifactInstallationEpoch::from_database(indexed_epoch)?;
    record
        .installed
        .validate()
        .map_err(|_| StoreError::CorruptRecord)?;
    if record.schema_version != REMOVAL_RECORD_SCHEMA_VERSION
        || record.installed.artifact_id.digest().as_str() != indexed_id
        || record.installation_epoch != epoch.get()
        || record.phase.key() != indexed_phase
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(StoredArtifactRemoval {
        selection: StoredArtifactInstallation {
            installed: record.installed,
            epoch,
        },
        phase: record.phase,
    })
}

pub(super) fn write_removal(
    connection: &Connection,
    removal: &StoredArtifactRemoval,
) -> StoreResult<()> {
    connection.execute(
        "INSERT INTO artifact_removals
             (artifact_id, installation_epoch, phase, record_json)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(artifact_id) DO UPDATE SET
             installation_epoch = excluded.installation_epoch,
             phase = excluded.phase,
             record_json = excluded.record_json",
        params![
            removal.selection.installed.artifact_id.digest().as_str(),
            removal.selection.epoch.database_value()?,
            removal.phase.key(),
            removal.encode()?
        ],
    )?;
    Ok(())
}
