use super::*;

#[test]
fn cancelled_publication_cleans_the_exact_synced_tree() {
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
    staging
        .ensure_directory(&path("weights/shard"))
        .expect("create nested staging directories");
    let mut file = staging
        .create_file(&path("weights/shard/model.bin"))
        .expect("create staged file");
    file.file.write_all(b"model").expect("write staged file");
    drop(file);
    staging
        .sync_bottom_up(&CancellationToken::new())
        .expect("sync staging tree");
    let synced = staging.into_synced().expect("take sync proof");
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        synced.publish_no_replace(
            &final_parent,
            std::ffi::OsStr::new("set-root"),
            2,
            &cancellation
        ),
        Err(ArtifactInventoryError::Cancelled)
    ));
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read staging parent")
            .count(),
        0
    );
    assert_eq!(
        fs::read_dir(&final_path)
            .expect("read final parent")
            .count(),
        0
    );
}

#[test]
fn closed_publication_failure_cleans_nested_tree_after_timestamp_changes() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let staging_parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let mut staging =
        OwnedStagingTree::create(&staging_parent, limits(4), 2, &CancellationToken::new())
            .expect("create staging tree");
    staging
        .ensure_directory(&path("weights/shard"))
        .expect("create nested staging directories");
    let mut file = staging
        .create_file(&path("weights/shard/model.bin"))
        .expect("create staged file");
    file.file.write_all(b"model").expect("write staged file");
    drop(file);
    staging
        .sync_bottom_up(&CancellationToken::new())
        .expect("sync staging tree");

    staging
        .into_synced()
        .expect("take sync proof")
        .cleanup_after_closed_publication_failure()
        .expect("clean exact closed-handle ledger");
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read staging parent")
            .count(),
        0
    );
}
