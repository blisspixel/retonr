use super::*;

const fn migration_limits() -> ArtifactRepositoryMigrationLimits {
    ArtifactRepositoryMigrationLimits {
        maximum_state_bytes: 16 * 1024 * 1024,
        maximum_repository_entries: 64,
    }
}

#[test]
fn current_repository_migration_is_an_exact_no_op() {
    let (_directory, repository, _imported) = imported_repository();
    let before = directory_snapshot(&repository.data_directory);
    let result = repository
        .migrate(migration_limits(), &CancellationToken::new())
        .expect("inspect current repository");
    assert_eq!(
        result.disposition,
        ArtifactRepositoryMigrationDisposition::AlreadyCurrent
    );
    assert_eq!(result.from_schema, result.to_schema);
    assert!(result.backup_key.is_none());
    assert_eq!(directory_snapshot(&repository.data_directory), before);
}

#[test]
fn schema_two_migration_retains_a_verified_backup_and_restores_commands() {
    let (_directory, repository, imported) = imported_repository();
    downgrade_to_schema_two(&repository);
    assert!(matches!(
        repository.inventory(inventory_limits(), &CancellationToken::new()),
        Err(ArtifactRepositoryError::State(
            rewrite_model_store::StoreError::MigrationRequired {
                found: 2,
                current: 6
            }
        ))
    ));

    let result = repository
        .migrate(migration_limits(), &CancellationToken::new())
        .expect("migrate schema two");
    assert_eq!(
        result.disposition,
        ArtifactRepositoryMigrationDisposition::Migrated
    );
    assert_eq!((result.from_schema, result.to_schema), (2, 6));
    let backup_key = result.backup_key.expect("migration retains backup");
    let backup = repository.data_directory.join(backup_key.as_str());
    assert_eq!(
        ArtifactStateStore::inspect_existing_schema(&backup)
            .expect("inspect retained backup")
            .found,
        2
    );
    let report = repository
        .inventory(inventory_limits(), &CancellationToken::new())
        .expect("inventory migrated repository");
    assert_eq!(
        report.registered[0].installation.artifact_id(),
        imported.key.artifact_id()
    );
}

#[test]
fn schema_three_migration_retains_a_verified_backup() {
    let (_directory, repository, _imported) = imported_repository();
    downgrade_to_schema_three(&repository);

    let result = repository
        .migrate(migration_limits(), &CancellationToken::new())
        .expect("migrate schema three");
    assert_eq!(
        result.disposition,
        ArtifactRepositoryMigrationDisposition::Migrated
    );
    assert_eq!((result.from_schema, result.to_schema), (3, 6));
    let backup_key = result.backup_key.expect("migration retains backup");
    let backup = repository.data_directory.join(backup_key.as_str());
    assert_eq!(
        ArtifactStateStore::inspect_existing_schema(&backup)
            .expect("inspect retained backup")
            .found,
        3
    );
}

#[test]
fn migration_cancellation_and_limits_do_not_change_legacy_state() {
    let (_directory, repository, _imported) = imported_repository();
    downgrade_to_schema_two(&repository);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        repository.migrate(migration_limits(), &cancelled),
        Err(ArtifactRepositoryError::Cancelled)
    ));
    assert_eq!(
        ArtifactStateStore::inspect_existing_schema(&repository.state_database())
            .expect("legacy state remains valid")
            .found,
        2
    );
    assert!(matches!(
        repository.migrate(
            ArtifactRepositoryMigrationLimits {
                maximum_state_bytes: 1,
                maximum_repository_entries: 64,
            },
            &CancellationToken::new(),
        ),
        Err(ArtifactRepositoryError::MigrationLimitExceeded)
    ));
    assert_eq!(
        ArtifactStateStore::inspect_existing_schema(&repository.state_database())
            .expect("limit refusal preserves legacy state")
            .found,
        2
    );
    let entry_limit_error = repository
        .migrate(
            ArtifactRepositoryMigrationLimits {
                maximum_state_bytes: 16 * 1024 * 1024,
                maximum_repository_entries: 1,
            },
            &CancellationToken::new(),
        )
        .expect_err("repository entry limit must prevent backup reservation");
    assert_eq!(
        entry_limit_error.kind(),
        ArtifactRepositoryErrorKind::ResourceLimit
    );
    assert_eq!(
        ArtifactStateStore::inspect_existing_schema(&repository.state_database())
            .expect("entry-limit refusal preserves legacy state")
            .found,
        2
    );
}

#[test]
fn migration_rejects_missing_invalid_busy_and_aliased_boundaries() {
    let directory = tempdir().expect("temporary directory");
    let missing = ArtifactRepository::new(directory.path().join("missing"))
        .expect("derive missing repository");
    assert!(matches!(
        missing.migrate(migration_limits(), &CancellationToken::new()),
        Err(ArtifactRepositoryError::NotInitialized)
    ));

    let (_directory, repository, _imported) = imported_repository();
    assert!(matches!(
        repository.migrate(
            ArtifactRepositoryMigrationLimits {
                maximum_state_bytes: 0,
                maximum_repository_entries: 64,
            },
            &CancellationToken::new(),
        ),
        Err(ArtifactRepositoryError::InvalidLimits)
    ));
    let guard = repository
        .pin_data_directory(RepositoryLockMode::ExistingShared)
        .expect("hold shared repository lock");
    assert!(matches!(
        repository.migrate(migration_limits(), &CancellationToken::new()),
        Err(ArtifactRepositoryError::RepositoryInUse)
    ));
    drop(guard);

    let alias = repository.data_directory.join("state-alias.sqlite3");
    fs::hard_link(repository.state_database(), &alias).expect("create state alias");
    assert!(matches!(
        repository.migrate(migration_limits(), &CancellationToken::new()),
        Err(ArtifactRepositoryError::UnsafeDataDirectory)
    ));
    fs::remove_file(alias).expect("remove state alias");
}

#[test]
fn corrupt_legacy_state_is_refused_before_backup_reservation() {
    let (_directory, repository, _imported) = imported_repository();
    downgrade_to_schema_two(&repository);
    let connection =
        rusqlite::Connection::open(repository.state_database()).expect("open legacy fixture");
    connection
        .execute("CREATE TABLE unexpected_state(value TEXT) STRICT", [])
        .expect("corrupt legacy shape");
    drop(connection);
    let before = directory_snapshot(&repository.data_directory);
    assert!(matches!(
        repository.migrate(migration_limits(), &CancellationToken::new()),
        Err(ArtifactRepositoryError::State(
            rewrite_model_store::StoreError::CorruptRecord
        ))
    ));
    assert_eq!(directory_snapshot(&repository.data_directory), before);
}

#[test]
fn post_backup_failure_reports_the_opaque_backup_key_and_source_kind() {
    let backup_key = ArtifactRepositoryBackupKey::from_file_name(
        ".artifact-state-backup-00000000000000000000000000000000".to_owned(),
    );
    let error = ArtifactRepositoryError::MigrationFailed {
        backup_key: backup_key.clone(),
        source: Box::new(ArtifactRepositoryError::MigrationLimitExceeded),
    };
    assert_eq!(error.migration_backup_key(), Some(&backup_key),);
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::ResourceLimit);
}

#[test]
fn live_runtime_lease_blocks_repository_migration() {
    let (_directory, repository, imported) = imported_repository();
    let store = ArtifactStateStore::open_existing_read_only(&repository.state_database())
        .expect("open current state");
    let (selection, _) = store
        .artifact_removal_state(imported.key.artifact_id())
        .expect("read installation");
    let lease = crate::RuntimeArtifactLease::acquire(
        repository.managed_storage(),
        &store,
        selection.expect("current installation"),
        crate::RuntimeArtifactLeaseLimits {
            maximum_artifact_bytes: 1024 * 1024,
            maximum_storage_entries: 64,
        },
        &CancellationToken::new(),
    )
    .expect("acquire runtime lease");
    assert!(matches!(
        repository.migrate(migration_limits(), &CancellationToken::new()),
        Err(ArtifactRepositoryError::RepositoryInUse)
    ));
    drop(lease);
}

fn downgrade_to_schema_two(repository: &ArtifactRepository) {
    let connection = rusqlite::Connection::open(repository.state_database())
        .expect("open fixture database for downgrade");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE native_load_observations;
             DROP TABLE model_package_manifests;
             DROP TABLE runtime_package_manifests;
             DROP TABLE artifact_set_removals;
             DROP TABLE installed_artifact_sets;
             DROP TABLE qualification_v2_records;
             DROP TABLE effective_package_evidence;
             DROP TABLE effective_runtime_states;
             DROP TABLE runtime_build_identities;
             DROP TABLE artifact_set_manifests;
             PRAGMA user_version = 2;",
        )
        .expect("downgrade fixture to exact schema two");
}

fn downgrade_to_schema_three(repository: &ArtifactRepository) {
    let connection = rusqlite::Connection::open(repository.state_database())
        .expect("open fixture database for downgrade");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE native_load_observations;
             DROP TABLE model_package_manifests;
             DROP TABLE runtime_package_manifests;
             DROP TABLE artifact_set_removals;
             DROP TABLE installed_artifact_sets;
             PRAGMA user_version = 3;",
        )
        .expect("downgrade fixture to exact schema three");
}
