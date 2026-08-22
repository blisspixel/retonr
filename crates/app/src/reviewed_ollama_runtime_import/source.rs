use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{Read as _, Seek as _},
    path::Path,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::{
    MemberOpenError, ReconstructedRuntimePackage, RuntimeLayoutLimits, RuntimePackageLayout,
    RuntimeReconstructionError, reconstruct_runtime_package_with_limits,
};
use rewrite_types::CancellationToken;

use crate::artifact_storage::{
    ManagedFile, PinnedDirectory, StableMetadataFingerprint, fingerprint_std_file, is_indirect,
};
use crate::{ArtifactInventoryError, OllamaRuntimeImportError};

use super::ReviewedOllamaRuntimeSource;

struct BoundFile {
    name: String,
    opened: ManagedFile,
}

struct BoundMember {
    path: ArtifactSetRelativePath,
    opened: ManagedFile,
}

pub(crate) struct PinnedReviewedOllamaRuntime {
    layout_parent: PinnedDirectory,
    layout: BoundFile,
    member_root_path: std::path::PathBuf,
    member_root: PinnedDirectory,
    member_root_baseline: StableMetadataFingerprint,
    members: Vec<BoundMember>,
    reconstructed: ReconstructedRuntimePackage,
}

impl PinnedReviewedOllamaRuntime {
    pub(crate) fn open_and_reconstruct(
        selection: &ReviewedOllamaRuntimeSource,
        limits: &RuntimeLayoutLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, OllamaRuntimeImportError> {
        ensure_not_cancelled(cancellation)?;
        limits.validate()?;
        reject_indirect_source(selection.layout_path())?;
        reject_indirect_source(selection.member_root())?;
        let layout_parent_path = selection
            .layout_path()
            .parent()
            .ok_or(OllamaRuntimeImportError::UnsafeSource)?;
        let layout_name = selection
            .layout_path()
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(OllamaRuntimeImportError::UnsafeSource)?;
        let layout_parent =
            PinnedDirectory::open_existing(layout_parent_path).map_err(map_source)?;
        let mut layout = open_bound_file(&layout_parent, layout_name)?;
        let raw_layout = read_layout(&mut layout, limits.layout_bytes, cancellation)?;
        let plan = RuntimePackageLayout::parse(&raw_layout, *limits)?;
        let member_root =
            PinnedDirectory::open_existing(selection.member_root()).map_err(map_source)?;
        let member_root_baseline = member_root.fingerprint().map_err(map_source)?.stable();
        let members = open_members(&member_root, &plan)?;
        let reconstructed = reconstruct_runtime_package_with_limits::<File, _, _>(
            &raw_layout,
            limits,
            |path| clone_member(&members, path),
            || cancellation.is_cancelled(),
        )?;
        let source = Self {
            layout_parent,
            layout,
            member_root_path: selection.member_root().to_path_buf(),
            member_root,
            member_root_baseline,
            members,
            reconstructed,
        };
        source.recheck()?;
        Ok(source)
    }

    pub(crate) const fn reconstructed(&self) -> &ReconstructedRuntimePackage {
        &self.reconstructed
    }

    pub(crate) fn recheck(&self) -> Result<(), OllamaRuntimeImportError> {
        recheck_bound_file(&self.layout_parent, &self.layout)?;
        let held_member_root = self.member_root.fingerprint().map_err(map_source)?.stable();
        let named_member_root = PinnedDirectory::fingerprint_path(&self.member_root_path)
            .map_err(map_source)?
            .stable();
        if held_member_root != self.member_root_baseline
            || named_member_root != self.member_root_baseline
        {
            return Err(OllamaRuntimeImportError::SourceChanged);
        }
        for member in &self.members {
            recheck_open_file(&member.opened)?;
            self.member_root
                .recheck_relative_regular_file(&member.path, &member.opened.fingerprint)
                .map_err(map_source)?;
        }
        Ok(())
    }
}

fn open_members(
    root: &PinnedDirectory,
    layout: &RuntimePackageLayout,
) -> Result<Vec<BoundMember>, OllamaRuntimeImportError> {
    layout
        .members()
        .iter()
        .map(|member| {
            let opened = root
                .open_relative_regular_file(member.relative_path())
                .map_err(map_source)?;
            if !opened.fingerprint.has_single_link() {
                return Err(OllamaRuntimeImportError::UnsafeSource);
            }
            if opened.byte_size != member.byte_size() {
                return Err(OllamaRuntimeImportError::Reconstruction(
                    RuntimeReconstructionError::MemberSizeMismatch,
                ));
            }
            Ok(BoundMember {
                path: member.relative_path().clone(),
                opened,
            })
        })
        .collect()
}

fn clone_member(
    members: &[BoundMember],
    path: &ArtifactSetRelativePath,
) -> Result<File, MemberOpenError> {
    let member = members
        .iter()
        .find(|member| member.path.as_str() == path.as_str())
        .ok_or(MemberOpenError)?;
    let mut file = member
        .opened
        .file
        .try_clone()
        .map_err(|_| MemberOpenError)?;
    file.seek(std::io::SeekFrom::Start(0))
        .map_err(|_| MemberOpenError)?;
    Ok(file)
}

fn open_bound_file(
    parent: &PinnedDirectory,
    name: &str,
) -> Result<BoundFile, OllamaRuntimeImportError> {
    let relative = ArtifactSetRelativePath::new(name.to_owned())
        .map_err(|_| OllamaRuntimeImportError::UnsafeSource)?;
    let opened = parent
        .open_relative_regular_file(&relative)
        .map_err(map_source)?;
    if !opened.fingerprint.has_single_link() {
        return Err(OllamaRuntimeImportError::UnsafeSource);
    }
    Ok(BoundFile {
        name: name.to_owned(),
        opened,
    })
}

fn read_layout(
    layout: &mut BoundFile,
    maximum: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, OllamaRuntimeImportError> {
    let size = usize::try_from(layout.opened.byte_size).map_err(|_| {
        OllamaRuntimeImportError::Reconstruction(RuntimeReconstructionError::LayoutTooLarge)
    })?;
    if size > maximum {
        return Err(OllamaRuntimeImportError::Reconstruction(
            RuntimeReconstructionError::LayoutTooLarge,
        ));
    }
    let mut bytes = vec![0_u8; size];
    layout
        .opened
        .file
        .seek(std::io::SeekFrom::Start(0))
        .map_err(OllamaRuntimeImportError::SourceIo)?;
    for chunk in bytes.chunks_mut(64 * 1024) {
        ensure_not_cancelled(cancellation)?;
        layout
            .opened
            .file
            .read_exact(chunk)
            .map_err(OllamaRuntimeImportError::SourceIo)?;
    }
    let mut trailing = [0_u8; 1];
    if layout
        .opened
        .file
        .read(&mut trailing)
        .map_err(OllamaRuntimeImportError::SourceIo)?
        != 0
    {
        return Err(OllamaRuntimeImportError::SourceChanged);
    }
    recheck_open_file(&layout.opened)?;
    Ok(bytes)
}

fn recheck_bound_file(
    parent: &PinnedDirectory,
    bound: &BoundFile,
) -> Result<(), OllamaRuntimeImportError> {
    recheck_open_file(&bound.opened)?;
    let relative = ArtifactSetRelativePath::new(bound.name.clone())
        .map_err(|_| OllamaRuntimeImportError::SourceChanged)?;
    parent
        .recheck_relative_regular_file(&relative, &bound.opened.fingerprint)
        .map_err(map_source)
}

fn recheck_open_file(opened: &ManagedFile) -> Result<(), OllamaRuntimeImportError> {
    let current = fingerprint_std_file(&opened.file).map_err(map_source)?;
    if current == opened.fingerprint && current.has_single_link() {
        Ok(())
    } else {
        Err(OllamaRuntimeImportError::SourceChanged)
    }
}

fn reject_indirect_source(path: &Path) -> Result<(), OllamaRuntimeImportError> {
    let metadata = fs::symlink_metadata(path).map_err(OllamaRuntimeImportError::SourceIo)?;
    if is_indirect(&metadata) {
        Err(OllamaRuntimeImportError::UnsafeSource)
    } else {
        Ok(())
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), OllamaRuntimeImportError> {
    if cancellation.is_cancelled() {
        Err(OllamaRuntimeImportError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_source(error: ArtifactInventoryError) -> OllamaRuntimeImportError {
    match error {
        ArtifactInventoryError::StorageIo(error) => OllamaRuntimeImportError::SourceIo(error),
        ArtifactInventoryError::ConcurrentModification => OllamaRuntimeImportError::SourceChanged,
        ArtifactInventoryError::Cancelled => OllamaRuntimeImportError::Cancelled,
        _ => OllamaRuntimeImportError::UnsafeSource,
    }
}
