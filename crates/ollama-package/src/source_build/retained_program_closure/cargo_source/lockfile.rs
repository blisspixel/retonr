use std::{collections::BTreeSet, str::FromStr as _};

use cargo_lock::{Lockfile, Package, ResolveVersion};
use rewrite_types::Digest;

use super::{CargoSourceClosureError, CargoSourceClosureLimits};

const CRATES_IO_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct PackageKey {
    pub(super) name: String,
    pub(super) source: Option<String>,
    pub(super) version: String,
}

impl PackageKey {
    pub(super) fn from_package(package: &Package) -> Self {
        Self {
            name: package.name.to_string(),
            source: package.source.as_ref().map(ToString::to_string),
            version: package.version.to_string(),
        }
    }
}

pub(super) struct LockReview {
    pub(super) edge_count: usize,
    pub(super) lockfile: Lockfile,
    pub(super) path_packages: BTreeSet<PackageKey>,
    pub(super) registry_packages: BTreeSet<PackageKey>,
}

pub(super) fn parse_lockfile(
    bytes: &[u8],
    limits: CargoSourceClosureLimits,
) -> Result<LockReview, CargoSourceClosureError> {
    if bytes.is_empty() || bytes.len() > limits.maximum_lock_bytes {
        return Err(CargoSourceClosureError::LockfileQuotaExceeded);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| CargoSourceClosureError::InvalidLockfile)?;
    let lockfile =
        Lockfile::from_str(text).map_err(|_| CargoSourceClosureError::InvalidLockfile)?;
    if lockfile.version != ResolveVersion::V4
        || lockfile.root.is_some()
        || lockfile.packages.is_empty()
        || lockfile.packages.len() > limits.maximum_packages
    {
        return Err(CargoSourceClosureError::InvalidLockfile);
    }
    validate_textual_checksums(text, lockfile.packages.len())?;
    let mut keys = BTreeSet::new();
    let mut registry_packages = BTreeSet::new();
    let mut path_packages = BTreeSet::new();
    let mut string_bytes = 0_usize;
    let mut edge_count = 0_usize;
    for package in &lockfile.packages {
        validate_package(package, &mut string_bytes, limits.maximum_string_bytes)?;
        let key = PackageKey::from_package(package);
        if !keys.insert(key.clone()) {
            return Err(CargoSourceClosureError::DuplicatePackageIdentity);
        }
        if package.source.is_some() {
            registry_packages.insert(key);
        } else {
            path_packages.insert(key);
        }
        edge_count = edge_count
            .checked_add(package.dependencies.len())
            .filter(|count| *count <= limits.maximum_dependency_edges)
            .ok_or(CargoSourceClosureError::LockfileQuotaExceeded)?;
    }
    validate_edges(&lockfile, &mut string_bytes, limits.maximum_string_bytes)?;
    Ok(LockReview {
        edge_count,
        lockfile,
        path_packages,
        registry_packages,
    })
}

fn validate_package(
    package: &Package,
    string_bytes: &mut usize,
    maximum: usize,
) -> Result<(), CargoSourceClosureError> {
    account(string_bytes, package.name.as_str(), maximum)?;
    account(string_bytes, &package.version.to_string(), maximum)?;
    if package.replace.is_some() {
        return Err(CargoSourceClosureError::InvalidLockfile);
    }
    match (&package.source, &package.checksum) {
        (Some(source), Some(checksum))
            if source.to_string() == CRATES_IO_SOURCE && checksum.is_sha256() =>
        {
            account(string_bytes, CRATES_IO_SOURCE, maximum)?;
            account(string_bytes, &checksum.to_string(), maximum)
        }
        (None, None) => Ok(()),
        _ => Err(CargoSourceClosureError::UnsupportedPackageSource),
    }
}

fn validate_edges(
    lockfile: &Lockfile,
    string_bytes: &mut usize,
    maximum: usize,
) -> Result<(), CargoSourceClosureError> {
    for package in &lockfile.packages {
        for dependency in &package.dependencies {
            account(string_bytes, dependency.name.as_str(), maximum)?;
            account(string_bytes, &dependency.version.to_string(), maximum)?;
            if let Some(source) = &dependency.source {
                let source = source.to_string();
                account(string_bytes, &source, maximum)?;
                if source != CRATES_IO_SOURCE {
                    return Err(CargoSourceClosureError::UnsupportedPackageSource);
                }
            }
            let matches = lockfile
                .packages
                .iter()
                .filter(|candidate| {
                    candidate.name == dependency.name
                        && candidate.version == dependency.version
                        && dependency
                            .source
                            .as_ref()
                            .is_none_or(|source| candidate.source.as_ref() == Some(source))
                })
                .count();
            if matches != 1 {
                return Err(CargoSourceClosureError::AmbiguousDependencyEdge);
            }
        }
    }
    Ok(())
}

fn validate_textual_checksums(
    text: &str,
    expected_packages: usize,
) -> Result<(), CargoSourceClosureError> {
    let value = toml::from_str::<toml::Value>(text)
        .map_err(|_| CargoSourceClosureError::InvalidLockfile)?;
    let packages = value
        .get("package")
        .and_then(toml::Value::as_array)
        .filter(|packages| packages.len() == expected_packages)
        .ok_or(CargoSourceClosureError::InvalidLockfile)?;
    for package in packages {
        let table = package
            .as_table()
            .ok_or(CargoSourceClosureError::InvalidLockfile)?;
        match (table.get("source"), table.get("checksum")) {
            (Some(source), Some(checksum))
                if source.as_str() == Some(CRATES_IO_SOURCE)
                    && checksum.as_str().is_some_and(valid_lower_sha256) => {}
            (None, None) => {}
            _ => return Err(CargoSourceClosureError::UnsupportedPackageSource),
        }
    }
    Ok(())
}

fn valid_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn account(total: &mut usize, value: &str, maximum: usize) -> Result<(), CargoSourceClosureError> {
    *total = total
        .checked_add(value.len())
        .filter(|total| *total <= maximum)
        .ok_or(CargoSourceClosureError::LockfileQuotaExceeded)?;
    Ok(())
}

pub(super) fn lock_digest(bytes: &[u8]) -> Digest {
    Digest::sha256(bytes)
}
