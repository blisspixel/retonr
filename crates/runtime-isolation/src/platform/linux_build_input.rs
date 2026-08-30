use std::{
    collections::BTreeSet,
    fs::{self, File, Metadata},
    os::{fd::AsRawFd as _, unix::fs::MetadataExt as _},
    path::{Path, PathBuf},
};

use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};

use crate::{
    ControlledBuildInputFile, IsolationError, IsolationResult,
    MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES, MAXIMUM_CONTROLLED_BUILD_INPUT_FILES,
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES, error::native,
};

use super::linux_build::BuildRequest;

const MAXIMUM_DECLARED_PATH_BYTES: usize = MAXIMUM_CONTROLLED_BUILD_INPUT_FILES * 4_096;
const MAXIMUM_EXPANDED_PATH_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn validate(
    request: &BuildRequest<'_>,
    program: &Metadata,
) -> IsolationResult<Vec<Metadata>> {
    if request.input_files.is_empty()
        || request.input_files.len() > MAXIMUM_CONTROLLED_BUILD_INPUT_FILES
    {
        return Err(IsolationError::ControlledBuildObjectMismatch);
    }
    request.input_files.iter().try_fold(0_u64, |total, input| {
        total
            .checked_add(input.expected_bytes())
            .filter(|bytes| *bytes <= MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES)
            .ok_or(IsolationError::ControlledBuildObjectMismatch)
    })?;
    let mut paths = BTreeSet::new();
    let mut metadata = Vec::with_capacity(request.input_files.len());
    let mut program_is_mapped = false;
    for input_file in request.input_files {
        if request.cancellation.is_cancelled() {
            return Err(IsolationError::Cancelled);
        }
        if !paths.insert(input_file.relative_path()) {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        let retained = input_file
            .file()
            .metadata()
            .map_err(|error| native("read-build-input-file-metadata", &error))?;
        let reopened = open_original(
            request.input_root,
            Path::new(input_file.relative_path()),
            false,
        )?;
        let named = reopened
            .metadata()
            .map_err(|error| native("read-named-build-input-file-metadata", &error))?;
        if !retained.is_file()
            || retained.nlink() != 1
            || !same_object(&retained, &named)
            || retained.len() != named.len()
            || retained.len() != input_file.expected_bytes()
        {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        if input_file.relative_path() == request.specification.program_relative_path() {
            program_is_mapped = same_object(program, &retained)
                && program.len() == retained.len()
                && input_file.expected_bytes() == request.specification.program_bytes()
                && input_file.expected_digest() == request.specification.program_digest();
        }
        metadata.push(retained);
    }
    validate_exact_tree(request.input_root, request.input_files)?;
    if program_is_mapped {
        Ok(metadata)
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

fn validate_exact_tree(
    input_root: &File,
    input_files: &[ControlledBuildInputFile],
) -> IsolationResult<()> {
    let (expected_files, expected_directories) = compile_expected_tree(input_files)?;
    let mut observed = BTreeSet::new();
    let mut directories = vec![(
        input_root
            .try_clone()
            .map_err(|error| native("clone-build-input-root", &error))?,
        PathBuf::new(),
    )];
    while let Some((directory, prefix)) = directories.pop() {
        let path = format!("/proc/self/fd/{}", directory.as_raw_fd());
        for raw in
            fs::read_dir(path).map_err(|error| native("enumerate-build-input-root", &error))?
        {
            let raw = raw.map_err(|error| native("read-build-input-entry", &error))?;
            let relative = prefix.join(raw.file_name());
            let relative_text = relative
                .to_str()
                .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
            if !observed.insert(relative_text.to_owned()) {
                return Err(IsolationError::ControlledBuildObjectMismatch);
            }
            if expected_directories.contains(relative_text) {
                directories.push((open_original(input_root, &relative, true)?, relative));
            } else if expected_files.contains(relative_text) {
                let opened = open_original(input_root, &relative, false)?;
                let metadata = opened
                    .metadata()
                    .map_err(|error| native("read-build-input-entry-metadata", &error))?;
                if !metadata.is_file() || metadata.nlink() != 1 {
                    return Err(IsolationError::ControlledBuildObjectMismatch);
                }
            } else {
                return Err(IsolationError::ControlledBuildObjectMismatch);
            }
        }
    }
    if observed.len()
        == expected_files
            .len()
            .saturating_add(expected_directories.len())
    {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

fn compile_expected_tree(
    input_files: &[ControlledBuildInputFile],
) -> IsolationResult<(BTreeSet<String>, BTreeSet<String>)> {
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    let mut declared_path_bytes = 0_usize;
    let mut expanded_path_bytes = 0_usize;
    for input in input_files {
        declared_path_bytes = declared_path_bytes
            .checked_add(input.relative_path().len())
            .filter(|bytes| *bytes <= MAXIMUM_DECLARED_PATH_BYTES)
            .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
        if !files.insert(input.relative_path().to_owned()) {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        require_tree_bound(files.len(), directories.len())?;
        let components = input.relative_path().split('/').collect::<Vec<_>>();
        let mut prefix = String::new();
        for component in &components[..components.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            expanded_path_bytes = expanded_path_bytes
                .checked_add(prefix.len())
                .filter(|bytes| *bytes <= MAXIMUM_EXPANDED_PATH_BYTES)
                .ok_or(IsolationError::ControlledBuildObjectMismatch)?;
            if files.contains(&prefix) {
                return Err(IsolationError::ControlledBuildObjectMismatch);
            }
            directories.insert(prefix.clone());
            require_tree_bound(files.len(), directories.len())?;
        }
        if directories.contains(input.relative_path()) {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
    }
    Ok((files, directories))
}

fn require_tree_bound(files: usize, directories: usize) -> IsolationResult<()> {
    if u64::try_from(files.saturating_add(directories)).unwrap_or(u64::MAX)
        <= MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES
    {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

fn open_original(input_root: &File, path: &Path, directory: bool) -> IsolationResult<File> {
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    if directory {
        flags |= OFlags::DIRECTORY;
    }
    openat2(
        input_root,
        path,
        flags,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| IsolationError::ControlledBuildObjectMismatch)
}

pub(super) fn revalidate(file: &File, expected: &Metadata) -> IsolationResult<()> {
    let current = file
        .metadata()
        .map_err(|error| native("revalidate-build-input-file", &error))?;
    if current.is_file()
        && current.nlink() == 1
        && same_object(&current, expected)
        && current.len() == expected.len()
    {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildObjectMismatch)
    }
}

fn same_object(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}
