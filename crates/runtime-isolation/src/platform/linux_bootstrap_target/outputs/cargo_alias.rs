use super::{HelperFailure, RESOLUTION, Stamp, has_elf_magic, open_with_links, recipe, stamp};
use rustix::fs::{Dir, Mode, OFlags, openat2};
use std::fs::File;

pub(super) struct Candidate {
    pub(super) file: File,
    path: String,
    before: Stamp,
    alias: Option<(String, File)>,
}

impl Candidate {
    pub(super) fn open(target: &File, build: recipe::Build) -> Result<Self, HelperFailure> {
        let path = build.output();
        let file = open_with_links(target, &path, true, 2)?;
        if !has_elf_magic(&file).map_err(|_| HelperFailure::BootstrapRootVerification)? {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        let before = stamp(&file)?;
        let alias = if before.links == 2 {
            Some(find_alias(target, build, &before)?)
        } else {
            None
        };
        let candidate = Self {
            file,
            path,
            before,
            alias,
        };
        candidate.revalidate(target)?;
        Ok(candidate)
    }

    pub(super) fn revalidate(&self, target: &File) -> Result<(), HelperFailure> {
        if stamp(&self.file)? != self.before
            || stamp(&open_with_links(target, &self.path, true, 2)?)? != self.before
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        if let Some((path, file)) = &self.alias
            && (stamp(file)? != self.before
                || stamp(&open_with_links(target, path, true, 2)?)? != self.before)
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        Ok(())
    }
}

fn find_alias(
    target: &File,
    build: recipe::Build,
    expected: &Stamp,
) -> Result<(String, File), HelperFailure> {
    let directory = openat2(
        target,
        "release/deps",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        RESOLUTION,
    )
    .map(File::from)
    .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let prefix = format!("{}-", build.program.replace('-', "_"));
    let mut found = None;
    for (index, entry) in Dir::read_from(&directory)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?
        .enumerate()
    {
        if index >= 65_536 {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        let entry = entry.map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let bytes = entry.file_name().to_bytes();
        let Ok(name) = std::str::from_utf8(bytes) else {
            continue;
        };
        let Some(hash) = name.strip_prefix(&prefix) else {
            continue;
        };
        if hash.len() != 16
            || !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            continue;
        }
        let path = format!("release/deps/{name}");
        let file = open_with_links(target, &path, true, 2)?;
        let observed = stamp(&file)?;
        if observed.device == expected.device && observed.inode == expected.inode {
            if observed != *expected || found.is_some() {
                return Err(HelperFailure::BootstrapRootVerification);
            }
            found = Some((path, file));
        }
    }
    found.ok_or(HelperFailure::BootstrapRootVerification)
}
