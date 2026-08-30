use super::*;

#[test]
fn publication_preflight_fails_cleanly_when_atomic_rename_is_unavailable() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    super::super::platform::inject_no_replace_rename_failure_once();

    assert!(matches!(
        OwnedStagingTree::preflight_no_replace_publication(
            &parent,
            &parent,
            limits(4),
            2,
            2,
            &CancellationToken::new()
        ),
        Err(ArtifactInventoryError::StorageIo(ref error))
            if error.kind() == std::io::ErrorKind::InvalidInput
    ));
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read staging parent after failed preflight")
            .count(),
        0
    );
}

#[test]
fn preserving_preflight_retains_primary_and_cleanup_failures() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    super::super::platform::inject_no_replace_rename_failure_once();
    super::super::staging::inject_closed_ledger_cleanup_failure_once();

    let Err(failure) = OwnedStagingTree::preflight_no_replace_publication_preserving_cleanup(
        &parent,
        &parent,
        limits(4),
        2,
        2,
        &CancellationToken::new(),
    ) else {
        panic!("injected probe failures must fail");
    };
    let (primary, cleanup, committed) = failure.into_parts();
    assert!(matches!(
        primary,
        ArtifactInventoryError::StorageIo(ref error)
            if error.kind() == std::io::ErrorKind::InvalidInput
    ));
    assert!(matches!(
        cleanup,
        Some(ArtifactInventoryError::StorageIo(_))
    ));
    assert!(!committed);
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read retained failed probe")
            .count(),
        1,
    );
}

#[test]
fn publication_preflight_removes_its_successful_probe() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");

    OwnedStagingTree::preflight_no_replace_publication(
        &parent,
        &parent,
        limits(4),
        2,
        2,
        &CancellationToken::new(),
    )
    .expect("preflight no-replace publication");
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read staging parent after successful preflight")
            .count(),
        0
    );
}

#[test]
fn same_parent_publication_at_the_exact_entry_limit_does_not_require_extra_capacity() {
    let fixture = tempdir().expect("temporary directory");
    let parent = PinnedDirectory::open_existing(fixture.path()).expect("pin shared parent");
    let mut staging = OwnedStagingTree::create(&parent, limits(1), 1, &CancellationToken::new())
        .expect("create the only allowed parent entry");
    staging
        .sync_bottom_up(&CancellationToken::new())
        .expect("sync empty staging tree");
    let published = staging
        .into_synced()
        .expect("take sync proof")
        .publish_no_replace(
            &parent,
            std::ffi::OsStr::new("published"),
            1,
            &CancellationToken::new(),
        )
        .expect("same-parent rename preserves the exact entry count");
    assert!(fixture.path().join("published").is_dir());
    remove_verified_managed_tree(
        &parent,
        std::ffi::OsStr::new("published"),
        published,
        limits(1),
        1,
    )
    .expect("remove exact published probe");
}

#[test]
fn publication_never_replaces_an_existing_destination() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    let final_path = fixture.path().join("sets");
    fs::create_dir(&staging_path).expect("create staging parent");
    fs::create_dir_all(final_path.join("set-root")).expect("create destination");
    fs::write(final_path.join("set-root/existing"), b"existing").expect("write destination");
    let staging_parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let final_parent = PinnedDirectory::open_existing(&final_path).expect("pin final parent");
    let mut staging =
        OwnedStagingTree::create(&staging_parent, limits(4), 2, &CancellationToken::new())
            .expect("create staging tree");
    staging
        .sync_bottom_up(&CancellationToken::new())
        .expect("sync staging tree");
    let synced = staging.into_synced().expect("take sync proof");

    assert!(matches!(
        synced.publish_no_replace(
            &final_parent,
            std::ffi::OsStr::new("set-root"),
            2,
            &CancellationToken::new()
        ),
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
    assert_eq!(
        fs::read(final_path.join("set-root/existing")).expect("read destination"),
        b"existing"
    );
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read staging parent")
            .count(),
        0
    );
}

#[test]
fn platform_no_replace_rename_preserves_every_existing_destination_kind() {
    #[derive(Clone, Copy)]
    enum DestinationKind {
        EmptyDirectory,
        NonemptyDirectory,
        RegularFile,
    }

    for (case_name, destination_kind) in [
        ("empty-directory", DestinationKind::EmptyDirectory),
        ("nonempty-directory", DestinationKind::NonemptyDirectory),
        ("regular-file", DestinationKind::RegularFile),
    ] {
        let fixture = tempdir().expect("temporary directory");
        let source_parent_path = fixture.path().join("staging");
        let destination_parent_path = fixture.path().join("published");
        let source_path = source_parent_path.join("source");
        let destination_path = destination_parent_path.join("destination");
        fs::create_dir_all(&source_path).expect("create source directory");
        fs::create_dir(&destination_parent_path).expect("create destination parent");
        fs::write(source_path.join("source-marker"), b"source").expect("write source marker");
        match destination_kind {
            DestinationKind::EmptyDirectory => {
                fs::create_dir(&destination_path).expect("create empty destination directory");
            }
            DestinationKind::NonemptyDirectory => {
                fs::create_dir(&destination_path).expect("create destination directory");
                fs::write(destination_path.join("destination-marker"), b"destination")
                    .expect("write destination marker");
            }
            DestinationKind::RegularFile => {
                fs::write(&destination_path, b"destination").expect("write destination file");
            }
        }
        let source_parent =
            PinnedDirectory::open_existing(&source_parent_path).expect("pin source parent");
        let destination_parent = PinnedDirectory::open_existing(&destination_parent_path)
            .expect("pin destination parent");

        let result = super::super::platform::rename_directory_no_replace(
            &source_parent.handle,
            std::ffi::OsStr::new("source"),
            &destination_parent.handle,
            std::ffi::OsStr::new("destination"),
        );

        assert!(result.is_err(), "destination case {case_name} was replaced");
        assert_eq!(
            fs::read(source_path.join("source-marker")).expect("read preserved source"),
            b"source",
            "source changed for destination case {case_name}"
        );
        match destination_kind {
            DestinationKind::EmptyDirectory => {
                assert!(destination_path.is_dir());
                assert_eq!(
                    fs::read_dir(&destination_path)
                        .expect("read empty destination")
                        .count(),
                    0
                );
            }
            DestinationKind::NonemptyDirectory => assert_eq!(
                fs::read(destination_path.join("destination-marker"))
                    .expect("read destination marker"),
                b"destination"
            ),
            DestinationKind::RegularFile => assert_eq!(
                fs::read(&destination_path).expect("read destination file"),
                b"destination"
            ),
        }
    }
}
