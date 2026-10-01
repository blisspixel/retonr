use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::unix::fs::MetadataExt as _,
};

use rewrite_types::Digest;
use rustix::fs::{Dir, Mode, OFlags, ResolveFlags, fchmod, mkdirat, openat2};
use sha2::{Digest as _, Sha256};

use crate::{
    ControlledBuildOutputTree, ControlledBuildOutputTreeEntry,
    MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES, MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES,
};

use super::{linux_build_mount::open_private_output, linux_helper_setup::HelperFailure};

const COPY_BUFFER_BYTES: usize = 1024 * 1024;

pub(super) fn inspect_private_output() -> Result<ControlledBuildOutputTree, HelperFailure> {
    let output = open_private_output()?;
    inspect_retained_output(&output)
}

pub(super) fn inspect_retained_output(
    output: &File,
) -> Result<ControlledBuildOutputTree, HelperFailure> {
    collect_tree(output).map(|collected| collected.tree)
}

pub(super) fn publish_private_output(
    host_output: &File,
    expected: &ControlledBuildOutputTree,
) -> Result<(), HelperFailure> {
    let private_output = open_private_output()?;
    publish_retained_output(host_output, &private_output, expected)
}

pub(super) fn publish_retained_output(
    host_output: &File,
    private_output: &File,
    expected: &ControlledBuildOutputTree,
) -> Result<(), HelperFailure> {
    require_empty(host_output)?;
    let mut collected = collect_tree(private_output)?;
    if &collected.tree != expected {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    collected.entries.sort_unstable_by(|left, right| {
        left.relative_path
            .as_bytes()
            .cmp(right.relative_path.as_bytes())
    });
    let published_directories = create_directories(host_output, &collected.entries, |_| {})?;
    for entry in &mut collected.entries {
        if let CollectedKind::RegularFile {
            file,
            byte_size,
            digest,
            unix_mode,
        } = &mut entry.kind
        {
            let digest = digest
                .as_ref()
                .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
            copy_file(
                host_output,
                &entry.relative_path,
                file,
                *byte_size,
                digest,
                *unix_mode,
            )?;
        }
    }
    for (_path, (directory, unix_mode)) in published_directories.iter().rev() {
        set_and_verify_mode(directory, *unix_mode)?;
    }
    let published = collect_tree(host_output)?;
    if &published.tree == expected {
        Ok(())
    } else {
        Err(HelperFailure::ControlledBuildObjectMismatch)
    }
}

fn create_directories(
    host_output: &File,
    entries: &[CollectedEntry],
    mut after_create: impl FnMut(&str),
) -> Result<BTreeMap<String, (File, u32)>, HelperFailure> {
    let mut directories = BTreeMap::new();
    directories.insert(
        String::new(),
        (
            host_output
                .try_clone()
                .map_err(|_| HelperFailure::FilesystemAliasOutput)?,
            0,
        ),
    );
    for entry in entries
        .iter()
        .filter(|entry| matches!(entry.kind, CollectedKind::Directory { .. }))
    {
        let (parent, name) = entry
            .relative_path
            .rsplit_once('/')
            .map_or(("", entry.relative_path.as_str()), |(parent, name)| {
                (parent, name)
            });
        let parent = &directories
            .get(parent)
            .ok_or(HelperFailure::ControlledBuildObjectMismatch)?
            .0;
        mkdirat(parent, name, Mode::RUSR | Mode::WUSR | Mode::XUSR)
            .map_err(|_| HelperFailure::FilesystemAliasOutput)?;
        let directory = open_direct_directory(parent, name)?;
        let CollectedKind::Directory { unix_mode } = &entry.kind else {
            return Err(HelperFailure::ControlledBuildObjectMismatch);
        };
        directories.insert(entry.relative_path.clone(), (directory, *unix_mode));
        after_create(&entry.relative_path);
    }
    directories.remove("");
    Ok(directories)
}

fn open_direct_directory(parent: &File, name: &str) -> Result<File, HelperFailure> {
    openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::FilesystemAliasOutput)
}

struct CollectedTree {
    tree: ControlledBuildOutputTree,
    entries: Vec<CollectedEntry>,
}

struct CollectedEntry {
    relative_path: String,
    kind: CollectedKind,
}

enum CollectedKind {
    Directory {
        unix_mode: u32,
    },
    RegularFile {
        file: File,
        byte_size: u64,
        digest: Option<Digest>,
        unix_mode: u32,
    },
}

fn read_directory(directory: &File) -> Result<Dir, HelperFailure> {
    if !directory
        .metadata()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        .is_dir()
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    // read_from opens '.' relative to this held descriptor. Every traversal
    // starts with its own directory cursor and needs no proc filesystem.
    Dir::read_from(directory).map_err(|_| HelperFailure::ControlledBuildObjectMismatch)
}

fn collect_tree(root: &File) -> Result<CollectedTree, HelperFailure> {
    let mut directories = vec![(
        root.try_clone()
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
        String::new(),
    )];
    let mut entries = Vec::new();
    let mut total_file_bytes = 0_u64;
    while let Some((directory, prefix)) = directories.pop() {
        for raw in read_directory(&directory)? {
            let raw = raw.map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
            if matches!(raw.file_name().to_bytes(), b"." | b"..") {
                continue;
            }
            if entries.len() == MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES {
                return Err(HelperFailure::ControlledBuildObjectMismatch);
            }
            let name = raw
                .file_name()
                .to_str()
                .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
                .to_owned();
            let relative_path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let opened = open_beneath(root, &relative_path)?;
            let metadata = opened
                .metadata()
                .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
            let complete_mode = metadata.mode();
            if complete_mode & 0o7_000 != 0 {
                return Err(HelperFailure::ControlledBuildObjectMismatch);
            }
            let unix_mode = complete_mode & 0o777;
            let kind = if metadata.is_dir() {
                directories.push((
                    opened
                        .try_clone()
                        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
                    relative_path.clone(),
                ));
                CollectedKind::Directory { unix_mode }
            } else if metadata.is_file() && metadata.nlink() == 1 {
                let byte_size = metadata.len();
                total_file_bytes = total_file_bytes
                    .checked_add(byte_size)
                    .filter(|total| *total <= MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES)
                    .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
                CollectedKind::RegularFile {
                    file: opened,
                    byte_size,
                    digest: None,
                    unix_mode,
                }
            } else {
                return Err(HelperFailure::ControlledBuildObjectMismatch);
            };
            entries.push(CollectedEntry {
                relative_path,
                kind,
            });
        }
    }
    for entry in &mut entries {
        if let CollectedKind::RegularFile {
            file,
            byte_size,
            digest,
            ..
        } = &mut entry.kind
        {
            *digest = Some(digest_file(file, *byte_size)?);
        }
    }
    let tree_entries = entries
        .iter()
        .map(|entry| match &entry.kind {
            CollectedKind::Directory { unix_mode } => {
                ControlledBuildOutputTreeEntry::directory(&entry.relative_path, *unix_mode)
            }
            CollectedKind::RegularFile {
                byte_size,
                digest,
                unix_mode,
                ..
            } => ControlledBuildOutputTreeEntry::regular_file(
                &entry.relative_path,
                *byte_size,
                digest
                    .clone()
                    .ok_or(crate::IsolationError::ControlledBuildObjectMismatch)?,
                *unix_mode,
            ),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let tree = ControlledBuildOutputTree::compile(tree_entries)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    Ok(CollectedTree { tree, entries })
}

fn open_beneath(root: &File, relative_path: &str) -> Result<File, HelperFailure> {
    openat2(
        root,
        relative_path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)
}

fn require_empty(root: &File) -> Result<(), HelperFailure> {
    if !root
        .metadata()
        .map_err(|_| HelperFailure::FilesystemAliasOutput)?
        .is_dir()
    {
        return Err(HelperFailure::FilesystemAliasOutput);
    }
    for entry in Dir::read_from(root).map_err(|_| HelperFailure::FilesystemAliasOutput)? {
        let entry = entry.map_err(|_| HelperFailure::FilesystemAliasOutput)?;
        if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
            return Err(HelperFailure::ControlledBuildOutputNotEmpty);
        }
    }
    Ok(())
}

fn copy_file(
    host_output: &File,
    relative_path: &str,
    source: &mut File,
    byte_size: u64,
    expected_digest: &Digest,
    unix_mode: u32,
) -> Result<(), HelperFailure> {
    let descriptor = openat2(
        host_output,
        relative_path,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
        ResolveFlags::BENEATH
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_XDEV,
    )
    .map_err(|_| HelperFailure::FilesystemAliasOutput)?;
    let mut destination = File::from(descriptor);
    source
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    let mut observed = 0_u64;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    while observed < byte_size {
        let maximum = usize::try_from(byte_size - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = source
            .read(&mut buffer[..maximum])
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
        if read == 0 {
            return Err(HelperFailure::ControlledBuildObjectMismatch);
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|_| HelperFailure::FilesystemAliasOutput)?;
        hasher.update(&buffer[..read]);
        observed = observed
            .checked_add(
                u64::try_from(read).map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
            )
            .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
    }
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        != 0
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
    if &digest != expected_digest {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    set_and_verify_mode(&destination, unix_mode)
}

fn digest_file(file: &mut File, byte_size: u64) -> Result<Digest, HelperFailure> {
    let mut observed = 0_u64;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    while observed < byte_size {
        let maximum = usize::try_from(byte_size - observed)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = file
            .read(&mut buffer[..maximum])
            .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?;
        if read == 0 {
            return Err(HelperFailure::ControlledBuildObjectMismatch);
        }
        hasher.update(&buffer[..read]);
        observed = observed
            .checked_add(
                u64::try_from(read).map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?,
            )
            .ok_or(HelperFailure::ControlledBuildObjectMismatch)?;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)?
        != 0
    {
        return Err(HelperFailure::ControlledBuildObjectMismatch);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::ControlledBuildObjectMismatch)
}

fn exact_mode(unix_mode: u32) -> Result<Mode, HelperFailure> {
    Mode::from_bits(unix_mode).ok_or(HelperFailure::ControlledBuildObjectMismatch)
}

fn set_and_verify_mode(file: &File, unix_mode: u32) -> Result<(), HelperFailure> {
    set_and_verify_mode_with(file, unix_mode, |file, mode| fchmod(file, mode))
}

fn set_and_verify_mode_with(
    file: &File,
    unix_mode: u32,
    setter: impl FnOnce(&File, Mode) -> rustix::io::Result<()>,
) -> Result<(), HelperFailure> {
    setter(file, exact_mode(unix_mode)?).map_err(|_| HelperFailure::FilesystemAliasOutput)?;
    let observed = file
        .metadata()
        .map_err(|_| HelperFailure::FilesystemAliasOutput)?
        .mode()
        & 0o777;
    if observed == unix_mode {
        Ok(())
    } else {
        Err(HelperFailure::FilesystemAliasOutput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::linux_build_mount::BUILD_OUTPUT_ROOT;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt as _, symlink},
    };

    mod native;

    #[test]
    fn private_output_path_is_fixed() {
        assert_eq!(BUILD_OUTPUT_ROOT, "/tmp/retonr-controlled-build/output");
        assert_eq!(MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES, 8_589_934_592);
    }

    #[test]
    fn aggregate_sparse_length_is_rejected_before_hashing() {
        let root = tempfile::tempdir().expect("temporary output");
        for name in ["first", "second"] {
            let file = File::create(root.path().join(name)).expect("create sparse file");
            file.set_len(MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES / 2 + 1)
                .expect("set sparse length");
        }
        assert!(matches!(
            collect_tree(&File::open(root.path()).expect("open sparse root")),
            Err(HelperFailure::ControlledBuildObjectMismatch)
        ));
    }

    #[test]
    fn special_permission_bits_are_rejected_before_commitment() {
        for (mode, directory) in [(0o4_755, false), (0o2_755, false), (0o1_755, true)] {
            let root = tempfile::tempdir().expect("temporary output");
            let path = root.path().join("special");
            if directory {
                fs::create_dir(&path).expect("create special-mode directory");
            } else {
                File::create(&path).expect("create special-mode file");
            }
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))
                .expect("set special permission bits");
            assert_eq!(
                fs::symlink_metadata(&path)
                    .expect("inspect special-mode entry")
                    .mode()
                    & 0o7_000,
                mode & 0o7_000,
                "fixture filesystem must retain the selected special permission bit"
            );
            assert!(matches!(
                collect_tree(&File::open(root.path()).expect("open special-mode root")),
                Err(HelperFailure::ControlledBuildObjectMismatch)
            ));
        }
    }

    #[test]
    fn nested_directory_creation_never_follows_a_substituted_parent() {
        let output = tempfile::tempdir().expect("temporary retained output");
        let outside = tempfile::tempdir().expect("temporary outside root");
        let entries = vec![
            CollectedEntry {
                relative_path: "parent".to_owned(),
                kind: CollectedKind::Directory { unix_mode: 0o755 },
            },
            CollectedEntry {
                relative_path: "parent/nested".to_owned(),
                kind: CollectedKind::Directory { unix_mode: 0o755 },
            },
        ];
        let root = File::open(output.path()).expect("open retained output");
        let _result = create_directories(&root, &entries, |path| {
            if path == "parent" {
                fs::remove_dir(output.path().join("parent")).expect("remove created parent");
                symlink(outside.path(), output.path().join("parent"))
                    .expect("substitute parent symlink");
            }
        });
        assert!(!outside.path().join("nested").exists());
    }

    #[test]
    fn ignored_mode_changes_are_rejected_as_output_fidelity_failures() {
        let directory = tempfile::tempdir().expect("temporary output");
        let file = File::create(directory.path().join("artifact")).expect("create artifact");
        set_and_verify_mode(&file, 0o600).expect("set exact baseline mode");
        assert_eq!(
            set_and_verify_mode_with(&file, 0o755, |_file, _mode| Ok(())),
            Err(HelperFailure::FilesystemAliasOutput)
        );
        set_and_verify_mode(&file, 0o755).expect("set and verify exact mode");
    }
}
