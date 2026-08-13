use std::fs;

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::{ArtifactRemovalPhase, ArtifactStateStore, StoreError};
use rewrite_types::{CancellationToken, Digest};
use tempfile::TempDir;

use super::{
    ArtifactRemovalDisposition, ArtifactRemovalError, ArtifactRemovalLimits,
    ArtifactRemovalRequest, ArtifactRemovalService, ArtifactRemovalStage, ArtifactRemovalTestFault,
};

fn manifest(bytes: &[u8]) -> ArtifactManifest {
    let digest = Digest::sha256(bytes);
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(digest.clone()),
        source: ArtifactSource {
            origin: "fixture/model".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: digest,
        byte_size: u64::try_from(bytes.len()).expect("fixture size"),
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
    }
}

fn installed(manifest: &ArtifactManifest) -> InstalledArtifact {
    InstalledArtifact {
        artifact_id: manifest.artifact_id.clone(),
        artifact_digest: manifest.artifact_digest.clone(),
        byte_size: manifest.byte_size,
        storage_key: format!("artifacts/{}", manifest.artifact_digest.as_str()),
    }
}

fn limits() -> ArtifactRemovalLimits {
    ArtifactRemovalLimits {
        maximum_artifact_bytes: 1024,
        maximum_storage_entries: 8,
    }
}

fn initialized(bytes: &[u8]) -> (TempDir, ArtifactStateStore, ArtifactRemovalRequest) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("managed");
    fs::create_dir(&root).expect("create root");
    fs::create_dir(root.join("artifacts")).expect("create artifacts");
    fs::write(root.join(".artifact-import.lock"), []).expect("create lock");
    let value = manifest(bytes);
    fs::write(
        root.join("artifacts").join(value.artifact_digest.as_str()),
        bytes,
    )
    .expect("write managed bytes");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("open state");
    let selection = store
        .put_installation(&value, &installed(&value))
        .expect("register installation")
        .installation;
    (directory, store, ArtifactRemovalRequest { selection })
}

#[test]
fn removes_exact_inactive_bytes_and_retries_without_touching_reinstall() {
    let bytes = b"artifact";
    let (directory, mut store, request) = initialized(bytes);
    let root = directory.path().join("managed");
    let canonical = root
        .join("artifacts")
        .join(request.selection.installed.artifact_digest.as_str());
    let mut progress = Vec::new();
    let result = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open removal")
        .remove(&request, &CancellationToken::new(), |value| {
            progress.push(value);
        })
        .expect("remove artifact");
    assert_eq!(result.disposition, ArtifactRemovalDisposition::Removed);
    assert!(!canonical.exists());
    assert_eq!(
        progress.iter().map(|value| value.stage).collect::<Vec<_>>(),
        vec![
            ArtifactRemovalStage::InspectingSelection,
            ArtifactRemovalStage::VerifyingInactiveBytes,
            ArtifactRemovalStage::VerifyingInactiveBytes,
            ArtifactRemovalStage::PreparingRemoval,
        ]
    );
    let state = store
        .artifact_removal_state(&request.selection.installed.artifact_id)
        .expect("inspect completed state");
    assert_eq!(state.0, None);
    assert_eq!(
        state.1.as_ref().map(|value| value.phase),
        Some(ArtifactRemovalPhase::Completed)
    );

    fs::write(&canonical, bytes).expect("restore bytes for deliberate reinstall");
    let manifest = manifest(bytes);
    let reinstalled = store
        .put_installation(&manifest, &installed(&manifest))
        .expect("reinstall")
        .installation;
    assert!(reinstalled.epoch > request.selection.epoch);
    let retry = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open old retry")
        .remove(&request, &CancellationToken::new(), |_| {})
        .expect("old retry is complete");
    assert_eq!(
        retry.disposition,
        ArtifactRemovalDisposition::AlreadyRemoved
    );
    assert_eq!(fs::read(&canonical).expect("new bytes remain"), bytes);
}

#[test]
fn a_reinstalled_generation_can_be_removed_with_a_new_selection() {
    let bytes = b"artifact";
    let (directory, mut store, first_request) = initialized(bytes);
    let root = directory.path().join("managed");
    let canonical = root
        .join("artifacts")
        .join(first_request.selection.installed.artifact_digest.as_str());
    ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open first removal")
        .remove(&first_request, &CancellationToken::new(), |_| {})
        .expect("remove first generation");
    fs::write(&canonical, bytes).expect("restore exact bytes");
    let value = manifest(bytes);
    let second_selection = store
        .put_installation(&value, &installed(&value))
        .expect("register second generation")
        .installation;
    let second_request = ArtifactRemovalRequest {
        selection: second_selection,
    };
    let result = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open second removal")
        .remove(&second_request, &CancellationToken::new(), |_| {})
        .expect("remove second generation");
    assert_eq!(result.disposition, ArtifactRemovalDisposition::Removed);
    assert!(!canonical.exists());
}

#[test]
fn resumes_prepared_removal_after_state_reopen_with_present_or_missing_bytes() {
    for remove_before_retry in [false, true] {
        let (directory, mut store, request) = initialized(b"artifact");
        let root = directory.path().join("managed");
        let canonical = root
            .join("artifacts")
            .join(request.selection.installed.artifact_digest.as_str());
        crate::artifact_storage::test_support::prepare_artifact_removal(
            &root,
            &mut store,
            &request.selection,
        )
        .expect("simulate crash after preparation");
        drop(store);
        if remove_before_retry {
            fs::remove_file(&canonical).expect("simulate crash after unlink");
        }
        let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
            .expect("reopen durable state");
        let result = ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open recovery")
            .remove(&request, &CancellationToken::new(), |_| {})
            .expect("recover removal");
        assert_eq!(result.disposition, ArtifactRemovalDisposition::Recovered);
        assert!(!canonical.exists());
    }
}

#[test]
fn lowered_byte_ceiling_cannot_misreport_prepared_recovery_as_fresh_refusal() {
    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    crate::artifact_storage::test_support::prepare_artifact_removal(
        &root,
        &mut store,
        &request.selection,
    )
    .expect("prepare removal under original ceiling");
    let error = ArtifactRemovalService::open_existing(
        &root,
        &mut store,
        ArtifactRemovalLimits {
            maximum_artifact_bytes: 1,
            maximum_storage_entries: 8,
        },
    )
    .expect("open recovery")
    .remove(&request, &CancellationToken::new(), |_| {})
    .expect_err("smaller recovery ceiling needs explicit recovery");
    assert!(matches!(error, ArtifactRemovalError::RecoveryRequired(_)));
}

#[test]
fn prepared_recovery_ignores_cancellation_and_emits_no_progress() {
    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    crate::artifact_storage::test_support::prepare_artifact_removal(
        &root,
        &mut store,
        &request.selection,
    )
    .expect("prepare removal");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let mut progress = Vec::new();
    let result = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open recovery")
        .remove(&request, &cancellation, |value| progress.push(value))
        .expect("recover despite stale cancellation");
    assert_eq!(result.disposition, ArtifactRemovalDisposition::Recovered);
    assert!(progress.is_empty());
}

#[test]
fn prepared_conflict_remains_recoverable_and_never_restores_authority() {
    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    let canonical = root
        .join("artifacts")
        .join(request.selection.installed.artifact_digest.as_str());
    crate::artifact_storage::test_support::prepare_artifact_removal(
        &root,
        &mut store,
        &request.selection,
    )
    .expect("prepare removal");
    fs::write(&canonical, b"conflict").expect("replace bytes with conflict");
    let error = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open recovery")
        .remove(&request, &CancellationToken::new(), |_| {})
        .expect_err("conflict requires explicit recovery");
    assert!(matches!(error, ArtifactRemovalError::RecoveryRequired(_)));
    let (installation, removal) = store
        .artifact_removal_state(&request.selection.installed.artifact_id)
        .expect("inspect pending state");
    assert_eq!(installation, None);
    assert_eq!(
        removal.map(|value| value.phase),
        Some(ArtifactRemovalPhase::Prepared)
    );
}

#[test]
fn rejects_missing_conflicting_aliased_and_nonregular_targets_before_preparation() {
    for mode in ["missing", "conflict", "directory"] {
        let (directory, mut store, request) = initialized(b"artifact");
        let root = directory.path().join("managed");
        let canonical = root
            .join("artifacts")
            .join(request.selection.installed.artifact_digest.as_str());
        fs::remove_file(&canonical).expect("remove exact bytes");
        match mode {
            "missing" => {}
            "conflict" => fs::write(&canonical, b"conflict").expect("write conflict"),
            "directory" => fs::create_dir(&canonical).expect("create directory"),
            _ => unreachable!(),
        }
        let error = ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open removal")
            .remove(&request, &CancellationToken::new(), |_| {})
            .expect_err("unsafe target must fail");
        assert!(matches!(
            (mode, error),
            ("missing", ArtifactRemovalError::BytesMissing)
                | ("conflict", ArtifactRemovalError::StorageConflict)
                | ("directory", ArtifactRemovalError::StorageChanged)
        ));
        assert_eq!(
            store
                .artifact_removal_state(&request.selection.installed.artifact_id)
                .expect("state remains installed")
                .0,
            Some(request.selection)
        );
    }

    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    let canonical = root
        .join("artifacts")
        .join(request.selection.installed.artifact_digest.as_str());
    let alias = directory.path().join("external-alias");
    fs::hard_link(&canonical, &alias).expect("create hard-link alias");
    assert!(matches!(
        ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open removal")
            .remove(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactRemovalError::StorageChanged)
    ));
    assert!(canonical.exists());
    assert!(alias.exists());
}

#[test]
fn cancellation_and_callback_mutation_leave_installed_state() {
    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    let token = CancellationToken::new();
    let callback_token = token.clone();
    let error = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open removal")
        .remove(&request, &token, |value| {
            if value.stage == ArtifactRemovalStage::PreparingRemoval {
                callback_token.cancel();
            }
        })
        .expect_err("callback cancellation must win");
    assert!(matches!(error, ArtifactRemovalError::Cancelled));
    assert_eq!(
        store
            .artifact_removal_state(&request.selection.installed.artifact_id)
            .expect("state remains")
            .0,
        Some(request.selection)
    );
}

#[test]
fn final_callback_hard_link_mutation_fails_before_preparation() {
    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    let canonical = root
        .join("artifacts")
        .join(request.selection.installed.artifact_digest.as_str());
    let alias = directory.path().join("callback-alias");
    let error = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open removal")
        .remove(&request, &CancellationToken::new(), |value| {
            if value.stage == ArtifactRemovalStage::PreparingRemoval {
                fs::hard_link(&canonical, &alias).expect("create callback alias");
            }
        })
        .expect_err("final alias must fail before preparation");
    assert!(matches!(error, ArtifactRemovalError::StorageChanged));
    assert_eq!(
        store
            .artifact_removal_state(&request.selection.installed.artifact_id)
            .expect("state remains installed")
            .0,
        Some(request.selection)
    );
    assert!(canonical.exists());
    assert!(alias.exists());
}

#[test]
fn final_callback_unlink_and_replacement_fail_before_preparation() {
    for replace in [false, true] {
        let bytes = b"artifact";
        let (directory, mut store, request) = initialized(bytes);
        let root = directory.path().join("managed");
        let canonical = root
            .join("artifacts")
            .join(request.selection.installed.artifact_digest.as_str());
        let error = ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open removal")
            .remove(&request, &CancellationToken::new(), |value| {
                if value.stage == ArtifactRemovalStage::PreparingRemoval {
                    fs::remove_file(&canonical).expect("unlink callback target");
                    if replace {
                        fs::write(&canonical, bytes).expect("write callback replacement");
                    }
                }
            })
            .expect_err("final namespace mutation must fail before preparation");
        assert!(matches!(error, ArtifactRemovalError::StorageChanged));
        assert_eq!(
            store
                .artifact_removal_state(&request.selection.installed.artifact_id)
                .expect("state remains installed")
                .0,
            Some(request.selection)
        );
        assert_eq!(canonical.exists(), replace);
    }
}

#[test]
fn every_post_preparation_failure_is_retryable() {
    for fault in [
        ArtifactRemovalTestFault::BeforeUnlink,
        ArtifactRemovalTestFault::BeforeDirectorySync,
        ArtifactRemovalTestFault::BeforeLayoutRecheck,
        ArtifactRemovalTestFault::BeforeCompletion,
    ] {
        let (directory, mut store, request) = initialized(b"artifact");
        let root = directory.path().join("managed");
        let canonical = root
            .join("artifacts")
            .join(request.selection.installed.artifact_digest.as_str());
        let mut service = ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open removal");
        service.fault = fault;
        assert!(matches!(
            service.remove(&request, &CancellationToken::new(), |_| {}),
            Err(ArtifactRemovalError::RecoveryRequired(_))
        ));
        drop(service);
        let (installation, removal) = store
            .artifact_removal_state(&request.selection.installed.artifact_id)
            .expect("inspect prepared state");
        assert_eq!(installation, None);
        assert_eq!(
            removal.map(|value| value.phase),
            Some(ArtifactRemovalPhase::Prepared)
        );
        assert_eq!(
            canonical.exists(),
            fault == ArtifactRemovalTestFault::BeforeUnlink
        );
        let recovered = ArtifactRemovalService::open_existing(&root, &mut store, limits())
            .expect("open recovery")
            .remove(&request, &CancellationToken::new(), |_| {})
            .expect("retry completes prepared removal");
        assert_eq!(recovered.disposition, ArtifactRemovalDisposition::Recovered);
        assert!(!canonical.exists());
    }
}

#[test]
fn rejects_invalid_limits_selection_and_byte_ceiling() {
    let (directory, mut store, mut request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    assert!(matches!(
        ArtifactRemovalService::open_existing(
            &root,
            &mut store,
            ArtifactRemovalLimits {
                maximum_artifact_bytes: 0,
                maximum_storage_entries: 1,
            }
        ),
        Err(ArtifactRemovalError::InvalidLimits)
    ));
    request.selection.installed.storage_key = "../outside".to_owned();
    let error = ArtifactRemovalService::open_existing(&root, &mut store, limits())
        .expect("open removal")
        .remove(&request, &CancellationToken::new(), |_| {})
        .expect_err("invalid selection");
    assert!(matches!(error, ArtifactRemovalError::InvalidSelection));

    let (directory, mut store, request) = initialized(b"artifact");
    let root = directory.path().join("managed");
    let error = ArtifactRemovalService::open_existing(
        &root,
        &mut store,
        ArtifactRemovalLimits {
            maximum_artifact_bytes: 1,
            maximum_storage_entries: 8,
        },
    )
    .expect("open removal")
    .remove(&request, &CancellationToken::new(), |_| {})
    .expect_err("byte ceiling must fail");
    assert!(matches!(
        error,
        ArtifactRemovalError::ArtifactTooLarge { .. }
    ));
}

#[test]
fn pending_state_blocks_reconciliation_registration() {
    let (directory, mut store, request) = initialized(b"artifact");
    crate::artifact_storage::test_support::prepare_artifact_removal(
        &directory.path().join("managed"),
        &mut store,
        &request.selection,
    )
    .expect("prepare removal");
    assert!(matches!(
        store.put_installation(&manifest(b"artifact"), &request.selection.installed),
        Err(StoreError::RemovalPending)
    ));
    let _ = directory;
}
