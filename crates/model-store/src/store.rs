use std::path::Path;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use rewrite_model::{
    ActivationAction, ActivationDecision, ActivationId, ActiveArtifactBinding, ArtifactId,
    ArtifactManifest, ArtifactRole, InstalledArtifact, QualificationId, QualificationInvalidation,
    QualificationRecord, activate,
};

use crate::{
    StoreError, StoreResult,
    binding::{load_active_binding, load_active_bindings, role_key},
    record::{
        decode_record, encode_record, immutable_disposition, insert_immutable, load_record,
        load_required,
    },
    schema,
};

/// Outcome of writing an immutable record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteDisposition {
    /// A new immutable record was inserted.
    Inserted,
    /// The exact record already existed under the same identifier.
    AlreadyPresent,
}

/// Outcome of atomically registering one manifest and installed artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstallationWriteDisposition {
    /// Manifest write outcome.
    pub manifest: WriteDisposition,
    /// Installed-artifact write outcome.
    pub installed: WriteDisposition,
}

/// SQLite-backed artifact state repository.
pub struct ArtifactStateStore {
    connection: Connection,
}

impl ArtifactStateStore {
    /// Opens or creates an artifact state database and applies supported migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the database cannot be opened, configured, or
    /// migrated without losing existing state.
    pub fn open(path: &Path) -> StoreResult<Self> {
        let mut connection = Connection::open(path)?;
        schema::initialize(&mut connection)?;
        Ok(Self { connection })
    }

    /// Stores one validated immutable artifact manifest.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation, encoding, or durable insertion fails,
    /// or when the identifier already names different content.
    pub fn put_manifest(&self, manifest: &ArtifactManifest) -> StoreResult<WriteDisposition> {
        manifest.validate().map_err(StoreError::InvalidManifest)?;
        let key = manifest.artifact_id.digest().as_str();
        let encoded = encode_record(manifest)?;
        insert_immutable(
            &self.connection,
            "artifact_manifests",
            "artifact_id",
            key,
            &encoded,
        )
    }

    /// Returns a manifest by exact content-derived identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when stored state cannot be read or decoded.
    pub fn manifest(&self, artifact_id: &ArtifactId) -> StoreResult<Option<ArtifactManifest>> {
        let manifest = load_record::<ArtifactManifest>(
            &self.connection,
            "artifact_manifests",
            "artifact_id",
            artifact_id.digest().as_str(),
        )?;
        manifest
            .map(|manifest| {
                manifest.validate().map_err(StoreError::InvalidManifest)?;
                if &manifest.artifact_id == artifact_id {
                    Ok(manifest)
                } else {
                    Err(StoreError::CorruptRecord)
                }
            })
            .transpose()
    }

    /// Stores verified installation state after confirming the matching manifest.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] for invalid, missing, mismatched, conflicting, or
    /// non-durable state.
    pub fn put_installed(&self, installed: &InstalledArtifact) -> StoreResult<WriteDisposition> {
        installed
            .validate()
            .map_err(StoreError::InvalidInstallation)?;
        let manifest = self
            .manifest(&installed.artifact_id)?
            .ok_or(StoreError::MissingRecord)?;
        if manifest.artifact_digest != installed.artifact_digest
            || manifest.byte_size != installed.byte_size
        {
            return Err(StoreError::ImmutableConflict);
        }
        let key = installed.artifact_id.digest().as_str();
        let encoded = encode_record(installed)?;
        insert_immutable(
            &self.connection,
            "installed_artifacts",
            "artifact_id",
            key,
            &encoded,
        )
    }

    /// Atomically stores a manifest and its verified installation state.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when either record is invalid, the records disagree,
    /// an immutable identifier already names different state, or the transaction
    /// cannot commit in full.
    pub fn put_installation(
        &mut self,
        manifest: &ArtifactManifest,
        installed: &InstalledArtifact,
    ) -> StoreResult<InstallationWriteDisposition> {
        manifest.validate().map_err(StoreError::InvalidManifest)?;
        installed
            .validate()
            .map_err(StoreError::InvalidInstallation)?;
        if manifest.artifact_id != installed.artifact_id
            || manifest.artifact_digest != installed.artifact_digest
            || manifest.byte_size != installed.byte_size
        {
            return Err(StoreError::ImmutableConflict);
        }

        let manifest_key = manifest.artifact_id.digest().as_str();
        let manifest_record = encode_record(manifest)?;
        let installed_record = encode_record(installed)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let manifest_disposition = insert_immutable(
            &transaction,
            "artifact_manifests",
            "artifact_id",
            manifest_key,
            &manifest_record,
        )?;
        let installed_disposition = insert_immutable(
            &transaction,
            "installed_artifacts",
            "artifact_id",
            manifest_key,
            &installed_record,
        )?;
        transaction.commit()?;
        Ok(InstallationWriteDisposition {
            manifest: manifest_disposition,
            installed: installed_disposition,
        })
    }

    /// Stores one validated immutable qualification record.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the manifest is absent, identities differ, or the
    /// record cannot be validated and inserted immutably.
    pub fn put_qualification(
        &self,
        qualification: &QualificationRecord,
    ) -> StoreResult<WriteDisposition> {
        qualification
            .validate()
            .map_err(StoreError::InvalidQualification)?;
        let manifest = self
            .manifest(&qualification.artifact_id)?
            .ok_or(StoreError::MissingRecord)?;
        if manifest.artifact_digest != qualification.artifact_digest {
            return Err(StoreError::ImmutableConflict);
        }
        let qualification_id = qualification
            .qualification_id()
            .map_err(StoreError::InvalidQualification)?;
        let key = qualification_id.digest().as_str();
        let encoded = encode_record(qualification)?;
        let changed = self.connection.execute(
            "INSERT INTO qualification_records
                 (qualification_id, artifact_id, record_json)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(qualification_id) DO NOTHING",
            params![key, qualification.artifact_id.digest().as_str(), encoded],
        )?;
        immutable_disposition(
            &self.connection,
            "qualification_records",
            "qualification_id",
            key,
            &encoded,
            changed,
        )
    }

    /// Appends an invalidation and removes any active pointer it invalidates in one
    /// transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the invalidation is malformed, its qualification
    /// is absent, or the transaction cannot commit.
    pub fn invalidate(&mut self, invalidation: &QualificationInvalidation) -> StoreResult<()> {
        invalidation
            .validate()
            .map_err(StoreError::InvalidInvalidation)?;
        let encoded = encode_record(invalidation)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        load_qualification(&transaction, &invalidation.qualification_id)?
            .ok_or(StoreError::MissingRecord)?;
        transaction.execute(
            "INSERT INTO qualification_invalidations
                 (qualification_id, reason_code, record_json)
             VALUES (?1, ?2, ?3)",
            params![
                invalidation.qualification_id.digest().as_str(),
                invalidation.reason_code,
                encoded
            ],
        )?;
        transaction.execute(
            "DELETE FROM active_bindings WHERE qualification_id = ?1",
            [invalidation.qualification_id.digest().as_str()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Atomically appends an activation decision and changes the active role pointer
    /// after revalidating all durable evidence inside the transaction.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when any required record is absent, invalidated,
    /// inconsistent, conflicting, corrupt, or cannot be committed atomically.
    pub fn activate(
        &mut self,
        activation_id: ActivationId,
        role: ArtifactRole,
        verified_installed: &InstalledArtifact,
        qualification_id: &QualificationId,
    ) -> StoreResult<ActiveArtifactBinding> {
        verified_installed
            .validate()
            .map_err(StoreError::InvalidInstallation)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_absent_activation(&transaction, &activation_id)?;
        let installed: InstalledArtifact = load_required(
            &transaction,
            "installed_artifacts",
            "artifact_id",
            verified_installed.artifact_id.digest().as_str(),
        )?;
        if &installed != verified_installed {
            return Err(StoreError::VerificationFailed);
        }
        let qualification =
            load_qualification(&transaction, qualification_id)?.ok_or(StoreError::MissingRecord)?;
        let invalidations = load_invalidations(&transaction, qualification_id)?;
        let binding = activate(&installed, &qualification, &invalidations, role)
            .map_err(|_| StoreError::InvalidActiveBinding)?;
        let decision = ActivationDecision {
            activation_id,
            action: ActivationAction::Activate,
            role,
            artifact_id: Some(binding.artifact_id.clone()),
            qualification_id: Some(binding.qualification_id.clone()),
        };
        decision.validate().map_err(StoreError::InvalidDecision)?;
        insert_decision(&transaction, &decision)?;
        let binding_json = encode_record(&binding)?;
        transaction.execute(
            "INSERT INTO active_bindings
                 (role, artifact_id, qualification_id, record_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(role) DO UPDATE SET
                 artifact_id = excluded.artifact_id,
                 qualification_id = excluded.qualification_id,
                 record_json = excluded.record_json",
            params![
                role_key(role),
                binding.artifact_id.digest().as_str(),
                binding.qualification_id.digest().as_str(),
                binding_json
            ],
        )?;
        transaction.commit()?;
        Ok(binding)
    }

    /// Atomically appends a deactivation decision and clears one active pointer.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when no active binding exists, the decision conflicts,
    /// or the transaction cannot commit.
    pub fn deactivate(
        &mut self,
        activation_id: ActivationId,
        role: ArtifactRole,
    ) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_absent_activation(&transaction, &activation_id)?;
        require_record(&transaction, "active_bindings", "role", role_key(role))?;
        let decision = ActivationDecision {
            activation_id,
            action: ActivationAction::Deactivate,
            role,
            artifact_id: None,
            qualification_id: None,
        };
        decision.validate().map_err(StoreError::InvalidDecision)?;
        insert_decision(&transaction, &decision)?;
        transaction.execute(
            "DELETE FROM active_bindings WHERE role = ?1",
            [role_key(role)],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Returns one active binding only after revalidating it against current durable
    /// installation, qualification, and invalidation state.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] instead of a binding when persisted state is corrupt or
    /// no longer authorizes the role.
    pub fn active_binding<F>(
        &self,
        role: ArtifactRole,
        mut verify_installed: F,
    ) -> StoreResult<Option<ActiveArtifactBinding>>
    where
        F: FnMut(&InstalledArtifact) -> bool,
    {
        load_active_binding(&self.connection, role)?
            .map(|binding| validate_binding(&self.connection, &binding, &mut verify_installed))
            .transpose()
    }

    /// Revalidates every active binding before returning any recovered state.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] with no partial result if any binding is corrupt,
    /// incomplete, invalidated, or inconsistent.
    pub fn recover_active_bindings<F>(
        &self,
        mut verify_installed: F,
    ) -> StoreResult<Vec<ActiveArtifactBinding>>
    where
        F: FnMut(&InstalledArtifact) -> bool,
    {
        let bindings = load_active_bindings(&self.connection)?;
        let mut recovered = Vec::with_capacity(bindings.len());
        for binding in bindings {
            recovered.push(validate_binding(
                &self.connection,
                &binding,
                &mut verify_installed,
            )?);
        }
        Ok(recovered)
    }

    /// Removes verified installation state only when no active role points at it.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the installation is active, absent, or cannot be
    /// removed atomically.
    pub fn remove_installed(&mut self, artifact_id: &ArtifactId) -> StoreResult<()> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active_bindings = load_active_bindings(&transaction)?;
        if active_bindings
            .iter()
            .any(|binding| &binding.artifact_id == artifact_id)
        {
            return Err(StoreError::ActiveArtifact);
        }
        let changed = transaction.execute(
            "DELETE FROM installed_artifacts WHERE artifact_id = ?1",
            [artifact_id.digest().as_str()],
        )?;
        if changed == 0 {
            return Err(StoreError::MissingRecord);
        }
        transaction.commit()?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn connection(&self) -> &Connection {
        &self.connection
    }
}

fn require_record(
    transaction: &Transaction<'_>,
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

fn require_absent_activation(
    transaction: &Transaction<'_>,
    activation_id: &ActivationId,
) -> StoreResult<()> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM activation_decisions WHERE activation_id = ?1
         )",
        [activation_id.digest().as_str()],
        |row| row.get(0),
    )?;
    if exists {
        Err(StoreError::ImmutableConflict)
    } else {
        Ok(())
    }
}

fn insert_decision(
    transaction: &Transaction<'_>,
    decision: &ActivationDecision,
) -> StoreResult<()> {
    let encoded = encode_record(decision)?;
    transaction.execute(
        "INSERT INTO activation_decisions (activation_id, role, record_json)
         VALUES (?1, ?2, ?3)",
        params![
            decision.activation_id.digest().as_str(),
            role_key(decision.role),
            encoded
        ],
    )?;
    Ok(())
}

fn load_invalidations(
    connection: &Connection,
    qualification_id: &QualificationId,
) -> StoreResult<Vec<QualificationInvalidation>> {
    let mut statement = connection.prepare(
        "SELECT reason_code, record_json FROM qualification_invalidations
         WHERE qualification_id = ?1 ORDER BY sequence ASC",
    )?;
    let rows = statement.query_map([qualification_id.digest().as_str()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut invalidations = Vec::new();
    for row in rows {
        let (stored_reason, encoded) = row?;
        let invalidation = decode_record::<QualificationInvalidation>(&encoded)?;
        invalidation
            .validate()
            .map_err(StoreError::InvalidInvalidation)?;
        if invalidation.qualification_id != *qualification_id
            || invalidation.reason_code != stored_reason
        {
            return Err(StoreError::CorruptRecord);
        }
        invalidations.push(invalidation);
    }
    Ok(invalidations)
}

fn load_qualification(
    connection: &Connection,
    qualification_id: &QualificationId,
) -> StoreResult<Option<QualificationRecord>> {
    let qualification = load_record::<QualificationRecord>(
        connection,
        "qualification_records",
        "qualification_id",
        qualification_id.digest().as_str(),
    )?;
    qualification
        .map(|qualification| {
            qualification
                .validate()
                .map_err(StoreError::InvalidQualification)?;
            if qualification
                .qualification_id()
                .map_err(StoreError::InvalidQualification)?
                == *qualification_id
            {
                Ok(qualification)
            } else {
                Err(StoreError::CorruptRecord)
            }
        })
        .transpose()
}

fn validate_binding<F>(
    connection: &Connection,
    binding: &ActiveArtifactBinding,
    verify_installed: &mut F,
) -> StoreResult<ActiveArtifactBinding>
where
    F: FnMut(&InstalledArtifact) -> bool,
{
    let installed = load_record::<InstalledArtifact>(
        connection,
        "installed_artifacts",
        "artifact_id",
        binding.artifact_id.digest().as_str(),
    )?
    .ok_or(StoreError::InvalidActiveBinding)?;
    if !verify_installed(&installed) {
        return Err(StoreError::VerificationFailed);
    }
    let qualification = load_qualification(connection, &binding.qualification_id)?
        .ok_or(StoreError::InvalidActiveBinding)?;
    let invalidations = load_invalidations(connection, &binding.qualification_id)?;
    let recovered = activate(&installed, &qualification, &invalidations, binding.role)
        .map_err(|_| StoreError::InvalidActiveBinding)?;
    if &recovered == binding {
        Ok(recovered)
    } else {
        Err(StoreError::InvalidActiveBinding)
    }
}

#[cfg(test)]
mod tests;
