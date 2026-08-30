use std::collections::BTreeSet;

use rewrite_types::Digest;

use super::{RedactedDigestBuilder, build::MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES};
use crate::{IsolationError, IsolationResult};

/// Maximum final files plus directories in a controlled-build output tree.
pub const MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES: usize = 4_096;
/// Maximum aggregate final regular-file bytes.
pub const MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Maximum bytes in the namespace-private build workspace.
pub const MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
/// Maximum inodes in the namespace-private build workspace.
pub const MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES: u64 = 262_144;
const MINIMUM_CONTROLLED_BUILD_WORKSPACE_SCRATCH_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAXIMUM_PORTABLE_PATH_BYTES: usize = 4_096;

const _: () = assert!(
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES
        >= MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES
            + MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES
            + MINIMUM_CONTROLLED_BUILD_WORKSPACE_SCRATCH_BYTES
);

/// One content-bound entry used to compile a controlled-build output tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledBuildOutputTreeEntry {
    relative_path: String,
    kind: ControlledBuildOutputTreeEntryKind,
    unix_mode: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ControlledBuildOutputTreeEntryKind {
    Directory,
    RegularFile { byte_size: u64, digest: Digest },
}

impl ControlledBuildOutputTreeEntry {
    /// Describes one direct directory with portable permission bits.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::ControlledBuildObjectMismatch`] for an invalid
    /// relative path or mode.
    pub fn directory(relative_path: impl Into<String>, unix_mode: u32) -> IsolationResult<Self> {
        Self::new(
            relative_path.into(),
            ControlledBuildOutputTreeEntryKind::Directory,
            unix_mode,
        )
    }

    /// Describes one direct regular file and its exact bytes.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::ControlledBuildObjectMismatch`] for an invalid
    /// relative path, mode, or byte length.
    pub fn regular_file(
        relative_path: impl Into<String>,
        byte_size: u64,
        digest: Digest,
        unix_mode: u32,
    ) -> IsolationResult<Self> {
        if byte_size > MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        Self::new(
            relative_path.into(),
            ControlledBuildOutputTreeEntryKind::RegularFile { byte_size, digest },
            unix_mode,
        )
    }

    fn new(
        relative_path: String,
        kind: ControlledBuildOutputTreeEntryKind,
        unix_mode: u32,
    ) -> IsolationResult<Self> {
        if !valid_portable_relative_path(&relative_path) || unix_mode & !0o777 != 0 {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        Ok(Self {
            relative_path,
            kind,
            unix_mode,
        })
    }

    pub(crate) fn relative_path(&self) -> &str {
        &self.relative_path
    }

    pub(crate) const fn is_directory(&self) -> bool {
        matches!(self.kind, ControlledBuildOutputTreeEntryKind::Directory)
    }
}

/// Aggregate commitment to one exact, bounded, portable output tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledBuildOutputTree {
    digest: Digest,
    entry_count: u32,
    regular_file_count: u32,
    total_file_bytes: u64,
}

impl ControlledBuildOutputTree {
    /// Compiles a canonical commitment after sorting and validating all entries.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::ControlledBuildObjectMismatch`] for an empty,
    /// oversized, duplicate, structurally incomplete, or otherwise invalid tree.
    pub fn compile(mut entries: Vec<ControlledBuildOutputTreeEntry>) -> IsolationResult<Self> {
        if entries.is_empty() || entries.len() > MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        entries.sort_unstable_by(|left, right| {
            left.relative_path
                .as_bytes()
                .cmp(right.relative_path.as_bytes())
        });
        if entries
            .windows(2)
            .any(|pair| pair[0].relative_path == pair[1].relative_path)
            || !has_complete_directory_shape(&entries)
        {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        let mut regular_file_count = 0_u32;
        let mut total_file_bytes = 0_u64;
        let mut digest = RedactedDigestBuilder::new(b"controlled-build-output-tree/v1");
        digest.push_usize(entries.len());
        for entry in &entries {
            digest.push_bytes(entry.relative_path.as_bytes());
            digest.push_u32(entry.unix_mode);
            match &entry.kind {
                ControlledBuildOutputTreeEntryKind::Directory => digest.push_u8(0),
                ControlledBuildOutputTreeEntryKind::RegularFile {
                    byte_size,
                    digest: file_digest,
                } => {
                    digest.push_u8(1);
                    digest.push_u64(*byte_size);
                    digest.push_bytes(file_digest.as_str().as_bytes());
                    regular_file_count = regular_file_count
                        .checked_add(1)
                        .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
                    total_file_bytes = total_file_bytes
                        .checked_add(*byte_size)
                        .filter(|bytes| *bytes <= MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES)
                        .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
                }
            }
        }
        Self::from_parts(
            digest.finish(),
            u32::try_from(entries.len())
                .map_err(|_| IsolationError::ControlledBuildObjectMismatch)?,
            regular_file_count,
            total_file_bytes,
        )
    }

    pub(crate) fn from_parts(
        digest: Digest,
        entry_count: u32,
        regular_file_count: u32,
        total_file_bytes: u64,
    ) -> IsolationResult<Self> {
        if entry_count == 0
            || usize::try_from(entry_count).map_or(true, |count| {
                count > MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES
            })
            || regular_file_count == 0
            || regular_file_count > entry_count
            || total_file_bytes > MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES
        {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        Ok(Self {
            digest,
            entry_count,
            regular_file_count,
            total_file_bytes,
        })
    }

    /// Returns the exact aggregate tree digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Returns the number of files plus directories.
    #[must_use]
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }

    /// Returns the number of regular files.
    #[must_use]
    pub const fn regular_file_count(&self) -> u32 {
        self.regular_file_count
    }

    /// Returns the aggregate regular-file byte length.
    #[must_use]
    pub const fn total_file_bytes(&self) -> u64 {
        self.total_file_bytes
    }
}

fn has_complete_directory_shape(entries: &[ControlledBuildOutputTreeEntry]) -> bool {
    let directories = entries
        .iter()
        .filter(|entry| entry.is_directory())
        .map(ControlledBuildOutputTreeEntry::relative_path)
        .collect::<BTreeSet<_>>();
    entries.iter().all(|entry| {
        let mut path = entry.relative_path.as_str();
        while let Some((parent, _name)) = path.rsplit_once('/') {
            if !directories.contains(parent) {
                return false;
            }
            path = parent;
        }
        true
    })
}

fn valid_portable_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PORTABLE_PATH_BYTES
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<ControlledBuildOutputTreeEntry> {
        vec![
            ControlledBuildOutputTreeEntry::regular_file(
                "bin/tool",
                4,
                Digest::sha256(b"tool"),
                0o755,
            )
            .expect("file"),
            ControlledBuildOutputTreeEntry::directory("bin", 0o755).expect("directory"),
        ]
    }

    #[test]
    fn output_tree_is_canonical_bounded_and_complete() {
        let expected = ControlledBuildOutputTree::compile(fixture()).expect("tree");
        let mut reversed = fixture();
        reversed.reverse();
        assert_eq!(
            ControlledBuildOutputTree::compile(reversed).expect("reordered tree"),
            expected
        );
        assert_eq!(expected.entry_count(), 2);
        assert_eq!(expected.regular_file_count(), 1);
        assert_eq!(expected.total_file_bytes(), 4);
        assert_eq!(expected.digest().as_str().len(), 64);
        assert!(matches!(
            ControlledBuildOutputTree::compile(vec![
                ControlledBuildOutputTreeEntry::regular_file(
                    "missing-parent/tool",
                    4,
                    Digest::sha256(b"tool"),
                    0o755,
                )
                .expect("file")
            ]),
            Err(IsolationError::ControlledBuildObjectMismatch)
        ));
    }

    #[test]
    fn every_bound_field_changes_the_tree_commitment() {
        let baseline = ControlledBuildOutputTree::compile(fixture()).expect("tree");
        for changed in [
            ControlledBuildOutputTreeEntry::regular_file(
                "bin/tool",
                5,
                Digest::sha256(b"tool"),
                0o755,
            ),
            ControlledBuildOutputTreeEntry::regular_file(
                "bin/tool",
                4,
                Digest::sha256(b"changed"),
                0o755,
            ),
            ControlledBuildOutputTreeEntry::regular_file(
                "bin/tool",
                4,
                Digest::sha256(b"tool"),
                0o700,
            ),
        ] {
            let changed = changed.expect("changed entry");
            let tree = ControlledBuildOutputTree::compile(vec![
                ControlledBuildOutputTreeEntry::directory("bin", 0o755).expect("directory"),
                changed,
            ])
            .expect("changed tree");
            assert_ne!(tree.digest(), baseline.digest());
        }
    }

    #[test]
    fn workspace_reserves_capacity_beyond_maximum_inputs_and_outputs() {
        assert_eq!(MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES, 17_179_869_184);
        assert_eq!(
            MINIMUM_CONTROLLED_BUILD_WORKSPACE_SCRATCH_BYTES,
            4_294_967_296
        );
        assert_eq!(
            MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES
                - MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES
                - MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES,
            MINIMUM_CONTROLLED_BUILD_WORKSPACE_SCRATCH_BYTES
        );
    }
}
