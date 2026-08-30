use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::{self, File},
    io::{Read as _, Seek as _},
    path::Path,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::{
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError,
};
use rewrite_types::CancellationToken;

use crate::{
    ArtifactInventoryError,
    artifact_storage::{
        ManagedFile, ManagedTreeEntryKind, ManagedTreeLimits, ManagedTreeSnapshot, PinnedDirectory,
        StableMetadataFingerprint, fingerprint_std_file, is_indirect,
    },
};

use super::contract::paths_overlap;
use super::{
    RuntimeSourceBuildBundleError, RuntimeSourceBuildBundleLimits, RuntimeSourceBuildBundleSource,
};

struct BoundManifest {
    name: String,
    opened: ManagedFile,
}

struct BoundComponent {
    path: ArtifactSetRelativePath,
    opened: ManagedFile,
}

pub(super) struct PinnedRuntimeSourceBuildBundle {
    manifest_path: std::path::PathBuf,
    manifest_parent: PinnedDirectory,
    manifest: BoundManifest,
    manifest_bytes: Vec<u8>,
    component_root_path: std::path::PathBuf,
    component_root: PinnedDirectory,
    component_root_baseline: StableMetadataFingerprint,
    initial_tree: ManagedTreeSnapshot,
    tree_limits: ManagedTreeLimits,
    components: Vec<BoundComponent>,
}

impl PinnedRuntimeSourceBuildBundle {
    pub(super) fn open(
        selection: &RuntimeSourceBuildBundleSource,
        limits: RuntimeSourceBuildBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, RuntimeSourceBuildBundleError> {
        ensure_not_cancelled(cancellation)?;
        reject_indirect_source(selection.manifest_path())?;
        reject_indirect_source(selection.component_root())?;
        let manifest_parent_path = selection
            .manifest_path()
            .parent()
            .ok_or(RuntimeSourceBuildBundleError::UnsafeSource)?;
        let manifest_name = selection
            .manifest_path()
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(RuntimeSourceBuildBundleError::UnsafeSource)?;
        let manifest_parent =
            PinnedDirectory::open_existing(manifest_parent_path).map_err(map_source)?;
        let mut manifest = open_manifest(&manifest_parent, manifest_name)?;
        let manifest_bytes = read_manifest(
            &mut manifest.opened,
            limits.inputs.manifest_bytes,
            cancellation,
        )?;
        let parsed = RuntimeSourceBuildInputManifest::parse(&manifest_bytes, limits.inputs)?;
        let component_root =
            PinnedDirectory::open_existing(selection.component_root()).map_err(map_source)?;
        let component_root_baseline = component_root.fingerprint().map_err(map_source)?.stable();
        let tree_limits =
            ManagedTreeLimits::new(limits.maximum_tree_entries).map_err(map_source)?;
        let initial_tree = component_root
            .enumerate_tree(tree_limits, cancellation)
            .map_err(map_source)?;
        validate_tree_shape(&initial_tree, &parsed)?;
        let components = open_components(&component_root, &parsed)?;
        let source = Self {
            manifest_path: selection.manifest_path().to_path_buf(),
            manifest_parent,
            manifest,
            manifest_bytes,
            component_root_path: selection.component_root().to_path_buf(),
            component_root,
            component_root_baseline,
            initial_tree,
            tree_limits,
            components,
        };
        source.recheck(cancellation)?;
        Ok(source)
    }

    pub(super) fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    pub(super) fn overlaps_path(&self, path: &Path) -> bool {
        paths_overlap(&self.manifest_path, path) || paths_overlap(&self.component_root_path, path)
    }

    pub(super) fn clone_component(
        &self,
        path: &ArtifactSetRelativePath,
    ) -> Result<File, RuntimeSourceBuildInputOpenError> {
        let component = self
            .components
            .iter()
            .find(|component| component.path.as_str() == path.as_str())
            .ok_or(RuntimeSourceBuildInputOpenError)?;
        let mut file = component
            .opened
            .file
            .try_clone()
            .map_err(|_| RuntimeSourceBuildInputOpenError)?;
        file.seek(std::io::SeekFrom::Start(0))
            .map_err(|_| RuntimeSourceBuildInputOpenError)?;
        Ok(file)
    }

    pub(super) fn clone_component_capability(
        &self,
        path: &ArtifactSetRelativePath,
    ) -> Result<File, RuntimeSourceBuildBundleError> {
        let component = self
            .components
            .iter()
            .find(|component| component.path.as_str() == path.as_str())
            .ok_or(RuntimeSourceBuildBundleError::SourceChanged)?;
        let mut file = component
            .opened
            .file
            .try_clone()
            .map_err(RuntimeSourceBuildBundleError::SourceIo)?;
        file.seek(std::io::SeekFrom::Start(0))
            .map_err(RuntimeSourceBuildBundleError::SourceIo)?;
        Ok(file)
    }

    pub(super) fn clone_component_root_capability(
        &self,
    ) -> Result<File, RuntimeSourceBuildBundleError> {
        self.component_root.clone_handle().map_err(map_source)
    }

    pub(super) fn clone_all_component_capabilities(
        &self,
    ) -> Result<Vec<(ArtifactSetRelativePath, File)>, RuntimeSourceBuildBundleError> {
        self.components
            .iter()
            .map(|component| {
                component
                    .opened
                    .file
                    .try_clone()
                    .map(|file| (component.path.clone(), file))
                    .map_err(RuntimeSourceBuildBundleError::SourceIo)
            })
            .collect()
    }

    pub(super) fn recheck(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildBundleError> {
        ensure_not_cancelled(cancellation)?;
        recheck_open_file(&self.manifest.opened)?;
        let manifest_path = ArtifactSetRelativePath::new(self.manifest.name.clone())
            .map_err(|_| RuntimeSourceBuildBundleError::SourceChanged)?;
        self.manifest_parent
            .recheck_relative_regular_file(&manifest_path, &self.manifest.opened.fingerprint)
            .map_err(map_source)?;
        let held_root = self
            .component_root
            .fingerprint()
            .map_err(map_source)?
            .stable();
        let named_root = PinnedDirectory::fingerprint_path(&self.component_root_path)
            .map_err(map_source)?
            .stable();
        if held_root != self.component_root_baseline || named_root != self.component_root_baseline {
            return Err(RuntimeSourceBuildBundleError::SourceChanged);
        }
        for component in &self.components {
            ensure_not_cancelled(cancellation)?;
            recheck_open_file(&component.opened)?;
            self.component_root
                .recheck_relative_regular_file(&component.path, &component.opened.fingerprint)
                .map_err(map_source)?;
        }
        let final_tree = self
            .component_root
            .enumerate_tree(self.tree_limits, cancellation)
            .map_err(map_source)?;
        if final_tree != self.initial_tree {
            return Err(RuntimeSourceBuildBundleError::SourceChanged);
        }
        Ok(())
    }
}

fn open_manifest(
    parent: &PinnedDirectory,
    name: &str,
) -> Result<BoundManifest, RuntimeSourceBuildBundleError> {
    let relative = ArtifactSetRelativePath::new(name.to_owned())
        .map_err(|_| RuntimeSourceBuildBundleError::UnsafeSource)?;
    let opened = parent
        .open_relative_regular_file(&relative)
        .map_err(map_source)?;
    if !opened.fingerprint.has_single_link() {
        return Err(RuntimeSourceBuildBundleError::UnsafeSource);
    }
    Ok(BoundManifest {
        name: name.to_owned(),
        opened,
    })
}

fn read_manifest(
    opened: &mut ManagedFile,
    maximum: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, RuntimeSourceBuildBundleError> {
    let size = usize::try_from(opened.byte_size)
        .map_err(|_| RuntimeSourceBuildBundleError::LimitExceeded)?;
    if size > maximum {
        return Err(RuntimeSourceBuildInputError::LimitExceeded.into());
    }
    let mut bytes = vec![0_u8; size];
    opened
        .file
        .seek(std::io::SeekFrom::Start(0))
        .map_err(RuntimeSourceBuildBundleError::SourceIo)?;
    for chunk in bytes.chunks_mut(64 * 1024) {
        ensure_not_cancelled(cancellation)?;
        opened
            .file
            .read_exact(chunk)
            .map_err(RuntimeSourceBuildBundleError::SourceIo)?;
    }
    let mut trailing = [0_u8; 1];
    if opened
        .file
        .read(&mut trailing)
        .map_err(RuntimeSourceBuildBundleError::SourceIo)?
        != 0
    {
        return Err(RuntimeSourceBuildBundleError::SourceChanged);
    }
    recheck_open_file(opened)?;
    Ok(bytes)
}

fn open_components(
    root: &PinnedDirectory,
    manifest: &RuntimeSourceBuildInputManifest,
) -> Result<Vec<BoundComponent>, RuntimeSourceBuildBundleError> {
    manifest
        .components()
        .iter()
        .map(|component| {
            let opened = root
                .open_relative_regular_file(component.relative_path())
                .map_err(map_source)?;
            if !opened.fingerprint.has_single_link() {
                return Err(RuntimeSourceBuildBundleError::UnsafeSource);
            }
            if opened.byte_size != component.byte_size() {
                return Err(RuntimeSourceBuildInputError::ComponentSizeMismatch.into());
            }
            Ok(BoundComponent {
                path: component.relative_path().clone(),
                opened,
            })
        })
        .collect()
}

fn validate_tree_shape(
    snapshot: &ManagedTreeSnapshot,
    manifest: &RuntimeSourceBuildInputManifest,
) -> Result<(), RuntimeSourceBuildBundleError> {
    let mut expected = BTreeMap::new();
    for component in manifest.components() {
        let parts = component
            .relative_path()
            .as_str()
            .split('/')
            .collect::<Vec<_>>();
        let mut prefix = String::new();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            expected.insert(prefix.clone(), (ManagedTreeEntryKind::Directory, 0));
        }
        expected.insert(
            component.relative_path().as_str().to_owned(),
            (ManagedTreeEntryKind::RegularFile, component.byte_size()),
        );
    }
    let matches = snapshot.entries().len() == expected.len()
        && snapshot.entries().iter().all(|entry| {
            expected
                .get(entry.relative_path().as_str())
                .is_some_and(|(kind, size)| {
                    entry.kind() == *kind
                        && entry.byte_size() == *size
                        && (entry.kind() != ManagedTreeEntryKind::RegularFile
                            || entry.has_single_link())
                })
        });
    if matches {
        Ok(())
    } else {
        Err(RuntimeSourceBuildBundleError::SourceTreeMismatch)
    }
}

fn recheck_open_file(opened: &ManagedFile) -> Result<(), RuntimeSourceBuildBundleError> {
    let current = fingerprint_std_file(&opened.file).map_err(map_source)?;
    if current == opened.fingerprint && current.has_single_link() {
        Ok(())
    } else {
        Err(RuntimeSourceBuildBundleError::SourceChanged)
    }
}

fn reject_indirect_source(path: &Path) -> Result<(), RuntimeSourceBuildBundleError> {
    let metadata = fs::symlink_metadata(path).map_err(RuntimeSourceBuildBundleError::SourceIo)?;
    if is_indirect(&metadata) {
        Err(RuntimeSourceBuildBundleError::UnsafeSource)
    } else {
        Ok(())
    }
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), RuntimeSourceBuildBundleError> {
    if cancellation.is_cancelled() {
        Err(RuntimeSourceBuildBundleError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_source(error: ArtifactInventoryError) -> RuntimeSourceBuildBundleError {
    match error {
        ArtifactInventoryError::StorageIo(error) => RuntimeSourceBuildBundleError::SourceIo(error),
        ArtifactInventoryError::ConcurrentModification => {
            RuntimeSourceBuildBundleError::SourceChanged
        }
        ArtifactInventoryError::Cancelled => RuntimeSourceBuildBundleError::Cancelled,
        ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::InvalidLimits => RuntimeSourceBuildBundleError::LimitExceeded,
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::StorageInUse
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::State(_) => RuntimeSourceBuildBundleError::UnsafeSource,
    }
}
