use std::{fs::File, io};

use rewrite_model::{ArtifactSetManifest, ModelPackageManifest};
use rewrite_ollama_package::{
    BlobOpenError, OllamaLocalArchiveFoundationEvidence, ReconstructionLimits,
    verify_ollama_local_archive_foundation, verify_ollama_local_archive_foundation_bindings,
};
use rewrite_types::{CancellationToken, Digest};

use super::ManagedOllamaModelPackageError;
use crate::package_attestation::RetainedModelMember;

const PROVENANCE_PATH: &str = "provenance/ollama-manifest-v2.json";

pub(super) fn verify_full(
    retained: &[RetainedModelMember],
    artifact_set: &ArtifactSetManifest,
    package: &ModelPackageManifest,
    limits: &ReconstructionLimits,
    cancellation: &CancellationToken,
) -> Result<OllamaLocalArchiveFoundationEvidence, ManagedOllamaModelPackageError> {
    let raw_manifest = read_raw_manifest(retained, limits, cancellation)?;
    let mut opened = vec![false; retained.len()];
    let evidence = verify_ollama_local_archive_foundation::<PositionedReader, _, _>(
        &raw_manifest,
        artifact_set,
        package,
        limits,
        |digest| open_blob_once(retained, &mut opened, digest),
        || cancellation.is_cancelled(),
    )
    .map_err(ManagedOllamaModelPackageError::Foundation)?;
    let all_descriptor_members_opened = retained
        .iter()
        .enumerate()
        .all(|(index, member)| member.relative_path.as_str() == PROVENANCE_PATH || opened[index]);
    if all_descriptor_members_opened {
        Ok(evidence)
    } else {
        Err(ManagedOllamaModelPackageError::FoundationBindingChanged)
    }
}

pub(super) fn verify_lightweight(
    retained: &[RetainedModelMember],
    artifact_set: &ArtifactSetManifest,
    package: &ModelPackageManifest,
    limits: &ReconstructionLimits,
    cancellation: &CancellationToken,
) -> Result<OllamaLocalArchiveFoundationEvidence, ManagedOllamaModelPackageError> {
    let raw_manifest = read_raw_manifest(retained, limits, cancellation)?;
    verify_ollama_local_archive_foundation_bindings(&raw_manifest, artifact_set, package, limits)
        .map_err(ManagedOllamaModelPackageError::Foundation)
}

fn read_raw_manifest(
    retained: &[RetainedModelMember],
    limits: &ReconstructionLimits,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, ManagedOllamaModelPackageError> {
    let member = retained
        .iter()
        .find(|member| member.relative_path.as_str() == PROVENANCE_PATH)
        .ok_or(ManagedOllamaModelPackageError::FoundationBindingChanged)?;
    let size = usize::try_from(member.byte_size)
        .ok()
        .filter(|size| *size <= limits.manifest_bytes)
        .ok_or(ManagedOllamaModelPackageError::FoundationBindingChanged)?;
    let mut bytes = vec![0_u8; size];
    let mut offset = 0_usize;
    while offset < size {
        ensure_not_cancelled(cancellation)?;
        let read = read_positioned(
            &member.file,
            &mut bytes[offset..],
            u64::try_from(offset)
                .map_err(|_error| ManagedOllamaModelPackageError::FoundationBindingChanged)?,
        )
        .map_err(package_io)?;
        if read == 0 {
            return Err(ManagedOllamaModelPackageError::FoundationBindingChanged);
        }
        offset = offset
            .checked_add(read)
            .ok_or(ManagedOllamaModelPackageError::FoundationBindingChanged)?;
    }
    ensure_not_cancelled(cancellation)?;
    let mut trailing = [0_u8; 1];
    if read_positioned(&member.file, &mut trailing, member.byte_size).map_err(package_io)? != 0 {
        return Err(ManagedOllamaModelPackageError::FoundationBindingChanged);
    }
    Ok(bytes)
}

fn open_blob_once(
    retained: &[RetainedModelMember],
    opened: &mut [bool],
    digest: &Digest,
) -> Result<PositionedReader, BlobOpenError> {
    retained
        .iter()
        .enumerate()
        .find(|(index, member)| {
            !opened[*index]
                && member.relative_path.as_str() != PROVENANCE_PATH
                && member.artifact_id.digest() == digest
        })
        .and_then(|(index, member)| {
            let file = member.file.try_clone().ok()?;
            opened[index] = true;
            Some(file)
        })
        .map(PositionedReader::new)
        .ok_or(BlobOpenError)
}

fn ensure_not_cancelled(
    cancellation: &CancellationToken,
) -> Result<(), ManagedOllamaModelPackageError> {
    if cancellation.is_cancelled() {
        Err(ManagedOllamaModelPackageError::Package(
            crate::PackageAttestationError::Cancelled,
        ))
    } else {
        Ok(())
    }
}

fn package_io(error: io::Error) -> ManagedOllamaModelPackageError {
    ManagedOllamaModelPackageError::Package(crate::PackageAttestationError::MemberIo(error))
}

struct PositionedReader {
    file: File,
    offset: u64,
}

impl PositionedReader {
    const fn new(file: File) -> Self {
        Self { file, offset: 0 }
    }
}

impl io::Read for PositionedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = read_positioned(&self.file, buffer, self.offset)?;
        self.offset = self
            .offset
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::other("retained reader offset overflow"))?;
        Ok(read)
    }
}

#[cfg(unix)]
fn read_positioned(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::unix::fs::FileExt as _;

    file.read_at(buffer, offset)
}

#[cfg(windows)]
fn read_positioned(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<usize> {
    use std::os::windows::fs::FileExt as _;

    file.seek_read(buffer, offset)
}
