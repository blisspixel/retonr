use std::collections::BTreeMap;

use rewrite_model::{ArtifactSetRelativePath, MAX_ARTIFACT_SET_MEMBERS};
use rewrite_types::Digest;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ReconstructedRuntimePackage, json::validate_unique_json};

use super::{
    RuntimeSourceBuildEvidenceKind, RuntimeSourceBuildEvidenceRecord, RuntimeSourceBuildReportError,
};

/// Portable output-tree sidecar schema version.
pub const RUNTIME_SOURCE_BUILD_OUTPUT_TREE_SCHEMA_VERSION: u32 = 1;

const MAXIMUM_OUTPUT_TREE_BYTES: usize = 1024 * 1024;
const MAXIMUM_OUTPUT_TREE_ENTRIES: usize = MAX_ARTIFACT_SET_MEMBERS + 4_096;
const MAXIMUM_OUTPUT_TREE_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const OUTPUT_TREE_DIGEST_DOMAIN: &[u8] = b"retonr:runtime-source-build-output-tree:v1\0";

/// Kind of one portable controlled-build output-tree entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeSourceBuildOutputTreeEntryKind {
    /// Directory needed by the exact output tree.
    Directory,
    /// Single-link regular file whose size and digest are retained.
    RegularFile,
}

/// One path, kind, mode, and optional byte identity in a portable output-tree sidecar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildOutputTreeEntry {
    relative_path: ArtifactSetRelativePath,
    kind: RuntimeSourceBuildOutputTreeEntryKind,
    unix_mode: u32,
    byte_size: Option<u64>,
    digest: Option<Digest>,
}

impl RuntimeSourceBuildOutputTreeEntry {
    /// Creates one portable directory entry.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReportError::InvalidOutputTree`] when the mode
    /// contains bits outside the portable permission mask.
    pub fn directory(
        relative_path: ArtifactSetRelativePath,
        unix_mode: u32,
    ) -> Result<Self, RuntimeSourceBuildReportError> {
        Self::new(
            relative_path,
            RuntimeSourceBuildOutputTreeEntryKind::Directory,
            unix_mode,
            None,
            None,
        )
    }

    /// Creates one portable regular-file entry.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReportError::InvalidOutputTree`] when the mode
    /// or declared byte length exceeds the controlled-build output contract.
    pub fn regular_file(
        relative_path: ArtifactSetRelativePath,
        unix_mode: u32,
        byte_size: u64,
        digest: Digest,
    ) -> Result<Self, RuntimeSourceBuildReportError> {
        Self::new(
            relative_path,
            RuntimeSourceBuildOutputTreeEntryKind::RegularFile,
            unix_mode,
            Some(byte_size),
            Some(digest),
        )
    }

    fn new(
        relative_path: ArtifactSetRelativePath,
        kind: RuntimeSourceBuildOutputTreeEntryKind,
        unix_mode: u32,
        byte_size: Option<u64>,
        digest: Option<Digest>,
    ) -> Result<Self, RuntimeSourceBuildReportError> {
        let fields_match_kind = match kind {
            RuntimeSourceBuildOutputTreeEntryKind::Directory => {
                byte_size.is_none() && digest.is_none()
            }
            RuntimeSourceBuildOutputTreeEntryKind::RegularFile => {
                byte_size.is_some() && digest.is_some()
            }
        };
        if unix_mode & !0o777 != 0
            || byte_size.is_some_and(|bytes| bytes > MAXIMUM_OUTPUT_TREE_FILE_BYTES)
            || !fields_match_kind
        {
            return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
        }
        Ok(Self {
            relative_path,
            kind,
            unix_mode,
            byte_size,
            digest,
        })
    }

    /// Returns the portable relative path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns whether this entry is a directory or regular file.
    #[must_use]
    pub const fn kind(&self) -> RuntimeSourceBuildOutputTreeEntryKind {
        self.kind
    }

    /// Returns the original portable Unix permission bits.
    #[must_use]
    pub const fn unix_mode(&self) -> u32 {
        self.unix_mode
    }

    /// Returns the exact byte length for a regular file.
    #[must_use]
    pub const fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }

    /// Returns the exact byte digest for a regular file.
    #[must_use]
    pub const fn digest(&self) -> Option<&Digest> {
        self.digest.as_ref()
    }

    fn canonical_value(&self) -> Value {
        match (&self.kind, self.byte_size, &self.digest) {
            (RuntimeSourceBuildOutputTreeEntryKind::Directory, None, None) => json!({
                "kind": "directory",
                "relative_path": self.relative_path.as_str(),
                "unix_mode": self.unix_mode
            }),
            (RuntimeSourceBuildOutputTreeEntryKind::RegularFile, Some(byte_size), Some(digest)) => {
                json!({
                    "byte_size": byte_size,
                    "digest": digest,
                    "kind": "regular_file",
                    "relative_path": self.relative_path.as_str(),
                    "unix_mode": self.unix_mode
                })
            }
            _ => unreachable!("output-tree entries are constructed with matching fields"),
        }
    }
}

/// Canonical portable identity of one complete controlled-build output tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildOutputTree {
    entries: Vec<RuntimeSourceBuildOutputTreeEntry>,
    canonical_bytes: Vec<u8>,
    digest: Digest,
}

impl RuntimeSourceBuildOutputTree {
    /// Compiles one canonical path, mode, and byte identity closure.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReportError::InvalidOutputTree`] for an empty,
    /// oversized, colliding, incomplete, or otherwise inconsistent tree.
    pub fn compile(
        mut entries: Vec<RuntimeSourceBuildOutputTreeEntry>,
    ) -> Result<Self, RuntimeSourceBuildReportError> {
        if entries.is_empty() || entries.len() > MAXIMUM_OUTPUT_TREE_ENTRIES {
            return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
        }
        entries.sort_unstable_by(|left, right| {
            left.relative_path
                .as_str()
                .as_bytes()
                .cmp(right.relative_path.as_str().as_bytes())
        });
        validate_entries(&entries)?;
        let canonical_bytes = canonical_bytes(&entries)?;
        if canonical_bytes.len() > MAXIMUM_OUTPUT_TREE_BYTES {
            return Err(RuntimeSourceBuildReportError::LimitExceeded);
        }
        let mut digest_material = Vec::with_capacity(
            OUTPUT_TREE_DIGEST_DOMAIN
                .len()
                .saturating_add(canonical_bytes.len()),
        );
        digest_material.extend_from_slice(OUTPUT_TREE_DIGEST_DOMAIN);
        digest_material.extend_from_slice(&canonical_bytes);
        Ok(Self {
            entries,
            digest: Digest::sha256(&digest_material),
            canonical_bytes,
        })
    }

    /// Parses and revalidates canonical portable output-tree JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReportError`] for excessive, malformed,
    /// noncanonical, unsupported, or structurally invalid bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, RuntimeSourceBuildReportError> {
        if bytes.len() > MAXIMUM_OUTPUT_TREE_BYTES {
            return Err(RuntimeSourceBuildReportError::LimitExceeded);
        }
        validate_unique_json(bytes)
            .map_err(|()| RuntimeSourceBuildReportError::InvalidOutputTree)?;
        let wire: OutputTreeWire = serde_json::from_slice(bytes)
            .map_err(|_| RuntimeSourceBuildReportError::InvalidOutputTree)?;
        if wire.schema_version != RUNTIME_SOURCE_BUILD_OUTPUT_TREE_SCHEMA_VERSION {
            return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
        }
        let entries = wire
            .entries
            .into_iter()
            .map(RuntimeSourceBuildOutputTreeEntry::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let tree = Self::compile(entries)?;
        if tree.canonical_bytes != bytes {
            return Err(RuntimeSourceBuildReportError::NoncanonicalOutputTree);
        }
        Ok(tree)
    }

    /// Returns entries in canonical path order.
    #[must_use]
    pub fn entries(&self) -> &[RuntimeSourceBuildOutputTreeEntry] {
        &self.entries
    }

    /// Returns the exact canonical sidecar bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated semantic output-tree digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    pub(super) fn validate_binding(
        &self,
        evidence: &[RuntimeSourceBuildEvidenceRecord],
        runtime: &ReconstructedRuntimePackage,
    ) -> Result<(), RuntimeSourceBuildReportError> {
        let mut expected = BTreeMap::<String, ExpectedEntry>::new();
        for item in evidence.iter().filter(|item| {
            matches!(
                item.kind,
                RuntimeSourceBuildEvidenceKind::RuntimeLayout
                    | RuntimeSourceBuildEvidenceKind::Sbom
                    | RuntimeSourceBuildEvidenceKind::Provenance
                    | RuntimeSourceBuildEvidenceKind::Transformation
            )
        }) {
            insert_expected_file(
                &mut expected,
                item.relative_path
                    .as_str()
                    .rsplit_once('/')
                    .map_or(item.relative_path.as_str(), |(_, name)| name),
                item.byte_size,
                item.digest.clone(),
            )?;
        }
        for member in runtime.artifact_set().members() {
            insert_expected_file(
                &mut expected,
                member.relative_path().as_str(),
                member.byte_size(),
                member.artifact_id().digest().clone(),
            )?;
        }
        if expected.len() != self.entries.len() {
            return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
        }
        for entry in &self.entries {
            let matches = expected
                .get(entry.relative_path.as_str())
                .is_some_and(|expected| expected.matches(entry));
            if !matches {
                return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputTreeWire {
    entries: Vec<OutputTreeEntryWire>,
    schema_version: u32,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum OutputTreeEntryWire {
    Directory {
        relative_path: String,
        unix_mode: u32,
    },
    RegularFile {
        byte_size: u64,
        digest: Digest,
        relative_path: String,
        unix_mode: u32,
    },
}

impl TryFrom<OutputTreeEntryWire> for RuntimeSourceBuildOutputTreeEntry {
    type Error = RuntimeSourceBuildReportError;

    fn try_from(wire: OutputTreeEntryWire) -> Result<Self, Self::Error> {
        match wire {
            OutputTreeEntryWire::Directory {
                relative_path,
                unix_mode,
            } => Self::directory(parse_path(relative_path)?, unix_mode),
            OutputTreeEntryWire::RegularFile {
                byte_size,
                digest,
                relative_path,
                unix_mode,
            } => Self::regular_file(parse_path(relative_path)?, unix_mode, byte_size, digest),
        }
    }
}

fn parse_path(value: String) -> Result<ArtifactSetRelativePath, RuntimeSourceBuildReportError> {
    ArtifactSetRelativePath::new(value)
        .map_err(|_| RuntimeSourceBuildReportError::InvalidOutputTree)
}

fn canonical_bytes(
    entries: &[RuntimeSourceBuildOutputTreeEntry],
) -> Result<Vec<u8>, RuntimeSourceBuildReportError> {
    let entries = entries
        .iter()
        .map(RuntimeSourceBuildOutputTreeEntry::canonical_value)
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "entries": entries,
        "schema_version": RUNTIME_SOURCE_BUILD_OUTPUT_TREE_SCHEMA_VERSION
    }))
    .map_err(|_| RuntimeSourceBuildReportError::InvalidOutputTree)
}

fn validate_entries(
    entries: &[RuntimeSourceBuildOutputTreeEntry],
) -> Result<(), RuntimeSourceBuildReportError> {
    let mut by_path = BTreeMap::<String, &RuntimeSourceBuildOutputTreeEntry>::new();
    let mut total_bytes = 0_u64;
    let mut regular_files = 0_usize;
    for entry in entries {
        let key = entry.relative_path.as_str().to_ascii_lowercase();
        if by_path.insert(key, entry).is_some() {
            return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
        }
        if let Some(byte_size) = entry.byte_size {
            regular_files += 1;
            total_bytes = total_bytes
                .checked_add(byte_size)
                .filter(|total| *total <= MAXIMUM_OUTPUT_TREE_FILE_BYTES)
                .ok_or(RuntimeSourceBuildReportError::InvalidOutputTree)?;
        }
    }
    if regular_files == 0 {
        return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
    }
    for entry in entries {
        let mut prefix = String::new();
        let components = entry.relative_path.as_str().split('/').collect::<Vec<_>>();
        for component in components.iter().take(components.len().saturating_sub(1)) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            let parent = by_path
                .get(&prefix.to_ascii_lowercase())
                .ok_or(RuntimeSourceBuildReportError::InvalidOutputTree)?;
            if parent.relative_path.as_str() != prefix
                || parent.kind != RuntimeSourceBuildOutputTreeEntryKind::Directory
            {
                return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
enum ExpectedEntry {
    Directory,
    RegularFile { byte_size: u64, digest: Digest },
}

impl ExpectedEntry {
    fn matches(&self, entry: &RuntimeSourceBuildOutputTreeEntry) -> bool {
        match (self, entry.kind, entry.byte_size, entry.digest.as_ref()) {
            (Self::Directory, RuntimeSourceBuildOutputTreeEntryKind::Directory, None, None) => true,
            (
                Self::RegularFile { byte_size, digest },
                RuntimeSourceBuildOutputTreeEntryKind::RegularFile,
                Some(observed_size),
                Some(observed_digest),
            ) => *byte_size == observed_size && digest == observed_digest,
            _ => false,
        }
    }
}

fn insert_expected_file(
    expected: &mut BTreeMap<String, ExpectedEntry>,
    path: &str,
    byte_size: u64,
    digest: Digest,
) -> Result<(), RuntimeSourceBuildReportError> {
    let components = path.split('/').collect::<Vec<_>>();
    let mut prefix = String::new();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        match expected.get(&prefix) {
            Some(ExpectedEntry::Directory) => {}
            Some(ExpectedEntry::RegularFile { .. }) => {
                return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
            }
            None => {
                expected.insert(prefix.clone(), ExpectedEntry::Directory);
            }
        }
    }
    if expected
        .insert(
            path.to_owned(),
            ExpectedEntry::RegularFile { byte_size, digest },
        )
        .is_some()
    {
        return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
    }
    Ok(())
}
