use std::io::Cursor;

use tempfile::tempdir;

use super::*;

fn tar(entries: &[(&str, EntryType, u32, &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut bytes);
        for (path, kind, mode, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(*kind);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mode(*mode);
            header.set_mtime(SOURCE_DATE_EPOCH);
            header.set_size(u64::try_from(contents.len()).expect("fixture size"));
            header.set_cksum();
            builder
                .append_data(&mut header, path, Cursor::new(*contents))
                .expect("append fixture");
        }
        builder.finish().expect("finish fixture");
    }
    bytes
}

fn canonical(entries: &[(&str, EntryType, &[u8])]) -> Vec<u8> {
    let entries = entries
        .iter()
        .map(|(path, kind, contents)| {
            (
                *path,
                *kind,
                if kind.is_dir() { 0o755 } else { 0o644 },
                *contents,
            )
        })
        .collect::<Vec<_>>();
    tar(&entries)
}

#[test]
fn exact_regular_tree_extracts_under_one_root() {
    let root = tempdir().expect("temporary root");
    let source = root.path().join("source.tar");
    fs::write(
        &source,
        canonical(&[
            ("source", EntryType::Directory, b""),
            ("source/directory", EntryType::Directory, b""),
            ("source/directory/file.txt", EntryType::Regular, b"contents"),
        ]),
    )
    .expect("write archive");
    let output = root.path().join("output");
    extract_exact_tar(&source, &output, "source").expect("extract exact archive");
    assert_eq!(
        fs::read(output.join("source/directory/file.txt")).expect("read output"),
        b"contents"
    );
}

#[test]
fn bounded_gnu_long_name_extracts() {
    let root = tempdir().expect("temporary root");
    let long_component = "a".repeat(180);
    let directory = format!("source/{long_component}");
    let file = format!("{directory}/file");
    let entries = [
        ("source", EntryType::Directory, b"".as_slice()),
        (directory.as_str(), EntryType::Directory, b"".as_slice()),
        (file.as_str(), EntryType::Regular, b"contents".as_slice()),
    ];
    let archive = root.path().join("source.tar");
    fs::write(&archive, canonical(&entries)).expect("archive");
    let output = root.path().join("output");
    extract_exact_tar(&archive, &output, "source").expect("long path extraction");
    assert_eq!(fs::read(output.join(file)).expect("file"), b"contents");
}

#[test]
fn links_wrong_roots_and_duplicates_fail_closed() {
    for entries in [
        vec![("other", EntryType::Directory, b"".as_slice())],
        vec![
            ("source", EntryType::Directory, b"".as_slice()),
            ("source/link", EntryType::Symlink, b"target".as_slice()),
        ],
        vec![
            ("source", EntryType::Directory, b"".as_slice()),
            ("source/file", EntryType::Regular, b"a".as_slice()),
            ("source/file", EntryType::Regular, b"b".as_slice()),
        ],
    ] {
        rejects(&canonical(&entries));
    }
}

#[test]
fn root_must_be_first_exact_and_a_directory() {
    for bytes in [
        canonical(&[("source", EntryType::Regular, b"")]),
        canonical(&[
            ("source/file", EntryType::Regular, b"value"),
            ("source", EntryType::Directory, b""),
        ]),
        canonical(&[
            ("source/", EntryType::Directory, b""),
            ("source/file", EntryType::Regular, b"value"),
        ]),
    ] {
        rejects(&bytes);
    }
}

#[test]
fn ordering_and_explicit_parent_directories_are_required() {
    for bytes in [
        canonical(&[
            ("source", EntryType::Directory, b""),
            ("source/z", EntryType::Regular, b"z"),
            ("source/a", EntryType::Regular, b"a"),
        ]),
        canonical(&[
            ("source", EntryType::Directory, b""),
            ("source/missing/file", EntryType::Regular, b"value"),
        ]),
    ] {
        rejects(&bytes);
    }
}

#[test]
fn backslashes_and_noncanonical_headers_are_rejected() {
    let mut backslash = canonical(&[
        ("source", EntryType::Directory, b""),
        ("source/file", EntryType::Regular, b"value"),
    ]);
    let header = &mut backslash[512..1024];
    header[6] = b'\\';
    rewrite_checksum(header);
    rejects(&backslash);
    rejects(&tar(&[
        ("source", EntryType::Directory, 0o755, b""),
        ("source/file", EntryType::Regular, 0o600, b"value"),
    ]));
}

fn rewrite_checksum(header: &mut [u8]) {
    header[148..156].fill(b' ');
    let sum = header.iter().map(|byte| u64::from(*byte)).sum::<u64>();
    let checksum = format!("{sum:06o}\0 ");
    header[148..156].copy_from_slice(checksum.as_bytes());
}

#[test]
fn extension_metadata_is_bounded_and_pax_is_rejected() {
    let oversized = vec![b'a'; MAXIMUM_EXTENSION_BYTES + 1];
    rejects(&tar(&[
        ("././@LongLink", EntryType::GNULongName, 0o644, &oversized),
        ("source", EntryType::Directory, 0o755, b""),
    ]));
    rejects(&tar(&[
        ("pax", EntryType::XHeader, 0o644, b"11 path=x\n"),
        ("source", EntryType::Directory, 0o755, b""),
    ]));
}

#[test]
fn malformed_archive_size_and_partial_outputs_are_rejected_and_cleaned() {
    let root = tempdir().expect("root");
    let archive = root.path().join("source.tar");
    let mut bytes = canonical(&[
        ("source", EntryType::Directory, b""),
        ("source/file", EntryType::Regular, b"value"),
    ]);
    bytes.push(0);
    fs::write(&archive, bytes).expect("archive");
    let output = root.path().join("output");
    assert!(extract_exact_tar(&archive, &output, "source").is_err());
    assert!(!output.exists());
}

#[test]
fn exact_two_block_end_marker_is_required() {
    let canonical = canonical(&[("source", EntryType::Directory, b"")]);
    let mut extra_block = canonical.clone();
    extra_block.extend_from_slice(&[0_u8; 512]);
    rejects(&extra_block);

    let mut missing_block = canonical.clone();
    missing_block.truncate(missing_block.len() - 512);
    rejects(&missing_block);

    let mut nonzero_terminal_block = canonical;
    let terminal = nonzero_terminal_block.len() - 512;
    nonzero_terminal_block[terminal] = 1;
    rejects(&nonzero_terminal_block);
}

#[cfg(unix)]
#[test]
fn hard_linked_archive_input_is_rejected() {
    let root = tempdir().expect("root");
    let archive = root.path().join("source.tar");
    fs::write(
        &archive,
        canonical(&[("source", EntryType::Directory, b"")]),
    )
    .expect("archive");
    fs::hard_link(&archive, root.path().join("alias.tar")).expect("hard link");
    assert!(extract_exact_tar(&archive, &root.path().join("output"), "source").is_err());
}

fn rejects(bytes: &[u8]) {
    let root = tempdir().expect("temporary root");
    let source = root.path().join("source.tar");
    fs::write(&source, bytes).expect("write archive");
    let output = root.path().join("output");
    assert!(extract_exact_tar(&source, &output, "source").is_err());
    assert!(!output.exists());
}
