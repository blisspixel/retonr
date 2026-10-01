use std::{
    fs::File,
    io::{self, Seek as _},
    path::Path,
};

use sha2::{Digest as _, Sha256};

use super::{
    linux_bootstrap_sandbox::EstablishedBootstrapSandbox,
    linux_build_mount::require_read_only_mount, linux_helper_setup::HelperFailure,
};

mod snapshot;
use snapshot::{EntryKind, Snapshot, Stamp, open_relative};

const SOURCE_DATE_EPOCH: u64 = 1_725_000_000;

// Only a successfully established private bootstrap sandbox supplies these roots.
// Mutable public source normalization keeps its separate held-file implementation.
pub(super) fn write(
    sandbox: &EstablishedBootstrapSandbox,
    index: usize,
    archive_root: &str,
    destination: &Path,
) -> Result<(), HelperFailure> {
    let root = sandbox
        .archive_roots()
        .get(index)
        .ok_or(HelperFailure::BootstrapRootVerification)?;
    serialize_root(root, archive_root, destination)
}

fn serialize_root(
    root: &File,
    archive_root: &str,
    destination: &Path,
) -> Result<(), HelperFailure> {
    require_read_only_mount(root)?;
    if !matches!(
        archive_root,
        "retonr-source" | "cargo-vendor" | "cargo-crates"
    ) {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let original = Snapshot::acquire(root)?;
    let primary = write_and_readback(root, archive_root, &original, destination);
    let final_snapshot = Snapshot::acquire(root);
    primary?;
    if final_snapshot? != original {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(())
}

fn write_and_readback(
    root: &File,
    archive_root: &str,
    original: &Snapshot,
    destination: &Path,
) -> Result<(), HelperFailure> {
    let parent = File::open(
        destination
            .parent()
            .ok_or(HelperFailure::BootstrapRootVerification)?,
    )
    .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(HelperFailure::BootstrapRootVerification)?;
    let mut output = snapshot::create_output(&parent, name)?;
    let expected = serialize_into(root, archive_root, original, &mut output)?;
    output
        .sync_all()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    verify_readback(&parent, name, &mut output, expected)
}

fn verify_readback(
    parent: &File,
    name: &str,
    output: &mut File,
    expected: ([u8; 32], u64),
) -> Result<(), HelperFailure> {
    let stamp = Stamp::from_file(output)?;
    let mut reopened = open_relative(parent, name)?;
    if Stamp::from_file(&reopened)? != stamp {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    output
        .rewind()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let retained = snapshot::hash_file(output)?;
    let readback = snapshot::hash_file(&mut reopened)?;
    if retained != expected
        || readback != expected
        || Stamp::from_file(output)? != stamp
        || Stamp::from_file(&reopened)? != stamp
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(())
}

fn serialize_into(
    root: &File,
    archive_root: &str,
    original: &Snapshot,
    output: &mut File,
) -> Result<([u8; 32], u64), HelperFailure> {
    let mut writer = HashingWriter {
        file: output,
        hash: Sha256::new(),
        written: 0,
    };
    let mut archive = tar::Builder::new(&mut writer);
    append_directory(&mut archive, archive_root)?;
    for (relative, entry) in &original.entries {
        let mut file = open_relative(root, relative)?;
        if Stamp::from_file(&file)? != entry.stamp {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        let path = format!("{archive_root}/{relative}");
        match entry.kind {
            EntryKind::Directory => append_directory(&mut archive, &path)?,
            EntryKind::File => {
                let mut header = header(tar::EntryType::Regular, 0o644, entry.stamp.length)?;
                let mut reader = HashingReader::new(&mut file, entry.stamp.length);
                archive
                    .append_data(&mut header, path, &mut reader)
                    .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
                let (digest, bytes) = reader.finish();
                if Some(digest) != entry.digest || bytes != entry.stamp.length {
                    return Err(HelperFailure::BootstrapRootVerification);
                }
            }
        }
        if Stamp::from_file(&file)? != entry.stamp
            || Stamp::from_file(&open_relative(root, relative)?)? != entry.stamp
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    archive
        .finish()
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    drop(archive);
    Ok((writer.hash.finalize().into(), writer.written))
}

struct HashingWriter<'a> {
    file: &'a mut File,
    hash: Sha256,
    written: u64,
}

impl io::Write for HashingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.file.write(buffer)?;
        self.hash.update(&buffer[..written]);
        self.written = self
            .written
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn append_directory<W: io::Write>(
    archive: &mut tar::Builder<W>,
    path: &str,
) -> Result<(), HelperFailure> {
    let mut header = header(tar::EntryType::Directory, 0o755, 0)?;
    archive
        .append_data(&mut header, path, io::empty())
        .map_err(|_| HelperFailure::BootstrapRootPreparation)
}

fn header(kind: tar::EntryType, mode: u32, size: u64) -> Result<tar::Header, HelperFailure> {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(kind);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mode(mode);
    header.set_mtime(SOURCE_DATE_EPOCH);
    header.set_size(size);
    header
        .set_username("")
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    header
        .set_groupname("")
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    header.set_cksum();
    Ok(header)
}

struct HashingReader<'a> {
    file: &'a mut File,
    hash: Sha256,
    remaining: u64,
    consumed: u64,
}

impl<'a> HashingReader<'a> {
    fn new(file: &'a mut File, remaining: u64) -> Self {
        Self {
            file,
            hash: Sha256::new(),
            remaining,
            consumed: 0,
        }
    }

    fn finish(self) -> ([u8; 32], u64) {
        (self.hash.finalize().into(), self.consumed)
    }
}

impl io::Read for HashingReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let length = usize::try_from(self.remaining.min(buffer.len() as u64))
            .expect("read limit fits usize");
        let read = self.file.read(&mut buffer[..length])?;
        self.hash.update(&buffer[..read]);
        self.remaining -= read as u64;
        self.consumed += read as u64;
        Ok(read)
    }
}

#[cfg(test)]
mod tests;
