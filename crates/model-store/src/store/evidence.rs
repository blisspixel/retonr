use rusqlite::{Connection, TransactionBehavior};

use rewrite_model::{
    ArtifactSetId, ArtifactSetManifest, EffectivePackageEvidence, EffectivePackageEvidenceId,
    EffectiveRuntimeState, EffectiveRuntimeStateId, QualificationId, QualificationRecord,
    QualificationRecordV2, QualificationV2Id, RuntimeBuildId, RuntimeBuildIdentity,
};

use super::{ArtifactStateStore, WriteDisposition, load_qualification};
use crate::{StoreError, StoreResult, record::immutable_disposition};

pub(super) mod read;

use read::{
    load_artifact_set, load_package_evidence, load_qualification_dependencies,
    load_qualification_v2, load_runtime_build, load_runtime_state,
};

impl ArtifactStateStore {
    /// Reloads one qualification record and rechecks its content-derived identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the record is absent, malformed, or no longer
    /// matches its indexed identity.
    pub fn qualification(
        &self,
        qualification_id: &QualificationId,
    ) -> StoreResult<Option<QualificationRecord>> {
        load_qualification(&self.connection, qualification_id)
    }

    /// Stores one validated immutable artifact-set manifest.
    ///
    /// This record is inert evidence and does not install or activate its members.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when validation, encoding, or immutable insertion fails.
    pub fn put_artifact_set_manifest(
        &self,
        manifest: &ArtifactSetManifest,
    ) -> StoreResult<WriteDisposition> {
        manifest
            .validate()
            .map_err(StoreError::InvalidArtifactSet)?;
        let encoded = manifest.canonical_json();
        let parsed = ArtifactSetManifest::from_json_bytes(encoded.as_bytes())
            .map_err(StoreError::InvalidArtifactSet)?;
        if &parsed != manifest {
            return Err(StoreError::CorruptRecord);
        }
        put_record(
            &self.connection,
            "artifact_set_manifests",
            "artifact_set_id",
            manifest.artifact_set_id().digest().as_str(),
            &[],
            &encoded,
        )
    }

    /// Returns an artifact-set manifest after revalidating its exact indexed identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when durable state is malformed or inconsistent.
    pub fn artifact_set_manifest(
        &self,
        id: &ArtifactSetId,
    ) -> StoreResult<Option<ArtifactSetManifest>> {
        load_artifact_set(&self.connection, id.digest().as_str())
    }

    /// Stores one immutable runtime-build identity.
    ///
    /// This record is structural evidence only. It does not attest a live process.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when encoding, validation, or immutable insertion fails.
    pub fn put_runtime_build_identity(
        &self,
        build: &RuntimeBuildIdentity,
    ) -> StoreResult<WriteDisposition> {
        let encoded = serde_json::to_string(build)?;
        let parsed = RuntimeBuildIdentity::from_json_bytes(encoded.as_bytes())
            .map_err(StoreError::InvalidRuntimeBuild)?;
        if &parsed != build {
            return Err(StoreError::CorruptRecord);
        }
        put_record(
            &self.connection,
            "runtime_build_identities",
            "runtime_build_id",
            build.runtime_build_id().digest().as_str(),
            &[],
            &encoded,
        )
    }

    /// Returns a runtime-build identity after revalidating its indexed identity.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when durable state is malformed or inconsistent.
    pub fn runtime_build_identity(
        &self,
        id: &RuntimeBuildId,
    ) -> StoreResult<Option<RuntimeBuildIdentity>> {
        load_runtime_build(&self.connection, id.digest().as_str())
    }

    /// Stores an effective runtime state after resolving its runtime-build dependency.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the referenced build is absent, any identity is
    /// inconsistent, or the transaction cannot commit.
    pub fn put_effective_runtime_state(
        &mut self,
        state: &EffectiveRuntimeState,
    ) -> StoreResult<WriteDisposition> {
        let encoded = serde_json::to_string(state)?;
        let parsed = EffectiveRuntimeState::from_json_bytes(encoded.as_bytes())
            .map_err(StoreError::InvalidRuntimeState)?;
        if &parsed != state {
            return Err(StoreError::CorruptRecord);
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        load_runtime_build(&transaction, state.runtime_build_id().digest().as_str())?
            .ok_or(StoreError::MissingRecord)?;
        let disposition = put_state_record(&transaction, state, &encoded)?;
        let stored = load_runtime_state(
            &transaction,
            state.effective_runtime_state_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        if stored != *state {
            return Err(StoreError::ImmutableConflict);
        }
        transaction.commit()?;
        Ok(disposition)
    }

    /// Returns an effective runtime state after recursively validating its build.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an existing record is malformed or has a missing or
    /// inconsistent runtime-build dependency. An absent requested record returns `None`.
    pub fn effective_runtime_state(
        &self,
        id: &EffectiveRuntimeStateId,
    ) -> StoreResult<Option<EffectiveRuntimeState>> {
        load_runtime_state(&self.connection, id.digest().as_str())
    }

    /// Stores effective-package evidence after resolving all exact dependencies.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a dependency is absent, the record and referenced
    /// identities disagree, or the transaction cannot commit.
    pub fn put_effective_package_evidence(
        &mut self,
        evidence: &EffectivePackageEvidence,
    ) -> StoreResult<WriteDisposition> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact_set =
            load_artifact_set(&transaction, evidence.artifact_set_id().digest().as_str())?
                .ok_or(StoreError::MissingRecord)?;
        let build =
            load_runtime_build(&transaction, evidence.runtime_build_id().digest().as_str())?
                .ok_or(StoreError::MissingRecord)?;
        let state = load_runtime_state(
            &transaction,
            evidence.effective_runtime_state_id().digest().as_str(),
        )?
        .ok_or(StoreError::MissingRecord)?;
        evidence
            .validate_against(&artifact_set, &build, &state)
            .map_err(StoreError::InvalidEffectivePackage)?;
        let encoded = serde_json::to_string(evidence)?;
        let parsed = EffectivePackageEvidence::from_json_bytes(
            encoded.as_bytes(),
            &artifact_set,
            &build,
            &state,
        )
        .map_err(StoreError::InvalidEffectivePackage)?;
        if &parsed != evidence {
            return Err(StoreError::CorruptRecord);
        }
        let disposition = put_package_record(&transaction, evidence, &encoded)?;
        let stored = load_package_evidence(
            &transaction,
            evidence.effective_package_evidence_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        if stored != *evidence {
            return Err(StoreError::ImmutableConflict);
        }
        transaction.commit()?;
        Ok(disposition)
    }

    /// Returns effective-package evidence after recursively validating every reference.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an existing record is malformed or has a missing or
    /// inconsistent dependency. An absent requested record returns `None`.
    pub fn effective_package_evidence(
        &self,
        id: &EffectivePackageEvidenceId,
    ) -> StoreResult<Option<EffectivePackageEvidence>> {
        load_package_evidence(&self.connection, id.digest().as_str())
    }

    /// Stores inert qualification-v2 evidence after resolving its complete subject.
    ///
    /// This method deliberately creates no active binding and grants no runtime authority.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a dependency is absent, the cross-record join fails,
    /// or the transaction cannot commit.
    pub fn put_qualification_v2(
        &mut self,
        qualification: &QualificationRecordV2,
    ) -> StoreResult<WriteDisposition> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let dependencies = load_qualification_dependencies(
            &transaction,
            qualification.artifact_set_id().digest().as_str(),
            qualification.runtime_build_id().digest().as_str(),
            qualification.effective_runtime_state_id().digest().as_str(),
            qualification
                .effective_package_evidence_id()
                .digest()
                .as_str(),
        )?;
        qualification
            .validate_against(
                &dependencies.artifact_set,
                &dependencies.build,
                &dependencies.state,
                &dependencies.package,
            )
            .map_err(StoreError::InvalidQualificationV2)?;
        let encoded = serde_json::to_string(qualification)?;
        let parsed = QualificationRecordV2::from_json_bytes(
            encoded.as_bytes(),
            &dependencies.artifact_set,
            &dependencies.build,
            &dependencies.state,
            &dependencies.package,
        )
        .map_err(StoreError::InvalidQualificationV2)?;
        if &parsed != qualification {
            return Err(StoreError::CorruptRecord);
        }
        let disposition = put_qualification_record(&transaction, qualification, &encoded)?;
        let stored = load_qualification_v2(
            &transaction,
            qualification.qualification_v2_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        if stored != *qualification {
            return Err(StoreError::ImmutableConflict);
        }
        transaction.commit()?;
        Ok(disposition)
    }

    /// Returns qualification-v2 evidence after recursively validating its subject.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when an existing record is malformed or has a missing or
    /// inconsistent subject record. An absent requested record returns `None`.
    pub fn qualification_v2(
        &self,
        id: &QualificationV2Id,
    ) -> StoreResult<Option<QualificationRecordV2>> {
        load_qualification_v2(&self.connection, id.digest().as_str())
    }
}

fn put_record(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    columns: &[(&str, &str)],
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    let mut names = vec![key_column];
    names.extend(columns.iter().map(|(name, _)| *name));
    names.push("record_json");
    let placeholders = (1..=names.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES ({placeholders}) \
         ON CONFLICT({key_column}) DO NOTHING",
        names.join(", ")
    );
    let mut values = vec![key];
    values.extend(columns.iter().map(|(_, value)| *value));
    values.push(encoded);
    let changed = connection.execute(&sql, rusqlite::params_from_iter(values))?;
    immutable_disposition(connection, table, key_column, key, encoded, changed)
}

fn put_state_record(
    connection: &Connection,
    state: &EffectiveRuntimeState,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    put_record(
        connection,
        "effective_runtime_states",
        "effective_runtime_state_id",
        state.effective_runtime_state_id().digest().as_str(),
        &[(
            "runtime_build_id",
            state.runtime_build_id().digest().as_str(),
        )],
        encoded,
    )
}

fn put_package_record(
    connection: &Connection,
    evidence: &EffectivePackageEvidence,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    put_record(
        connection,
        "effective_package_evidence",
        "effective_package_evidence_id",
        evidence.effective_package_evidence_id().digest().as_str(),
        &[
            (
                "artifact_set_id",
                evidence.artifact_set_id().digest().as_str(),
            ),
            (
                "runtime_build_id",
                evidence.runtime_build_id().digest().as_str(),
            ),
            (
                "effective_runtime_state_id",
                evidence.effective_runtime_state_id().digest().as_str(),
            ),
        ],
        encoded,
    )
}

fn put_qualification_record(
    connection: &Connection,
    qualification: &QualificationRecordV2,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    put_record(
        connection,
        "qualification_v2_records",
        "qualification_v2_id",
        qualification.qualification_v2_id().digest().as_str(),
        &[
            (
                "artifact_set_id",
                qualification.artifact_set_id().digest().as_str(),
            ),
            (
                "effective_package_evidence_id",
                qualification
                    .effective_package_evidence_id()
                    .digest()
                    .as_str(),
            ),
            (
                "runtime_build_id",
                qualification.runtime_build_id().digest().as_str(),
            ),
            (
                "effective_runtime_state_id",
                qualification.effective_runtime_state_id().digest().as_str(),
            ),
        ],
        encoded,
    )
}
