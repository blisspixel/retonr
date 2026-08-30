use std::{collections::BTreeMap, fmt, io::Read};

use cargo_lock::Package;
use rewrite_types::Digest;
use serde::{Deserialize, Deserializer, de};

use super::{CargoSourceClosureError, CargoSourceClosureLimits, MemberMeasurement};
use crate::source_build::retained_program_closure::cargo_source::{
    archive::{FileFact, TreeSnapshot, scan_canonical_tar_with},
    lockfile::{LockReview, PackageKey},
    raw_crates::{RawPackageTree, expected_crate_name, scan_crate},
};

const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub(super) struct RawBundle {
    pub(super) packages: BTreeMap<PackageKey, RawPackageTree>,
    pub(super) source_file_count: usize,
    pub(super) tree_id: Digest,
}

pub(super) fn scan_raw_bundle<R, C>(
    stream: R,
    measurement: &MemberMeasurement,
    lock: &LockReview,
    limits: CargoSourceClosureLimits,
    cancelled: &mut C,
) -> Result<RawBundle, CargoSourceClosureError>
where
    R: Read,
    C: FnMut() -> bool,
{
    let expected = expected_packages(lock)?;
    let mut packages = BTreeMap::new();
    let snapshot = scan_canonical_tar_with(
        stream,
        measurement,
        "cargo-crates",
        limits,
        cancelled,
        |path, declared, reader| {
            if path.contains('/') {
                return Err(CargoSourceClosureError::RawCrateSetMismatch);
            }
            let package = expected
                .get(path)
                .ok_or(CargoSourceClosureError::RawCrateSetMismatch)?;
            let (fact, tree) = scan_crate(reader, declared, package, limits)?;
            verify_package_manifest(&tree.cargo_toml, package)?;
            if packages
                .insert(PackageKey::from_package(package), tree)
                .is_some()
            {
                return Err(CargoSourceClosureError::RawCrateSetMismatch);
            }
            Ok(fact)
        },
    )?;
    if packages.len() != expected.len() {
        return Err(CargoSourceClosureError::RawCrateSetMismatch);
    }
    let source_file_count = packages.values().try_fold(0_usize, |total, tree| {
        total
            .checked_add(tree.files.len())
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)
    })?;
    Ok(RawBundle {
        packages,
        source_file_count,
        tree_id: snapshot.id,
    })
}

fn expected_packages(
    lock: &LockReview,
) -> Result<BTreeMap<String, &Package>, CargoSourceClosureError> {
    let mut expected = BTreeMap::new();
    for package in lock
        .lockfile
        .packages
        .iter()
        .filter(|package| package.source.is_some())
    {
        if expected
            .insert(expected_crate_name(package), package)
            .is_some()
        {
            return Err(CargoSourceClosureError::RawCrateSetMismatch);
        }
    }
    Ok(expected)
}

fn verify_package_manifest(bytes: &[u8], package: &Package) -> Result<(), CargoSourceClosureError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| CargoSourceClosureError::CrateManifestMismatch)?;
    let manifest = toml::from_str::<toml::Value>(text)
        .map_err(|_| CargoSourceClosureError::CrateManifestMismatch)?;
    let table = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or(CargoSourceClosureError::CrateManifestMismatch)?;
    if table.get("name").and_then(toml::Value::as_str) != Some(package.name.as_str())
        || table.get("version").and_then(toml::Value::as_str)
            != Some(package.version.to_string().as_str())
        || package.source.as_ref().map(ToString::to_string).as_deref() != Some(CRATES_IO_SOURCE)
    {
        return Err(CargoSourceClosureError::CrateManifestMismatch);
    }
    Ok(())
}

pub(super) fn verify_vendor(
    snapshot: &TreeSnapshot,
    raw: &RawBundle,
) -> Result<(), CargoSourceClosureError> {
    let groups = group_vendor_files(snapshot)?;
    let mut seen = BTreeMap::new();
    for (directory, files) in groups {
        let manifest = snapshot
            .captured
            .get(&format!("{directory}/Cargo.toml"))
            .ok_or(CargoSourceClosureError::VendorTreeMismatch)?;
        let key = registry_manifest_key(manifest)?;
        let raw_tree = raw
            .packages
            .get(&key)
            .ok_or(CargoSourceClosureError::VendorTreeMismatch)?;
        if seen.insert(key, directory.clone()).is_some() {
            return Err(CargoSourceClosureError::VendorTreeMismatch);
        }
        let checksum = snapshot
            .captured
            .get(&format!("{directory}/.cargo-checksum.json"))
            .ok_or(CargoSourceClosureError::VendorChecksumMismatch)?;
        compare_vendor_package(&files, checksum, raw_tree)?;
    }
    if seen.len() != raw.packages.len() {
        return Err(CargoSourceClosureError::VendorTreeMismatch);
    }
    Ok(())
}

fn group_vendor_files(
    snapshot: &TreeSnapshot,
) -> Result<BTreeMap<String, BTreeMap<String, FileFact>>, CargoSourceClosureError> {
    let mut groups = BTreeMap::<String, BTreeMap<String, FileFact>>::new();
    for (path, fact) in &snapshot.files {
        let (directory, relative) = path
            .split_once('/')
            .filter(|(_, relative)| !relative.is_empty())
            .ok_or(CargoSourceClosureError::VendorTreeMismatch)?;
        if groups
            .entry(directory.to_owned())
            .or_default()
            .insert(relative.to_owned(), fact.clone())
            .is_some()
        {
            return Err(CargoSourceClosureError::VendorTreeMismatch);
        }
    }
    Ok(groups)
}

fn registry_manifest_key(bytes: &[u8]) -> Result<PackageKey, CargoSourceClosureError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| CargoSourceClosureError::CrateManifestMismatch)?;
    let manifest = toml::from_str::<toml::Value>(text)
        .map_err(|_| CargoSourceClosureError::CrateManifestMismatch)?;
    let package = manifest
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or(CargoSourceClosureError::CrateManifestMismatch)?;
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or(CargoSourceClosureError::CrateManifestMismatch)?;
    let version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .ok_or(CargoSourceClosureError::CrateManifestMismatch)?;
    Ok(PackageKey {
        name: name.to_owned(),
        source: Some(CRATES_IO_SOURCE.to_owned()),
        version: version.to_owned(),
    })
}

fn compare_vendor_package(
    files: &BTreeMap<String, FileFact>,
    checksum_bytes: &[u8],
    raw: &RawPackageTree,
) -> Result<(), CargoSourceClosureError> {
    let checksum: CargoChecksum = serde_json::from_slice(checksum_bytes)
        .map_err(|_| CargoSourceClosureError::VendorChecksumMismatch)?;
    if checksum.package != raw.package_checksum.as_str() || checksum.files.len() != raw.files.len()
    {
        return Err(CargoSourceClosureError::VendorChecksumMismatch);
    }
    let actual = files
        .iter()
        .filter(|(path, _)| path.as_str() != ".cargo-checksum.json")
        .collect::<BTreeMap<_, _>>();
    if actual.len() != raw.files.len() {
        return Err(CargoSourceClosureError::VendorTreeMismatch);
    }
    for (path, raw_fact) in &raw.files {
        let vendor_fact = actual
            .get(path)
            .ok_or(CargoSourceClosureError::VendorTreeMismatch)?;
        if *vendor_fact != raw_fact
            || checksum.files.get(path).map(String::as_str) != Some(raw_fact.digest.as_str())
        {
            return Err(CargoSourceClosureError::VendorChecksumMismatch);
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CargoChecksum {
    #[serde(deserialize_with = "deserialize_files")]
    files: BTreeMap<String, String>,
    package: String,
}

fn deserialize_files<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct FilesVisitor;

    impl<'de> de::Visitor<'de> for FilesVisitor {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a collision-free Cargo checksum file map")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            let mut files = BTreeMap::new();
            while let Some((path, digest)) = map.next_entry::<String, String>()? {
                if files.insert(path, digest).is_some() {
                    return Err(de::Error::custom("duplicate Cargo checksum path"));
                }
            }
            Ok(files)
        }
    }

    deserializer.deserialize_map(FilesVisitor)
}
