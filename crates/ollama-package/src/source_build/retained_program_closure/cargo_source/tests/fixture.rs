use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};

use rewrite_types::Digest;
use tar::EntryType;

use super::super::{
    CargoSourceClosureError, CargoSourceClosureLimits,
    archive::{TreeSnapshot, scan_canonical_tar, validate_raw_tar},
    lockfile::parse_lockfile,
    repository::verify_repository,
    vendor::{scan_raw_bundle, verify_vendor},
};
use super::{gzip, measurement};

pub(super) const SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
pub(super) const EPOCH: u64 = 1_725_000_000;

#[derive(Clone)]
pub(super) struct Fixture {
    pub(super) lock: Vec<u8>,
    pub(super) raw: Vec<u8>,
    pub(super) repository: Vec<u8>,
    pub(super) vendor: Vec<u8>,
}

impl Fixture {
    pub(super) fn valid() -> Self {
        let crate_files = crate_files();
        let crate_bytes = crate_archive("dep-1.0.0", &crate_files, None);
        let crate_digest = Digest::sha256(&crate_bytes);
        let lock = lockfile(crate_digest.as_str());
        let raw = canonical_tar(
            "cargo-crates",
            &BTreeMap::from([("dep-1.0.0.crate".to_owned(), crate_bytes)]),
        );
        let repository = repository_archive(&lock, "../dep-unused");
        let checksum = cargo_checksum(&crate_files, crate_digest.as_str());
        let mut vendor_files = crate_files;
        vendor_files.insert(".cargo-checksum.json".to_owned(), checksum);
        let vendor = canonical_tar(
            "cargo-vendor",
            &vendor_files
                .into_iter()
                .map(|(path, bytes)| (format!("dep/{path}"), bytes))
                .collect(),
        );
        Self {
            lock,
            raw,
            repository,
            vendor,
        }
    }

    pub(super) fn verify(&self) -> Result<(), CargoSourceClosureError> {
        let limits = CargoSourceClosureLimits::default();
        let lock = parse_lockfile(&self.lock, limits)?;
        let repository = scan_repository(self, limits)?;
        verify_repository(&repository, &lock, &self.lock, limits)?;
        let mut cancelled = || false;
        let raw_measurement = measurement("raw", &self.raw);
        validate_raw_tar(
            Cursor::new(&self.raw),
            &raw_measurement,
            limits,
            &mut cancelled,
        )?;
        let raw = scan_raw_bundle(
            Cursor::new(&self.raw),
            &raw_measurement,
            &lock,
            limits,
            &mut cancelled,
        )?;
        let vendor = scan_vendor(self, limits)?;
        verify_vendor(&vendor, &raw)
    }
}

pub(super) fn scan_repository(
    fixture: &Fixture,
    limits: CargoSourceClosureLimits,
) -> Result<TreeSnapshot, CargoSourceClosureError> {
    let measurement = measurement("repository", &fixture.repository);
    let mut cancelled = || false;
    validate_raw_tar(
        Cursor::new(&fixture.repository),
        &measurement,
        limits,
        &mut cancelled,
    )?;
    scan_canonical_tar(
        Cursor::new(&fixture.repository),
        &measurement,
        "retonr-source",
        limits,
        &mut cancelled,
        |path| path == "Cargo.lock" || path == "Cargo.toml" || path.ends_with("/Cargo.toml"),
    )
}

pub(super) fn scan_vendor(
    fixture: &Fixture,
    limits: CargoSourceClosureLimits,
) -> Result<TreeSnapshot, CargoSourceClosureError> {
    let measurement = measurement("vendor", &fixture.vendor);
    let mut cancelled = || false;
    validate_raw_tar(
        Cursor::new(&fixture.vendor),
        &measurement,
        limits,
        &mut cancelled,
    )?;
    scan_canonical_tar(
        Cursor::new(&fixture.vendor),
        &measurement,
        "cargo-vendor",
        limits,
        &mut cancelled,
        |path| path.ends_with("/Cargo.toml") || path.ends_with("/.cargo-checksum.json"),
    )
}

pub(super) fn crate_files() -> BTreeMap<String, Vec<u8>> {
    BTreeMap::from([
        (
            "Cargo.toml".to_owned(),
            b"[package]\nname = \"dep\"\nversion = \"1.0.0\"\n".to_vec(),
        ),
        (
            "src/lib.rs".to_owned(),
            b"pub fn value() -> u8 { 1 }\n".to_vec(),
        ),
    ])
}

pub(super) fn lockfile(checksum: &str) -> Vec<u8> {
    format!(
        "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\n \"dep\",\n \"unused\",\n]\n\n[[package]]\nname = \"dep\"\nversion = \"1.0.0\"\nsource = \"{SOURCE}\"\nchecksum = \"{checksum}\"\n\n[[package]]\nname = \"unused\"\nversion = \"0.1.0\"\n"
    )
    .into_bytes()
}

pub(super) fn ambiguous_lockfile() -> String {
    let checksum = "a".repeat(64);
    format!(
        "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"dep 1.0.0\"]\n\n[[package]]\nname = \"dep\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"dep\"\nversion = \"1.0.0\"\nsource = \"{SOURCE}\"\nchecksum = \"{checksum}\"\n"
    )
}

pub(super) fn repository_archive(lock: &[u8], unused_path: &str) -> Vec<u8> {
    let root = b"[workspace]\nmembers = [\"app\"]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"0.1.0\"\n";
    let app = format!(
        "[package]\nname = \"app\"\nversion.workspace = true\n\n[dev-dependencies]\nunused = {{ path = \"{unused_path}\", optional = true }}\n"
    );
    let unused = b"[package]\nname = \"unused\"\nversion.workspace = true\n";
    canonical_tar(
        "retonr-source",
        &BTreeMap::from([
            ("Cargo.lock".to_owned(), lock.to_vec()),
            ("Cargo.toml".to_owned(), root.to_vec()),
            ("app/Cargo.toml".to_owned(), app.into_bytes()),
            ("app/src/lib.rs".to_owned(), b"pub fn app() {}\n".to_vec()),
            ("dep-unused/Cargo.toml".to_owned(), unused.to_vec()),
        ]),
    )
}

pub(super) fn cargo_checksum(files: &BTreeMap<String, Vec<u8>>, package: &str) -> Vec<u8> {
    let file_digests = files
        .iter()
        .map(|(path, bytes)| (path, Digest::sha256(bytes).as_str().to_owned()))
        .collect::<BTreeMap<_, _>>();
    serde_json::to_vec(&serde_json::json!({"files": file_digests, "package": package}))
        .expect("checksum fixture serializes")
}

pub(super) fn vendor_files_from_fixture() -> BTreeMap<String, Vec<u8>> {
    let crate_files = crate_files();
    let crate_bytes = crate_archive("dep-1.0.0", &crate_files, None);
    let package = Digest::sha256(&crate_bytes);
    let checksum = cargo_checksum(&crate_files, package.as_str());
    let mut vendor = crate_files
        .into_iter()
        .map(|(path, bytes)| (format!("dep/{path}"), bytes))
        .collect::<BTreeMap<_, _>>();
    vendor.insert("dep/.cargo-checksum.json".to_owned(), checksum);
    vendor
}

pub(super) fn rewritten_vendor_checksum(files: &BTreeMap<String, Vec<u8>>, lock: &[u8]) -> Vec<u8> {
    let package = std::str::from_utf8(lock)
        .expect("lock fixture is UTF-8")
        .lines()
        .find_map(|line| {
            line.strip_prefix("checksum = \"")
                .and_then(|line| line.strip_suffix('"'))
        })
        .expect("lock fixture has checksum");
    let package_files = files
        .iter()
        .filter_map(|(path, bytes)| {
            path.strip_prefix("dep/")
                .filter(|path| *path != ".cargo-checksum.json")
                .map(|path| (path.to_owned(), bytes.clone()))
        })
        .collect();
    cargo_checksum(&package_files, package)
}

pub(super) fn canonical_tar(root: &str, files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut directories = BTreeSet::from([root.to_owned()]);
    for path in files.keys() {
        let mut current = root.to_owned();
        let components = path.split('/').collect::<Vec<_>>();
        for component in &components[..components.len() - 1] {
            current.push('/');
            current.push_str(component);
            directories.insert(current.clone());
        }
    }
    let mut output = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut output);
        let mut paths = directories
            .iter()
            .map(|path| (path.clone(), None))
            .chain(
                files
                    .iter()
                    .map(|(path, bytes)| (format!("{root}/{path}"), Some(bytes.as_slice()))),
            )
            .collect::<Vec<_>>();
        paths.sort_by(|left, right| left.0.cmp(&right.0));
        for (path, content) in paths {
            append_tar_entry(&mut builder, &path, content, None);
        }
        builder.finish().expect("fixture tar finishes");
    }
    output
}

pub(super) fn crate_archive(
    root: &str,
    files: &BTreeMap<String, Vec<u8>>,
    special: Option<EntryType>,
) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        for (path, bytes) in files {
            append_tar_entry(
                &mut builder,
                &format!("{root}/{path}"),
                Some(bytes),
                special,
            );
        }
        builder.finish().expect("crate tar finishes");
    }
    gzip(&tar_bytes)
}

pub(super) fn duplicate_crate_archive(root: &str, files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        for _ in 0..2 {
            for (path, bytes) in files {
                append_tar_entry(&mut builder, &format!("{root}/{path}"), Some(bytes), None);
            }
        }
        builder.finish().expect("crate tar finishes");
    }
    gzip(&tar_bytes)
}

pub(super) fn append_tar_entry<W: std::io::Write>(
    builder: &mut tar::Builder<W>,
    path: &str,
    content: Option<&[u8]>,
    special: Option<EntryType>,
) {
    let mut header = tar::Header::new_gnu();
    let entry_type = special.unwrap_or_else(|| {
        if content.is_some() {
            EntryType::Regular
        } else {
            EntryType::Directory
        }
    });
    header.set_entry_type(entry_type);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mode(if content.is_some() { 0o644 } else { 0o755 });
    header.set_mtime(EPOCH);
    header.set_size(content.map_or(0, |bytes| bytes.len() as u64));
    header.set_username("").expect("fixture username");
    header.set_groupname("").expect("fixture groupname");
    if entry_type.is_symlink() {
        header.set_link_name("target").expect("fixture link target");
    }
    header.set_cksum();
    builder
        .append_data(&mut header, path, content.unwrap_or_default())
        .expect("fixture entry appends");
}
