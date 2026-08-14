use std::{fs::File, io};

use super::is_indirect;

#[cfg_attr(unix, derive(Clone, Copy))]
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MetadataFingerprint {
    file_type: FileTypeFingerprint,
    pub(super) length: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
    #[cfg(windows)]
    identity: same_file::Handle,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    #[cfg(windows)]
    volume_serial_number: u64,
    #[cfg(windows)]
    file_index: u64,
    link_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StableMetadataFingerprint {
    file_type: FileTypeFingerprint,
    length: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
    #[cfg(windows)]
    volume_serial_number: u64,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    last_write_time: u64,
    link_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileTypeFingerprint {
    File,
    Directory,
    Symlink,
    Other,
}

impl MetadataFingerprint {
    pub(super) fn from_file(file: &File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        #[cfg(windows)]
        let windows_information = winx::winapi_util::file::information(file)?;
        Ok(Self {
            file_type: file_type(&metadata),
            length: metadata.len(),
            #[cfg(unix)]
            device: std::os::unix::fs::MetadataExt::dev(&metadata),
            #[cfg(unix)]
            inode: std::os::unix::fs::MetadataExt::ino(&metadata),
            #[cfg(unix)]
            modified_seconds: std::os::unix::fs::MetadataExt::mtime(&metadata),
            #[cfg(unix)]
            modified_nanoseconds: std::os::unix::fs::MetadataExt::mtime_nsec(&metadata),
            #[cfg(unix)]
            changed_seconds: std::os::unix::fs::MetadataExt::ctime(&metadata),
            #[cfg(unix)]
            changed_nanoseconds: std::os::unix::fs::MetadataExt::ctime_nsec(&metadata),
            #[cfg(windows)]
            identity: same_file::Handle::from_file(file.try_clone()?)?,
            #[cfg(windows)]
            creation_time: std::os::windows::fs::MetadataExt::creation_time(&metadata),
            #[cfg(windows)]
            last_write_time: std::os::windows::fs::MetadataExt::last_write_time(&metadata),
            #[cfg(windows)]
            volume_serial_number: windows_information.volume_serial_number(),
            #[cfg(windows)]
            file_index: windows_information.file_index(),
            #[cfg(unix)]
            link_count: std::os::unix::fs::MetadataExt::nlink(&metadata),
            #[cfg(windows)]
            link_count: windows_information.number_of_links(),
        })
    }

    pub(crate) fn has_single_link(&self) -> bool {
        self.link_count == 1
    }

    pub(crate) fn same_identity(&self, other: &Self) -> bool {
        #[cfg(unix)]
        {
            self.device == other.device && self.inode == other.inode
        }
        #[cfg(windows)]
        {
            self.identity == other.identity
        }
    }

    pub(crate) fn same_filesystem(&self, other: &Self) -> bool {
        #[cfg(unix)]
        {
            self.device == other.device
        }
        #[cfg(windows)]
        {
            self.volume_serial_number == other.volume_serial_number
        }
    }

    pub(crate) fn stable(&self) -> StableMetadataFingerprint {
        StableMetadataFingerprint {
            file_type: self.file_type,
            length: self.length,
            #[cfg(unix)]
            device: self.device,
            #[cfg(unix)]
            inode: self.inode,
            #[cfg(unix)]
            modified_seconds: self.modified_seconds,
            #[cfg(unix)]
            modified_nanoseconds: self.modified_nanoseconds,
            #[cfg(unix)]
            changed_seconds: self.changed_seconds,
            #[cfg(unix)]
            changed_nanoseconds: self.changed_nanoseconds,
            #[cfg(windows)]
            volume_serial_number: self.volume_serial_number,
            #[cfg(windows)]
            file_index: self.file_index,
            #[cfg(windows)]
            creation_time: self.creation_time,
            #[cfg(windows)]
            last_write_time: self.last_write_time,
            link_count: self.link_count,
        }
    }

    pub(super) fn release(self) {
        #[cfg(windows)]
        drop(self);
        #[cfg(not(windows))]
        let _ = self;
    }
}

impl StableMetadataFingerprint {
    pub(crate) fn same_identity(&self, observed: &MetadataFingerprint) -> bool {
        #[cfg(unix)]
        {
            self.device == observed.device && self.inode == observed.inode
        }
        #[cfg(windows)]
        {
            self.volume_serial_number == observed.volume_serial_number
                && self.file_index == observed.file_index
        }
    }
}

fn file_type(metadata: &std::fs::Metadata) -> FileTypeFingerprint {
    if is_indirect(metadata) {
        FileTypeFingerprint::Symlink
    } else if metadata.is_file() {
        FileTypeFingerprint::File
    } else if metadata.is_dir() {
        FileTypeFingerprint::Directory
    } else {
        FileTypeFingerprint::Other
    }
}
