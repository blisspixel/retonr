use rusqlite::{Connection, TransactionBehavior, params};

use rewrite_model::{
    ModelPackageManifest, ModelPackageManifestId, NativeLoadObservation, NativeLoadObservationId,
    PackageTransformation, RuntimePackageManifest, RuntimePackageManifestId,
};

use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult, record::immutable_disposition};

mod read;

use read::{load_model_package, load_native_load, load_runtime_package};

impl ArtifactStateStore {
    /// Stores one immutable runtime-package manifest after resolving its exact byte set.
    ///
    /// This record is inert structural evidence. It does not install or execute a runtime.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the byte set is absent, validation fails, durable
    /// state conflicts, or the transaction cannot commit.
    pub fn put_runtime_package_manifest(
        &mut self,
        manifest: &RuntimePackageManifest,
    ) -> StoreResult<WriteDisposition> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact_set = super::evidence::read::load_artifact_set(
            &transaction,
            manifest.artifact_set_id().digest().as_str(),
        )?
        .ok_or(StoreError::MissingRecord)?;
        manifest
            .validate_against(&artifact_set)
            .map_err(StoreError::InvalidRuntimePackage)?;
        require_source_artifact_set(&transaction, manifest.transformation())?;
        let encoded = canonical_runtime_package(manifest, &artifact_set)?;
        let disposition = insert_runtime_package(&transaction, manifest, &encoded)?;
        let stored = load_runtime_package(
            &transaction,
            manifest.runtime_package_manifest_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        if stored != *manifest {
            return Err(StoreError::ImmutableConflict);
        }
        transaction.commit()?;
        Ok(disposition)
    }

    /// Returns one runtime-package manifest after recursively validating its byte set.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an existing record or dependency is missing,
    /// malformed, noncanonical, or inconsistent.
    pub fn runtime_package_manifest(
        &self,
        id: &RuntimePackageManifestId,
    ) -> StoreResult<Option<RuntimePackageManifest>> {
        let transaction = self.connection.unchecked_transaction()?;
        let record = load_runtime_package(&transaction, id.digest().as_str())?;
        transaction.commit()?;
        Ok(record)
    }

    /// Stores one immutable model-package manifest after resolving its exact byte set.
    ///
    /// This record is inert structural evidence and grants no model-use authority.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the byte set is absent, validation fails, durable
    /// state conflicts, or the transaction cannot commit.
    pub fn put_model_package_manifest(
        &mut self,
        manifest: &ModelPackageManifest,
    ) -> StoreResult<WriteDisposition> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact_set = super::evidence::read::load_artifact_set(
            &transaction,
            manifest.artifact_set_id().digest().as_str(),
        )?
        .ok_or(StoreError::MissingRecord)?;
        manifest
            .validate_against(&artifact_set)
            .map_err(StoreError::InvalidModelPackage)?;
        require_source_artifact_set(&transaction, manifest.transformation())?;
        let encoded = canonical_model_package(manifest, &artifact_set)?;
        let disposition = insert_model_package(&transaction, manifest, &encoded)?;
        let stored = load_model_package(
            &transaction,
            manifest.model_package_manifest_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        if stored != *manifest {
            return Err(StoreError::ImmutableConflict);
        }
        transaction.commit()?;
        Ok(disposition)
    }

    /// Returns one model-package manifest after recursively validating its byte set.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an existing record or dependency is missing,
    /// malformed, noncanonical, or inconsistent.
    pub fn model_package_manifest(
        &self,
        id: &ModelPackageManifestId,
    ) -> StoreResult<Option<ModelPackageManifest>> {
        let transaction = self.connection.unchecked_transaction()?;
        let record = load_model_package(&transaction, id.digest().as_str())?;
        transaction.commit()?;
        Ok(record)
    }

    /// Stores one immutable native-load observation bound to an exact runtime package.
    ///
    /// The observation remains evidence only and does not qualify or activate a runtime.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the runtime package is absent, validation fails,
    /// durable state conflicts, or the transaction cannot commit.
    pub fn put_native_load_observation(
        &mut self,
        observation: &NativeLoadObservation,
    ) -> StoreResult<WriteDisposition> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let package = load_runtime_package(
            &transaction,
            observation.runtime_package_manifest_id().digest().as_str(),
        )?
        .ok_or(StoreError::MissingRecord)?;
        let encoded = canonical_native_load(observation, &package)?;
        let disposition = insert_native_load(&transaction, observation, &encoded)?;
        let stored = load_native_load(
            &transaction,
            observation.native_load_observation_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        if stored != *observation {
            return Err(StoreError::ImmutableConflict);
        }
        transaction.commit()?;
        Ok(disposition)
    }

    /// Returns one native-load observation after recursively validating its package.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an existing record or dependency is missing,
    /// malformed, noncanonical, or inconsistent.
    pub fn native_load_observation(
        &self,
        id: &NativeLoadObservationId,
    ) -> StoreResult<Option<NativeLoadObservation>> {
        let transaction = self.connection.unchecked_transaction()?;
        let record = load_native_load(&transaction, id.digest().as_str())?;
        transaction.commit()?;
        Ok(record)
    }
}

fn canonical_runtime_package(
    manifest: &RuntimePackageManifest,
    artifact_set: &rewrite_model::ArtifactSetManifest,
) -> StoreResult<String> {
    let encoded = crate::record::encode_record(manifest)?;
    let parsed = RuntimePackageManifest::from_json_bytes(encoded.as_bytes(), artifact_set)
        .map_err(StoreError::InvalidRuntimePackage)?;
    if parsed == *manifest && serde_json::to_string(&parsed)? == encoded {
        Ok(encoded)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn canonical_model_package(
    manifest: &ModelPackageManifest,
    artifact_set: &rewrite_model::ArtifactSetManifest,
) -> StoreResult<String> {
    let encoded = crate::record::encode_record(manifest)?;
    let parsed = ModelPackageManifest::from_json_bytes(encoded.as_bytes(), artifact_set)
        .map_err(StoreError::InvalidModelPackage)?;
    if parsed == *manifest && serde_json::to_string(&parsed)? == encoded {
        Ok(encoded)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn canonical_native_load(
    observation: &NativeLoadObservation,
    package: &RuntimePackageManifest,
) -> StoreResult<String> {
    let encoded = crate::record::encode_record(observation)?;
    let parsed = NativeLoadObservation::from_json_bytes(encoded.as_bytes(), package)
        .map_err(StoreError::InvalidNativeLoad)?;
    if parsed == *observation && serde_json::to_string(&parsed)? == encoded {
        Ok(encoded)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_source_artifact_set(
    connection: &Connection,
    transformation: &PackageTransformation,
) -> StoreResult<()> {
    let Some(source_id) = source_artifact_set_id(transformation) else {
        return Ok(());
    };
    super::evidence::read::load_artifact_set(connection, source_id)?
        .ok_or(StoreError::MissingRecord)?;
    Ok(())
}

fn source_artifact_set_id(transformation: &PackageTransformation) -> Option<&str> {
    match transformation {
        PackageTransformation::Untransformed { .. } => None,
        PackageTransformation::Transformed {
            source_artifact_set_id,
            ..
        } => Some(source_artifact_set_id.digest().as_str()),
    }
}

fn insert_runtime_package(
    connection: &Connection,
    manifest: &RuntimePackageManifest,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    let key = manifest.runtime_package_manifest_id();
    let changed = connection.execute(
        "INSERT INTO runtime_package_manifests
             (runtime_package_manifest_id, artifact_set_id, source_artifact_set_id, record_json)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(runtime_package_manifest_id) DO NOTHING",
        params![
            key.digest().as_str(),
            manifest.artifact_set_id().digest().as_str(),
            source_artifact_set_id(manifest.transformation()),
            encoded
        ],
    )?;
    immutable_disposition(
        connection,
        "runtime_package_manifests",
        "runtime_package_manifest_id",
        key.digest().as_str(),
        encoded,
        changed,
    )
}

fn insert_model_package(
    connection: &Connection,
    manifest: &ModelPackageManifest,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    let key = manifest.model_package_manifest_id();
    let changed = connection.execute(
        "INSERT INTO model_package_manifests
             (model_package_manifest_id, artifact_set_id, source_artifact_set_id, record_json)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(model_package_manifest_id) DO NOTHING",
        params![
            key.digest().as_str(),
            manifest.artifact_set_id().digest().as_str(),
            source_artifact_set_id(manifest.transformation()),
            encoded
        ],
    )?;
    immutable_disposition(
        connection,
        "model_package_manifests",
        "model_package_manifest_id",
        key.digest().as_str(),
        encoded,
        changed,
    )
}

fn insert_native_load(
    connection: &Connection,
    observation: &NativeLoadObservation,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    let key = observation.native_load_observation_id();
    let changed = connection.execute(
        "INSERT INTO native_load_observations
             (native_load_observation_id, runtime_package_manifest_id, record_json)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(native_load_observation_id) DO NOTHING",
        params![
            key.digest().as_str(),
            observation.runtime_package_manifest_id().digest().as_str(),
            encoded
        ],
    )?;
    immutable_disposition(
        connection,
        "native_load_observations",
        "native_load_observation_id",
        key.digest().as_str(),
        encoded,
        changed,
    )
}

#[cfg(test)]
#[path = "package_contracts/tests.rs"]
mod tests;
