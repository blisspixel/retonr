use std::{
    ffi::OsStr,
    fs::File,
    io::{Read as _, Seek as _, Write as _},
};

use rewrite_model::{ArtifactSetManifest, ArtifactSetRelativePath};
use rewrite_ollama_package::{
    BlobDescriptor, BlobOpenError, OllamaManifestPlan, ReconstructedModelPackage,
    ReconstructionLimits, parse_manifest_v2, reconstruct_model_package_with_limits,
};
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::artifact_storage::{
    ManagedFile, OwnedStagingTree, PinnedDirectory, StableMetadataFingerprint, fingerprint_std_file,
};
use crate::{ArtifactInventoryError, OllamaModelImportError};

use super::InstalledOllamaModelSource;

const COPY_BUFFER_BYTES: usize = 1024 * 1024;
const CONFIG_PATH: &str = "config/ollama-config.json";
const PARAMETERS_PATH: &str = "config/parameters.json";
const LICENSE_PATH: &str = "legal/license.txt";
const MODEL_PATH: &str = "model/model.gguf";
const TEMPLATE_PATH: &str = "prompts/template.go.tmpl";
const PROVENANCE_PATH: &str = "provenance/ollama-manifest-v2.json";

struct BoundDirectory {
    name: String,
    directory: PinnedDirectory,
    baseline: StableMetadataFingerprint,
}

struct BoundFile {
    name: String,
    opened: ManagedFile,
}

struct BoundBlob {
    descriptor: BlobDescriptor,
    source: BoundFile,
}

pub(crate) struct PinnedInstalledOllamaModel {
    root_path: std::path::PathBuf,
    root: PinnedDirectory,
    root_baseline: StableMetadataFingerprint,
    manifests: BoundDirectory,
    registry: BoundDirectory,
    namespace: BoundDirectory,
    model: BoundDirectory,
    blobs: BoundDirectory,
    manifest: BoundFile,
    package_blobs: Vec<BoundBlob>,
    reconstructed: ReconstructedModelPackage,
}

impl PinnedInstalledOllamaModel {
    pub(crate) fn open_and_reconstruct(
        selection: &InstalledOllamaModelSource,
        limits: &ReconstructionLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self, OllamaModelImportError> {
        ensure_not_cancelled(cancellation)?;
        let root = PinnedDirectory::open_existing(selection.models_root()).map_err(map_source)?;
        let root_baseline = root.fingerprint().map_err(map_source)?.stable();
        let manifests = open_bound_directory(&root, "manifests")?;
        let registry =
            open_bound_directory(&manifests.directory, selection.reference().registry())?;
        let namespace =
            open_bound_directory(&registry.directory, selection.reference().namespace())?;
        let model = open_bound_directory(&namespace.directory, selection.reference().model())?;
        let blobs = open_bound_directory(&root, "blobs")?;
        let mut manifest = open_bound_file(&model.directory, selection.reference().tag())?;
        let raw_manifest = read_manifest(&mut manifest, limits.manifest_bytes, cancellation)?;
        let plan = parse_manifest_v2(&raw_manifest, limits)?;
        let package_blobs = open_blobs(&blobs.directory, &plan)?;
        let source_locator = selection.reference().source_locator();
        let reconstructed = reconstruct_from_blobs(
            &package_blobs,
            &raw_manifest,
            &source_locator,
            limits,
            cancellation,
        )?;
        let source = Self {
            root_path: selection.models_root().to_path_buf(),
            root,
            root_baseline,
            manifests,
            registry,
            namespace,
            model,
            blobs,
            manifest,
            package_blobs,
            reconstructed,
        };
        source.recheck()?;
        Ok(source)
    }

    pub(crate) const fn reconstructed(&self) -> &ReconstructedModelPackage {
        &self.reconstructed
    }

    pub(crate) fn copy_into_staging(
        &mut self,
        staging: &OwnedStagingTree,
        manifest: &ArtifactSetManifest,
        cancellation: &CancellationToken,
    ) -> Result<(), OllamaModelImportError> {
        self.recheck()?;
        for member in manifest.members() {
            ensure_not_cancelled(cancellation)?;
            let path = member.relative_path().as_str();
            let source = self.source_file_mut(path)?;
            copy_exact_member(
                source,
                staging,
                member.relative_path(),
                member.byte_size(),
                member.artifact_id().digest(),
                cancellation,
            )?;
        }
        self.recheck()
    }

    fn source_file_mut(
        &mut self,
        logical_path: &str,
    ) -> Result<&mut BoundFile, OllamaModelImportError> {
        let descriptor = match logical_path {
            CONFIG_PATH => self.reconstructed.plan().config(),
            MODEL_PATH => self.reconstructed.plan().model(),
            TEMPLATE_PATH => self.reconstructed.plan().template(),
            LICENSE_PATH => self.reconstructed.plan().license(),
            PARAMETERS_PATH => self.reconstructed.plan().parameters(),
            PROVENANCE_PATH => return Ok(&mut self.manifest),
            _ => return Err(OllamaModelImportError::SourceChanged),
        };
        self.package_blobs
            .iter_mut()
            .find(|blob| blob.descriptor.digest() == descriptor.digest())
            .map(|blob| &mut blob.source)
            .ok_or(OllamaModelImportError::SourceChanged)
    }

    pub(crate) fn recheck(&self) -> Result<(), OllamaModelImportError> {
        let held_root = self.root.fingerprint().map_err(map_source)?.stable();
        let named_root = PinnedDirectory::fingerprint_path(&self.root_path)
            .map_err(map_source)?
            .stable();
        if held_root != self.root_baseline || named_root != self.root_baseline {
            return Err(OllamaModelImportError::SourceChanged);
        }
        recheck_bound_directory(&self.root, &self.manifests)?;
        recheck_bound_directory(&self.manifests.directory, &self.registry)?;
        recheck_bound_directory(&self.registry.directory, &self.namespace)?;
        recheck_bound_directory(&self.namespace.directory, &self.model)?;
        recheck_bound_directory(&self.root, &self.blobs)?;
        recheck_bound_file(&self.model.directory, &self.manifest)?;
        for blob in &self.package_blobs {
            recheck_bound_file(&self.blobs.directory, &blob.source)?;
        }
        Ok(())
    }
}

fn reconstruct_from_blobs(
    blobs: &[BoundBlob],
    raw_manifest: &[u8],
    source_locator: &str,
    limits: &ReconstructionLimits,
    cancellation: &CancellationToken,
) -> Result<ReconstructedModelPackage, OllamaModelImportError> {
    reconstruct_model_package_with_limits::<File, _, _>(
        raw_manifest,
        source_locator,
        limits,
        |digest| clone_blob(blobs, digest),
        || cancellation.is_cancelled(),
    )
    .map_err(OllamaModelImportError::from)
}

fn clone_blob(blobs: &[BoundBlob], digest: &Digest) -> Result<File, BlobOpenError> {
    let blob = blobs
        .iter()
        .find(|blob| blob.descriptor.digest() == digest)
        .ok_or(BlobOpenError)?;
    let mut file = blob
        .source
        .opened
        .file
        .try_clone()
        .map_err(|_| BlobOpenError)?;
    file.seek(std::io::SeekFrom::Start(0))
        .map_err(|_| BlobOpenError)?;
    Ok(file)
}

fn open_bound_directory(
    parent: &PinnedDirectory,
    name: &str,
) -> Result<BoundDirectory, OllamaModelImportError> {
    let directory = parent
        .open_child_directory(OsStr::new(name))
        .map_err(map_source)?;
    let baseline = directory.fingerprint().map_err(map_source)?.stable();
    let named = parent
        .child_directory_fingerprint(OsStr::new(name))
        .map_err(map_source)?
        .stable();
    if baseline != named {
        return Err(OllamaModelImportError::SourceChanged);
    }
    Ok(BoundDirectory {
        name: name.to_owned(),
        directory,
        baseline,
    })
}

fn open_bound_file(
    parent: &PinnedDirectory,
    name: &str,
) -> Result<BoundFile, OllamaModelImportError> {
    let relative = ArtifactSetRelativePath::new(name.to_owned())
        .map_err(|_| OllamaModelImportError::InvalidReference)?;
    let opened = parent
        .open_relative_regular_file(&relative)
        .map_err(map_source)?;
    if !opened.fingerprint.has_single_link() {
        return Err(OllamaModelImportError::UnsafeSource);
    }
    Ok(BoundFile {
        name: name.to_owned(),
        opened,
    })
}

fn open_blobs(
    blobs: &PinnedDirectory,
    plan: &OllamaManifestPlan,
) -> Result<Vec<BoundBlob>, OllamaModelImportError> {
    [
        plan.config(),
        plan.model(),
        plan.template(),
        plan.license(),
        plan.parameters(),
    ]
    .into_iter()
    .map(|descriptor| {
        let name = format!("sha256-{}", descriptor.digest().as_str());
        let source = open_bound_file(blobs, &name)?;
        if source.opened.byte_size != descriptor.size() {
            return Err(OllamaModelImportError::Reconstruction(
                rewrite_ollama_package::ReconstructionError::BlobSizeMismatch,
            ));
        }
        Ok(BoundBlob {
            descriptor: descriptor.clone(),
            source,
        })
    })
    .collect()
}

fn read_manifest(
    manifest: &mut BoundFile,
    maximum: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, OllamaModelImportError> {
    let size = usize::try_from(manifest.opened.byte_size).map_err(|_| {
        OllamaModelImportError::Reconstruction(
            rewrite_ollama_package::ReconstructionError::ManifestTooLarge,
        )
    })?;
    if size > maximum {
        return Err(OllamaModelImportError::Reconstruction(
            rewrite_ollama_package::ReconstructionError::ManifestTooLarge,
        ));
    }
    let mut bytes = vec![0_u8; size];
    manifest
        .opened
        .file
        .seek(std::io::SeekFrom::Start(0))
        .map_err(OllamaModelImportError::SourceIo)?;
    for chunk in bytes.chunks_mut(64 * 1024) {
        ensure_not_cancelled(cancellation)?;
        manifest
            .opened
            .file
            .read_exact(chunk)
            .map_err(OllamaModelImportError::SourceIo)?;
    }
    let mut trailing = [0_u8; 1];
    if manifest
        .opened
        .file
        .read(&mut trailing)
        .map_err(OllamaModelImportError::SourceIo)?
        != 0
    {
        return Err(OllamaModelImportError::SourceChanged);
    }
    recheck_open_file(&manifest.opened)?;
    Ok(bytes)
}

fn copy_exact_member(
    source: &mut BoundFile,
    staging: &OwnedStagingTree,
    path: &ArtifactSetRelativePath,
    expected_size: u64,
    expected_digest: &Digest,
    cancellation: &CancellationToken,
) -> Result<(), OllamaModelImportError> {
    let mut destination = staging.create_file(path).map_err(map_staging)?;
    source
        .opened
        .file
        .seek(std::io::SeekFrom::Start(0))
        .map_err(OllamaModelImportError::SourceIo)?;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    let mut observed_size = 0_u64;
    let mut hasher = Sha256::new();
    while observed_size < expected_size {
        ensure_not_cancelled(cancellation)?;
        let amount = usize::try_from(expected_size - observed_size)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let count = source
            .opened
            .file
            .read(&mut buffer[..amount])
            .map_err(OllamaModelImportError::SourceIo)?;
        if count == 0 {
            return Err(OllamaModelImportError::Reconstruction(
                rewrite_ollama_package::ReconstructionError::BlobSizeMismatch,
            ));
        }
        destination
            .file
            .write_all(&buffer[..count])
            .map_err(|error| {
                OllamaModelImportError::ArtifactSet(crate::ArtifactSetImportError::StorageIo(error))
            })?;
        hasher.update(&buffer[..count]);
        observed_size = observed_size
            .checked_add(u64::try_from(count).map_err(|_| OllamaModelImportError::SourceChanged)?)
            .ok_or(OllamaModelImportError::SourceChanged)?;
    }
    let mut trailing = [0_u8; 1];
    if source
        .opened
        .file
        .read(&mut trailing)
        .map_err(OllamaModelImportError::SourceIo)?
        != 0
    {
        return Err(OllamaModelImportError::Reconstruction(
            rewrite_ollama_package::ReconstructionError::BlobSizeMismatch,
        ));
    }
    let observed_digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| OllamaModelImportError::SourceChanged)?;
    if observed_size != expected_size || &observed_digest != expected_digest {
        return Err(OllamaModelImportError::Reconstruction(
            rewrite_ollama_package::ReconstructionError::BlobDigestMismatch,
        ));
    }
    recheck_open_file(&source.opened)?;
    if destination
        .file
        .metadata()
        .map_err(|error| {
            OllamaModelImportError::ArtifactSet(crate::ArtifactSetImportError::StorageIo(error))
        })?
        .len()
        != expected_size
    {
        return Err(OllamaModelImportError::SourceChanged);
    }
    Ok(())
}

fn recheck_bound_directory(
    parent: &PinnedDirectory,
    bound: &BoundDirectory,
) -> Result<(), OllamaModelImportError> {
    let held = bound.directory.fingerprint().map_err(map_source)?.stable();
    let named = parent
        .child_directory_fingerprint(OsStr::new(&bound.name))
        .map_err(map_source)?
        .stable();
    if held == bound.baseline && named == bound.baseline {
        Ok(())
    } else {
        Err(OllamaModelImportError::SourceChanged)
    }
}

fn recheck_bound_file(
    parent: &PinnedDirectory,
    bound: &BoundFile,
) -> Result<(), OllamaModelImportError> {
    recheck_open_file(&bound.opened)?;
    let relative = ArtifactSetRelativePath::new(bound.name.clone())
        .map_err(|_| OllamaModelImportError::SourceChanged)?;
    parent
        .recheck_relative_regular_file(&relative, &bound.opened.fingerprint)
        .map_err(map_source)
}

fn recheck_open_file(opened: &ManagedFile) -> Result<(), OllamaModelImportError> {
    let current = fingerprint_std_file(&opened.file).map_err(map_source)?;
    if current == opened.fingerprint && current.has_single_link() {
        Ok(())
    } else {
        Err(OllamaModelImportError::SourceChanged)
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), OllamaModelImportError> {
    if cancellation.is_cancelled() {
        Err(OllamaModelImportError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_source(error: ArtifactInventoryError) -> OllamaModelImportError {
    match error {
        ArtifactInventoryError::StorageIo(error) => OllamaModelImportError::SourceIo(error),
        ArtifactInventoryError::ConcurrentModification => OllamaModelImportError::SourceChanged,
        ArtifactInventoryError::Cancelled => OllamaModelImportError::Cancelled,
        _ => OllamaModelImportError::UnsafeSource,
    }
}

fn map_staging(error: ArtifactInventoryError) -> OllamaModelImportError {
    OllamaModelImportError::ArtifactSet(crate::artifact_set_import::map_managed_tree(error))
}
