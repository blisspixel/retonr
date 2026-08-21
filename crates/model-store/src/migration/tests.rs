use std::{
    cell::Cell,
    fs::{self, File, OpenOptions},
    path::Path,
    time::Duration,
};

use rewrite_model::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord,
};
use rewrite_types::Digest;
use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::{StoreMigrationDisposition, StoreSchemaStatus};
use crate::{ArtifactStateStore, StoreError};

#[path = "tests/schema_five_packages.rs"]
mod schema_five_packages;
#[path = "tests/schema_three_evidence.rs"]
mod schema_three_evidence;

#[test]
fn inspection_accepts_each_supported_schema_without_mutation() {
    let directory = tempdir().expect("temporary directory");
    for version in 1..=6 {
        let path = directory.path().join(format!("schema-{version}.db"));
        create_schema(&path, version);
        let before = fs::read(&path).expect("read before inspection");
        assert_eq!(
            ArtifactStateStore::inspect_existing_schema(&path).expect("inspect supported schema"),
            StoreSchemaStatus {
                found: version,
                current: 6,
            }
        );
        assert_eq!(fs::read(&path).expect("read after inspection"), before);
    }
}

#[test]
fn compatibility_opens_require_the_explicit_session_for_every_older_schema() {
    let directory = tempdir().expect("temporary directory");
    for version in 1..6 {
        let path = directory
            .path()
            .join(format!("compatibility-schema-{version}.db"));
        create_schema(&path, version);
        let before = fs::read(&path).expect("read older schema before compatibility opens");
        for result in [
            ArtifactStateStore::open(&path),
            ArtifactStateStore::open_or_create_and_migrate(&path),
            ArtifactStateStore::open_existing_and_migrate(&path),
        ] {
            assert!(matches!(
                result,
                Err(StoreError::MigrationRequired { found, current: 6 }) if found == i64::from(version)
            ));
            assert_eq!(
                fs::read(&path).expect("reread rejected older schema"),
                before
            );
            assert_eq!(schema_version(&path), i64::from(version));
        }
    }
}

#[test]
fn session_migrates_v1_through_current_v6_after_verified_backup() {
    let directory = tempdir().expect("temporary directory");
    for version in 1..=6 {
        let source = directory.path().join(format!("source-{version}.db"));
        let backup = directory.path().join(format!("backup-{version}.db"));
        create_schema(&source, version);
        let mut backup_file = reserve_file(&backup);
        let mut session = ArtifactStateStore::begin_existing_migration(&source)
            .expect("begin supported migration");
        assert_eq!(session.schema_status().found, version);
        session
            .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
            .expect("write verified backup");
        let result = session.migrate().expect("migrate supported schema");
        assert_eq!((result.from_schema, result.to_schema), (version, 6));
        assert_eq!(
            result.disposition,
            if version == 6 {
                StoreMigrationDisposition::AlreadyCurrent
            } else {
                StoreMigrationDisposition::Migrated
            }
        );
        assert_eq!(schema_version(&source), 6);
        assert_eq!(schema_version(&backup), i64::from(version));
    }
}

#[test]
fn session_writes_only_the_exact_reserved_handle_after_path_replacement() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.db");
    let original = directory.path().join("reserved.db");
    let moved = directory.path().join("moved-reserved.db");
    create_schema(&source, 2);
    let mut reserved = reserve_file(&original);
    fs::rename(&original, &moved).expect("move caller-held destination");
    fs::write(&original, b"replacement sentinel").expect("replace old path");

    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin migration");
    session
        .backup_to(&mut reserved, 16 * 1024 * 1024, || false)
        .expect("back up through reserved handle");

    assert_eq!(
        fs::read(&original).expect("read replacement"),
        b"replacement sentinel"
    );
    assert_eq!(schema_version(&moved), 2);
    session.migrate().expect("commit migration");
}

#[test]
fn session_rejects_a_hardlinked_destination_without_writing() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.db");
    let backup = directory.path().join("backup.db");
    let alias = directory.path().join("backup-alias.db");
    create_schema(&source, 2);
    let mut backup_file = reserve_file(&backup);
    fs::hard_link(&backup, &alias).expect("create backup alias");
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin migration");

    assert!(matches!(
        session.backup_to(&mut backup_file, 16 * 1024 * 1024, || false),
        Err(StoreError::InvalidBackupDestination)
    ));
    assert_eq!(fs::metadata(&backup).expect("backup metadata").len(), 0);
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 2);
}

#[test]
fn failed_backup_attempt_clears_prior_session_authority() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.db");
    let first = directory.path().join("first-backup.db");
    let invalid = directory.path().join("invalid-backup.db");
    let invalid_alias = directory.path().join("invalid-backup-alias.db");
    create_schema(&source, 2);
    let mut first_file = reserve_file(&first);
    let mut invalid_file = reserve_file(&invalid);
    fs::hard_link(&invalid, &invalid_alias).expect("create invalid backup alias");
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin migration");
    session
        .backup_to(&mut first_file, 16 * 1024 * 1024, || false)
        .expect("complete first backup");

    assert!(matches!(
        session.backup_to(&mut invalid_file, 16 * 1024 * 1024, || false),
        Err(StoreError::InvalidBackupDestination)
    ));
    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 2);
}

#[test]
fn cancellation_leaves_source_unchanged_and_drop_releases_reservation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("large-source.db");
    let backup = directory.path().join("partial-backup.db");
    create_large_schema(&source, 2);
    let mut backup_file = reserve_file(&backup);
    let checks = Cell::new(0u32);
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin migration");

    assert!(matches!(
        session.backup_to(&mut backup_file, 16 * 1024 * 1024, || {
            let next = checks.get().saturating_add(1);
            checks.set(next);
            next >= 4
        }),
        Err(StoreError::BackupCancelled)
    ));
    assert!(checks.get() >= 4);
    drop(session);
    assert_eq!(schema_version(&source), 2);
    Connection::open(&source)
        .expect("open writer after rollback")
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .expect("reservation released on drop");
}

#[test]
fn migrate_without_backup_fails_and_rolls_back() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.db");
    create_schema(&source, 2);
    let session = ArtifactStateStore::begin_existing_migration(&source).expect("begin migration");

    assert!(matches!(session.migrate(), Err(StoreError::BackupRequired)));
    assert_eq!(schema_version(&source), 2);
}

#[test]
fn competing_sqlite_writer_cannot_cross_the_session_reservation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.db");
    let backup = directory.path().join("backup.db");
    create_schema(&source, 2);
    let mut backup_file = reserve_file(&backup);
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin migration");
    let competitor = Connection::open(&source).expect("open competing connection");
    competitor
        .busy_timeout(Duration::ZERO)
        .expect("disable competitor wait");
    assert!(competitor.execute_batch("BEGIN IMMEDIATE").is_err());

    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("back up while reservation remains live");
    assert!(competitor.execute_batch("BEGIN IMMEDIATE").is_err());
    session.migrate().expect("commit migration");
    competitor
        .execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        .expect("writer proceeds after commit");
}

#[test]
fn wal_resident_state_is_included_in_the_v2_backup_and_v4_migration() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("wal-source.db");
    let backup = directory.path().join("wal-backup.db");
    create_schema(&source, 2);
    let manifest = wal_manifest();
    let manifest_key = manifest.artifact_id.digest().as_str().to_owned();
    let record = crate::record::encode_record(&manifest).expect("encode valid manifest");
    let writer = Connection::open(&source).expect("open WAL writer");
    writer
        .pragma_update(None, "journal_mode", "WAL")
        .expect("enable WAL mode");
    writer
        .pragma_update(None, "wal_autocheckpoint", 0)
        .expect("disable automatic checkpoint");
    writer
        .execute(
            "INSERT INTO artifact_manifests (artifact_id, record_json) VALUES (?1, ?2)",
            params![manifest_key, record],
        )
        .expect("commit valid manifest into WAL");
    let wal_path = source.with_extension("db-wal");
    let main_bytes = fs::read(&source).expect("read main database");
    let wal_bytes = fs::read(&wal_path).expect("read live WAL");
    assert!(!contains_bytes(&main_bytes, manifest_key.as_bytes()));
    assert!(contains_bytes(&wal_bytes, manifest_key.as_bytes()));

    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin migration over WAL snapshot");
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("back up complete logical WAL state");
    session.migrate().expect("migrate WAL source");

    assert_eq!(schema_version(&backup), 2);
    let backup_record: String = Connection::open(&backup)
        .expect("open retained backup")
        .query_row(
            "SELECT record_json FROM artifact_manifests WHERE artifact_id = ?1",
            [&manifest_key],
            |row| row.get(0),
        )
        .expect("load manifest from retained backup");
    let recovered: ArtifactManifest =
        crate::record::decode_record(&backup_record).expect("decode retained manifest");
    assert_eq!(recovered, manifest);
    let source_store =
        ArtifactStateStore::open_existing_read_only(&source).expect("open migrated source at v4");
    assert_eq!(
        source_store
            .manifest(&manifest.artifact_id)
            .expect("load migrated manifest"),
        Some(manifest)
    );
    drop(writer);
}

#[test]
fn byte_limit_and_initial_cancellation_write_nothing() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.db");
    create_schema(&source, 2);
    let required = fs::metadata(&source).expect("source metadata").len();

    let too_small = directory.path().join("too-small.db");
    let mut too_small_file = reserve_file(&too_small);
    let mut session =
        ArtifactStateStore::begin_existing_migration(&source).expect("begin limited migration");
    assert!(matches!(
        session.backup_to(&mut too_small_file, required.saturating_sub(1), || false),
        Err(StoreError::BackupTooLarge)
    ));
    assert_eq!(fs::metadata(&too_small).expect("backup metadata").len(), 0);

    let cancelled = directory.path().join("cancelled.db");
    let mut cancelled_file = reserve_file(&cancelled);
    assert!(matches!(
        session.backup_to(&mut cancelled_file, required, || true),
        Err(StoreError::BackupCancelled)
    ));
    assert_eq!(fs::metadata(&cancelled).expect("backup metadata").len(), 0);
}

#[test]
fn corrupt_zero_future_and_missing_state_never_start_a_session() {
    let directory = tempdir().expect("temporary directory");
    let corrupt = directory.path().join("corrupt-v2.db");
    create_schema(&corrupt, 2);
    Connection::open(&corrupt)
        .expect("open corrupt fixture")
        .execute_batch("CREATE TABLE unexpected_state(value TEXT) STRICT;")
        .expect("corrupt exact shape");
    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&corrupt),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&corrupt), 2);

    let zero = directory.path().join("zero.db");
    drop(Connection::open(&zero).expect("create empty database"));
    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&zero),
        Err(StoreError::MigrationRequired {
            found: 0,
            current: 6
        })
    ));

    let future = directory.path().join("future.db");
    Connection::open(&future)
        .expect("create future fixture")
        .pragma_update(None, "user_version", 7)
        .expect("set future version");
    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&future),
        Err(StoreError::UnsupportedSchema(7))
    ));

    let missing = directory.path().join("missing.db");
    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&missing),
        Err(StoreError::NotInitialized)
    ));
    assert!(!missing.exists());
}

#[test]
fn foreign_key_corruption_is_rejected_before_session_authority() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("foreign-key-corrupt.db");
    create_schema(&source, 3);
    let connection = Connection::open(&source).expect("open fixture");
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable fixture foreign-key enforcement");
    connection
        .execute(
            "INSERT INTO installed_artifacts
                 (artifact_id, installation_epoch, record_json)
             VALUES (?1, 1, '{}')",
            ["b".repeat(64)],
        )
        .expect("insert foreign-key violation");
    drop(connection);

    assert!(matches!(
        ArtifactStateStore::begin_existing_migration(&source),
        Err(StoreError::CorruptRecord)
    ));
    assert_eq!(schema_version(&source), 3);
}

fn reserve_file(path: &Path) -> File {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
        .expect("reserve backup file")
}

fn create_schema(path: &Path, version: u32) {
    let connection = Connection::open(path).expect("create schema fixture");
    match version {
        1 => crate::schema::create_schema_one_fixture(&connection).expect("create schema one"),
        2 => crate::schema::create_schema_two_fixture(&connection).expect("create schema two"),
        3 => {
            crate::schema::create_schema_three_fixture(&connection).expect("create schema three");
        }
        4 => {
            crate::schema::create_schema_four_fixture(&connection).expect("create schema four");
        }
        5 => {
            crate::schema::create_schema_five_fixture(&connection).expect("create schema five");
        }
        6 => {
            drop(connection);
            drop(
                ArtifactStateStore::open_existing_or_initialize_empty(path)
                    .expect("create current schema"),
            );
        }
        _ => panic!("unsupported test schema"),
    }
}

fn create_large_schema(path: &Path, version: u32) {
    create_schema(path, version);
    Connection::open(path)
        .expect("open large fixture")
        .execute(
            "INSERT INTO artifact_manifests (artifact_id, record_json) VALUES (?1, ?2)",
            ["a".repeat(64), "x".repeat(900_000)],
        )
        .expect("inflate database beyond one write chunk");
}

fn wal_manifest() -> ArtifactManifest {
    let artifact_digest = Digest::sha256(b"wal-resident-artifact");
    ArtifactManifest {
        schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(artifact_digest.clone()),
        source: ArtifactSource {
            origin: "fixture/wal-model".to_owned(),
            revision: "revision-1".to_owned(),
        },
        artifact_digest,
        byte_size: 21,
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

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn schema_version(path: &Path) -> i64 {
    Connection::open(path)
        .expect("open schema fixture")
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version")
}
