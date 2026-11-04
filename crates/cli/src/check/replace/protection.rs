//! Fail-closed access metadata policy for replacement artifacts.

use super::{CommandName, RunFailure, usage};
use std::{fs::File, path::Path};

#[cfg(target_os = "linux")]
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
};

/// Supported source policy, captured before creating document-bearing files.
pub(super) struct Protection {
    #[cfg(target_os = "linux")]
    metadata: std::fs::Metadata,
}

fn unsupported(command: CommandName) -> RunFailure {
    usage(
        command,
        "in-place access protection is unsupported for this platform or file metadata; use a separately reviewed output path",
    )
}

impl Protection {
    pub(super) fn capture(source: &Path, command: CommandName) -> Result<Self, RunFailure> {
        #[cfg(target_os = "linux")]
        {
            let file = open(source, command)?;
            let metadata = file
                .metadata()
                .map_err(|_| RunFailure::operational(command))?;
            no_extended_attributes(&file, command)?;
            if !metadata.is_file() || metadata.nlink() != 1 || metadata.mode() & 0o7000 != 0 {
                return Err(unsupported(command));
            }
            Ok(Self { metadata })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = source;
            Err(unsupported(command))
        }
    }

    pub(super) fn write(
        &self,
        path: &Path,
        bytes: &[u8],
        command: CommandName,
    ) -> Result<(), RunFailure> {
        #[cfg(target_os = "linux")]
        {
            // Restrict access at creation, before any document bytes exist.
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        RunFailure::output_exists_for(command)
                    } else {
                        RunFailure::operational(command)
                    }
                })?;
            no_extended_attributes(&file, command)?;
            rustix::fs::fchown(
                &file,
                Some(rustix::fs::Uid::from_raw(self.metadata.uid())),
                Some(rustix::fs::Gid::from_raw(self.metadata.gid())),
            )
            .map_err(|_| unsupported(command))?;
            file.set_permissions(std::fs::Permissions::from_mode(
                self.metadata.mode() & 0o777,
            ))
            .map_err(|_| unsupported(command))?;
            self.verify_file(&file, command)?;
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|_| RunFailure::operational(command))
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (path, bytes);
            Err(unsupported(command))
        }
    }

    pub(super) fn verify_source(
        &self,
        path: &Path,
        command: CommandName,
    ) -> Result<(), RunFailure> {
        #[cfg(target_os = "linux")]
        {
            let file = open(path, command)?;
            let metadata = file
                .metadata()
                .map_err(|_| RunFailure::operational(command))?;
            if metadata.dev() != self.metadata.dev() || metadata.ino() != self.metadata.ino() {
                return Err(RunFailure::concurrent_modification(command));
            }
            self.verify_file(&file, command)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(unsupported(command))
        }
    }

    pub(super) fn verify_path(&self, path: &Path, command: CommandName) -> Result<(), RunFailure> {
        self.verify_file(&open(path, command)?, command)
    }

    fn verify_file(&self, file: &File, command: CommandName) -> Result<(), RunFailure> {
        #[cfg(target_os = "linux")]
        {
            let metadata = file
                .metadata()
                .map_err(|_| RunFailure::operational(command))?;
            no_extended_attributes(file, command)?;
            if !metadata.is_file()
                || metadata.nlink() != 1
                || metadata.uid() != self.metadata.uid()
                || metadata.gid() != self.metadata.gid()
                || metadata.mode() & 0o7777 != self.metadata.mode() & 0o7777
            {
                return Err(unsupported(command));
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = file;
            Err(unsupported(command))
        }
    }
}

fn open(path: &Path, command: CommandName) -> Result<File, RunFailure> {
    #[cfg(target_os = "linux")]
    {
        OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
            .open(path)
            .map_err(|_| RunFailure::operational(command))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err(unsupported(command))
    }
}

#[cfg(target_os = "linux")]
fn no_extended_attributes(file: &File, command: CommandName) -> Result<(), RunFailure> {
    // ACLs, capabilities and security labels need native preservation. Refuse
    // them (including inherited ACLs on empty artifacts) instead of dropping them.
    let mut names = [0_u8; 1];
    match rustix::fs::flistxattr(file, &mut names[..]) {
        Ok(0) => Ok(()),
        _ => Err(unsupported(command)),
    }
}
