use super::*;

mod native;

#[test]
fn archive_readback_refuses_corruption_and_replaced_output_identity() {
    use std::io::Write as _;

    let temporary = tempfile::tempdir().expect("output directory");
    let parent = File::open(temporary.path()).expect("held output directory");
    let mut output = snapshot::create_output(&parent, "output.tar").expect("exclusive output");
    output
        .write_all(b"canonical archive")
        .expect("archive bytes");
    let expected = (Sha256::digest(b"canonical archive").into(), 17);
    verify_readback(&parent, "output.tar", &mut output, expected).expect("exact readback");
    output.rewind().expect("rewind");
    output.write_all(b"corrupted archive").expect("corruption");
    assert!(verify_readback(&parent, "output.tar", &mut output, expected).is_err());
    output.rewind().expect("rewind");
    output
        .write_all(b"canonical archive")
        .expect("restore held bytes");
    std::fs::rename(
        temporary.path().join("output.tar"),
        temporary.path().join("retained.tar"),
    )
    .expect("move retained inode");
    std::fs::write(temporary.path().join("output.tar"), b"canonical archive")
        .expect("same-byte replacement");
    assert!(verify_readback(&parent, "output.tar", &mut output, expected).is_err());
}

#[test]
fn writable_sources_are_refused_before_any_archive_creation() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let root = File::open(temporary.path()).expect("root descriptor");
    let destination = temporary.path().join("output.tar");
    assert!(serialize_root(&root, "retonr-source", &destination).is_err());
    assert!(!destination.exists());
}

#[test]
fn canonical_header_fixes_identity_mode_time_and_size() {
    let value = header(tar::EntryType::Regular, 0o644, 7).expect("canonical header");
    assert_eq!(value.uid().expect("uid"), 0);
    assert_eq!(value.gid().expect("gid"), 0);
    assert_eq!(value.mode().expect("mode"), 0o644);
    assert_eq!(value.mtime().expect("mtime"), SOURCE_DATE_EPOCH);
    assert_eq!(value.size().expect("size"), 7);
    assert_eq!(value.username().expect("username"), Some(""));
    assert_eq!(value.groupname().expect("groupname"), Some(""));
}
