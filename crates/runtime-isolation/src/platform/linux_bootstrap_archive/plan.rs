use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Seek as _, SeekFrom},
    path::{Component, Path, PathBuf},
};

use rewrite_types::Digest as DomainDigest;
use sha2::{Digest as _, Sha256};

use super::{ArchiveLimits, LinkPolicy, MAXIMUM_PATH_BYTES, TAR_BLOCK_BYTES};
use crate::platform::linux_helper_setup::HelperFailure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntryKind {
    Directory,
    Regular,
    Symlink,
    Hardlink,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PlannedLink {
    pub(super) stored_target: PathBuf,
    pub(super) resolved_target: PathBuf,
    pub(super) deferred_proc_mtab: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PlannedEntry {
    pub(super) relative: PathBuf,
    pub(super) kind: EntryKind,
    pub(super) mode: u32,
    pub(super) bytes: u64,
    pub(super) link: Option<PlannedLink>,
}

pub(super) struct ArchivePlan {
    pub(super) entries: Vec<PlannedEntry>,
    pub(super) regular_bytes: u64,
    pub(super) link_plan_digest: DomainDigest,
    pub(super) deferred_proc_mtab: bool,
}

pub(super) fn validate(
    archive: &mut File,
    required_root: Option<&str>,
    policy: LinkPolicy,
    limits: ArchiveLimits,
) -> Result<ArchivePlan, HelperFailure> {
    archive
        .seek(SeekFrom::Start(0))
        .map_err(|_| HelperFailure::BootstrapRootPreparation)?;
    let mut tar = tar::Archive::new(&mut *archive);
    let mut entries = Vec::new();
    let mut paths = BTreeSet::new();
    let mut kinds = BTreeMap::new();
    let mut regular_bytes = 0_u64;
    let mut link_bytes = 0_usize;
    let mut links = 0_usize;
    for raw in tar
        .entries()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?
    {
        let entry = raw.map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let planned = plan_entry(&entry, required_root, policy, limits)?;
        if !paths.insert(planned.relative.clone()) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        require_parents(&planned, &kinds)?;
        kinds.insert(planned.relative.clone(), planned.kind);
        regular_bytes = regular_bytes
            .checked_add(planned.bytes)
            .filter(|bytes| *bytes <= limits.total_bytes)
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        if let Some(link) = &planned.link {
            links = links
                .checked_add(1)
                .filter(|count| *count <= limits.links)
                .ok_or(HelperFailure::BootstrapRootVerification)?;
            link_bytes = link_bytes
                .checked_add(link.stored_target.as_os_str().as_encoded_bytes().len())
                .filter(|bytes| *bytes <= limits.link_bytes)
                .ok_or(HelperFailure::BootstrapRootVerification)?;
        }
        entries.push(planned);
        if entries.len() > limits.entries {
            return Err(HelperFailure::BootstrapRootVerification);
        }
    }
    if entries.is_empty() {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let position = tar
        .into_inner()
        .stream_position()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    require_zero_trailer(archive, position)?;
    let (regular_bytes, deferred_proc_mtab) = validate_links(&entries, regular_bytes, limits)?;
    Ok(ArchivePlan {
        link_plan_digest: link_plan_digest(&entries)?,
        entries,
        regular_bytes,
        deferred_proc_mtab,
    })
}

pub(super) fn plan_entry<R: Read>(
    entry: &tar::Entry<'_, R>,
    required_root: Option<&str>,
    policy: LinkPolicy,
    limits: ArchiveLimits,
) -> Result<PlannedEntry, HelperFailure> {
    let entry_type = entry.header().entry_type();
    let kind = if entry_type.is_dir() {
        EntryKind::Directory
    } else if entry_type.is_file() {
        EntryKind::Regular
    } else if entry_type.is_symlink() && policy == LinkPolicy::AlpineMinirootfs {
        EntryKind::Symlink
    } else if entry_type.is_hard_link() && policy == LinkPolicy::AlpineMinirootfs {
        EntryKind::Hardlink
    } else {
        return Err(HelperFailure::BootstrapRootVerification);
    };
    let raw_path = entry.path_bytes();
    let text = std::str::from_utf8(raw_path.as_ref())
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let relative = canonical_member(text, required_root, kind, policy)?;
    let bytes = entry
        .header()
        .size()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if (kind != EntryKind::Regular && bytes != 0) || bytes > limits.file_bytes {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let mode = entry
        .header()
        .mode()
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let allowed_mode = if kind == EntryKind::Directory {
        0o1777
    } else {
        0o777
    };
    if mode & !allowed_mode != 0 {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let link = match kind {
        EntryKind::Symlink => Some(plan_symlink(entry, &relative)?),
        EntryKind::Hardlink => Some(plan_hardlink(entry, policy)?),
        EntryKind::Directory | EntryKind::Regular => None,
    };
    Ok(PlannedEntry {
        relative,
        kind,
        mode,
        bytes,
        link,
    })
}

fn canonical_member(
    text: &str,
    required_root: Option<&str>,
    kind: EntryKind,
    policy: LinkPolicy,
) -> Result<PathBuf, HelperFailure> {
    if text.is_empty()
        || text.len() > MAXIMUM_PATH_BYTES
        || text.contains('\\')
        || text.contains("//")
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let mut text = if kind == EntryKind::Directory {
        text.strip_suffix('/').unwrap_or(text)
    } else if text.ends_with('/') {
        return Err(HelperFailure::BootstrapRootVerification);
    } else {
        text
    };
    if policy == LinkPolicy::AlpineMinirootfs {
        if text == "." {
            text = "";
        } else if let Some(stripped) = text.strip_prefix("./") {
            text = stripped;
        }
    }
    if text.is_empty() {
        return if kind == EntryKind::Directory
            && (required_root.is_some() || policy == LinkPolicy::AlpineMinirootfs)
        {
            Ok(PathBuf::new())
        } else {
            Err(HelperFailure::BootstrapRootVerification)
        };
    }
    if text
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let path = Path::new(text);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let mut components = path.components();
    if let Some(root) = required_root
        && components
            .next()
            .and_then(|value| value.as_os_str().to_str())
            != Some(root)
    {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    let relative = components.collect::<PathBuf>();
    if relative.as_os_str().is_empty() && kind != EntryKind::Directory {
        Err(HelperFailure::BootstrapRootVerification)
    } else {
        Ok(relative)
    }
}

fn plan_symlink<R: Read>(
    entry: &tar::Entry<'_, R>,
    relative: &Path,
) -> Result<PlannedLink, HelperFailure> {
    let raw = link_text(entry)?;
    let normalized = raw.strip_prefix("./").unwrap_or(&raw);
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let resolved = resolve_symlink_target(parent, normalized)?;
    Ok(PlannedLink {
        stored_target: relative_target(parent, &resolved),
        resolved_target: resolved,
        deferred_proc_mtab: relative == Path::new("etc/mtab") && raw == "../proc/mounts",
    })
}

fn plan_hardlink<R: Read>(
    entry: &tar::Entry<'_, R>,
    policy: LinkPolicy,
) -> Result<PlannedLink, HelperFailure> {
    let raw = link_text(entry)?;
    let resolved = canonical_member(&raw, None, EntryKind::Regular, policy)?;
    Ok(PlannedLink {
        stored_target: resolved.clone(),
        resolved_target: resolved,
        deferred_proc_mtab: false,
    })
}

fn link_text<R: Read>(entry: &tar::Entry<'_, R>) -> Result<String, HelperFailure> {
    let raw = entry
        .link_name_bytes()
        .ok_or(HelperFailure::BootstrapRootVerification)?;
    let text =
        std::str::from_utf8(raw.as_ref()).map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if text.is_empty()
        || text.len() > MAXIMUM_PATH_BYTES
        || text.contains('\\')
        || text.contains("//")
    {
        Err(HelperFailure::BootstrapRootVerification)
    } else {
        Ok(text.to_owned())
    }
}

fn resolve_symlink_target(parent: &Path, raw: &str) -> Result<PathBuf, HelperFailure> {
    let mut components = if raw.starts_with('/') {
        Vec::new()
    } else {
        normal_components(parent)?
    };
    for component in raw.trim_start_matches('/').split('/') {
        if component == ".." {
            components
                .pop()
                .ok_or(HelperFailure::BootstrapRootVerification)?;
        } else if component.is_empty()
            || component == "."
            || !component.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        {
            return Err(HelperFailure::BootstrapRootVerification);
        } else {
            components.push(component.to_owned());
        }
    }
    if components.is_empty() {
        Err(HelperFailure::BootstrapRootVerification)
    } else {
        Ok(components.into_iter().collect())
    }
}

fn normal_components(path: &Path) -> Result<Vec<String>, HelperFailure> {
    path.components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(ToOwned::to_owned)
                .ok_or(HelperFailure::BootstrapRootVerification),
            _ => Err(HelperFailure::BootstrapRootVerification),
        })
        .collect()
}

fn relative_target(parent: &Path, target: &Path) -> PathBuf {
    let parent = parent.components().collect::<Vec<_>>();
    let target = target.components().collect::<Vec<_>>();
    let common = parent
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..parent.len() {
        relative.push("..");
    }
    for component in &target[common..] {
        relative.push(component.as_os_str());
    }
    relative
}

fn require_parents(
    entry: &PlannedEntry,
    kinds: &BTreeMap<PathBuf, EntryKind>,
) -> Result<(), HelperFailure> {
    let mut parent = entry.relative.parent();
    while let Some(path) = parent.filter(|path| !path.as_os_str().is_empty()) {
        if kinds.get(path) != Some(&EntryKind::Directory) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        parent = path.parent();
    }
    Ok(())
}

fn validate_links(
    entries: &[PlannedEntry],
    mut regular_bytes: u64,
    limits: ArchiveLimits,
) -> Result<(u64, bool), HelperFailure> {
    let index = entries
        .iter()
        .enumerate()
        .map(|(position, entry)| (entry.relative.clone(), position))
        .collect::<BTreeMap<_, _>>();
    let mut deferred = false;
    for (position, entry) in entries.iter().enumerate() {
        match entry.kind {
            EntryKind::Hardlink => {
                let target = entry
                    .link
                    .as_ref()
                    .ok_or(HelperFailure::BootstrapRootVerification)?;
                let target_position = *index
                    .get(&target.resolved_target)
                    .ok_or(HelperFailure::BootstrapRootVerification)?;
                let target_entry = &entries[target_position];
                if target_position >= position || target_entry.kind != EntryKind::Regular {
                    return Err(HelperFailure::BootstrapRootVerification);
                }
                regular_bytes = regular_bytes
                    .checked_add(target_entry.bytes)
                    .filter(|bytes| *bytes <= limits.total_bytes)
                    .ok_or(HelperFailure::BootstrapRootVerification)?;
            }
            EntryKind::Symlink => {
                let link = entry
                    .link
                    .as_ref()
                    .ok_or(HelperFailure::BootstrapRootVerification)?;
                match terminal(entries, &index, &link.resolved_target, limits.link_depth)? {
                    Some(EntryKind::Regular | EntryKind::Directory | EntryKind::Hardlink) => {}
                    Some(EntryKind::Symlink) => {
                        return Err(HelperFailure::BootstrapRootVerification);
                    }
                    None if link.deferred_proc_mtab && !deferred => deferred = true,
                    None => return Err(HelperFailure::BootstrapRootVerification),
                }
            }
            EntryKind::Directory | EntryKind::Regular => {}
        }
    }
    Ok((regular_bytes, deferred))
}

fn terminal(
    entries: &[PlannedEntry],
    index: &BTreeMap<PathBuf, usize>,
    start: &Path,
    maximum_depth: usize,
) -> Result<Option<EntryKind>, HelperFailure> {
    let mut current = start.to_path_buf();
    let mut seen = BTreeSet::new();
    for _ in 0..=maximum_depth {
        if !seen.insert(current.clone()) {
            return Err(HelperFailure::BootstrapRootVerification);
        }
        let Some(position) = index.get(&current) else {
            return Ok(None);
        };
        let entry = &entries[*position];
        if entry.kind != EntryKind::Symlink {
            return Ok(Some(entry.kind));
        }
        current.clone_from(
            &entry
                .link
                .as_ref()
                .ok_or(HelperFailure::BootstrapRootVerification)?
                .resolved_target,
        );
    }
    Err(HelperFailure::BootstrapRootVerification)
}

fn link_plan_digest(entries: &[PlannedEntry]) -> Result<DomainDigest, HelperFailure> {
    let mut hasher = Sha256::new();
    hasher.update(b"runtime-isolation/bootstrap-normalized-link-plan/v1\0");
    for entry in entries.iter().filter(|entry| entry.link.is_some()) {
        let link = entry
            .link
            .as_ref()
            .ok_or(HelperFailure::BootstrapRootVerification)?;
        hasher.update(match entry.kind {
            EntryKind::Symlink => b"s".as_slice(),
            EntryKind::Hardlink => b"h".as_slice(),
            EntryKind::Directory | EntryKind::Regular => {
                return Err(HelperFailure::BootstrapRootVerification);
            }
        });
        for value in [&entry.relative, &link.stored_target, &link.resolved_target] {
            hasher.update(value.as_os_str().as_encoded_bytes());
            hasher.update([0]);
        }
        hasher.update([u8::from(link.deferred_proc_mtab)]);
    }
    DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| HelperFailure::BootstrapRootVerification)
}

fn require_zero_trailer(archive: &mut File, position: u64) -> Result<(), HelperFailure> {
    if !position.is_multiple_of(TAR_BLOCK_BYTES) {
        return Err(HelperFailure::BootstrapRootVerification);
    }
    archive
        .seek(SeekFrom::Start(position))
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    let mut trailer = Vec::new();
    archive
        .read_to_end(&mut trailer)
        .map_err(|_| HelperFailure::BootstrapRootVerification)?;
    if trailer.len() < usize::try_from(TAR_BLOCK_BYTES).unwrap_or(usize::MAX)
        || trailer.iter().any(|byte| *byte != 0)
    {
        Err(HelperFailure::BootstrapRootVerification)
    } else {
        Ok(())
    }
}
