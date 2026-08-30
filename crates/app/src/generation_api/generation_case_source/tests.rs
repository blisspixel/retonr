use std::fs;
use std::path::PathBuf;

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, GenerationCaseManifestV1, GenerationCaseManifestV1Input,
    GenerationClusterRecordV1, InstalledArtifact, LicenseRecord,
};
use rewrite_model_store::{ArtifactStateStore, StoredArtifactInstallation};
use rewrite_types::{CancellationToken, Digest};
use tempfile::TempDir;

use super::{
    GenerationCaseSourceLease, GenerationCaseSourceLeaseError, GenerationCaseSourceLeaseLimits,
    MAX_GENERATION_CASE_SOURCE_BYTES, MAX_GENERATION_CASE_SOURCE_STORAGE_ENTRIES,
};
use crate::{
    ArtifactInventoryError,
    artifact_storage::{ExistingArtifactStorage, LifecycleLockMode},
};

const SOURCE: &[u8] = b"Retain Acme 42 exactly.";

struct Fixture {
    _directory: TempDir,
    root: PathBuf,
    database: PathBuf,
    canonical: PathBuf,
    store: ArtifactStateStore,
    selection: StoredArtifactInstallation,
    case: GenerationCaseManifestV1,
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn limits() -> GenerationCaseSourceLeaseLimits {
    GenerationCaseSourceLeaseLimits {
        maximum_source_bytes: 1_024,
        maximum_storage_entries: 8,
    }
}

fn build_fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("managed");
    let database = directory.path().join("state.sqlite3");
    fs::create_dir(&root).expect("create managed root");
    fs::create_dir(root.join("artifacts")).expect("create artifact directory");
    fs::write(root.join(".artifact-import.lock"), []).expect("create lifecycle lock");
    let source_digest = Digest::sha256(SOURCE);
    let canonical = root.join("artifacts").join(source_digest.as_str());
    fs::write(&canonical, SOURCE).expect("write source artifact");
    let artifact_id = ArtifactId::from_digest(source_digest.clone());
    let manifest = ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: artifact_id.clone(),
        source: ArtifactSource {
            origin: "fixture/generation-case".to_owned(),
            revision: "fixture-revision".to_owned(),
        },
        artifact_digest: source_digest.clone(),
        byte_size: SOURCE.len() as u64,
        format: "utf8".to_owned(),
        family: "generation-case-source".to_owned(),
        architecture: None,
        quantization: None,
        tokenizer: None,
        licenses: vec![LicenseRecord {
            component: "source".to_owned(),
            identifier: "CC0-1.0".to_owned(),
            text_digest: digest("license"),
        }],
        declared_capabilities: DeclaredCapabilities {
            roles: vec![ArtifactRole::Generation],
            languages: vec!["en".to_owned()],
            context_tokens: None,
        },
    };
    let installed = InstalledArtifact {
        artifact_id: artifact_id.clone(),
        artifact_digest: source_digest.clone(),
        byte_size: SOURCE.len() as u64,
        storage_key: format!("artifacts/{}", source_digest.as_str()),
    };
    let mut store = ArtifactStateStore::open(&database).expect("open artifact state");
    let selection = store
        .put_installation(&manifest, &installed)
        .expect("store source installation")
        .installation;
    let cluster = GenerationClusterRecordV1::new("literal", digest("cluster policy"))
        .expect("cluster is valid");
    let case = GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: "protected-literal".to_owned(),
            source_artifact_id: artifact_id,
            source_digest,
            source_byte_count: SOURCE.len() as u64,
            case_contract_digest: digest("case contract"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("case is valid");
    Fixture {
        _directory: directory,
        root,
        database,
        canonical,
        store,
        selection,
        case,
    }
}

fn acquire(fixture: &Fixture) -> GenerationCaseSourceLease<'_> {
    GenerationCaseSourceLease::acquire(
        &fixture.root,
        &fixture.store,
        fixture.selection.clone(),
        &fixture.case,
        limits(),
        &CancellationToken::new(),
    )
    .expect("source lease acquires")
}

#[test]
fn exact_source_is_callback_scoped_and_fully_bound() {
    let fixture = build_fixture();
    let lease = acquire(&fixture);
    assert_eq!(lease.case_id(), fixture.case.case_id());
    assert_eq!(
        lease.source_artifact_id(),
        fixture.case.source_artifact_id()
    );
    assert_eq!(lease.source_digest(), fixture.case.source_digest());
    assert_eq!(lease.source_byte_count(), SOURCE.len() as u64);
    assert!(lease.matches_case_manifest(&fixture.case));
    let observed = lease
        .with_source_bytes(&CancellationToken::new(), |bytes| {
            assert_eq!(bytes, SOURCE);
            Digest::sha256(bytes)
        })
        .expect("source callback completes");
    assert_eq!(&observed, fixture.case.source_digest());
    lease
        .revalidate(&CancellationToken::new())
        .expect("lease remains valid");

    let debug = format!("{lease:?}");
    assert!(debug.contains("case_id"));
    assert!(!debug.contains("Retain Acme"));
    assert!(!debug.contains(fixture.root.to_string_lossy().as_ref()));
}

#[test]
fn rejects_limits_case_substitution_and_stale_installation() {
    let fixture = build_fixture();
    for invalid in [
        GenerationCaseSourceLeaseLimits {
            maximum_source_bytes: 0,
            ..limits()
        },
        GenerationCaseSourceLeaseLimits {
            maximum_source_bytes: MAX_GENERATION_CASE_SOURCE_BYTES + 1,
            ..limits()
        },
        GenerationCaseSourceLeaseLimits {
            maximum_storage_entries: 0,
            ..limits()
        },
        GenerationCaseSourceLeaseLimits {
            maximum_storage_entries: MAX_GENERATION_CASE_SOURCE_STORAGE_ENTRIES + 1,
            ..limits()
        },
    ] {
        assert!(matches!(
            GenerationCaseSourceLease::acquire(
                &fixture.root,
                &fixture.store,
                fixture.selection.clone(),
                &fixture.case,
                invalid,
                &CancellationToken::new(),
            ),
            Err(GenerationCaseSourceLeaseError::InvalidLimits)
        ));
    }

    let cluster = GenerationClusterRecordV1::new("literal", digest("cluster policy"))
        .expect("cluster is valid");
    let other_digest = digest("other source");
    let other_case = GenerationCaseManifestV1::new(
        &cluster,
        GenerationCaseManifestV1Input {
            case_key: "other-case".to_owned(),
            source_artifact_id: ArtifactId::from_digest(other_digest.clone()),
            source_digest: other_digest,
            source_byte_count: 12,
            case_contract_digest: digest("case contract"),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("other case is valid");
    assert!(matches!(
        GenerationCaseSourceLease::acquire(
            &fixture.root,
            &fixture.store,
            fixture.selection.clone(),
            &other_case,
            limits(),
            &CancellationToken::new(),
        ),
        Err(GenerationCaseSourceLeaseError::CaseRelationshipMismatch)
    ));

    let mut stale_fixture = build_fixture();
    crate::artifact_storage::test_support::prepare_artifact_removal(
        &stale_fixture.root,
        &mut stale_fixture.store,
        &stale_fixture.selection,
    )
    .expect("prepare source removal");
    assert!(matches!(
        GenerationCaseSourceLease::acquire(
            &stale_fixture.root,
            &stale_fixture.store,
            stale_fixture.selection.clone(),
            &stale_fixture.case,
            limits(),
            &CancellationToken::new(),
        ),
        Err(GenerationCaseSourceLeaseError::InstallationChanged)
    ));
}

#[test]
fn shared_lease_blocks_exclusive_lifecycle_work_and_hard_links() {
    let fixture = build_fixture();
    let lease = acquire(&fixture);
    assert!(matches!(
        ExistingArtifactStorage::open(&fixture.root, LifecycleLockMode::Exclusive),
        Err(ArtifactInventoryError::StorageInUse)
    ));
    drop(lease);
    ExistingArtifactStorage::open(&fixture.root, LifecycleLockMode::Exclusive)
        .expect("exclusive lifecycle opens after lease drop");

    let aliased = build_fixture();
    fs::hard_link(&aliased.canonical, aliased.root.join("source-alias"))
        .expect("create source alias");
    assert!(matches!(
        GenerationCaseSourceLease::acquire(
            &aliased.root,
            &aliased.store,
            aliased.selection.clone(),
            &aliased.case,
            limits(),
            &CancellationToken::new(),
        ),
        Err(GenerationCaseSourceLeaseError::SourceChanged)
    ));
}

#[test]
fn cancellation_and_tree_limits_fail_before_bytes_escape() {
    let fixture = build_fixture();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        GenerationCaseSourceLease::acquire(
            &fixture.root,
            &fixture.store,
            fixture.selection.clone(),
            &fixture.case,
            limits(),
            &cancelled,
        ),
        Err(GenerationCaseSourceLeaseError::Boundary(
            ArtifactInventoryError::Cancelled
        ))
    ));

    fs::write(fixture.root.join("artifacts").join("extra"), b"extra").expect("write extra entry");
    let limited = GenerationCaseSourceLeaseLimits {
        maximum_storage_entries: 1,
        ..limits()
    };
    assert!(matches!(
        GenerationCaseSourceLease::acquire(
            &fixture.root,
            &fixture.store,
            fixture.selection.clone(),
            &fixture.case,
            limited,
            &CancellationToken::new(),
        ),
        Err(GenerationCaseSourceLeaseError::Boundary(
            ArtifactInventoryError::StorageEntryLimitExceeded
        ))
    ));
}

#[test]
fn post_callback_state_readback_discards_the_result() {
    let fixture = build_fixture();
    let lease = acquire(&fixture);
    let result = lease.with_source_bytes(&CancellationToken::new(), |bytes| {
        assert_eq!(bytes, SOURCE);
        let connection = rusqlite::Connection::open(&fixture.database).expect("open raw state");
        connection
            .execute(
                "UPDATE installed_artifacts SET installation_epoch = installation_epoch + 1",
                [],
            )
            .expect("corrupt installation generation");
        42
    });
    assert!(matches!(
        result,
        Err(GenerationCaseSourceLeaseError::Boundary(
            ArtifactInventoryError::State(_)
        ))
    ));
}

#[cfg(unix)]
#[test]
fn same_size_content_drift_is_rejected_before_and_after_callback_use() {
    let fixture = build_fixture();
    let lease = acquire(&fixture);
    fs::write(&fixture.canonical, b"Retain Acme 43 exactly.").expect("mutate source");
    assert!(matches!(
        lease.revalidate(&CancellationToken::new()),
        Err(GenerationCaseSourceLeaseError::SourceChanged)
    ));

    let callback_fixture = build_fixture();
    let callback_lease = acquire(&callback_fixture);
    let result = callback_lease.with_source_bytes(&CancellationToken::new(), |bytes| {
        assert_eq!(bytes, SOURCE);
        fs::write(&callback_fixture.canonical, b"Retain Acme 43 exactly.")
            .expect("mutate source during callback");
        42
    });
    assert!(matches!(
        result,
        Err(GenerationCaseSourceLeaseError::SourceChanged)
    ));
}

#[cfg(unix)]
#[test]
fn indirect_source_entry_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = build_fixture();
    let target = fixture.root.join("outside-source");
    fs::write(&target, SOURCE).expect("write symlink target");
    fs::remove_file(&fixture.canonical).expect("remove canonical source");
    symlink(&target, &fixture.canonical).expect("replace source with symlink");
    assert!(matches!(
        GenerationCaseSourceLease::acquire(
            &fixture.root,
            &fixture.store,
            fixture.selection.clone(),
            &fixture.case,
            limits(),
            &CancellationToken::new(),
        ),
        Err(GenerationCaseSourceLeaseError::Boundary(
            ArtifactInventoryError::UnsafeStorageLayout
        ))
    ));
}
