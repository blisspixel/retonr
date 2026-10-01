use std::{fs, os::unix::fs::symlink};

use super::*;

#[test]
fn library_payload_requires_regular_single_link_elf_and_read_only_mount() {
    let directory = tempfile::tempdir().expect("root");
    fs::create_dir_all(directory.path().join("lib")).expect("lib");
    let root = File::open(directory.path()).expect("root handle");
    let path = directory.path().join(LOADER);
    for bytes in [b"not ELF".as_slice(), b"\x7fELFfixture".as_slice()] {
        fs::write(&path, bytes).expect("payload");
        assert!(payload(&root, LOADER).is_err());
    }
    fs::hard_link(&path, directory.path().join("alias")).expect("hardlink");
    assert!(payload(&root, LOADER).is_err());
    fs::remove_file(&path).expect("remove payload");
    symlink("../alias", &path).expect("symbolic alias");
    assert!(payload(&root, LOADER).is_err());
    assert!(payload(&root, "lib").is_err());
    assert!(payload(&root, "missing").is_err());
}

mod native;
