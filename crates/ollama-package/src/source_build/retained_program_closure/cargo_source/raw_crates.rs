use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read},
};

use cargo_lock::Package;
use flate2::read::MultiGzDecoder;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::{CargoSourceClosureError, CargoSourceClosureLimits};
use crate::source_build::retained_program_closure::cargo_source::{
    archive::{FileFact, validate_portable_relative_path},
    stream::digest,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RawPackageTree {
    pub(super) cargo_toml: Vec<u8>,
    pub(super) files: BTreeMap<String, FileFact>,
    pub(super) package_checksum: Digest,
}

pub(super) fn expected_crate_name(package: &Package) -> String {
    format!("{}-{}.crate", package.name, package.version)
}

pub(super) fn scan_crate(
    stream: &mut dyn Read,
    declared: u64,
    package: &Package,
    limits: CargoSourceClosureLimits,
) -> Result<(FileFact, RawPackageTree), CargoSourceClosureError> {
    if declared > limits.maximum_crate_bytes {
        return Err(CargoSourceClosureError::ArchiveQuotaExceeded);
    }
    let expected_checksum = package
        .checksum
        .as_ref()
        .ok_or(CargoSourceClosureError::InvalidLockfile)?
        .to_string();
    let mut raw = CrateReader::new(stream, declared);
    let mut decoder = MultiGzDecoder::new(&mut raw);
    let tree = scan_inner_tar(&mut decoder, package, limits)?;
    io::copy(&mut decoder, &mut io::sink())
        .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?;
    drop(decoder);
    let raw_fact = raw.finish()?;
    if raw_fact.digest.as_str() != expected_checksum {
        return Err(CargoSourceClosureError::CrateChecksumMismatch);
    }
    let package_checksum = raw_fact.digest.clone();
    Ok((
        raw_fact,
        RawPackageTree {
            cargo_toml: tree.cargo_toml,
            files: tree.files,
            package_checksum,
        },
    ))
}

struct InnerTree {
    cargo_toml: Vec<u8>,
    files: BTreeMap<String, FileFact>,
}

fn scan_inner_tar<R: Read>(
    stream: R,
    package: &Package,
    limits: CargoSourceClosureLimits,
) -> Result<InnerTree, CargoSourceClosureError> {
    let root = format!("{}-{}", package.name, package.version);
    let mut archive = tar::Archive::new(stream);
    let entries = archive
        .entries()
        .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?
        .raw(true);
    let mut directories = BTreeSet::new();
    let mut files = BTreeMap::new();
    let mut portable_keys = BTreeSet::new();
    let mut cargo_toml = None;
    let mut total_bytes = 0_u64;
    let mut total_paths = 0_usize;
    let mut count = 0_usize;
    let mut pending_long_name = None;
    for entry in entries {
        let mut entry = entry.map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?;
        count = count
            .checked_add(1)
            .filter(|count| *count <= limits.maximum_crate_entries)
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_gnu_longname() {
            if pending_long_name.is_some() {
                return Err(CargoSourceClosureError::UnsafeArchiveEntry);
            }
            pending_long_name = Some(read_inner_long_name(&mut entry, limits.maximum_path_bytes)?);
            continue;
        }
        if !matches!(
            entry_type,
            tar::EntryType::Regular | tar::EntryType::Directory
        ) {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
        let path_bytes = pending_long_name
            .take()
            .unwrap_or_else(|| entry.path_bytes().into_owned());
        let path = validate_inner_path(&path_bytes, &root, limits.maximum_path_bytes)?;
        if !portable_keys.insert(path.to_ascii_lowercase()) {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
        total_paths = total_paths
            .checked_add(path.len())
            .filter(|total| *total <= limits.maximum_total_path_bytes)
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
        if entry_type.is_dir() {
            if !directories.insert(path) {
                return Err(CargoSourceClosureError::UnsafeArchiveEntry);
            }
            continue;
        }
        if !entry_type.is_file() {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
        let relative = path
            .strip_prefix(&root)
            .and_then(|path| path.strip_prefix('/'))
            .ok_or(CargoSourceClosureError::ArchiveRootMismatch)?;
        let declared = entry
            .header()
            .size()
            .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?;
        total_bytes = total_bytes
            .checked_add(declared)
            .filter(|total| *total <= limits.maximum_crate_unpacked_bytes)
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
        let retain = relative == "Cargo.toml";
        let (fact, captured) = hash_inner_file(&mut entry, declared, retain, limits)?;
        if files.insert(relative.to_owned(), fact).is_some() {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
        if let Some(bytes) = captured {
            cargo_toml = Some(bytes);
        }
    }
    if pending_long_name.is_some() {
        return Err(CargoSourceClosureError::InvalidCrateArchive);
    }
    let mut stream = archive.into_inner();
    validate_inner_terminal(&mut stream)?;
    Ok(InnerTree {
        cargo_toml: cargo_toml.ok_or(CargoSourceClosureError::CrateManifestMismatch)?,
        files,
    })
}

fn read_inner_long_name<R: Read>(
    entry: &mut R,
    maximum: usize,
) -> Result<Vec<u8>, CargoSourceClosureError> {
    let mut value = Vec::new();
    entry
        .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(2))
        .read_to_end(&mut value)
        .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?;
    if value.len() <= 101
        || value.len() > maximum.saturating_add(1)
        || value.last() != Some(&0)
        || value[..value.len() - 1].contains(&0)
    {
        return Err(CargoSourceClosureError::NonportableArchivePath);
    }
    value.pop();
    Ok(value)
}

fn validate_inner_terminal<R: Read>(stream: &mut R) -> Result<(), CargoSourceClosureError> {
    let mut terminal = [0_u8; 512];
    stream
        .read_exact(&mut terminal)
        .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?;
    if terminal.iter().any(|byte| *byte != 0) {
        return Err(CargoSourceClosureError::InvalidCrateArchive);
    }
    let mut extra = [0_u8; 1];
    if stream
        .read(&mut extra)
        .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?
        != 0
    {
        return Err(CargoSourceClosureError::InvalidCrateArchive);
    }
    Ok(())
}

fn validate_inner_path(
    bytes: &[u8],
    root: &str,
    maximum: usize,
) -> Result<String, CargoSourceClosureError> {
    let path =
        std::str::from_utf8(bytes).map_err(|_| CargoSourceClosureError::NonportableArchivePath)?;
    validate_portable_relative_path(path, maximum)?;
    if path == root || path.starts_with(&format!("{root}/")) {
        Ok(path.to_owned())
    } else {
        Err(CargoSourceClosureError::ArchiveRootMismatch)
    }
}

fn hash_inner_file<R: Read>(
    stream: &mut R,
    declared: u64,
    retain: bool,
    limits: CargoSourceClosureLimits,
) -> Result<(FileFact, Option<Vec<u8>>), CargoSourceClosureError> {
    if retain && declared > limits.maximum_captured_file_bytes {
        return Err(CargoSourceClosureError::ArchiveQuotaExceeded);
    }
    let mut retained = if retain { Some(Vec::new()) } else { None };
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES].into_boxed_slice();
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|_| CargoSourceClosureError::InvalidCrateArchive)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|total| *total <= declared)
            .ok_or(CargoSourceClosureError::InvalidCrateArchive)?;
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    if total != declared {
        return Err(CargoSourceClosureError::InvalidCrateArchive);
    }
    Ok((
        FileFact {
            digest: digest(hasher),
            size: total,
        },
        retained,
    ))
}

struct CrateReader<'a> {
    declared: u64,
    hasher: Sha256,
    inner: &'a mut dyn Read,
    read: u64,
}

impl<'a> CrateReader<'a> {
    fn new(inner: &'a mut dyn Read, declared: u64) -> Self {
        Self {
            declared,
            hasher: Sha256::new(),
            inner,
            read: 0,
        }
    }

    fn finish(self) -> Result<FileFact, CargoSourceClosureError> {
        if self.read != self.declared {
            return Err(CargoSourceClosureError::InvalidCrateArchive);
        }
        Ok(FileFact {
            digest: digest(self.hasher),
            size: self.read,
        })
    }
}

impl Read for CrateReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.read = self
            .read
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|read| *read <= self.declared)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        self.hasher.update(&buffer[..read]);
        Ok(read)
    }
}
