use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};
use tar::EntryType;

use super::{CargoSourceClosureError, CargoSourceClosureLimits, MemberMeasurement};
use crate::source_build::retained_program_closure::cargo_source::stream::{
    MeasuredReader, append, checked_count, digest,
};

const SOURCE_DATE_EPOCH: u64 = 1_725_000_000;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileFact {
    pub(super) digest: Digest,
    pub(super) size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TreeSnapshot {
    pub(super) captured: BTreeMap<String, Vec<u8>>,
    pub(super) files: BTreeMap<String, FileFact>,
    pub(super) id: Digest,
}

pub(super) fn validate_raw_tar<R, C>(
    stream: R,
    measurement: &MemberMeasurement,
    limits: CargoSourceClosureLimits,
    cancelled: &mut C,
) -> Result<(), CargoSourceClosureError>
where
    R: Read,
    C: FnMut() -> bool,
{
    let mut measured = MeasuredReader::new(stream, measurement, cancelled);
    let result = {
        let mut archive = tar::Archive::new(&mut measured);
        let validation = (|| {
            let entries = archive
                .entries()
                .map_err(|_| CargoSourceClosureError::InvalidArchive)?
                .raw(true);
            validate_raw_entries(entries, limits)
        })();
        if validation.is_ok() {
            archive.into_inner().validate_terminal_block()
        } else {
            validation
        }
    };
    if measured.cancellation_observed() {
        return Err(CargoSourceClosureError::Cancelled);
    }
    result?;
    measured.finish()
}

fn validate_raw_entries<R: Read>(
    entries: tar::Entries<'_, R>,
    limits: CargoSourceClosureLimits,
) -> Result<(), CargoSourceClosureError> {
    let mut count = 0_usize;
    let mut extension_bytes = 0_usize;
    let mut pending_long_name = false;
    for entry in entries {
        let mut entry = entry.map_err(|_| CargoSourceClosureError::InvalidArchive)?;
        count = checked_count(count, limits.maximum_archive_entries.saturating_mul(2))?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_gnu_longname() {
            if pending_long_name {
                return Err(CargoSourceClosureError::NoncanonicalArchive);
            }
            let size = usize::try_from(
                entry
                    .header()
                    .size()
                    .map_err(|_| CargoSourceClosureError::InvalidArchive)?,
            )
            .ok()
            .filter(|size| (2..=limits.maximum_path_bytes.saturating_add(1)).contains(size))
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
            if size <= 101 {
                return Err(CargoSourceClosureError::NoncanonicalArchive);
            }
            extension_bytes = extension_bytes
                .checked_add(size)
                .filter(|total| *total <= limits.maximum_total_path_bytes)
                .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
            let mut value = Vec::with_capacity(size);
            entry
                .read_to_end(&mut value)
                .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
            validate_long_name(&value, size)?;
            pending_long_name = true;
        } else if matches!(entry_type, EntryType::Regular | EntryType::Directory) {
            pending_long_name = false;
        } else {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
    }
    if count == 0 || pending_long_name {
        Err(CargoSourceClosureError::InvalidArchive)
    } else {
        Ok(())
    }
}

fn validate_long_name(value: &[u8], expected: usize) -> Result<(), CargoSourceClosureError> {
    if value.len() != expected
        || value.last() != Some(&0)
        || value[..value.len() - 1].contains(&0)
        || std::str::from_utf8(&value[..value.len() - 1]).is_err()
    {
        Err(CargoSourceClosureError::NonportableArchivePath)
    } else {
        Ok(())
    }
}

pub(super) fn scan_canonical_tar<R, C, P>(
    stream: R,
    measurement: &MemberMeasurement,
    root: &str,
    limits: CargoSourceClosureLimits,
    cancelled: &mut C,
    capture: P,
) -> Result<TreeSnapshot, CargoSourceClosureError>
where
    R: Read,
    C: FnMut() -> bool,
    P: Fn(&str) -> bool,
{
    let mut measured = MeasuredReader::new(stream, measurement, cancelled);
    let result = {
        let mut archive = tar::Archive::new(&mut measured);
        (|| {
            let entries = archive
                .entries()
                .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
            scan_canonical_entries(entries, root, limits, capture)
        })()
    };
    if measured.cancellation_observed() {
        return Err(CargoSourceClosureError::Cancelled);
    }
    let snapshot = result?;
    measured.finish()?;
    Ok(snapshot)
}

pub(super) fn scan_canonical_tar_with<R, C, F>(
    stream: R,
    measurement: &MemberMeasurement,
    root: &str,
    limits: CargoSourceClosureLimits,
    cancelled: &mut C,
    mut visit: F,
) -> Result<TreeSnapshot, CargoSourceClosureError>
where
    R: Read,
    C: FnMut() -> bool,
    F: FnMut(&str, u64, &mut dyn Read) -> Result<FileFact, CargoSourceClosureError>,
{
    let mut measured = MeasuredReader::new(stream, measurement, cancelled);
    let result = {
        let mut archive = tar::Archive::new(&mut measured);
        (|| {
            let entries = archive
                .entries()
                .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
            scan_entries_with(entries, root, limits, &mut visit)
        })()
    };
    if measured.cancellation_observed() {
        return Err(CargoSourceClosureError::Cancelled);
    }
    let snapshot = result?;
    measured.finish()?;
    Ok(snapshot)
}

fn scan_entries_with<R: Read, F>(
    entries: tar::Entries<'_, R>,
    root: &str,
    limits: CargoSourceClosureLimits,
    visit: &mut F,
) -> Result<TreeSnapshot, CargoSourceClosureError>
where
    F: FnMut(&str, u64, &mut dyn Read) -> Result<FileFact, CargoSourceClosureError>,
{
    validate_root(root)?;
    let mut state = ScanState::new(root);
    for entry in entries {
        let mut entry = entry.map_err(|_| CargoSourceClosureError::InvalidArchive)?;
        state.observe_with(&mut entry, limits, visit)?;
    }
    state.finish()
}

fn scan_canonical_entries<R: Read, P: Fn(&str) -> bool>(
    entries: tar::Entries<'_, R>,
    root: &str,
    limits: CargoSourceClosureLimits,
    capture: P,
) -> Result<TreeSnapshot, CargoSourceClosureError> {
    validate_root(root)?;
    let mut state = ScanState::new(root);
    for entry in entries {
        let mut entry = entry.map_err(|_| CargoSourceClosureError::InvalidArchive)?;
        state.observe(&mut entry, limits, &capture)?;
    }
    state.finish()
}

struct ScanState<'a> {
    captured: BTreeMap<String, Vec<u8>>,
    directories: BTreeSet<String>,
    files: BTreeMap<String, FileFact>,
    previous: Option<Vec<u8>>,
    portable_keys: BTreeSet<String>,
    root: &'a str,
    total_bytes: u64,
    total_path_bytes: usize,
}

impl<'a> ScanState<'a> {
    fn new(root: &'a str) -> Self {
        Self {
            captured: BTreeMap::new(),
            directories: BTreeSet::new(),
            files: BTreeMap::new(),
            previous: None,
            portable_keys: BTreeSet::new(),
            root,
            total_bytes: 0,
            total_path_bytes: 0,
        }
    }

    fn observe<R: Read, P: Fn(&str) -> bool>(
        &mut self,
        entry: &mut tar::Entry<'_, R>,
        limits: CargoSourceClosureLimits,
        capture: &P,
    ) -> Result<(), CargoSourceClosureError> {
        let mut retained_pair = None;
        self.observe_with(entry, limits, &mut |relative, declared, reader| {
            let (fact, retained) = hash_entry(reader, declared, capture(relative), limits)?;
            if let Some(bytes) = retained {
                retained_pair = Some((relative.to_owned(), bytes));
            }
            Ok(fact)
        })?;
        if let Some((relative, bytes)) = retained_pair {
            self.captured.insert(relative, bytes);
        }
        Ok(())
    }

    fn observe_with<R: Read, F>(
        &mut self,
        entry: &mut tar::Entry<'_, R>,
        limits: CargoSourceClosureLimits,
        visit: &mut F,
    ) -> Result<(), CargoSourceClosureError>
    where
        F: FnMut(&str, u64, &mut dyn Read) -> Result<FileFact, CargoSourceClosureError>,
    {
        let (path, entry_type) = self.begin_entry(entry, limits)?;
        if entry_type.is_dir() {
            self.directories.insert(path);
            return Ok(());
        }
        if !entry_type.is_file() || self.files.contains_key(&path) {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
        let declared = entry
            .header()
            .size()
            .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
        self.total_bytes = self
            .total_bytes
            .checked_add(declared)
            .filter(|total| *total <= limits.maximum_archive_bytes)
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
        let relative = strip_root(&path, self.root)?;
        let fact = visit(relative, declared, entry)?;
        if fact.size != declared {
            return Err(CargoSourceClosureError::InvalidArchive);
        }
        self.files.insert(relative.to_owned(), fact);
        Ok(())
    }

    fn begin_entry<R: Read>(
        &mut self,
        entry: &tar::Entry<'_, R>,
        limits: CargoSourceClosureLimits,
    ) -> Result<(String, EntryType), CargoSourceClosureError> {
        let count = self.files.len().saturating_add(self.directories.len());
        checked_count(count, limits.maximum_archive_entries)?;
        let raw_path = entry.path_bytes().into_owned();
        let path = validate_path(&raw_path, self.root, limits.maximum_path_bytes)?;
        if !self.portable_keys.insert(path.to_ascii_lowercase()) {
            return Err(CargoSourceClosureError::UnsafeArchiveEntry);
        }
        self.total_path_bytes = self
            .total_path_bytes
            .checked_add(raw_path.len())
            .filter(|total| *total <= limits.maximum_total_path_bytes)
            .ok_or(CargoSourceClosureError::ArchiveQuotaExceeded)?;
        if self
            .previous
            .as_ref()
            .is_some_and(|previous| previous >= &raw_path)
        {
            return Err(CargoSourceClosureError::NoncanonicalArchive);
        }
        self.previous = Some(raw_path);
        let entry_type = entry.header().entry_type();
        validate_header(entry.header(), entry_type)?;
        self.validate_parent_and_root(&path, entry_type)?;
        Ok((path, entry_type))
    }

    fn validate_parent_and_root(
        &self,
        path: &str,
        entry_type: EntryType,
    ) -> Result<(), CargoSourceClosureError> {
        if self
            .previous
            .as_ref()
            .is_some_and(|_| self.files.is_empty() && self.directories.is_empty())
            && (path != self.root || !entry_type.is_dir())
        {
            return Err(CargoSourceClosureError::ArchiveRootMismatch);
        }
        if path != self.root {
            let parent = path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .ok_or(CargoSourceClosureError::ArchiveRootMismatch)?;
            if !self.directories.contains(parent) {
                return Err(CargoSourceClosureError::NoncanonicalArchive);
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<TreeSnapshot, CargoSourceClosureError> {
        if !self.directories.contains(self.root) || self.files.is_empty() {
            return Err(CargoSourceClosureError::InvalidArchive);
        }
        let id = tree_id(&self.files);
        Ok(TreeSnapshot {
            captured: self.captured,
            files: self.files,
            id,
        })
    }
}

fn hash_entry<R: Read + ?Sized>(
    entry: &mut R,
    declared: u64,
    retain: bool,
    limits: CargoSourceClosureLimits,
) -> Result<(FileFact, Option<Vec<u8>>), CargoSourceClosureError> {
    if retain && declared > limits.maximum_captured_file_bytes {
        return Err(CargoSourceClosureError::ArchiveQuotaExceeded);
    }
    let capacity = retain
        .then(|| usize::try_from(declared))
        .transpose()
        .map_err(|_| CargoSourceClosureError::ArchiveQuotaExceeded)?;
    let mut captured = capacity.map(Vec::with_capacity);
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES].into_boxed_slice();
    loop {
        let read = entry
            .read(&mut buffer)
            .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .filter(|total| *total <= declared)
            .ok_or(CargoSourceClosureError::InvalidArchive)?;
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut captured {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    if total != declared {
        return Err(CargoSourceClosureError::InvalidArchive);
    }
    Ok((
        FileFact {
            digest: digest(hasher),
            size: total,
        },
        captured,
    ))
}

fn validate_header(
    header: &tar::Header,
    entry_type: EntryType,
) -> Result<(), CargoSourceClosureError> {
    let size = header
        .size()
        .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
    let mode = header
        .mode()
        .map_err(|_| CargoSourceClosureError::InvalidArchive)?;
    if header.as_gnu().is_none()
        || header.username_bytes() != Some(b"".as_slice())
        || header.groupname_bytes() != Some(b"".as_slice())
        || header
            .uid()
            .map_err(|_| CargoSourceClosureError::InvalidArchive)?
            != 0
        || header
            .gid()
            .map_err(|_| CargoSourceClosureError::InvalidArchive)?
            != 0
        || header
            .mtime()
            .map_err(|_| CargoSourceClosureError::InvalidArchive)?
            != SOURCE_DATE_EPOCH
        || (entry_type.is_dir() && (size != 0 || mode != 0o755))
        || (entry_type.is_file() && !matches!(mode, 0o644 | 0o755))
    {
        Err(CargoSourceClosureError::NoncanonicalArchive)
    } else {
        Ok(())
    }
}

pub(super) fn validate_portable_relative_path(
    path: &str,
    maximum: usize,
) -> Result<(), CargoSourceClosureError> {
    if path.is_empty()
        || path.len() > maximum
        || !path.is_ascii()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
    {
        return Err(CargoSourceClosureError::NonportableArchivePath);
    }
    for component in path.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || component.starts_with(' ')
            || component.ends_with([' ', '.'])
            || component.contains([':', '<', '>', '"', '|', '?', '*'])
            || reserved_name(component.split('.').next().unwrap_or(component))
        {
            return Err(CargoSourceClosureError::NonportableArchivePath);
        }
    }
    Ok(())
}

fn validate_path(
    bytes: &[u8],
    root: &str,
    maximum: usize,
) -> Result<String, CargoSourceClosureError> {
    let path =
        std::str::from_utf8(bytes).map_err(|_| CargoSourceClosureError::NonportableArchivePath)?;
    validate_portable_relative_path(path, maximum)?;
    if path != root && !path.starts_with(&format!("{root}/")) {
        return Err(CargoSourceClosureError::ArchiveRootMismatch);
    }
    Ok(path.to_owned())
}

fn validate_root(root: &str) -> Result<(), CargoSourceClosureError> {
    validate_portable_relative_path(root, 64)?;
    if root.contains('/') {
        Err(CargoSourceClosureError::ArchiveRootMismatch)
    } else {
        Ok(())
    }
}

fn strip_root<'a>(path: &'a str, root: &str) -> Result<&'a str, CargoSourceClosureError> {
    path.strip_prefix(root)
        .and_then(|path| path.strip_prefix('/'))
        .filter(|path| !path.is_empty())
        .ok_or(CargoSourceClosureError::ArchiveRootMismatch)
}

fn reserved_name(value: &str) -> bool {
    value.eq_ignore_ascii_case("con")
        || value.eq_ignore_ascii_case("prn")
        || value.eq_ignore_ascii_case("aux")
        || value.eq_ignore_ascii_case("nul")
        || (value.len() == 4
            && (value[..3].eq_ignore_ascii_case("com") || value[..3].eq_ignore_ascii_case("lpt"))
            && matches!(value.as_bytes()[3], b'1'..=b'9'))
}

fn tree_id(files: &BTreeMap<String, FileFact>) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"retonr:cargo-source-tree:v1\0");
    for (path, fact) in files {
        append(&mut hasher, path.as_bytes());
        hasher.update(fact.size.to_be_bytes());
        append(&mut hasher, fact.digest.as_str().as_bytes());
    }
    digest(hasher)
}
