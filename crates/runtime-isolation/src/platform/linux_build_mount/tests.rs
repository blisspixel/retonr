use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::FileExt as _,
    path::Path,
    time::{Duration, Instant},
};

use rewrite_types::Digest as DomainDigest;
use tempfile::{NamedTempFile, tempdir};

use super::*;

fn retained(root: &Path, relative: &str) -> ControlledBuildInputFile {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("directories");
    fs::write(&path, relative.as_bytes()).expect("file");
    ControlledBuildInputFile::new(
        relative,
        DomainDigest::sha256(relative.as_bytes()),
        u64::try_from(relative.len()).expect("length"),
        File::open(path).expect("open"),
    )
    .expect("retained")
}

#[test]
fn expected_tree_is_exact_and_rejects_file_directory_conflicts() {
    let temporary = tempdir().expect("temporary");
    let files = [
        retained(temporary.path(), "bin/tool"),
        retained(temporary.path(), "source/archive.tar"),
    ];
    assert_eq!(
        expected_tree(&files).expect("tree"),
        BTreeMap::from([
            ("bin".to_owned(), ExpectedKind::Directory),
            ("bin/tool".to_owned(), ExpectedKind::RegularFile),
            ("source".to_owned(), ExpectedKind::Directory),
            ("source/archive.tar".to_owned(), ExpectedKind::RegularFile),
        ])
    );
    let member_source = temporary.path().join("collision-member-source");
    fs::write(&member_source, b"collision/member").expect("member source");
    let conflict = [
        retained(temporary.path(), "collision"),
        ControlledBuildInputFile::new(
            "collision/member",
            DomainDigest::sha256(b"collision/member"),
            u64::try_from("collision/member".len()).expect("length"),
            File::open(member_source).expect("open member source"),
        )
        .expect("retained member"),
    ];
    assert_eq!(
        expected_tree(&conflict),
        Err(HelperFailure::ControlledBuildObjectMismatch)
    );
}

#[test]
fn completed_snapshot_is_independent_from_later_source_mutation() {
    let original = vec![b'a'; SNAPSHOT_BUFFER_BYTES + 17];
    let source = NamedTempFile::new().expect("source");
    fs::write(source.path(), &original).expect("write source");
    let retained = File::open(source.path()).expect("open retained source");
    let mut destination = tempfile::tempfile().expect("destination");
    copy_exact_bytes(
        &retained,
        &mut destination,
        u64::try_from(original.len()).expect("source length"),
        &DomainDigest::sha256(&original),
        Instant::now() + Duration::from_secs(1),
        |_| {},
    )
    .expect("copy exact bytes");

    fs::write(source.path(), vec![b'b'; original.len()]).expect("mutate source after copy");
    let mut observed = vec![0_u8; original.len()];
    assert_eq!(
        destination
            .read_at(&mut observed, 0)
            .expect("read destination"),
        original.len()
    );
    assert_eq!(observed, original);
}

#[test]
fn synchronized_during_copy_mutate_and_restore_fails_closed() {
    let original = vec![b'a'; SNAPSHOT_BUFFER_BYTES * 3];
    let source = NamedTempFile::new().expect("source");
    fs::write(source.path(), &original).expect("write source");
    let retained = File::open(source.path()).expect("open retained source");
    let writer = OpenOptions::new()
        .write(true)
        .open(source.path())
        .expect("open source writer");
    let mutated_chunk = vec![b'b'; SNAPSHOT_BUFFER_BYTES];
    let mut destination = tempfile::tempfile().expect("destination");
    let result = copy_exact_bytes(
        &retained,
        &mut destination,
        u64::try_from(original.len()).expect("source length"),
        &DomainDigest::sha256(&original),
        Instant::now() + Duration::from_secs(1),
        |copied| {
            if copied == u64::try_from(SNAPSHOT_BUFFER_BYTES).expect("buffer length") {
                writer
                    .write_all_at(&mutated_chunk, copied)
                    .expect("mutate next source chunk");
            } else if copied
                == u64::try_from(SNAPSHOT_BUFFER_BYTES * 2).expect("two buffer lengths")
            {
                writer
                    .write_all_at(
                        &original[SNAPSHOT_BUFFER_BYTES..SNAPSHOT_BUFFER_BYTES * 2],
                        u64::try_from(SNAPSHOT_BUFFER_BYTES).expect("buffer length"),
                    )
                    .expect("restore already copied source chunk");
            }
        },
    );
    assert_eq!(result, Err(HelperFailure::ControlledBuildObjectMismatch));
    assert_eq!(
        fs::read(source.path()).expect("read restored source"),
        original
    );
}
