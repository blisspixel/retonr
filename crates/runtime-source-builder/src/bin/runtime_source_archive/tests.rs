use super::*;
use std::fs::OpenOptions;
use tempfile::tempdir;

#[test]
fn archive_is_repeatable_and_has_normalized_headers() {
    let root = tempdir().expect("root");
    let source = root.path().join("source");
    fs::create_dir_all(source.join("bin")).expect("bin");
    fs::write(source.join("bin/go"), b"tool").expect("tool");
    fs::write(source.join("README"), b"text").expect("text");
    let mut inventory = Inventory::acquire(&source, Limits::HARD).expect("inventory");
    let mut first = Vec::new();
    let mut second = Vec::new();
    write_archive(&mut first, &source, Kind::Go, &mut inventory).expect("first archive");
    write_archive(&mut second, &source, Kind::Go, &mut inventory).expect("second archive");
    assert_eq!(first, second);
    let mut archive = tar::Archive::new(first.as_slice());
    let headers = archive
        .entries()
        .expect("entries")
        .map(|entry| {
            let entry = entry.expect("entry");
            (
                entry.path().expect("path").to_string_lossy().into_owned(),
                entry.header().mode().expect("mode"),
                entry.header().mtime().expect("mtime"),
            )
        })
        .collect::<Vec<_>>();
    assert!(headers.iter().all(|entry| entry.2 == SOURCE_DATE_EPOCH));
    assert!(headers.contains(&("go/bin/go".to_owned(), 0o755, SOURCE_DATE_EPOCH)));
    assert!(headers.contains(&("go/README".to_owned(), 0o644, SOURCE_DATE_EPOCH)));
}

#[test]
fn same_size_file_path_substitution_is_rejected() {
    let fixture = fixture(&[("payload", b"first")]);
    let mut inventory = Inventory::acquire(&fixture.source, Limits::HARD).expect("inventory");
    fs::rename(
        fixture.source.join("payload"),
        fixture.root.path().join("original"),
    )
    .expect("retain original");
    fs::write(fixture.source.join("payload"), b"other").expect("replacement");
    assert!(write_archive(Vec::new(), &fixture.source, Kind::Ollama, &mut inventory).is_err());
}

#[test]
fn in_place_same_size_mutation_is_rejected() {
    let fixture = fixture(&[("payload", b"first")]);
    let mut inventory = Inventory::acquire(&fixture.source, Limits::HARD).expect("inventory");
    fs::write(fixture.source.join("payload"), b"other").expect("mutation");
    assert!(write_archive(Vec::new(), &fixture.source, Kind::Ollama, &mut inventory).is_err());
}

#[test]
fn mid_stream_mutate_and_restore_attempt_is_rejected() {
    const LENGTH: usize = 1024 * 1024;
    let original = vec![b'a'; LENGTH];
    let fixture = fixture(&[("payload", &original)]);
    let mut inventory = Inventory::acquire(&fixture.source, Limits::HARD).expect("inventory");
    let mut output = MutateRestoreWriter::new(fixture.source.join("payload"), original);
    assert!(write_archive(&mut output, &fixture.source, Kind::Ollama, &mut inventory,).is_err());
    assert!(output.mutated);
    assert!(output.restored);
    assert_eq!(
        fs::read(fixture.source.join("payload")).expect("restored payload"),
        output.original
    );
}

#[cfg(unix)]
#[test]
fn directory_path_substitution_is_rejected() {
    let fixture = fixture(&[("nested/payload", b"bound")]);
    let mut inventory = Inventory::acquire(&fixture.source, Limits::HARD).expect("inventory");
    fs::rename(
        fixture.source.join("nested"),
        fixture.root.path().join("original-directory"),
    )
    .expect("retain original directory");
    fs::create_dir(fixture.source.join("nested")).expect("replacement directory");
    fs::write(fixture.source.join("nested/payload"), b"bound").expect("replacement file");
    assert!(write_archive(Vec::new(), &fixture.source, Kind::Ollama, &mut inventory).is_err());
}

#[test]
fn added_and_removed_entries_are_rejected() {
    let added = fixture(&[("payload", b"bound")]);
    let mut added_inventory =
        Inventory::acquire(&added.source, Limits::HARD).expect("added inventory");
    fs::write(added.source.join("extra"), b"extra").expect("extra");
    assert!(
        write_archive(
            Vec::new(),
            &added.source,
            Kind::Ollama,
            &mut added_inventory,
        )
        .is_err()
    );

    let removed = fixture(&[("payload", b"bound")]);
    fs::create_dir(removed.source.join("empty")).expect("empty");
    let mut removed_inventory =
        Inventory::acquire(&removed.source, Limits::HARD).expect("removed inventory");
    fs::remove_dir(removed.source.join("empty")).expect("remove empty");
    assert!(
        write_archive(
            Vec::new(),
            &removed.source,
            Kind::Ollama,
            &mut removed_inventory,
        )
        .is_err()
    );
}

#[test]
fn inventory_enforces_entry_and_byte_limits() {
    let entries = fixture(&[("first", b"1"), ("second", b"2")]);
    assert!(
        Inventory::acquire(
            &entries.source,
            Limits {
                entries: 1,
                ..Limits::HARD
            },
        )
        .is_err()
    );
    let bytes = fixture(&[("payload", b"four")]);
    assert!(
        Inventory::acquire(
            &bytes.source,
            Limits {
                bytes: 3,
                ..Limits::HARD
            },
        )
        .is_err()
    );

    let fanout = fixture(&[("first", b"1"), ("second", b"2")]);
    assert!(
        Inventory::acquire(
            &fanout.source,
            Limits {
                directory_entries: 1,
                ..Limits::HARD
            },
        )
        .is_err()
    );
    let paths = fixture(&[("long-name", b"1")]);
    assert!(
        Inventory::acquire(
            &paths.source,
            Limits {
                path_bytes: 8,
                ..Limits::HARD
            },
        )
        .is_err()
    );
    let retained = fixture(&[("first", b"1"), ("second", b"2")]);
    assert!(
        Inventory::acquire(
            &retained.source,
            Limits {
                retained_files: 1,
                ..Limits::HARD
            },
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn links_are_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = fixture(&[("payload", b"bound")]);
    symlink("payload", fixture.source.join("link")).expect("link");
    assert!(Inventory::acquire(&fixture.source, Limits::HARD).is_err());
}

#[test]
fn duplicate_hard_link_identities_are_rejected() {
    let linked = fixture(&[("payload", b"bound")]);
    fs::hard_link(linked.source.join("payload"), linked.source.join("alias")).expect("hard link");
    assert!(Inventory::acquire(&linked.source, Limits::HARD).is_err());
}

#[cfg(unix)]
#[test]
fn backslash_names_are_rejected() {
    let backslash = fixture(&[("bad\\name", b"bound")]);
    assert!(Inventory::acquire(&backslash.source, Limits::HARD).is_err());
}

#[test]
fn retained_program_kinds_have_distinct_fixed_roots() {
    for (name, root) in [
        ("ollama", "ollama"),
        ("llama-cpp", "llama-cpp"),
        ("go", "go"),
        ("go-module-cache", "gomodcache"),
        ("cmake", "cmake"),
        ("ninja", "ninja"),
        ("zig", "zig"),
        ("retonr-source", "retonr-source"),
        ("cargo-crates", "cargo-crates"),
        ("cargo-vendor", "cargo-vendor"),
    ] {
        assert_eq!(Kind::parse(name.as_ref()).expect("kind").root(), root);
    }
    assert!(Kind::parse("unknown".as_ref()).is_err());
    assert!(!Kind::RetonrSource.executable("crates/app/src/lib.rs"));
    assert!(!Kind::CargoCrates.executable("serde-1.0.0.crate"));
    assert!(!Kind::CargoVendor.executable("serde/build.rs"));
}

#[test]
fn executable_policy_is_closed_by_kind_and_path() {
    assert!(Kind::Go.executable("bin/gofmt"));
    assert!(Kind::Go.executable("pkg/tool/linux_amd64/compile"));
    assert!(Kind::Cmake.executable("bin/cmake"));
    assert!(Kind::Ninja.executable("ninja"));
    assert!(Kind::Zig.executable("zig"));
    assert!(!Kind::Go.executable("README"));
    assert!(!Kind::Cmake.executable("bin/other"));
    assert!(!Kind::Ninja.executable("bin/ninja"));
    assert!(!Kind::Zig.executable("zig.exe"));
}

#[test]
fn archive_path_policy_rejects_noncanonical_paths() {
    for path in [
        "",
        "/rooted",
        "trailing/",
        "back\\slash",
        ".",
        "..",
        "a//b",
        "a/./b",
    ] {
        assert!(validate_archive_path(path).is_err(), "accepted {path:?}");
    }
    assert!(validate_archive_path("nested/canonical").is_ok());
}

struct Fixture {
    root: tempfile::TempDir,
    source: PathBuf,
}

fn fixture(files: &[(&str, &[u8])]) -> Fixture {
    let root = tempdir().expect("root");
    let source = root.path().join("source");
    fs::create_dir(&source).expect("source");
    for (relative, contents) in files {
        let path = source.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(path, contents).expect("fixture file");
    }
    Fixture { root, source }
}

struct MutateRestoreWriter {
    path: PathBuf,
    original: Vec<u8>,
    written: usize,
    mutated: bool,
    restored: bool,
}

impl MutateRestoreWriter {
    fn new(path: PathBuf, original: Vec<u8>) -> Self {
        Self {
            path,
            original,
            written: 0,
            mutated: false,
            restored: false,
        }
    }

    fn replace_suffix(&self, byte: u8) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).open(&self.path)?;
        let midpoint = self.original.len() / 2;
        file.seek(io::SeekFrom::Start(midpoint as u64))?;
        let replacement = vec![byte; self.original.len() - midpoint];
        file.write_all(&replacement)
    }
}

impl io::Write for MutateRestoreWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.written = self.written.saturating_add(buffer.len());
        let content_written = self.written.saturating_sub(1024);
        if !self.mutated && content_written >= 64 * 1024 {
            self.replace_suffix(b'b')?;
            self.mutated = true;
        }
        if self.mutated && !self.restored && content_written >= self.original.len() * 3 / 4 {
            self.replace_suffix(b'a')?;
            self.restored = true;
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
