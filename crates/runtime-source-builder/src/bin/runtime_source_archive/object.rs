use std::{
    fs::{self, File, Metadata},
    path::Path,
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObjectStamp {
    kind: ObjectKind,
    pub(super) length: u64,
    pub(super) identity: Arc<same_file::Handle>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    links: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
    #[cfg(windows)]
    attributes: u32,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
}

impl ObjectStamp {
    pub(super) fn from_path(path: &Path, metadata: &Metadata) -> Result<Self, ()> {
        let identity = same_file::Handle::from_path(path).map_err(|_| ())?;
        Self::from_metadata(metadata, Arc::new(identity))
    }

    pub(super) fn from_file(file: &File, metadata: &Metadata) -> Result<Self, ()> {
        let clone = file.try_clone().map_err(|_| ())?;
        let identity = same_file::Handle::from_file(clone).map_err(|_| ())?;
        Self::from_metadata(metadata, Arc::new(identity))
    }

    fn from_metadata(metadata: &Metadata, identity: Arc<same_file::Handle>) -> Result<Self, ()> {
        let kind = if metadata.is_file() {
            ObjectKind::File
        } else if metadata.is_dir() {
            ObjectKind::Directory
        } else {
            return Err(());
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if kind == ObjectKind::File && metadata.nlink() != 1 {
                return Err(());
            }
            Ok(Self {
                kind,
                length: metadata.len(),
                identity,
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                links: metadata.nlink(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return Err(());
            }
            Ok(Self {
                kind,
                length: metadata.file_size(),
                identity,
                attributes: metadata.file_attributes(),
                creation_time: metadata.creation_time(),
                last_write_time: metadata.last_write_time(),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                kind,
                length: metadata.len(),
                identity,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObjectKind {
    Directory,
    File,
}

pub(super) fn validate_file_object(
    path: &Path,
    file: &File,
    expected: &ObjectStamp,
) -> Result<(), ()> {
    let handle = ObjectStamp::from_file(file, &file.metadata().map_err(|_| ())?)?;
    let path = ObjectStamp::from_path(path, &fs::symlink_metadata(path).map_err(|_| ())?)?;
    if handle == *expected && path == *expected && expected.kind == ObjectKind::File {
        Ok(())
    } else {
        Err(())
    }
}
