use std::{
    collections::BTreeMap,
    io::{Cursor, Read as _},
};

use rewrite_types::Digest;
use tar::EntryType;

use super::{
    super::{
        CargoSourceClosureError, CargoSourceClosureLimits,
        archive::{scan_canonical_tar, validate_raw_tar},
        lockfile::parse_lockfile,
        vendor::{scan_raw_bundle, verify_vendor},
    },
    Fixture, append_tar_entry, canonical_tar, cargo_checksum, crate_archive, crate_files, gzip,
    lockfile, measurement, scan_vendor, vendor_files_from_fixture,
};

#[test]
fn duplicate_lock_identity_is_rejected() {
    let fixture = Fixture::valid();
    let text = std::str::from_utf8(&fixture.lock).expect("fixture lock UTF-8");
    let duplicate = text.replace(
        "[[package]]\nname = \"unused\"",
        "[[package]]\nname = \"unused\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"unused\"",
    );
    assert_eq!(
        parse_lockfile(duplicate.as_bytes(), CargoSourceClosureLimits::default()).err(),
        Some(CargoSourceClosureError::DuplicatePackageIdentity)
    );
}

#[test]
fn raw_crate_manifest_identity_and_unpack_quota_are_enforced() {
    let mut wrong_files = crate_files();
    wrong_files.insert(
        "Cargo.toml".to_owned(),
        b"[package]\nname = \"other\"\nversion = \"1.0.0\"\n".to_vec(),
    );
    let wrong_crate = crate_archive("dep-1.0.0", &wrong_files, None);
    let lock_bytes = lockfile(Digest::sha256(&wrong_crate).as_str());
    let lock = parse_lockfile(&lock_bytes, CargoSourceClosureLimits::default())
        .expect("mutation lock parses");
    let raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("dep-1.0.0.crate".to_owned(), wrong_crate)]),
    );
    let raw_measurement = measurement("raw-name", &raw);
    assert_eq!(
        scan_raw_bundle(
            Cursor::new(&raw),
            &raw_measurement,
            &lock,
            CargoSourceClosureLimits::default(),
            &mut || false,
        )
        .err(),
        Some(CargoSourceClosureError::CrateManifestMismatch)
    );
    let limits = CargoSourceClosureLimits {
        maximum_crate_unpacked_bytes: 1,
        ..CargoSourceClosureLimits::default()
    };
    let fixture = Fixture::valid();
    let lock = parse_lockfile(&fixture.lock, limits).expect("fixture lock parses");
    assert_eq!(
        scan_raw_bundle(
            Cursor::new(&fixture.raw),
            &measurement("raw-bomb", &fixture.raw),
            &lock,
            limits,
            &mut || false,
        )
        .err(),
        Some(CargoSourceClosureError::ArchiveQuotaExceeded)
    );
}

#[test]
fn vendor_missing_mutated_and_duplicate_checksum_keys_are_rejected() {
    let mut fixture = Fixture::valid();
    let mut missing = vendor_files_from_fixture();
    missing.remove("dep/src/lib.rs");
    fixture.vendor = canonical_tar("cargo-vendor", &missing);
    assert!(matches!(
        fixture.verify(),
        Err(CargoSourceClosureError::VendorTreeMismatch
            | CargoSourceClosureError::VendorChecksumMismatch)
    ));

    let mut mutated = vendor_files_from_fixture();
    mutated.insert("dep/src/lib.rs".to_owned(), b"mutated\n".to_vec());
    fixture.vendor = canonical_tar("cargo-vendor", &mutated);
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::VendorChecksumMismatch)
    );

    let mut duplicate = vendor_files_from_fixture();
    let package = checksum_from_lock(&fixture.lock);
    let cargo_toml = Digest::sha256(&duplicate["dep/Cargo.toml"]);
    let lib = Digest::sha256(&duplicate["dep/src/lib.rs"]);
    duplicate.insert(
        "dep/.cargo-checksum.json".to_owned(),
        format!(
            "{{\"files\":{{\"Cargo.toml\":\"{cargo_toml}\",\"Cargo.toml\":\"{cargo_toml}\",\"src/lib.rs\":\"{lib}\"}},\"package\":\"{package}\"}}"
        )
        .into_bytes(),
    );
    fixture.vendor = canonical_tar("cargo-vendor", &duplicate);
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::VendorChecksumMismatch)
    );
}

#[test]
fn outer_root_link_duplicate_path_and_corrupt_terminal_are_rejected() {
    let fixture = Fixture::valid();
    let limits = CargoSourceClosureLimits::default();
    let wrong_root_measurement = measurement("wrong-root", &fixture.raw);
    assert_eq!(
        scan_canonical_tar(
            Cursor::new(&fixture.raw),
            &wrong_root_measurement,
            "wrong-root",
            limits,
            &mut || false,
            |_| false,
        )
        .err(),
        Some(CargoSourceClosureError::ArchiveRootMismatch)
    );

    let linked = special_outer_tar(EntryType::Symlink, false);
    assert_eq!(
        validate_raw_tar(
            Cursor::new(&linked),
            &measurement("linked", &linked),
            limits,
            &mut || false,
        ),
        Err(CargoSourceClosureError::UnsafeArchiveEntry)
    );
    let duplicate = special_outer_tar(EntryType::Regular, true);
    assert_eq!(
        scan_canonical_tar(
            Cursor::new(&duplicate),
            &measurement("duplicate", &duplicate),
            "cargo-crates",
            limits,
            &mut || false,
            |_| false,
        )
        .err(),
        Some(CargoSourceClosureError::UnsafeArchiveEntry)
    );

    let mut corrupt = fixture.raw;
    let last = corrupt.last_mut().expect("fixture archive is nonempty");
    *last = 1;
    assert_eq!(
        validate_raw_tar(
            Cursor::new(&corrupt),
            &measurement("corrupt", &corrupt),
            limits,
            &mut || false,
        ),
        Err(CargoSourceClosureError::NoncanonicalArchive)
    );
}

#[test]
fn vendor_package_bijection_rejects_an_extra_package() {
    let mut fixture = Fixture::valid();
    let mut files = vendor_files_from_fixture();
    let other_files = crate_files();
    let other_crate = crate_archive("other-1.0.0", &other_files, None);
    files.insert(
        "other/Cargo.toml".to_owned(),
        b"[package]\nname = \"other\"\nversion = \"1.0.0\"\n".to_vec(),
    );
    files.insert(
        "other/src/lib.rs".to_owned(),
        other_files["src/lib.rs"].clone(),
    );
    files.insert(
        "other/.cargo-checksum.json".to_owned(),
        cargo_checksum(&other_files, Digest::sha256(&other_crate).as_str()),
    );
    fixture.vendor = canonical_tar("cargo-vendor", &files);
    let vendor = scan_vendor(&fixture, CargoSourceClosureLimits::default())
        .expect("extra vendor archive is structurally valid");
    let lock = parse_lockfile(&fixture.lock, CargoSourceClosureLimits::default())
        .expect("fixture lock parses");
    let raw = scan_raw_bundle(
        Cursor::new(&fixture.raw),
        &measurement("raw", &fixture.raw),
        &lock,
        CargoSourceClosureLimits::default(),
        &mut || false,
    )
    .expect("fixture raw bundle verifies");
    assert_eq!(
        verify_vendor(&vendor, &raw),
        Err(CargoSourceClosureError::VendorTreeMismatch)
    );
}

fn checksum_from_lock(lock: &[u8]) -> &str {
    std::str::from_utf8(lock)
        .expect("fixture lock UTF-8")
        .lines()
        .find_map(|line| {
            line.strip_prefix("checksum = \"")
                .and_then(|line| line.strip_suffix('"'))
        })
        .expect("fixture checksum exists")
}

fn special_outer_tar(entry_type: EntryType, duplicate: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut bytes);
        append_tar_entry(&mut builder, "cargo-crates", None, None);
        for _ in 0..if duplicate { 2 } else { 1 } {
            append_tar_entry(
                &mut builder,
                "cargo-crates/member.crate",
                Some(b"member"),
                Some(entry_type),
            );
        }
        builder.finish().expect("fixture outer tar finishes");
    }
    bytes
}

#[test]
fn crate_gzip_rejects_trailing_invalid_member_data() {
    let mut crate_bytes = crate_archive("dep-1.0.0", &crate_files(), None);
    crate_bytes.extend_from_slice(b"not-a-gzip-member");
    let lock_bytes = lockfile(Digest::sha256(&crate_bytes).as_str());
    let lock = parse_lockfile(&lock_bytes, CargoSourceClosureLimits::default())
        .expect("mutation lock parses");
    let raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("dep-1.0.0.crate".to_owned(), crate_bytes)]),
    );
    assert_eq!(
        scan_raw_bundle(
            Cursor::new(&raw),
            &measurement("gzip-junk", &raw),
            &lock,
            CargoSourceClosureLimits::default(),
            &mut || false,
        )
        .err(),
        Some(CargoSourceClosureError::InvalidCrateArchive)
    );
}

#[test]
fn crate_tar_rejects_hidden_data_after_its_terminal_blocks() {
    let crate_bytes = crate_archive("dep-1.0.0", &crate_files(), None);
    let mut tar_bytes = Vec::new();
    flate2::read::GzDecoder::new(crate_bytes.as_slice())
        .read_to_end(&mut tar_bytes)
        .expect("fixture crate decompresses");
    tar_bytes.extend_from_slice(b"hidden-after-tar");
    let hidden = gzip(&tar_bytes);
    let lock_bytes = lockfile(Digest::sha256(&hidden).as_str());
    let lock = parse_lockfile(&lock_bytes, CargoSourceClosureLimits::default())
        .expect("mutation lock parses");
    let raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("dep-1.0.0.crate".to_owned(), hidden)]),
    );
    assert_eq!(
        scan_raw_bundle(
            Cursor::new(&raw),
            &measurement("hidden-tar-data", &raw),
            &lock,
            CargoSourceClosureLimits::default(),
            &mut || false,
        )
        .err(),
        Some(CargoSourceClosureError::InvalidCrateArchive)
    );
}

#[test]
fn helper_gzip_remains_a_real_multistream_decoder_fixture() {
    let compressed = gzip(b"not a tar");
    assert!(!compressed.is_empty());
}
