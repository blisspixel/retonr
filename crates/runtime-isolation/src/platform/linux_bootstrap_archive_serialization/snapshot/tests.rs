use super::*;

#[test]
fn snapshots_reject_nonregular_aliased_and_escaping_inputs() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = File::open(temporary.path()).expect("root descriptor");
    std::fs::write(temporary.path().join("file"), b"payload").expect("file");
    let initial = inventory(&root).expect("numeric snapshot");
    std::fs::write(temporary.path().join("file"), b"changed").expect("changed bytes");
    assert_ne!(inventory(&root).expect("changed snapshot"), initial);
    std::fs::hard_link(
        temporary.path().join("file"),
        temporary.path().join("alias"),
    )
    .expect("hard link");
    assert!(inventory(&root).is_err());
    std::fs::remove_file(temporary.path().join("alias")).expect("remove hard link");
    std::os::unix::fs::symlink("file", temporary.path().join("link")).expect("symlink");
    assert!(inventory(&root).is_err());
    for path in [
        "../file",
        "/file",
        "file/../file",
        "",
        "bad\\path",
        "./file",
        "file//child",
    ] {
        assert!(open_relative(&root, path).is_err());
    }
}

#[test]
fn actual_read_only_capability_refuses_writable_source_roots() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = File::open(temporary.path()).expect("root descriptor");
    assert!(Snapshot::acquire(&root).is_err());
}

#[test]
fn anchored_output_refuses_existing_members_and_path_components() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = File::open(temporary.path()).expect("root descriptor");
    create_output(&root, "archive.tar").expect("new file");
    assert!(create_output(&root, "archive.tar").is_err());
    assert!(create_output(&root, "nested/archive.tar").is_err());
    assert!(create_output(&root, "../archive.tar").is_err());
}
