use std::{
    fs::File,
    io::{self, Write as _},
    path::{Component, Path},
    sync::Arc,
};

use sha2::{Digest as _, Sha256};

#[cfg(not(target_os = "linux"))]
use std::path::PathBuf;

use super::{
    Inventory, Kind, ObjectStamp, hash_retained_file, validate_file_object, write_archive,
};

const TEMPORARY_PREFIX: &str = ".retonr-runtime-source-archive-";

pub(super) fn write_transactional(
    source: &Path,
    destination: &Path,
    kind: Kind,
    inventory: &mut Inventory,
) -> Result<(), ()> {
    let file_name = destination.file_name().ok_or(())?;
    if !matches!(
        Path::new(file_name).components().next(),
        Some(Component::Normal(_))
    ) || Path::new(file_name).components().count() != 1
    {
        return Err(());
    }
    let parent = destination
        .parent()
        .ok_or(())?
        .canonicalize()
        .map_err(|_| ())?;
    if !parent.is_dir() {
        return Err(());
    }
    let destination = parent.join(file_name);
    if destination.exists() {
        return Err(());
    }

    let mut temporary = tempfile::Builder::new()
        .prefix(TEMPORARY_PREFIX)
        .tempfile_in(&parent)
        .map_err(|_| ())?;
    let temporary_path = temporary.path().to_path_buf();
    let measurement = write_and_verify_staged(
        temporary.as_file_mut(),
        &temporary_path,
        source,
        kind,
        inventory,
    )?;
    let published = publish_noclobber(temporary, &parent, &destination)?;
    if verify_measurement(&destination, &published.file, &measurement).is_err()
        || !published.descriptor_path_matches(&measurement)
        || published.sync_parent().is_err()
        || verify_measurement(&destination, &published.file, &measurement).is_err()
    {
        published.cleanup_if_same(&measurement);
        return Err(());
    }
    Ok(())
}

fn write_and_verify_staged(
    file: &mut File,
    path: &Path,
    source: &Path,
    kind: Kind,
    inventory: &mut Inventory,
) -> Result<Measurement, ()> {
    let (digest, length) = {
        let mut writer = MeasuringWriter::new(file);
        write_archive(&mut writer, source, kind, inventory)?;
        writer.flush().map_err(|_| ())?;
        writer.finish()
    };
    file.sync_all().map_err(|_| ())?;
    let stamp = ObjectStamp::from_file(file, &file.metadata().map_err(|_| ())?)?;
    let measurement = Measurement {
        digest,
        length,
        identity: stamp.identity.clone(),
    };
    verify_measurement(path, file, &measurement)?;
    Ok(measurement)
}

fn verify_measurement(path: &Path, file: &File, expected: &Measurement) -> Result<(), ()> {
    let handle = ObjectStamp::from_file(file, &file.metadata().map_err(|_| ())?)?;
    let path_stamp =
        ObjectStamp::from_path(path, &std::fs::symlink_metadata(path).map_err(|_| ())?)?;
    if handle != path_stamp
        || handle.identity != expected.identity
        || handle.length != expected.length
    {
        return Err(());
    }
    let mut clone = file.try_clone().map_err(|_| ())?;
    let digest = hash_retained_file(&mut clone, &handle)?;
    validate_file_object(path, file, &handle)?;
    if digest == expected.digest {
        Ok(())
    } else {
        Err(())
    }
}

struct Measurement {
    digest: [u8; 32],
    length: u64,
    identity: Arc<same_file::Handle>,
}

struct MeasuringWriter<'a> {
    inner: &'a mut File,
    hasher: Sha256,
    length: u64,
}

impl<'a> MeasuringWriter<'a> {
    fn new(inner: &'a mut File) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            length: 0,
        }
    }

    fn finish(self) -> ([u8; 32], u64) {
        (self.hasher.finalize().into(), self.length)
    }
}

impl io::Write for MeasuringWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(buffer)?;
        self.hasher.update(&buffer[..count]);
        self.length = self
            .length
            .checked_add(u64::try_from(count).expect("write count fits u64"))
            .ok_or_else(|| io::Error::other("archive length overflow"))?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct Published {
    file: File,
    #[cfg(not(target_os = "linux"))]
    destination: PathBuf,
    #[cfg(target_os = "linux")]
    parent: File,
    #[cfg(target_os = "linux")]
    file_name: std::ffi::OsString,
}

impl Published {
    #[cfg(target_os = "linux")]
    fn descriptor_path_matches(&self, expected: &Measurement) -> bool {
        use rustix::fs::{Mode, OFlags, openat};

        let Ok(opened) = openat(
            &self.parent,
            &self.file_name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        ) else {
            return false;
        };
        let opened = File::from(opened);
        same_file::Handle::from_file(opened).is_ok_and(|identity| identity == *expected.identity)
    }

    #[cfg(not(target_os = "linux"))]
    fn descriptor_path_matches(&self, _expected: &Measurement) -> bool {
        !self.destination.as_os_str().is_empty()
    }

    #[cfg(target_os = "linux")]
    fn sync_parent(&self) -> Result<(), ()> {
        rustix::fs::fsync(&self.parent).map_err(|_| ())
    }

    #[cfg(not(target_os = "linux"))]
    fn sync_parent(&self) -> Result<(), ()> {
        self.destination
            .parent()
            .ok_or(())?
            .metadata()
            .map_err(|_| ())?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn cleanup_if_same(self, expected: &Measurement) {
        use rustix::fs::{AtFlags, Mode, OFlags, openat, unlinkat};

        let opened = openat(
            &self.parent,
            &self.file_name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        );
        if let Ok(opened) = opened
            && same_file::Handle::from_file(File::from(opened))
                .is_ok_and(|identity| identity == *expected.identity)
        {
            let _ = unlinkat(&self.parent, &self.file_name, AtFlags::empty());
            let _ = rustix::fs::fsync(&self.parent);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn cleanup_if_same(self, expected: &Measurement) {
        let same = same_file::Handle::from_path(&self.destination)
            .is_ok_and(|identity| identity == *expected.identity);
        drop(self.file);
        if same {
            let _ = std::fs::remove_file(&self.destination);
        }
    }
}

#[cfg(target_os = "linux")]
fn publish_noclobber(
    temporary: tempfile::NamedTempFile,
    parent_path: &Path,
    destination: &Path,
) -> Result<Published, ()> {
    use rustix::fs::{AtFlags, linkat};

    let parent = File::open(parent_path).map_err(|_| ())?;
    let parent_path_identity = same_file::Handle::from_path(parent_path).map_err(|_| ())?;
    let parent_file_identity =
        same_file::Handle::from_file(parent.try_clone().map_err(|_| ())?).map_err(|_| ())?;
    if parent_path_identity != parent_file_identity {
        return Err(());
    }
    let temporary_name = temporary.path().file_name().ok_or(())?.to_os_string();
    let file_name = destination.file_name().ok_or(())?.to_os_string();
    let (file, mut temporary_path) = temporary.into_parts();
    temporary_path.disable_cleanup(true);
    if linkat(&file, "", &parent, &file_name, AtFlags::EMPTY_PATH).is_err() {
        unlink_descriptor_name_if_same(&parent, &temporary_name, &file);
        return Err(());
    }
    if !unlink_descriptor_name_if_same(&parent, &temporary_name, &file) {
        unlink_descriptor_name_if_same(&parent, &file_name, &file);
        return Err(());
    }
    Ok(Published {
        file,
        parent,
        file_name,
    })
}

#[cfg(target_os = "linux")]
fn unlink_descriptor_name_if_same(parent: &File, name: &std::ffi::OsStr, file: &File) -> bool {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, unlinkat};

    let Ok(opened) = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    ) else {
        return false;
    };
    let Ok(opened_identity) = same_file::Handle::from_file(File::from(opened)) else {
        return false;
    };
    let Ok(clone) = file.try_clone() else {
        return false;
    };
    let Ok(file_identity) = same_file::Handle::from_file(clone) else {
        return false;
    };
    opened_identity == file_identity && unlinkat(parent, name, AtFlags::empty()).is_ok()
}

#[cfg(not(target_os = "linux"))]
fn publish_noclobber(
    temporary: tempfile::NamedTempFile,
    _parent: &Path,
    destination: &Path,
) -> Result<Published, ()> {
    let file = temporary.persist_noclobber(destination).map_err(|_| ())?;
    Ok(Published {
        file,
        destination: destination.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Limits;
    use tempfile::tempdir;

    #[test]
    fn publication_is_noclobber_and_leaves_no_temporary_name() {
        let root = tempdir().expect("root");
        let source = root.path().join("source");
        std::fs::create_dir(&source).expect("source");
        std::fs::write(source.join("payload"), b"bound").expect("payload");
        let mut inventory = Inventory::acquire(&source, Limits::HARD).expect("inventory");
        let destination = root.path().join("source.tar");
        write_transactional(&source, &destination, Kind::Ollama, &mut inventory)
            .expect("publication");
        assert!(destination.is_file());
        assert!(root.path().read_dir().expect("entries").all(|entry| {
            !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(TEMPORARY_PREFIX)
        }));

        let mut second = Inventory::acquire(&source, Limits::HARD).expect("inventory");
        assert!(write_transactional(&source, &destination, Kind::Ollama, &mut second).is_err());
        assert!(destination.is_file());
    }

    #[test]
    fn failed_write_cleans_temporary_output() {
        let root = tempdir().expect("root");
        let source = root.path().join("source");
        std::fs::create_dir(&source).expect("source");
        std::fs::write(source.join("payload"), b"bound").expect("payload");
        let mut inventory = Inventory::acquire(&source, Limits::HARD).expect("inventory");
        std::fs::write(source.join("payload"), b"other").expect("mutation");
        let destination = root.path().join("source.tar");
        assert!(write_transactional(&source, &destination, Kind::Ollama, &mut inventory).is_err());
        assert!(!destination.exists());
        assert!(root.path().read_dir().expect("entries").all(|entry| {
            !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(TEMPORARY_PREFIX)
        }));
    }

    #[test]
    fn atomic_publish_primitive_preserves_existing_destination() {
        let root = tempdir().expect("root");
        let mut temporary = tempfile::Builder::new()
            .prefix(TEMPORARY_PREFIX)
            .tempfile_in(root.path())
            .expect("temporary");
        temporary.write_all(b"candidate").expect("candidate");
        temporary.as_file().sync_all().expect("sync");
        let destination = root.path().join("destination.tar");
        std::fs::write(&destination, b"existing").expect("existing");
        assert!(publish_noclobber(temporary, root.path(), &destination).is_err());
        assert_eq!(
            std::fs::read(&destination).expect("destination"),
            b"existing"
        );
        assert!(root.path().read_dir().expect("entries").all(|entry| {
            !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(TEMPORARY_PREFIX)
        }));
    }
}
