use std::ffi::OsStr;

use rewrite_types::CancellationToken;

use crate::artifact_storage::PinnedDirectory;

use super::{
    BUNDLES_DIRECTORY, CandidateGenerationEvidenceRepositoryError, LAYOUT_VERSION_DIRECTORY,
    LIFECYCLE_LOCK_FILE, ROOT_MARKER_FILE, STAGING_DIRECTORY, map_active_storage,
    map_initial_storage,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum FixedEntryKind {
    Directory,
    RegularFile,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum FixedChildState {
    Absent,
    Present,
}

pub(super) fn validate_root_layout(
    root: &PinnedDirectory,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    validate_exact_layout(
        root,
        &[
            (LIFECYCLE_LOCK_FILE, FixedEntryKind::RegularFile),
            (ROOT_MARKER_FILE, FixedEntryKind::RegularFile),
            (STAGING_DIRECTORY, FixedEntryKind::Directory),
            (BUNDLES_DIRECTORY, FixedEntryKind::Directory),
        ],
    )
}

pub(super) fn validate_bundles_layout(
    bundles: &PinnedDirectory,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    validate_exact_layout(
        bundles,
        &[(LAYOUT_VERSION_DIRECTORY, FixedEntryKind::Directory)],
    )
}

fn validate_exact_layout(
    directory: &PinnedDirectory,
    expected: &[(&str, FixedEntryKind)],
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    let entries = directory
        .raw_entries(expected.len() + 1, &CancellationToken::new())
        .map_err(map_initial_storage)?;
    if entries.len() != expected.len() {
        return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
    }
    for entry in entries {
        let Some(name) = entry.name.to_str() else {
            return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
        };
        let Some((canonical, kind)) = expected
            .iter()
            .find(|(canonical, _)| name.eq_ignore_ascii_case(canonical))
        else {
            return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
        };
        if name != *canonical || !entry_matches(entry.direct_regular_file, entry.indirect, *kind) {
            return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
        }
    }
    Ok(())
}

pub(super) fn fixed_child_state(
    directory: &PinnedDirectory,
    canonical: &str,
    kind: FixedEntryKind,
    maximum_entries: usize,
) -> Result<FixedChildState, CandidateGenerationEvidenceRepositoryError> {
    let entries = directory
        .raw_entries(maximum_entries, &CancellationToken::new())
        .map_err(map_initial_storage)?;
    let mut found = false;
    for entry in entries {
        let Some(name) = entry.name.to_str() else {
            continue;
        };
        if name.eq_ignore_ascii_case(canonical) {
            if found
                || name != canonical
                || !entry_matches(entry.direct_regular_file, entry.indirect, kind)
            {
                return Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary);
            }
            found = true;
        }
    }
    Ok(if found {
        FixedChildState::Present
    } else {
        FixedChildState::Absent
    })
}

pub(super) fn open_or_create_canonical_directory(
    parent: &PinnedDirectory,
    canonical: &str,
    maximum_entries: usize,
) -> Result<PinnedDirectory, CandidateGenerationEvidenceRepositoryError> {
    let child = match fixed_child_state(
        parent,
        canonical,
        FixedEntryKind::Directory,
        maximum_entries,
    )? {
        FixedChildState::Present => parent
            .open_child_directory(OsStr::new(canonical))
            .map_err(map_initial_storage),
        FixedChildState::Absent => {
            let created = parent
                .ensure_child_directory(OsStr::new(canonical))
                .map_err(map_initial_storage)?;
            parent.sync().map_err(map_initial_storage)?;
            Ok(created)
        }
    }?;
    require_canonical_directory_binding(parent, canonical, &child, maximum_entries)?;
    Ok(child)
}

pub(super) fn open_existing_canonical_directory(
    parent: &PinnedDirectory,
    canonical: &str,
    maximum_entries: usize,
) -> Result<PinnedDirectory, CandidateGenerationEvidenceRepositoryError> {
    if fixed_child_state(
        parent,
        canonical,
        FixedEntryKind::Directory,
        maximum_entries,
    )? == FixedChildState::Absent
    {
        return Err(CandidateGenerationEvidenceRepositoryError::StorageChanged);
    }
    let child = parent
        .open_child_directory(OsStr::new(canonical))
        .map_err(map_initial_storage)?;
    require_canonical_directory_binding(parent, canonical, &child, maximum_entries)?;
    Ok(child)
}

fn require_canonical_directory_binding(
    parent: &PinnedDirectory,
    canonical: &str,
    child: &PinnedDirectory,
    maximum_entries: usize,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    if fixed_child_state(
        parent,
        canonical,
        FixedEntryKind::Directory,
        maximum_entries,
    )? != FixedChildState::Present
    {
        return Err(CandidateGenerationEvidenceRepositoryError::StorageChanged);
    }
    require_named_directory_identity(parent, canonical, child)
}

const fn entry_matches(regular_file: bool, indirect: bool, kind: FixedEntryKind) -> bool {
    !indirect
        && match kind {
            FixedEntryKind::Directory => !regular_file,
            FixedEntryKind::RegularFile => regular_file,
        }
}

pub(super) fn require_named_directory_identity(
    parent: &PinnedDirectory,
    name: &str,
    held: &PinnedDirectory,
) -> Result<(), CandidateGenerationEvidenceRepositoryError> {
    let named = parent
        .child_directory_fingerprint(OsStr::new(name))
        .map_err(map_active_storage)?;
    let held = held.fingerprint().map_err(map_active_storage)?;
    if held.same_identity(&named) && held.same_filesystem(&named) {
        Ok(())
    } else {
        Err(CandidateGenerationEvidenceRepositoryError::StorageChanged)
    }
}
