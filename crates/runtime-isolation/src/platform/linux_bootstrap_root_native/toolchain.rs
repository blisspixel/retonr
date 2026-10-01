use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read as _,
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::{Component, Path, PathBuf},
};

use super::{HelperFailure, tree::copy_file};

const MAXIMUM_MANIFEST_BYTES: u64 = 2 * 1024 * 1024;
const MAXIMUM_ENTRIES: usize = 262_144;

pub(super) fn install_component(source: &Path, destination: &Path) -> Result<(), HelperFailure> {
    let manifest = declarations(source)?;
    let (files, directories) = inventory(source)?;
    if files != manifest {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    // Validate the complete component and all destination collisions before copying.
    for relative in &files {
        if fs::symlink_metadata(destination.join(relative)).is_ok() {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    for (relative, mode) in &directories {
        let target = destination.join(relative);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.is_dir() && metadata.mode() & 0o777 == *mode => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) | Err(_) => return Err(HelperFailure::BootstrapRootVerification),
        }
    }
    for (relative, mode) in directories {
        let target = destination.join(relative);
        if !target.exists() {
            fs::create_dir(&target).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode))
                .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        }
    }
    for relative in files {
        let path = source.join(&relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        let file = File::open(path).map_err(|_| HelperFailure::BootstrapRootPreparation)?;
        copy_file(&file, &destination.join(relative), metadata.mode() & 0o777)?;
    }
    Ok(())
}

fn declarations(source: &Path) -> Result<BTreeSet<PathBuf>, HelperFailure> {
    let path = source.join("manifest.in");
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > MAXIMUM_MANIFEST_BYTES {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let mut text = String::new();
    File::open(path)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?
        .take(MAXIMUM_MANIFEST_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if text.len() > usize::try_from(MAXIMUM_MANIFEST_BYTES).unwrap_or(usize::MAX)
        || !text.ends_with('\n')
        || text.contains(['\r', '\0'])
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let mut files = BTreeSet::new();
    for line in text.lines() {
        let relative = line
            .strip_prefix("file:")
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        let path = PathBuf::from(relative);
        if relative.is_empty()
            || !relative.is_ascii()
            || relative.len() > 4096
            || relative.contains(['\\', '\r', '\0'])
            || relative
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || path == Path::new("manifest.in")
            || !files.insert(path)
            || files.len() > MAXIMUM_ENTRIES
        {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    if files.is_empty() {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok(files)
}

type ComponentInventory = (BTreeSet<PathBuf>, BTreeMap<PathBuf, u32>);

fn inventory(source: &Path) -> Result<ComponentInventory, HelperFailure> {
    let mut files = BTreeSet::new();
    let mut directories = BTreeMap::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(source.join(&relative))
            .map_err(|_| HelperFailure::BootstrapRootVerification)?
        {
            let entry = entry.map_err(|_| HelperFailure::BootstrapRootVerification)?;
            let child = relative.join(entry.file_name());
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| HelperFailure::BootstrapRootVerification)?;
            if metadata.is_dir() {
                directories.insert(child.clone(), metadata.mode() & 0o777);
                pending.push(child);
            } else if metadata.is_file() && metadata.nlink() == 1 {
                if child != Path::new("manifest.in") {
                    files.insert(child);
                }
            } else {
                return Err(HelperFailure::BootstrapRootVerification);
            }
            if files.len() + directories.len() > MAXIMUM_ENTRIES {
                return Err(HelperFailure::BootstrapRootVerification);
            }
        }
    }
    if directories
        .keys()
        .any(|directory| !files.iter().any(|file| file.starts_with(directory)))
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    Ok((files, directories))
}

#[cfg(test)]
mod tests;
