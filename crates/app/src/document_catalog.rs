//! Bounded, read-only filesystem catalogs shared by presentation layers.

use crate::{
    document_intake::{DocumentIntakeError, DocumentIntakeObservation, DocumentIntakeService},
    document_selection::RelativeDocumentPath,
};
use rewrite_types::CancellationToken;
use std::{
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

/// Maximum filesystem entries inspected during one catalog operation.
pub const MAX_DOCUMENT_CATALOG_ENTRIES: usize = 4096;
/// Maximum nested directory depth below the explicitly selected root.
pub const MAX_DOCUMENT_CATALOG_DEPTH: usize = 8;
/// Maximum combined raw document bytes inspected in one catalog.
pub const MAX_DOCUMENT_CATALOG_BYTES: usize = 64 * 1024 * 1024;

/// Caller budgets, bounded by the application hard ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentCatalogLimits {
    /// Maximum entries, including skipped entries and directories.
    pub entries: usize,
    /// Maximum nested depth when recursion is requested.
    pub depth: usize,
    /// Maximum combined raw bytes, including unsupported encodings.
    pub bytes: usize,
}
impl Default for DocumentCatalogLimits {
    fn default() -> Self {
        Self {
            entries: MAX_DOCUMENT_CATALOG_ENTRIES,
            depth: MAX_DOCUMENT_CATALOG_DEPTH,
            bytes: MAX_DOCUMENT_CATALOG_BYTES,
        }
    }
}

/// Complete inventory of one coherently read document; no content is retained.
#[derive(Debug)]
pub struct CatalogEntry {
    /// Exact validated path relative to the selected root.
    pub relative_path: RelativeDocumentPath,
    /// Encoding, complete-input digest and metadata observations.
    pub observation: DocumentIntakeObservation,
}

/// Explicit reason a discovered entry was not inventoried.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CatalogSkippedReason {
    /// A child directory when recursion was not requested.
    Directory,
    /// A child directory beyond the recursion ceiling.
    DepthLimit,
    /// A nonportable or undecodable filename.
    MalformedName,
    /// A hidden entry.
    Hidden,
    /// A development output or dependency directory.
    Ignored,
    /// Entry metadata could not be read.
    Unreadable,
    /// A symbolic link that discovery does not follow.
    Symlink,
    /// An entry that is neither directory nor regular file.
    NonRegular,
}
impl CatalogSkippedReason {
    /// Stable machine-readable reason for presentation adapters.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::DepthLimit => "depth_limit",
            Self::MalformedName => "malformed_name",
            Self::Hidden => "hidden",
            Self::Ignored => "ignored",
            Self::Unreadable => "unreadable",
            Self::Symlink => "symlink",
            Self::NonRegular => "non_regular",
        }
    }
}

/// One explicit skipped entry, with no invented replacement path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogSkippedEntry {
    /// Exact relative path when its filename is representable.
    pub relative_path: Option<String>,
    /// Reason this entry was not inspected.
    pub reason: CatalogSkippedReason,
}

/// Sorted complete discovery result, without rewrite or model authority.
#[derive(Debug)]
pub struct DocumentCatalog {
    /// Inventoried regular files, including unsupported text encodings.
    pub documents: Vec<CatalogEntry>,
    /// Explicit skipped-entry observations.
    pub skipped: Vec<CatalogSkippedEntry>,
}

/// Exact-path counterpart availability, without implying a candidate check.
#[derive(Debug)]
pub enum CatalogCounterpart<'a> {
    /// A supported document is available for a subsequent bounded review.
    Matched(&'a CatalogEntry),
    /// No entry with this exact relative path was discovered.
    Missing,
    /// The exact entry was explicitly skipped during discovery.
    Skipped(&'a CatalogSkippedEntry),
    /// The exact document has an unsupported encoding.
    Unsupported(&'a CatalogEntry),
    /// Supported metadata requires an explicit derivative decision.
    DecisionRequired(&'a CatalogEntry),
}

impl DocumentCatalog {
    /// Finds an exact, case-sensitive counterpart without normalizing labels.
    #[must_use]
    pub fn counterpart(&self, relative: &RelativeDocumentPath) -> CatalogCounterpart<'_> {
        use crate::document_intake::DerivativeDisposition;
        if let Some(entry) = self
            .documents
            .iter()
            .find(|entry| &entry.relative_path == relative)
        {
            if entry.observation.inventory.encoding != crate::TextEncoding::Utf8 {
                return CatalogCounterpart::Unsupported(entry);
            }
            if entry.observation.derivative != DerivativeDisposition::NotRequired {
                return CatalogCounterpart::DecisionRequired(entry);
            }
            return CatalogCounterpart::Matched(entry);
        }
        self.skipped
            .iter()
            .find(|entry| entry.relative_path.as_deref() == Some(relative.as_str()))
            .map_or(CatalogCounterpart::Missing, CatalogCounterpart::Skipped)
    }
}

/// Content-redacted catalog failure. Partial catalogs are never returned.
#[derive(thiserror::Error)]
pub enum DocumentCatalogError {
    /// Original caller cancellation discarded the operation.
    #[error("document catalog cancelled")]
    Cancelled,
    /// A caller budget or hard ceiling was exceeded.
    #[error("document catalog resource limit exceeded")]
    ResourceLimitExceeded,
    /// A selected path cannot satisfy the relative path contract.
    #[error("invalid document catalog path")]
    InvalidPath,
    /// Filesystem input could not be read coherently.
    #[error("document catalog input unreadable")]
    Input(#[source] std::io::Error),
    /// Complete byte or metadata inspection failed.
    #[error("document catalog intake failed")]
    Intake(#[source] DocumentIntakeError),
}

impl std::fmt::Debug for DocumentCatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "DocumentCatalogError::Cancelled",
            Self::ResourceLimitExceeded => "DocumentCatalogError::ResourceLimitExceeded",
            Self::InvalidPath => "DocumentCatalogError::InvalidPath",
            Self::Input(_) => "DocumentCatalogError::Input",
            Self::Intake(_) => "DocumentCatalogError::Intake",
        })
    }
}

/// Stateless catalog service with no rendering, serialization or write authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct DocumentCatalogService;
impl DocumentCatalogService {
    /// Discovers bounded regular-file inventories under an explicitly selected root.
    ///
    /// # Errors
    /// Refuses cancellation, invalid roots, excessive budgets, read failures and
    /// limits exceeded during accumulation. Each byte read uses retained parents.
    pub fn discover(
        root: &Path,
        recursive: bool,
        limits: DocumentCatalogLimits,
        cancellation: &CancellationToken,
    ) -> Result<DocumentCatalog, DocumentCatalogError> {
        if cancellation.is_cancelled() {
            return Err(DocumentCatalogError::Cancelled);
        }
        let outcome = (|| {
            if limits.entries > MAX_DOCUMENT_CATALOG_ENTRIES
                || limits.depth > MAX_DOCUMENT_CATALOG_DEPTH
                || limits.bytes > MAX_DOCUMENT_CATALOG_BYTES
            {
                return Err(DocumentCatalogError::ResourceLimitExceeded);
            }
            let metadata = fs::symlink_metadata(root).map_err(DocumentCatalogError::Input)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(DocumentCatalogError::InvalidPath);
            }
            walk_cancellable(
                root,
                recursive,
                limits.entries,
                limits.depth,
                limits.bytes,
                cancellation,
            )
        })();
        if cancellation.is_cancelled() {
            return Err(DocumentCatalogError::Cancelled);
        }
        let (mut documents, mut skipped) = outcome?;
        documents.sort_by(|left, right| {
            left.relative_path
                .as_str()
                .cmp(right.relative_path.as_str())
        });
        skipped.sort_by(|left, right| {
            left.relative_path
                .cmp(&right.relative_path)
                .then(left.reason.cmp(&right.reason))
        });
        if cancellation.is_cancelled() {
            return Err(DocumentCatalogError::Cancelled);
        }
        Ok(DocumentCatalog { documents, skipped })
    }
}

fn walk_cancellable(
    directory: &Path,
    recursive: bool,
    max_entries: usize,
    max_depth: usize,
    maximum_bytes: usize,
    cancellation: &rewrite_types::CancellationToken,
) -> Result<(Vec<CatalogEntry>, Vec<CatalogSkippedEntry>), DocumentCatalogError> {
    let mut pending = vec![Frame {
        path: directory.to_path_buf(),
        relative: String::new(),
        depth: 0,
    }];
    let mut documents = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = 0_usize;
    let mut inspected_bytes = 0_usize;
    while let Some(frame) = pending.pop() {
        if cancellation.is_cancelled() {
            return Err(DocumentCatalogError::Cancelled);
        }
        let mut entries =
            read_sorted_entries(&frame.path, max_entries.saturating_sub(seen), cancellation)?;
        seen = seen.saturating_add(entries.len());
        if seen > max_entries {
            return Err(DocumentCatalogError::ResourceLimitExceeded);
        }
        for entry in entries.drain(..) {
            if cancellation.is_cancelled() {
                return Err(DocumentCatalogError::Cancelled);
            }
            match classify_entry(&entry, &frame.relative) {
                Class::Document {
                    relative_path,
                    path,
                } => {
                    let remaining_bytes = maximum_bytes.saturating_sub(inspected_bytes);
                    let metadata =
                        fs::symlink_metadata(&path).map_err(DocumentCatalogError::Input)?;
                    if metadata.len() > remaining_bytes as u64 {
                        return Err(DocumentCatalogError::ResourceLimitExceeded);
                    }
                    let relative = path
                        .strip_prefix(directory)
                        .map_err(|_| DocumentCatalogError::InvalidPath)?;
                    let bytes = crate::document_input::read_directory_bounded(
                        directory,
                        relative,
                        remaining_bytes.min(crate::MAX_CANDIDATE_CHECK_BYTES),
                    )
                    .map_err(DocumentCatalogError::Input)?;
                    let observation =
                        DocumentIntakeService::inspect_bytes(Some(&path), &bytes, cancellation)
                            .map_err(|error| match error {
                                DocumentIntakeError::Cancelled => DocumentCatalogError::Cancelled,
                                error => DocumentCatalogError::Intake(error),
                            })?;
                    if cancellation.is_cancelled() {
                        return Err(DocumentCatalogError::Cancelled);
                    }
                    inspected_bytes = inspected_bytes
                        .checked_add(bytes.len())
                        .filter(|total| *total <= maximum_bytes)
                        .ok_or(DocumentCatalogError::ResourceLimitExceeded)?;
                    documents.push(CatalogEntry {
                        relative_path: RelativeDocumentPath::new(relative_path)
                            .map_err(|_| DocumentCatalogError::InvalidPath)?,
                        observation,
                    });
                }
                Class::Descend {
                    relative_path,
                    path,
                } => {
                    let depth = frame.depth.saturating_add(1);
                    if !recursive {
                        skipped.push(CatalogSkippedEntry {
                            relative_path: Some(relative_path),
                            reason: CatalogSkippedReason::Directory,
                        });
                    } else if depth > max_depth {
                        skipped.push(CatalogSkippedEntry {
                            relative_path: Some(relative_path),
                            reason: CatalogSkippedReason::DepthLimit,
                        });
                    } else {
                        pending.push(Frame {
                            path,
                            relative: relative_path,
                            depth,
                        });
                    }
                }
                Class::Skipped(entry) => skipped.push(entry),
            }
        }
    }
    Ok((documents, skipped))
}

fn read_sorted_entries(
    directory: &Path,
    maximum_entries: usize,
    cancellation: &rewrite_types::CancellationToken,
) -> Result<Vec<DirEntry>, DocumentCatalogError> {
    let mut entries = Vec::new();
    let reader = fs::read_dir(directory).map_err(DocumentCatalogError::Input)?;
    for entry in reader {
        if cancellation.is_cancelled() {
            return Err(DocumentCatalogError::Cancelled);
        }
        if entries.len() == maximum_entries {
            return Err(DocumentCatalogError::ResourceLimitExceeded);
        }
        entries.push(entry.map_err(DocumentCatalogError::Input)?);
    }
    entries.sort_by_key(DirEntry::file_name);
    Ok(entries)
}

fn classify_entry(entry: &DirEntry, prefix: &str) -> Class {
    let os_name = entry.file_name();
    let Some(name) = os_name.to_str() else {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: None,
            reason: CatalogSkippedReason::MalformedName,
        });
    };
    if !portable_component(name) {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: None,
            reason: CatalogSkippedReason::MalformedName,
        });
    }
    let relative_path = join_relative(prefix, name);
    if name.starts_with('.') {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: Some(relative_path),
            reason: CatalogSkippedReason::Hidden,
        });
    }
    if is_ignored(name) {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: Some(relative_path),
            reason: CatalogSkippedReason::Ignored,
        });
    }
    let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: Some(relative_path),
            reason: CatalogSkippedReason::Unreadable,
        });
    };
    if metadata.file_type().is_symlink() {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: Some(relative_path),
            reason: CatalogSkippedReason::Symlink,
        });
    }
    if metadata.is_dir() {
        return Class::Descend {
            relative_path,
            path: entry.path(),
        };
    }
    if !metadata.is_file() {
        return Class::Skipped(CatalogSkippedEntry {
            relative_path: Some(relative_path),
            reason: CatalogSkippedReason::NonRegular,
        });
    }
    Class::Document {
        relative_path,
        path: entry.path(),
    }
}

fn portable_component(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

fn is_ignored(name: &str) -> bool {
    name.eq_ignore_ascii_case("target") || name.eq_ignore_ascii_case("node_modules")
}

fn join_relative(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

struct Frame {
    path: PathBuf,
    relative: String,
    depth: usize,
}

enum Class {
    Document {
        relative_path: String,
        path: PathBuf,
    },
    Descend {
        relative_path: String,
        path: PathBuf,
    },
    Skipped(CatalogSkippedEntry),
}

#[cfg(test)]
mod tests;
