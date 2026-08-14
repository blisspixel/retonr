use std::{ffi::OsString, fs, io::Read as _, io::Write as _};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::{
    ArtifactInventoryError, ManagedTreeEntryKind, ManagedTreeLimits, OwnedStagingTree,
    PinnedDirectory,
};

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value.to_owned()).expect("valid fixture path")
}

fn limits(maximum_entries: usize) -> ManagedTreeLimits {
    ManagedTreeLimits::new(maximum_entries).expect("valid limits")
}

#[test]
fn source_tree_enumeration_and_relative_open_are_exact() {
    let fixture = tempdir().expect("temporary directory");
    fs::create_dir_all(fixture.path().join("weights/shard")).expect("create fixture tree");
    fs::write(fixture.path().join("config.json"), b"config").expect("write config");
    fs::write(fixture.path().join("weights/shard/model.bin"), b"weights").expect("write weights");
    let root = PinnedDirectory::open_existing(fixture.path()).expect("pin source root");

    let snapshot = root
        .enumerate_tree(limits(8), &CancellationToken::new())
        .expect("enumerate source");
    let observed = snapshot
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.relative_path().as_str(),
                entry.kind(),
                entry.byte_size(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("config.json", ManagedTreeEntryKind::RegularFile, 6),
            ("weights", ManagedTreeEntryKind::Directory, 0),
            ("weights/shard", ManagedTreeEntryKind::Directory, 0),
            (
                "weights/shard/model.bin",
                ManagedTreeEntryKind::RegularFile,
                7,
            ),
        ]
    );

    let mut opened = root
        .open_relative_regular_file(&path("weights/shard/model.bin"))
        .expect("open exact relative file");
    let mut bytes = Vec::new();
    opened.file.read_to_end(&mut bytes).expect("read file");
    assert_eq!(bytes, b"weights");
    root.recheck_relative_regular_file(&path("weights/shard/model.bin"), &opened.fingerprint)
        .expect("recheck exact file");
}

#[test]
fn source_hardlinks_are_reported_without_rejecting_read_only_input() {
    let fixture = tempdir().expect("temporary directory");
    fs::write(fixture.path().join("first.bin"), b"same").expect("write fixture");
    fs::hard_link(
        fixture.path().join("first.bin"),
        fixture.path().join("second.bin"),
    )
    .expect("create hard link");
    let root = PinnedDirectory::open_existing(fixture.path()).expect("pin source root");

    let snapshot = root
        .enumerate_tree(limits(2), &CancellationToken::new())
        .expect("source hardlinks remain readable");
    assert_eq!(snapshot.entries().len(), 2);
    assert!(
        snapshot
            .entries()
            .iter()
            .all(|entry| !entry.has_single_link())
    );
}

#[test]
fn tree_entry_limit_is_fail_closed() {
    let fixture = tempdir().expect("temporary directory");
    fs::write(fixture.path().join("one"), b"1").expect("write one");
    fs::write(fixture.path().join("two"), b"2").expect("write two");
    let root = PinnedDirectory::open_existing(fixture.path()).expect("pin source root");

    assert!(matches!(
        root.enumerate_tree(limits(1), &CancellationToken::new()),
        Err(ArtifactInventoryError::StorageEntryLimitExceeded)
    ));
}

#[test]
fn cancelled_tree_enumeration_returns_no_partial_snapshot() {
    let fixture = tempdir().expect("temporary directory");
    fs::write(fixture.path().join("one"), b"1").expect("write one");
    let root = PinnedDirectory::open_existing(fixture.path()).expect("pin source root");
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        root.enumerate_tree(limits(2), &cancellation),
        Err(ArtifactInventoryError::Cancelled)
    ));
}

#[cfg(unix)]
#[test]
fn indirect_source_descendant_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = tempdir().expect("temporary directory");
    fs::write(fixture.path().join("outside"), b"outside").expect("write target");
    symlink(
        fixture.path().join("outside"),
        fixture.path().join("linked"),
    )
    .expect("create symlink");
    let root = PinnedDirectory::open_existing(fixture.path()).expect("pin source root");

    assert!(matches!(
        root.enumerate_tree(limits(4), &CancellationToken::new()),
        Err(ArtifactInventoryError::UnsafeStorageLayout)
    ));
}

#[test]
fn staging_directory_creation_never_adopts_an_existing_entry() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let name = OsString::from(".set-import-fixed");
    let mut staging = OwnedStagingTree::create_with(&parent, limits(8), || Ok(name.clone()))
        .expect("create staging tree");
    fs::create_dir(staging_path.join(&name).join("injected")).expect("inject directory");

    assert!(staging.ensure_directory(&path("injected")).is_err());
}

#[test]
fn post_create_failures_remove_unledgered_entries() {
    let fixture = tempdir().expect("temporary directory");
    let parent = PinnedDirectory::open_existing(fixture.path()).expect("pin parent");

    assert!(
        super::staging::create_retained_directory_with_failure(
            &parent,
            std::ffi::OsStr::new("unledgered-directory"),
        )
        .is_err()
    );
    assert_eq!(
        fs::read_dir(fixture.path())
            .expect("read parent after directory failure")
            .count(),
        0
    );
    assert!(
        super::staging::create_retained_file_with_failure(
            &parent,
            std::ffi::OsStr::new("unledgered-file"),
            4,
        )
        .is_err()
    );
    assert_eq!(
        fs::read_dir(fixture.path()).expect("read parent").count(),
        0
    );
}

#[test]
fn staging_file_creation_is_no_replace_and_exposes_link_policy() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let mut staging = OwnedStagingTree::create(&parent, limits(8), 2, &CancellationToken::new())
        .expect("create staging tree");
    staging
        .ensure_directory(&path("weights/shard"))
        .expect("create directories");
    let mut created = staging
        .create_file(&path("weights/shard/model.bin"))
        .expect("create file");
    created.file.write_all(b"model").expect("write file");
    assert!(created.fingerprint.has_single_link());
    drop(created);

    assert!(
        staging
            .create_file(&path("weights/shard/model.bin"))
            .is_err()
    );
}

#[test]
fn nested_tree_sync_and_no_replace_publication_are_exact() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    let final_path = fixture.path().join("sets");
    fs::create_dir(&staging_path).expect("create staging parent");
    fs::create_dir(&final_path).expect("create final parent");
    let staging_parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let final_parent = PinnedDirectory::open_existing(&final_path).expect("pin final parent");
    let mut staging =
        OwnedStagingTree::create(&staging_parent, limits(8), 2, &CancellationToken::new())
            .expect("create staging tree");
    staging
        .ensure_directory(&path("weights/shard"))
        .expect("create directories");
    let mut file = staging
        .create_file(&path("weights/shard/model.bin"))
        .expect("create file");
    file.file.write_all(b"model").expect("write model");
    drop(file);
    let mut config = staging
        .create_file(&path("config.json"))
        .expect("create config");
    config.file.write_all(b"config").expect("write config");
    drop(config);

    staging
        .sync_bottom_up(&CancellationToken::new())
        .expect("sync exact tree");
    let synced = staging.into_synced().expect("take sync proof");
    assert_eq!(
        synced
            .root()
            .enumerate_tree(limits(8), &CancellationToken::new())
            .expect("inspect sync proof")
            .entries()
            .len(),
        4
    );
    let published = synced
        .publish_no_replace(
            &final_parent,
            std::ffi::OsStr::new("set-root"),
            2,
            &CancellationToken::new(),
        )
        .expect("publish exact tree");
    let snapshot = published
        .enumerate_tree(limits(8), &CancellationToken::new())
        .expect("enumerate published tree");
    assert_eq!(snapshot.entries().len(), 4);
    let mut opened = published
        .open_relative_regular_file(&path("weights/shard/model.bin"))
        .expect("open published file");
    let mut bytes = Vec::new();
    opened.file.read_to_end(&mut bytes).expect("read model");
    assert_eq!(bytes, b"model");
    assert!(
        fs::read_dir(&staging_path)
            .expect("read staging parent")
            .next()
            .is_none()
    );
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
fn cancellation_preserves_exact_cleanup_authority() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let mut staging = OwnedStagingTree::create(&parent, limits(4), 2, &CancellationToken::new())
        .expect("create staging tree");
    let mut file = staging
        .create_file(&path("model.bin"))
        .expect("create model");
    file.file.write_all(b"model").expect("write model");
    drop(file);
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        staging.sync_bottom_up(&cancellation),
        Err(ArtifactInventoryError::Cancelled)
    ));
    staging.cleanup().expect("clean exact cancelled tree");
    assert_eq!(
        fs::read_dir(&staging_path)
            .expect("read staging parent")
            .count(),
        0
    );
}

#[test]
fn cleanup_refuses_unexpected_entries_without_deleting_ledger_bytes() {
    let fixture = tempdir().expect("temporary directory");
    let staging_path = fixture.path().join("staging");
    fs::create_dir(&staging_path).expect("create staging parent");
    let parent = PinnedDirectory::open_existing(&staging_path).expect("pin staging parent");
    let name = OsString::from(".set-import-fixed");
    let staging = OwnedStagingTree::create_with(&parent, limits(4), || Ok(name.clone()))
        .expect("create staging tree");
    let mut file = staging
        .create_file(&path("model.bin"))
        .expect("create model");
    file.file.write_all(b"model").expect("write model");
    drop(file);
    fs::write(staging_path.join(&name).join("unexpected"), b"unexpected")
        .expect("inject unexpected file");

    assert!(staging.cleanup().is_err());
    assert_eq!(
        fs::read(staging_path.join(&name).join("model.bin")).expect("read ledger file"),
        b"model"
    );
    assert_eq!(
        fs::read(staging_path.join(&name).join("unexpected")).expect("read unexpected file"),
        b"unexpected"
    );
}

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
