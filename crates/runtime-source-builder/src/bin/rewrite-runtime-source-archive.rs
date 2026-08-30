#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, HashSet},
    env,
    fs::{self, File, Metadata},
    io::{self, Read as _, Seek as _, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

use sha2::{Digest as _, Sha256};

const SOURCE_DATE_EPOCH: u64 = 1_725_000_000;
const MAXIMUM_ENTRIES: usize = 262_144;
const MAXIMUM_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAXIMUM_DIRECTORY_ENTRIES: usize = 65_536;
const MAXIMUM_PATH_BYTES: usize = 256 * 1024 * 1024;
const MAXIMUM_RETAINED_FILES: usize = 32_768;
const COPY_BUFFER_BYTES: usize = 64 * 1024;

fn main() -> ExitCode {
    if run().is_ok() {
        ExitCode::SUCCESS
    } else {
        let _ = io::stderr().write_all(b"runtime-source-archive-error\n");
        ExitCode::from(70)
    }
}

fn run() -> Result<(), ()> {
    let mut arguments = env::args_os();
    let _program = arguments.next().ok_or(())?;
    let kind = Kind::parse(&arguments.next().ok_or(())?)?;
    let source = PathBuf::from(arguments.next().ok_or(())?);
    let destination = PathBuf::from(arguments.next().ok_or(())?);
    if arguments.next().is_some() || !source.is_absolute() || !destination.is_absolute() {
        return Err(());
    }
    let source = source.canonicalize().map_err(|_| ())?;
    if !source.is_dir() || destination.exists() {
        return Err(());
    }
    let mut inventory = Inventory::acquire(&source, Limits::HARD)?;
    publication::write_transactional(&source, &destination, kind, &mut inventory)
}

#[derive(Clone, Copy)]
enum Kind {
    Ollama,
    LlamaCpp,
    Go,
    GoModuleCache,
    Cmake,
    Ninja,
    Zig,
    RetonrSource,
    CargoCrates,
    CargoVendor,
}

impl Kind {
    fn parse(value: &std::ffi::OsStr) -> Result<Self, ()> {
        match value.to_str() {
            Some("ollama") => Ok(Self::Ollama),
            Some("llama-cpp") => Ok(Self::LlamaCpp),
            Some("go") => Ok(Self::Go),
            Some("go-module-cache") => Ok(Self::GoModuleCache),
            Some("cmake") => Ok(Self::Cmake),
            Some("ninja") => Ok(Self::Ninja),
            Some("zig") => Ok(Self::Zig),
            Some("retonr-source") => Ok(Self::RetonrSource),
            Some("cargo-crates") => Ok(Self::CargoCrates),
            Some("cargo-vendor") => Ok(Self::CargoVendor),
            _ => Err(()),
        }
    }

    const fn root(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LlamaCpp => "llama-cpp",
            Self::Go => "go",
            Self::GoModuleCache => "gomodcache",
            Self::Cmake => "cmake",
            Self::Ninja => "ninja",
            Self::Zig => "zig",
            Self::RetonrSource => "retonr-source",
            Self::CargoCrates => "cargo-crates",
            Self::CargoVendor => "cargo-vendor",
        }
    }

    fn executable(self, path: &str) -> bool {
        match self {
            Self::Go => {
                matches!(path, "bin/go" | "bin/gofmt") || path.starts_with("pkg/tool/linux_amd64/")
            }
            Self::Cmake => path == "bin/cmake",
            Self::Ninja => path == "ninja",
            Self::Zig => path == "zig",
            Self::Ollama
            | Self::LlamaCpp
            | Self::GoModuleCache
            | Self::RetonrSource
            | Self::CargoCrates
            | Self::CargoVendor => false,
        }
    }
}

#[derive(Clone, Copy)]
struct Limits {
    entries: usize,
    bytes: u64,
    directory_entries: usize,
    path_bytes: usize,
    retained_files: usize,
}

impl Limits {
    const HARD: Self = Self {
        entries: MAXIMUM_ENTRIES,
        bytes: MAXIMUM_BYTES,
        directory_entries: MAXIMUM_DIRECTORY_ENTRIES,
        path_bytes: MAXIMUM_PATH_BYTES,
        retained_files: MAXIMUM_RETAINED_FILES,
    };
}

struct Inventory {
    snapshot: TreeSnapshot,
    files: BTreeMap<String, RetainedFile>,
}

impl Inventory {
    fn acquire(root: &Path, limits: Limits) -> Result<Self, ()> {
        let root_metadata = fs::symlink_metadata(root).map_err(|_| ())?;
        if !root_metadata.is_dir() {
            return Err(());
        }
        let root_stamp = ObjectStamp::from_path(root, &root_metadata)?;
        let mut entries = BTreeMap::new();
        let mut files = BTreeMap::new();
        let mut file_identities = HashSet::new();
        let mut pending = vec![root.to_path_buf()];
        let mut total_bytes = 0_u64;
        let mut total_path_bytes = 0_usize;
        while let Some(directory) = pending.pop() {
            let mut children = Vec::new();
            for child in fs::read_dir(&directory).map_err(|_| ())? {
                if children.len() >= limits.directory_entries {
                    return Err(());
                }
                children.push(child.map_err(|_| ())?);
            }
            children.sort_by_key(fs::DirEntry::file_name);
            for child in children.into_iter().rev() {
                let path = child.path();
                let metadata = fs::symlink_metadata(&path).map_err(|_| ())?;
                let relative = archive_relative(root, &path)?;
                total_path_bytes = total_path_bytes
                    .checked_add(relative.len())
                    .filter(|bytes| *bytes <= limits.path_bytes)
                    .ok_or(())?;
                let snapshot_entry = if metadata.is_dir() {
                    let stamp = ObjectStamp::from_path(&path, &metadata)?;
                    pending.push(path);
                    SnapshotEntry::Directory(stamp)
                } else if metadata.is_file() {
                    total_bytes = total_bytes.checked_add(metadata.len()).ok_or(())?;
                    if total_bytes > limits.bytes {
                        return Err(());
                    }
                    let retained = RetainedFile::acquire(&path, &metadata)?;
                    if files.len() >= limits.retained_files
                        || !file_identities.insert(retained.stamp.identity.clone())
                    {
                        return Err(());
                    }
                    let snapshot_entry = SnapshotEntry::File {
                        stamp: retained.stamp.clone(),
                        digest: retained.digest,
                    };
                    if files.insert(relative.clone(), retained).is_some() {
                        return Err(());
                    }
                    snapshot_entry
                } else {
                    return Err(());
                };
                if entries.insert(relative, snapshot_entry).is_some()
                    || entries.len() > limits.entries
                {
                    return Err(());
                }
            }
        }
        if entries.is_empty() {
            return Err(());
        }
        let root_after =
            ObjectStamp::from_path(root, &fs::symlink_metadata(root).map_err(|_| ())?)?;
        if root_stamp != root_after {
            return Err(());
        }
        Ok(Self {
            snapshot: TreeSnapshot {
                root: root_stamp,
                entries,
            },
            files,
        })
    }

    fn validate_current_tree(&self, root: &Path) -> Result<(), ()> {
        let current = Self::acquire(root, Limits::HARD)?;
        if current.snapshot == self.snapshot {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TreeSnapshot {
    root: ObjectStamp,
    entries: BTreeMap<String, SnapshotEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SnapshotEntry {
    Directory(ObjectStamp),
    File {
        stamp: ObjectStamp,
        digest: [u8; 32],
    },
}

struct RetainedFile {
    file: File,
    stamp: ObjectStamp,
    digest: [u8; 32],
}

impl RetainedFile {
    fn acquire(path: &Path, path_metadata: &Metadata) -> Result<Self, ()> {
        if !path_metadata.is_file() {
            return Err(());
        }
        let expected = ObjectStamp::from_path(path, path_metadata)?;
        let mut file = File::open(path).map_err(|_| ())?;
        validate_file_object(path, &file, &expected)?;
        let digest = hash_retained_file(&mut file, &expected)?;
        validate_file_object(path, &file, &expected)?;
        Ok(Self {
            file,
            stamp: expected,
            digest,
        })
    }
}

fn archive_relative(root: &Path, path: &Path) -> Result<String, ()> {
    let relative = path.strip_prefix(root).map_err(|_| ())?;
    let relative = relative.to_str().ok_or(())?;
    #[cfg(windows)]
    let relative = relative.replace('\\', "/");
    #[cfg(not(windows))]
    let relative = relative.to_owned();
    validate_archive_path(&relative)?;
    Ok(relative)
}

fn hash_retained_file(file: &mut File, expected: &ObjectStamp) -> Result<[u8; 32], ()> {
    file.rewind().map_err(|_| ())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES].into_boxed_slice();
    let mut total = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|_| ())?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).ok_or(())?;
        if total > expected.length {
            return Err(());
        }
        hasher.update(&buffer[..count]);
    }
    if total != expected.length {
        return Err(());
    }
    Ok(hasher.finalize().into())
}

fn write_archive<W: io::Write>(
    output: W,
    source: &Path,
    kind: Kind,
    inventory: &mut Inventory,
) -> Result<(), ()> {
    inventory.validate_current_tree(source)?;
    let mut archive = tar::Builder::new(output);
    append_directory(&mut archive, kind.root())?;
    for (relative, snapshot_entry) in &inventory.snapshot.entries {
        let archive_path = format!("{}/{relative}", kind.root());
        match snapshot_entry {
            SnapshotEntry::Directory(stamp) => {
                let path = source.join(relative);
                let current = ObjectStamp::from_path(
                    &path,
                    &fs::symlink_metadata(source.join(relative)).map_err(|_| ())?,
                )?;
                if current != *stamp {
                    return Err(());
                }
                append_directory(&mut archive, &archive_path)?;
            }
            SnapshotEntry::File { stamp, digest } => {
                let retained = inventory.files.get_mut(relative).ok_or(())?;
                let path = source.join(relative);
                validate_file_object(&path, &retained.file, stamp)?;
                retained.file.rewind().map_err(|_| ())?;
                let mode = if kind.executable(relative) {
                    0o755
                } else {
                    0o644
                };
                let mut header = header(tar::EntryType::Regular, mode, stamp.length)?;
                let mut reader = HashingReader::new(&mut retained.file, stamp.length);
                archive
                    .append_data(&mut header, archive_path, &mut reader)
                    .map_err(|_| ())?;
                let (actual_digest, actual_length) = reader.finish();
                if actual_length != stamp.length || actual_digest != *digest {
                    return Err(());
                }
                validate_file_object(&path, &retained.file, stamp)?;
            }
        }
    }
    archive.finish().map_err(|_| ())?;
    inventory.validate_current_tree(source)
}

struct HashingReader<'a> {
    inner: &'a mut File,
    hasher: Sha256,
    remaining: u64,
    consumed: u64,
}

impl<'a> HashingReader<'a> {
    fn new(inner: &'a mut File, length: u64) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            remaining: length,
            consumed: 0,
        }
    }

    fn finish(self) -> ([u8; 32], u64) {
        (self.hasher.finalize().into(), self.consumed)
    }
}

impl io::Read for HashingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let limit = usize::try_from(self.remaining.min(buffer.len() as u64))
            .expect("read limit fits usize");
        if limit == 0 {
            return Ok(0);
        }
        let count = self.inner.read(&mut buffer[..limit])?;
        self.hasher.update(&buffer[..count]);
        self.remaining -= count as u64;
        self.consumed += count as u64;
        Ok(count)
    }
}

fn validate_archive_path(path: &str) -> Result<(), ()> {
    if path.is_empty()
        || path.len() > 4_096
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || path.chars().any(char::is_control)
    {
        Err(())
    } else {
        Ok(())
    }
}

fn append_directory<W: io::Write>(archive: &mut tar::Builder<W>, path: &str) -> Result<(), ()> {
    let mut header = header(tar::EntryType::Directory, 0o755, 0)?;
    archive
        .append_data(&mut header, path, io::empty())
        .map_err(|_| ())
}

fn header(entry_type: tar::EntryType, mode: u32, size: u64) -> Result<tar::Header, ()> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(entry_type);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mode(mode);
    header.set_mtime(SOURCE_DATE_EPOCH);
    header.set_size(size);
    header.set_username("").map_err(|_| ())?;
    header.set_groupname("").map_err(|_| ())?;
    header.set_cksum();
    Ok(header)
}

#[cfg(test)]
#[path = "runtime_source_archive/tests.rs"]
mod tests;

#[path = "runtime_source_archive/publication.rs"]
mod publication;

#[path = "runtime_source_archive/object.rs"]
mod object;

use object::{ObjectStamp, validate_file_object};
