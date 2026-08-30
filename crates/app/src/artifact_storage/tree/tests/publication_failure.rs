use super::*;

#[test]
fn preserving_publication_reports_primary_and_cleanup_without_deleting_suspicious_tree() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    let final_path = fixture.path().join("sets");
    fs::create_dir(&staging_path).expect("create staging parent");
    fs::create_dir(&final_path).expect("create final parent");
    let staging_parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let final_parent = PinnedDirectory::open_existing(&final_path).expect("pin final parent");
    let mut staging =
        OwnedStagingTree::create(&staging_parent, limits(4), 2, &CancellationToken::new())
            .expect("create staging tree");
    let mut file = staging
        .create_file(&path("model.bin"))
        .expect("create staged file");
    file.file.write_all(b"model").expect("write staged file");
    drop(file);
    staging
        .sync_bottom_up(&CancellationToken::new())
        .expect("sync staging tree");
    let synced = staging.into_synced().expect("take sync proof");
    synced
        .root()
        .create_child_directory_exclusive(std::ffi::OsStr::new("unexpected"))
        .expect("inject suspicious entry");
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let Err(failure) = synced.publish_no_replace_preserving_cleanup(
        &final_parent,
        std::ffi::OsStr::new("set-root"),
        2,
        &cancellation,
    ) else {
        panic!("publication must fail");
    };
    let (primary, cleanup, committed) = failure.into_parts();
    assert!(matches!(primary, ArtifactInventoryError::Cancelled));
    assert!(matches!(
        cleanup,
        Some(ArtifactInventoryError::ConcurrentModification)
    ));
    assert!(!committed);
    let retained = fs::read_dir(&staging_path)
        .expect("suspicious staging root is retained")
        .collect::<Result<Vec<_>, _>>()
        .expect("enumerate retained staging root");
    assert_eq!(retained.len(), 1);
    assert_eq!(
        fs::read(retained[0].path().join("model.bin")).expect("read retained ledger file"),
        b"model"
    );
    assert!(retained[0].path().join("unexpected").is_dir());
    assert_eq!(
        fs::read_dir(&final_path)
            .expect("read final parent")
            .count(),
        0
    );
}
