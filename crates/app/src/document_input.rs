//! Retained regular-file input with explicit link policy and coherent bounded reads.

use std::{
    fs::{self, File, Metadata},
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

mod platform;

#[derive(Clone, Copy, Eq, PartialEq)]
enum LinkPolicy {
    Follow,
    Direct,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileState {
    identity: platform::FileIdentity,
    length: u64,
    modified: Option<SystemTime>,
    links: u64,
    change: platform::ChangeStamp,
}

struct RetainedInput {
    file: File,
    path: PathBuf,
    policy: LinkPolicy,
    initial: FileState,
    parents: Vec<RetainedParent>,
    declared_root: Option<RetainedParent>,
}

/// Reads a selected regular file without altering its bytes. Explicitly selected symlinks are followed.
///
/// # Errors
///
/// Returns an I/O error when the selected input violates its read contract or cannot be read.
pub fn read_regular_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    RetainedInput::open(path, LinkPolicy::Follow, false)?.read(limit)
}

/// Reads a direct regular file, refusing symbolic links, indirect ancestors and hard-linked files.
///
/// # Errors
///
/// Returns an I/O error when the selected input violates its read contract or cannot be read.
pub fn read_direct_unaliased_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    RetainedInput::open(path, LinkPolicy::Direct, true)?.read(limit)
}

/// Reads a regular file beneath a selected directory, refusing relative escapes and indirect descendants.
///
/// # Errors
///
/// Returns an I/O error when the selected input violates its read contract or cannot be read.
pub fn read_directory_bounded(root: &Path, relative: &Path, limit: usize) -> io::Result<Vec<u8>> {
    RetainedInput::open_in_directory(root, relative, false)?.read(limit)
}

/// Reads a direct, singly linked regular file beneath a selected directory.
///
/// # Errors
///
/// Returns an I/O error when the selected input violates its read contract or cannot be read.
pub fn read_directory_unaliased_bounded(
    root: &Path,
    relative: &Path,
    limit: usize,
) -> io::Result<Vec<u8>> {
    RetainedInput::open_in_directory(root, relative, true)?.read(limit)
}

/// Opens and validates a selected regular file. This open alone does not provide a coherent bounded snapshot.
///
/// # Errors
///
/// Returns an I/O error when the selected input violates its read contract or cannot be read.
pub fn open_regular_file(path: &Path) -> io::Result<File> {
    let observed = fs::metadata(path)?;
    open_observed_regular(path, &observed)
}

fn open_observed_regular(path: &Path, observed: &Metadata) -> io::Result<File> {
    require_regular(observed, LinkPolicy::Follow)?;
    let file = platform::open_followed(path)?;
    require_regular(&file.metadata()?, LinkPolicy::Follow)?;
    Ok(file)
}

impl RetainedInput {
    fn open(path: &Path, policy: LinkPolicy, unaliased: bool) -> io::Result<Self> {
        let path = std::path::absolute(path)?;
        if policy == LinkPolicy::Direct {
            let parent = path.parent().ok_or_else(invalid_input)?;
            let relative = Path::new(path.file_name().ok_or_else(invalid_input)?);
            return Self::open_in_directory(parent, relative, unaliased);
        }
        let file = open_regular_file(&path)?;
        let parents = Vec::new();
        require_regular(&file.metadata()?, policy)?;
        let initial = file_state(&file)?;
        if unaliased && initial.links != 1 {
            return Err(invalid_input());
        }
        let input = Self {
            file,
            path,
            policy,
            initial,
            parents,
            declared_root: None,
        };
        input.require_current()?;
        Ok(input)
    }

    fn open_in_directory(root: &Path, relative: &Path, unaliased: bool) -> io::Result<Self> {
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(invalid_input());
        }
        let observed = fs::symlink_metadata(root)?;
        if !observed.is_dir() || platform::is_indirect(&observed) {
            return Err(invalid_input());
        }
        let canonical_root = fs::canonicalize(root)?;
        let declared_root =
            RetainedParent::new(platform::open_followed(root)?, std::path::absolute(root)?)?;
        let path = canonical_root.join(relative);
        let (file, parents) = platform::open_direct(&path)?;
        if parents
            .iter()
            .find(|parent| parent.path == canonical_root)
            .is_none_or(|parent| parent.identity != declared_root.identity)
        {
            return Err(changed());
        }
        require_regular(&file.metadata()?, LinkPolicy::Direct)?;
        let initial = file_state(&file)?;
        if unaliased && initial.links != 1 {
            return Err(invalid_input());
        }
        let input = Self {
            file,
            path,
            policy: LinkPolicy::Direct,
            initial,
            parents,
            declared_root: Some(declared_root),
        };
        input.require_current()?;
        Ok(input)
    }

    fn read(self, limit: usize) -> io::Result<Vec<u8>> {
        self.require_current()?;
        if self.initial.length > u64::try_from(limit).unwrap_or(u64::MAX) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "input exceeds the supported byte limit",
            ));
        }
        let bytes = read_bounded(&self.file, limit)?;
        self.require_current()?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != self.initial.length {
            return Err(changed());
        }
        Ok(bytes)
    }

    fn require_current(&self) -> io::Result<()> {
        if let Some(root) = &self.declared_root {
            root.require_current().map_err(|_| changed())?;
        }
        for parent in &self.parents {
            parent.require_current().map_err(|_| changed())?;
        }
        if file_state(&self.file).map_err(|_| changed())? != self.initial {
            return Err(changed());
        }
        let metadata = if self.policy == LinkPolicy::Direct {
            fs::symlink_metadata(&self.path)
        } else {
            fs::metadata(&self.path)
        }
        .map_err(|_| changed())?;
        require_regular(&metadata, self.policy).map_err(|_| changed())?;
        if platform::path_state(&self.path, &metadata).map_err(|_| changed())? != self.initial {
            return Err(changed());
        }
        Ok(())
    }
}

pub(super) struct RetainedParent {
    file: File,
    path: PathBuf,
    identity: platform::FileIdentity,
}

impl RetainedParent {
    fn new(file: File, path: PathBuf) -> io::Result<Self> {
        let metadata = file.metadata()?;
        if !metadata.is_dir() || platform::is_indirect(&metadata) {
            return Err(invalid_input());
        }
        let identity = platform::file_identity(&file, &metadata)?;
        Ok(Self {
            file,
            path,
            identity,
        })
    }

    fn require_current(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.is_dir() || platform::is_indirect(&metadata) {
            return Err(changed());
        }
        let opened = self.file.metadata()?;
        if platform::file_identity(&self.file, &opened)? != self.identity
            || platform::path_state(&self.path, &metadata)?.identity != self.identity
        {
            return Err(changed());
        }
        Ok(())
    }
}

fn file_state(file: &File) -> io::Result<FileState> {
    let metadata = file.metadata()?;
    Ok(FileState {
        identity: platform::file_identity(file, &metadata)?,
        length: metadata.len(),
        modified: metadata.modified().ok(),
        links: platform::links(file, &metadata)?,
        change: platform::change(&metadata),
    })
}

fn require_regular(metadata: &Metadata, policy: LinkPolicy) -> io::Result<()> {
    if !metadata.is_file() || (policy == LinkPolicy::Direct && platform::is_indirect(metadata)) {
        return Err(invalid_input());
    }
    Ok(())
}

fn invalid_input() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "input must be a direct regular file",
    )
}

/// Identifies the private concurrent-change error emitted by this reader, rather than unrelated interruptions.
#[must_use]
pub fn is_changed(error: &io::Error) -> bool {
    matches!(error.get_ref(), Some(source) if source.is::<ReadChanged>())
}

#[derive(Debug)]
struct ReadChanged;

impl std::fmt::Display for ReadChanged {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("input changed during the operation")
    }
}

impl std::error::Error for ReadChanged {}

fn changed() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, ReadChanged)
}

/// Reads at most `limit` bytes without transforming the input, rejecting anything longer.
///
/// # Errors
///
/// Returns an I/O error if reading fails, or `InvalidData` if the byte ceiling is exceeded.
pub fn read_bounded(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    reader.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input exceeds the supported byte limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
