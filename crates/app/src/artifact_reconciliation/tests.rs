use std::fs;

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_model_store::{ArtifactStateStore, WriteDisposition};
use rewrite_types::{CancellationToken, Digest};
use tempfile::{TempDir, tempdir};

use super::{
    ArtifactOrphanReconciliationRequest, ArtifactOrphanReconciliationService,
    ArtifactOrphanReconciliationStage, ArtifactReconciliationDisposition,
    ArtifactReconciliationError, ArtifactReconciliationLimits,
};
use crate::artifact_storage::ExactArtifactSync;
use crate::{ArtifactImportLimits, OfflineArtifactImportService};

mod concurrency;

#[cfg(unix)]
mod platform;
#[cfg(windows)]
mod windows;

const BYTES: &[u8] = b"selected orphan reconciliation fixture";

fn limits() -> ArtifactReconciliationLimits {
    ArtifactReconciliationLimits {
        maximum_artifact_bytes: 4_096,
        maximum_storage_entries: 32,
    }
}

fn manifest(bytes: &[u8]) -> ArtifactManifest {
    let digest = Digest::sha256(bytes);
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/reconciliation".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(bytes.len()).expect("fixture size fits u64"),
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        architecture: Some("transformer".to_owned()),
        quantization: Some("q4".to_owned()),
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "weights".to_owned(),
            identifier: "Apache-2.0".to_owned(),
            text_digest: Digest::sha256(b"fixture license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: Some(8_192),
        },
    }
}

fn initialize() -> (TempDir, ArtifactStateStore) {
    let directory = tempdir().expect("temporary directory");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    {
        let _service = OfflineArtifactImportService::open(
            storage(&directory),
            &mut store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
                maximum_storage_entries: 32,
            },
        )
        .expect("initialize managed storage");
    }
    (directory, store)
}

fn storage(directory: &TempDir) -> std::path::PathBuf {
    directory.path().join("storage")
}

fn artifacts(directory: &TempDir) -> std::path::PathBuf {
    storage(directory).join("artifacts")
}

fn write_orphan(directory: &TempDir, value: &ArtifactManifest, bytes: &[u8]) {
    fs::write(
        artifacts(directory).join(value.artifact_digest.as_str()),
        bytes,
    )
    .expect("write orphan fixture");
}

fn request(value: &ArtifactManifest) -> ArtifactOrphanReconciliationRequest {
    ArtifactOrphanReconciliationRequest {
        manifest: value.clone(),
    }
}

#[test]
fn registers_no_manifest_orphan_and_retries_idempotently() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    let mut progress = Vec::new();
    let first = service
        .reconcile(&request(&value), &CancellationToken::new(), |event| {
            progress.push(event);
        })
        .expect("register selected orphan");
    assert_eq!(
        first.disposition,
        ArtifactReconciliationDisposition::Registered
    );
    assert_eq!(first.installed.artifact_digest, value.artifact_digest);
    assert_eq!(
        progress.iter().map(|event| event.stage).collect::<Vec<_>>(),
        vec![
            ArtifactOrphanReconciliationStage::InspectingSelection,
            ArtifactOrphanReconciliationStage::VerifyingOrphan,
            ArtifactOrphanReconciliationStage::VerifyingOrphan,
        ]
    );
    let callbacks = progress.len();
    let repeated = service
        .reconcile(&request(&value), &CancellationToken::new(), |_| {})
        .expect("repeat exact reconciliation");
    assert_eq!(
        repeated.disposition,
        ArtifactReconciliationDisposition::AlreadyRegistered
    );
    assert_eq!(progress.len(), callbacks);
    drop(service);

    let states = store.artifact_inventory(8).expect("inspect state");
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].manifest, value);
    assert_eq!(
        states[0].installed.as_ref().map(|value| &value.installed),
        Some(&first.installed)
    );
    assert!(states[0].active_bindings.is_empty());
}

#[test]
fn completes_manifest_only_state_without_changing_bytes() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    assert_eq!(
        store.put_manifest(&value).expect("store manifest"),
        WriteDisposition::Inserted
    );
    let original =
        fs::read(artifacts(&directory).join(value.artifact_digest.as_str())).expect("read orphan");
    let result = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation")
    .reconcile(&request(&value), &CancellationToken::new(), |_| {})
    .expect("complete manifest-only state");
    assert_eq!(
        result.disposition,
        ArtifactReconciliationDisposition::Registered
    );
    assert_eq!(
        fs::read(artifacts(&directory).join(value.artifact_digest.as_str()))
            .expect("read unchanged orphan"),
        original
    );
}

#[test]
fn open_existing_creates_nothing_and_does_not_clean_staging() {
    let directory = tempdir().expect("temporary directory");
    let missing = directory.path().join("missing");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    assert!(matches!(
        ArtifactOrphanReconciliationService::open_existing(&missing, &mut store, limits()),
        Err(ArtifactReconciliationError::StorageNotInitialized)
    ));
    assert!(!missing.exists());

    let (directory, mut store) = initialize();
    let stale = storage(&directory).join(".staging/.import-stale");
    fs::write(&stale, b"stale").expect("write stale staging file");
    let _service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    assert_eq!(
        fs::read(stale).expect("staging remains untouched"),
        b"stale"
    );
}

#[test]
fn open_existing_does_not_recreate_missing_managed_boundaries() {
    for missing in [".artifact-import.lock", "artifacts"] {
        let (directory, mut store) = initialize();
        let path = storage(&directory).join(missing);
        if path.is_dir() {
            fs::remove_dir(&path).expect("remove empty managed directory");
        } else {
            fs::remove_file(&path).expect("remove managed lock file");
        }
        assert!(matches!(
            ArtifactOrphanReconciliationService::open_existing(
                storage(&directory),
                &mut store,
                limits()
            ),
            Err(ArtifactReconciliationError::StorageNotInitialized)
        ));
        assert!(!path.exists());
    }
}

#[test]
fn rejects_invalid_limits_manifest_and_byte_ceiling() {
    let (directory, mut store) = initialize();
    let mut invalid_limits = limits();
    invalid_limits.maximum_storage_entries = 0;
    assert!(matches!(
        ArtifactOrphanReconciliationService::open_existing(
            storage(&directory),
            &mut store,
            invalid_limits
        ),
        Err(ArtifactReconciliationError::InvalidLimits)
    ));

    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        ArtifactReconciliationLimits {
            maximum_artifact_bytes: 1,
            maximum_storage_entries: 32,
        },
    )
    .expect("open bounded reconciliation");
    let value = manifest(BYTES);
    assert!(matches!(
        service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::ArtifactTooLarge { .. })
    ));
    let mut invalid = value;
    invalid.artifact_id = ArtifactId::from_digest(Digest::sha256(b"other"));
    assert!(matches!(
        service.reconcile(&request(&invalid), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::InvalidManifest(_))
    ));
}

#[test]
fn rejects_missing_size_digest_and_nonregular_targets_without_state() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    {
        let mut service = ArtifactOrphanReconciliationService::open_existing(
            storage(&directory),
            &mut store,
            limits(),
        )
        .expect("open reconciliation");
        assert!(matches!(
            service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
            Err(ArtifactReconciliationError::OrphanNotFound)
        ));
    }
    write_orphan(&directory, &value, b"wrong");
    {
        let mut service = ArtifactOrphanReconciliationService::open_existing(
            storage(&directory),
            &mut store,
            limits(),
        )
        .expect("open reconciliation");
        assert!(matches!(
            service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
            Err(ArtifactReconciliationError::StorageConflict)
        ));
    }
    fs::write(
        artifacts(&directory).join(value.artifact_digest.as_str()),
        vec![b'x'; BYTES.len()],
    )
    .expect("write same-size conflict");
    {
        let mut service = ArtifactOrphanReconciliationService::open_existing(
            storage(&directory),
            &mut store,
            limits(),
        )
        .expect("open reconciliation");
        assert!(matches!(
            service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
            Err(ArtifactReconciliationError::StorageConflict)
        ));
    }
    fs::remove_file(artifacts(&directory).join(value.artifact_digest.as_str()))
        .expect("remove conflict");
    fs::create_dir(artifacts(&directory).join(value.artifact_digest.as_str()))
        .expect("create directory target");
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    assert!(matches!(
        service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::StorageChanged)
    ));
    drop(service);
    assert!(
        store
            .artifact_inventory(8)
            .expect("inspect state")
            .is_empty()
    );
}

#[test]
fn rejects_external_hard_link_alias_and_entry_overflow() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    fs::hard_link(
        artifacts(&directory).join(value.artifact_digest.as_str()),
        directory.path().join("external-alias"),
    )
    .expect("create hard-link alias");
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    assert!(matches!(
        service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::StorageChanged)
    ));
    drop(service);
    fs::remove_file(directory.path().join("external-alias")).expect("remove alias");
    fs::write(artifacts(&directory).join("unrelated"), b"x").expect("write extra entry");
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        ArtifactReconciliationLimits {
            maximum_artifact_bytes: 4_096,
            maximum_storage_entries: 1,
        },
    )
    .expect("open bounded reconciliation");
    assert!(matches!(
        service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::StorageEntryLimitExceeded)
    ));
}

#[test]
fn exact_target_at_entry_ceiling_succeeds() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let result = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        ArtifactReconciliationLimits {
            maximum_artifact_bytes: 4_096,
            maximum_storage_entries: 1,
        },
    )
    .expect("open bounded reconciliation")
    .reconcile(&request(&value), &CancellationToken::new(), |_| {})
    .expect("target at exact entry ceiling");
    assert_eq!(
        result.disposition,
        ArtifactReconciliationDisposition::Registered
    );
}

#[test]
fn cancellation_and_last_byte_mutation_fail_or_are_blocked() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let cancellation = CancellationToken::new();
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    let result = service.reconcile(&request(&value), &cancellation, |event| {
        if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan
            && event.completed_bytes == event.total_bytes
        {
            cancellation.cancel();
        }
    });
    assert!(matches!(
        result,
        Err(ArtifactReconciliationError::Cancelled)
    ));
    drop(service);
    assert!(
        store
            .artifact_inventory(8)
            .expect("inspect state")
            .is_empty()
    );

    let cancellation = CancellationToken::new();
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("reopen reconciliation");
    let target = artifacts(&directory).join(value.artifact_digest.as_str());
    let mut mutation_blocked = false;
    let result = service.reconcile(&request(&value), &cancellation, |event| {
        if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan
            && event.completed_bytes == event.total_bytes
            && let Err(error) = fs::write(&target, b"changed during verification")
        {
            mutation_blocked = error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(32);
            assert!(mutation_blocked, "unexpected mutation failure: {error}");
        }
    });
    if mutation_blocked {
        result.expect("reconcile after operating system blocks mutation");
    } else {
        assert!(matches!(
            result,
            Err(ArtifactReconciliationError::StorageConflict
                | ArtifactReconciliationError::StorageChanged
                | ArtifactReconciliationError::StorageIo(_))
        ));
    }
}

#[test]
fn valid_immutable_state_conflict_does_not_touch_orphan() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let mut conflicting = value.clone();
    conflicting.source.origin = "fixture/conflicting-origin".to_owned();
    store
        .put_manifest(&conflicting)
        .expect("store valid conflicting manifest");
    let original =
        fs::read(artifacts(&directory).join(value.artifact_digest.as_str())).expect("read orphan");
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    assert!(matches!(
        service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::StateConflict)
    ));
    assert_eq!(
        fs::read(artifacts(&directory).join(value.artifact_digest.as_str()))
            .expect("read untouched orphan"),
        original
    );
}

#[test]
fn synchronization_failures_leave_orphan_and_state_unchanged() {
    for failure in [
        ExactArtifactSync::FailFile,
        ExactArtifactSync::FailDirectory,
    ] {
        let (directory, mut store) = initialize();
        let value = manifest(BYTES);
        write_orphan(&directory, &value, BYTES);
        let target = artifacts(&directory).join(value.artifact_digest.as_str());
        let original = fs::read(&target).expect("read orphan fixture");
        let mut service = ArtifactOrphanReconciliationService::open_existing(
            storage(&directory),
            &mut store,
            limits(),
        )
        .expect("open reconciliation");
        service.inject_sync_failure(failure);
        assert!(matches!(
            service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
            Err(ArtifactReconciliationError::StorageIo(_))
        ));
        drop(service);
        assert_eq!(fs::read(target).expect("read unchanged orphan"), original);
        assert!(
            store
                .artifact_inventory(8)
                .expect("inspect state")
                .is_empty()
        );
    }
}
