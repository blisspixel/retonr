use rusqlite::Connection;
use tempfile::tempdir;

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ActivationId, ArtifactId, ArtifactManifest, ArtifactRole,
    ArtifactSource, DeclaredCapabilities, HardwareTier, InstalledArtifact, LicenseDecision,
    LicenseRecord, QUALIFICATION_SCHEMA_VERSION, QualificationInvalidation, QualificationRecord,
    QualificationStatus, RuntimeIdentity,
};
use rewrite_types::Digest;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

struct Fixture {
    manifest: ArtifactManifest,
    installed: InstalledArtifact,
    qualification: QualificationRecord,
}

fn fixture() -> Fixture {
    let artifact_digest = Digest::sha256(b"artifact");
    let artifact_id = ArtifactId::from_digest(artifact_digest.clone());
    Fixture {
        manifest: ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: artifact_id.clone(),
            source: ArtifactSource {
                origin: "fixture/model".to_owned(),
                revision: "revision-1".to_owned(),
            },
            artifact_digest: artifact_digest.clone(),
            byte_size: 8,
            format: "gguf".to_owned(),
            family: "fixture".to_owned(),
            architecture: Some("transformer".to_owned()),
            quantization: Some("q4".to_owned()),
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: Some(8_192),
            },
        },
        installed: InstalledArtifact {
            artifact_id: artifact_id.clone(),
            artifact_digest: artifact_digest.clone(),
            byte_size: 8,
            storage_key: "artifacts/fixture.gguf".to_owned(),
        },
        qualification: QualificationRecord {
            schema_version: QUALIFICATION_SCHEMA_VERSION,
            artifact_id,
            artifact_digest,
            runtime: RuntimeIdentity {
                backend: "fake".to_owned(),
                version: "1.0.0".to_owned(),
                digest: Some(Digest::sha256(b"runtime")),
            },
            operating_system: "test".to_owned(),
            hardware_tier: HardwareTier {
                id: "test".to_owned(),
                memory_mib: 8_192,
                accelerator: "none".to_owned(),
            },
            supported_roles: vec![ArtifactRole::Generation],
            source_byte_limit: 4_096,
            context_token_limit: 8_192,
            prompt_template_digest: Digest::sha256(b"prompt"),
            request_policy_digest: Digest::sha256(b"request"),
            threshold_policy_digest: Digest::sha256(b"threshold"),
            license_decision: LicenseDecision::LocalUseOnly,
            status: QualificationStatus::Qualified,
        },
    }
}

fn populate(store: &ArtifactStateStore, fixture: &Fixture) {
    store
        .put_manifest(&fixture.manifest)
        .expect("store manifest");
    store
        .put_installed(&fixture.installed)
        .expect("store installation");
    store
        .put_qualification(&fixture.qualification)
        .expect("store qualification");
}

fn qualification_id(fixture: &Fixture) -> rewrite_model::QualificationId {
    fixture
        .qualification
        .qualification_id()
        .expect("fixture qualification is valid")
}

#[test]
fn persists_and_recovers_an_exact_active_binding() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("artifacts.sqlite3");
    let fixture = fixture();
    {
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        populate(&store, &fixture);
        let binding = store
            .activate(
                ActivationId::from_digest(Digest::sha256(b"activation-1")),
                ArtifactRole::Generation,
                &fixture.installed,
                &qualification_id(&fixture),
            )
            .expect("activate exact fixture");
        assert_eq!(binding.artifact_id, fixture.installed.artifact_id);
    }

    let reopened = ArtifactStateStore::open(&path).expect("reopen store");
    let recovered = reopened
        .recover_active_bindings(|installed| installed == &fixture.installed)
        .expect("recover complete state");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].artifact_id, fixture.installed.artifact_id);
}

#[test]
fn immutable_records_are_idempotent_but_conflicts_fail() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let mut fixture = fixture();
    assert_eq!(
        store.put_manifest(&fixture.manifest).expect("insert"),
        WriteDisposition::Inserted
    );
    assert_eq!(
        store.put_manifest(&fixture.manifest).expect("idempotent"),
        WriteDisposition::AlreadyPresent
    );
    fixture.manifest.source.revision = "revision-2".to_owned();
    assert!(matches!(
        store.put_manifest(&fixture.manifest),
        Err(StoreError::ImmutableConflict)
    ));
}

#[test]
fn conflicting_activation_rolls_back_without_moving_the_pointer() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    let activation_id = ActivationId::from_digest(Digest::sha256(b"activation"));
    let first = store
        .activate(
            activation_id.clone(),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("initial activation");
    assert!(matches!(
        store.activate(
            activation_id,
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        ),
        Err(StoreError::ImmutableConflict)
    ));
    assert_eq!(
        store
            .active_binding(ArtifactRole::Generation, |_| true)
            .expect("read active binding"),
        Some(first)
    );
}

#[test]
fn failure_after_decision_insert_rolls_back_the_complete_activation() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    let first = store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"activation-1")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("initial activation");
    store
        .connection()
        .execute_batch(
            "CREATE TEMP TRIGGER fail_binding_update
             BEFORE UPDATE ON active_bindings
             BEGIN
                 SELECT RAISE(ABORT, 'injected activation failure');
             END;",
        )
        .expect("install failure trigger");

    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"activation-2")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        ),
        Err(StoreError::Database(_))
    ));
    assert_eq!(
        store
            .active_binding(ArtifactRole::Generation, |_| true)
            .expect("read old binding"),
        Some(first)
    );
    let decisions: u32 = store
        .connection()
        .query_row("SELECT COUNT(*) FROM activation_decisions", [], |row| {
            row.get(0)
        })
        .expect("count committed decisions");
    assert_eq!(decisions, 1);
}

#[test]
fn invalidation_clears_the_pointer_and_prevents_reactivation() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate");
    store
        .invalidate(&QualificationInvalidation {
            qualification_id: qualification_id(&fixture),
            reason_code: "runtime_drift".to_owned(),
        })
        .expect("invalidate");
    assert!(
        store
            .active_binding(ArtifactRole::Generation, |_| true)
            .expect("read state")
            .is_none()
    );
    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"activation-2")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        ),
        Err(StoreError::InvalidActiveBinding)
    ));
}

#[test]
fn removal_cannot_orphan_an_active_binding() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate");
    assert!(matches!(
        store.remove_installed(&fixture.installed.artifact_id),
        Err(StoreError::ActiveArtifact)
    ));
    store
        .deactivate(
            ActivationId::from_digest(Digest::sha256(b"deactivation")),
            ArtifactRole::Generation,
        )
        .expect("deactivate");
    store
        .remove_installed(&fixture.installed.artifact_id)
        .expect("remove inactive installation");
}

#[test]
fn recovery_fails_closed_on_a_tampered_binding() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    let mut binding = store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate");
    binding.role = ArtifactRole::Embedding;
    let tampered = serde_json::to_string(&binding).expect("serialize tampered binding");
    store
        .connection()
        .execute(
            "UPDATE active_bindings SET record_json = ?1 WHERE role = 'generation'",
            [tampered],
        )
        .expect("tamper fixture");
    assert!(matches!(
        store.recover_active_bindings(|_| true),
        Err(StoreError::InvalidActiveBinding)
    ));
}

#[test]
fn activation_and_recovery_require_current_byte_verification() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    let mut stale = fixture.installed.clone();
    stale.storage_key = "artifacts/replaced.gguf".to_owned();
    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"stale-activation")),
            ArtifactRole::Generation,
            &stale,
            &qualification_id(&fixture),
        ),
        Err(StoreError::VerificationFailed)
    ));
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate verified state");
    assert!(matches!(
        store.recover_active_bindings(|_| false),
        Err(StoreError::VerificationFailed)
    ));
}

#[test]
fn activation_rejects_qualification_content_changed_under_an_existing_id() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    let stored_id = qualification_id(&fixture);
    let mut changed = fixture.qualification.clone();
    changed.context_token_limit += 1;
    let changed_json = serde_json::to_string(&changed).expect("serialize changed qualification");
    store
        .connection()
        .execute(
            "UPDATE qualification_records SET record_json = ?1 WHERE qualification_id = ?2",
            rusqlite::params![changed_json, stored_id.digest().as_str()],
        )
        .expect("replace qualification content under existing identifier");

    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &stored_id,
        ),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn activation_rejects_an_invalidation_changed_under_indexed_columns() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    let stored_id = qualification_id(&fixture);
    store
        .invalidate(&QualificationInvalidation {
            qualification_id: stored_id.clone(),
            reason_code: "runtime_drift".to_owned(),
        })
        .expect("store invalidation");
    let changed = QualificationInvalidation {
        qualification_id: stored_id.clone(),
        reason_code: "artifact_drift".to_owned(),
    };
    let changed_json = serde_json::to_string(&changed).expect("serialize changed invalidation");
    store
        .connection()
        .execute(
            "UPDATE qualification_invalidations SET record_json = ?1",
            [changed_json],
        )
        .expect("replace invalidation content under indexed columns");

    assert!(matches!(
        store.activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &stored_id,
        ),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn newer_schema_is_rejected_without_migration() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("future.db");
    let connection = Connection::open(&path).expect("create database");
    connection
        .pragma_update(None, "user_version", 2)
        .expect("set future version");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open(&path),
        Err(StoreError::UnsupportedSchema(2))
    ));
}

#[test]
fn invalid_boundary_records_are_rejected_before_sql() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let mut fixture = fixture();
    let valid_qualification_id = qualification_id(&fixture);
    store
        .put_manifest(&fixture.manifest)
        .expect("store manifest");

    fixture.installed.storage_key = "../outside".to_owned();
    assert!(matches!(
        store.put_installed(&fixture.installed),
        Err(StoreError::InvalidInstallation(_))
    ));

    fixture.qualification.license_decision = LicenseDecision::Rejected;
    assert!(matches!(
        store.put_qualification(&fixture.qualification),
        Err(StoreError::InvalidQualification(_))
    ));

    assert!(matches!(
        store.invalidate(&QualificationInvalidation {
            qualification_id: valid_qualification_id,
            reason_code: "Runtime Drift".to_owned(),
        }),
        Err(StoreError::InvalidInvalidation(_))
    ));
}
