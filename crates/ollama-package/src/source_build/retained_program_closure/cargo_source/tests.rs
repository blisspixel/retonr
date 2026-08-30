use std::{collections::BTreeMap, io::Cursor};

use tar::EntryType;

use super::{
    CargoSourceClosureError, CargoSourceClosureLimits,
    archive::{scan_canonical_tar, validate_raw_tar},
    lockfile::parse_lockfile,
};

#[path = "tests/adversarial.rs"]
mod adversarial;
#[path = "tests/fixture.rs"]
mod fixture;
#[path = "tests/support.rs"]
mod support;

use fixture::{
    EPOCH, Fixture, SOURCE, ambiguous_lockfile, append_tar_entry, canonical_tar, cargo_checksum,
    crate_archive, crate_files, duplicate_crate_archive, lockfile, repository_archive,
    rewritten_vendor_checksum, scan_vendor, vendor_files_from_fixture,
};
use support::{gzip, measurement};
#[path = "tests/integration.rs"]
mod integration;

#[test]
fn complete_lock_raw_vendor_and_path_closure_verifies() {
    let fixture = Fixture::valid();
    let text = std::str::from_utf8(&fixture.lock).expect("lock UTF-8");
    let parsed = text
        .parse::<cargo_lock::Lockfile>()
        .expect("cargo-lock accepts fixture");
    toml::from_str::<toml::Value>(text).expect("workspace TOML parser accepts fixture");
    assert_eq!(parsed.version, cargo_lock::ResolveVersion::V4);
    assert!(parsed.root.is_none());
    assert_eq!(parsed.packages.len(), 3);
    assert!(
        parsed
            .packages
            .iter()
            .all(|package| package.replace.is_none())
    );
    assert_eq!(
        parsed.packages[1]
            .source
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some(SOURCE)
    );
    parse_lockfile(&fixture.lock, CargoSourceClosureLimits::default())
        .expect("closure parser accepts fixture");
    assert_eq!(fixture.verify(), Ok(()));
}

#[test]
fn lock_rejects_alternate_sources_checksum_drift_and_ambiguous_edges() {
    let fixture = Fixture::valid();
    let limits = CargoSourceClosureLimits::default();
    let alternate = String::from_utf8(fixture.lock.clone())
        .expect("fixture lock is UTF-8")
        .replace(SOURCE, "registry+https://example.invalid/index");
    assert_eq!(
        parse_lockfile(alternate.as_bytes(), limits).err(),
        Some(CargoSourceClosureError::UnsupportedPackageSource)
    );
    let lock_text = String::from_utf8(fixture.lock.clone()).expect("fixture lock is UTF-8");
    let checksum = lock_text
        .lines()
        .find_map(|line| line.strip_prefix("checksum = \"")?.strip_suffix('"'))
        .expect("fixture checksum exists");
    let uppercase = lock_text.replace(checksum, &checksum.to_ascii_uppercase());
    assert!(parse_lockfile(uppercase.as_bytes(), limits).is_err());
    assert_eq!(
        parse_lockfile(ambiguous_lockfile().as_bytes(), limits).err(),
        Some(CargoSourceClosureError::AmbiguousDependencyEdge)
    );
}

#[test]
fn raw_crate_flip_and_set_drift_fail_closed() {
    let mut fixture = Fixture::valid();
    let mut changed = crate_files();
    changed.insert("src/lib.rs".to_owned(), b"changed\n".to_vec());
    fixture.raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([(
            "dep-1.0.0.crate".to_owned(),
            crate_archive("dep-1.0.0", &changed, None),
        )]),
    );
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::CrateChecksumMismatch)
    );

    let mut missing = Fixture::valid();
    missing.raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("extra.crate".to_owned(), vec![1])]),
    );
    assert_eq!(
        missing.verify(),
        Err(CargoSourceClosureError::RawCrateSetMismatch)
    );
}

#[test]
fn inner_link_and_duplicate_are_rejected() {
    let mut fixture = Fixture::valid();
    let files = BTreeMap::from([("Cargo.toml".to_owned(), crate_files()["Cargo.toml"].clone())]);
    let linked = crate_archive("dep-1.0.0", &files, Some(EntryType::Symlink));
    fixture.raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("dep-1.0.0.crate".to_owned(), linked)]),
    );
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::UnsafeArchiveEntry)
    );
    let extended = crate_archive("dep-1.0.0", &files, Some(EntryType::XHeader));
    fixture.raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("dep-1.0.0.crate".to_owned(), extended)]),
    );
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::UnsafeArchiveEntry)
    );
    let duplicate = duplicate_crate_archive("dep-1.0.0", &files);
    fixture.raw = canonical_tar(
        "cargo-crates",
        &BTreeMap::from([("dep-1.0.0.crate".to_owned(), duplicate)]),
    );
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::UnsafeArchiveEntry)
    );
}

#[test]
fn rewritten_vendor_checksum_cannot_cover_an_extra_file() {
    let mut fixture = Fixture::valid();
    let mut files = vendor_files_from_fixture();
    files.insert("dep/build.rs".to_owned(), b"fn main() {}\n".to_vec());
    let checksum = rewritten_vendor_checksum(&files, &fixture.lock);
    files.insert("dep/.cargo-checksum.json".to_owned(), checksum);
    fixture.vendor = canonical_tar("cargo-vendor", &files);
    assert_eq!(
        fixture.verify(),
        Err(CargoSourceClosureError::VendorChecksumMismatch)
    );
}

#[test]
fn repository_lock_mismatch_and_path_escape_are_rejected() {
    let mut mismatch = Fixture::valid();
    mismatch.repository = repository_archive(b"version = 4\n", "../dep-unused");
    assert_eq!(
        mismatch.verify(),
        Err(CargoSourceClosureError::RepositoryLockMismatch)
    );
    let mut escape = Fixture::valid();
    escape.repository = repository_archive(&escape.lock, "../../../outside");
    assert_eq!(
        escape.verify(),
        Err(CargoSourceClosureError::PathPackageEscape)
    );
}

#[test]
fn exact_tar_terminal_layout_and_cancellation_are_enforced() {
    let fixture = Fixture::valid();
    let limits = CargoSourceClosureLimits::default();
    let mut appended = fixture.raw.clone();
    appended.extend_from_slice(&[0_u8; 512]);
    let appended_measurement = measurement("appended", &appended);
    assert_eq!(
        validate_raw_tar(
            Cursor::new(appended),
            &appended_measurement,
            limits,
            &mut || false,
        ),
        Err(CargoSourceClosureError::NoncanonicalArchive)
    );
    let mut truncated = fixture.raw.clone();
    truncated.truncate(truncated.len() - 512);
    let truncated_measurement = measurement("truncated", &truncated);
    assert!(
        validate_raw_tar(
            Cursor::new(truncated),
            &truncated_measurement,
            limits,
            &mut || false,
        )
        .is_err()
    );
    let raw_measurement = measurement("cancelled", &fixture.raw);
    assert_eq!(
        validate_raw_tar(
            Cursor::new(fixture.raw),
            &raw_measurement,
            limits,
            &mut || true,
        ),
        Err(CargoSourceClosureError::Cancelled)
    );
}

#[test]
fn limits_reject_zero_and_archive_entry_quota() {
    let invalid = CargoSourceClosureLimits {
        maximum_packages: 0,
        ..CargoSourceClosureLimits::default()
    };
    assert_eq!(
        invalid.validate(),
        Err(CargoSourceClosureError::InvalidLimits)
    );
    let fixture = Fixture::valid();
    let limits = CargoSourceClosureLimits {
        maximum_archive_entries: 1,
        ..CargoSourceClosureLimits::default()
    };
    let measurement = measurement("raw", &fixture.raw);
    assert_eq!(
        scan_canonical_tar(
            Cursor::new(fixture.raw),
            &measurement,
            "cargo-crates",
            limits,
            &mut || false,
            |_| false,
        )
        .err(),
        Some(CargoSourceClosureError::ArchiveQuotaExceeded)
    );
}
