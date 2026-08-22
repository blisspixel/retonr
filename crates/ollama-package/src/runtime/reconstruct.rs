use std::io::Read;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, RuntimePackageManifest,
    RuntimePackageMember,
};
use sha2::{Digest as _, Sha256};

use super::error::{MemberOpenError, RuntimeReconstructionError, RuntimeReconstructionResult};
use super::layout::{RuntimeLayoutLimits, RuntimePackageLayout, RuntimePackageLayoutMember};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Canonical runtime package reconstructed from a reviewed layout and exact bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconstructedRuntimePackage {
    layout: RuntimePackageLayout,
    artifact_set: ArtifactSetManifest,
    runtime_package: RuntimePackageManifest,
}

impl ReconstructedRuntimePackage {
    /// Returns the reviewed layout used for reconstruction.
    #[must_use]
    pub const fn layout(&self) -> &RuntimePackageLayout {
        &self.layout
    }

    /// Returns the exact canonical artifact set.
    #[must_use]
    pub const fn artifact_set(&self) -> &ArtifactSetManifest {
        &self.artifact_set
    }

    /// Returns the exact semantic runtime package.
    #[must_use]
    pub const fn runtime_package(&self) -> &RuntimePackageManifest {
        &self.runtime_package
    }
}

/// Reconstructs one admitted Ollama Linux runtime package from a reviewed layout.
///
/// The opener receives only validated relative paths. Each returned stream is
/// consumed once, checked for exact size and digest, and then dropped. This
/// function does not execute members or grant runtime authority.
///
/// # Errors
///
/// Returns [`RuntimeReconstructionError`] for any malformed, missing, changed,
/// unsupported, over-budget, or cancelled input.
pub fn reconstruct_runtime_package<R, F, C>(
    layout_bytes: &[u8],
    open_member: F,
    cancelled: C,
) -> RuntimeReconstructionResult<ReconstructedRuntimePackage>
where
    R: Read,
    F: FnMut(&rewrite_model::ArtifactSetRelativePath) -> Result<R, MemberOpenError>,
    C: FnMut() -> bool,
{
    reconstruct_runtime_package_with_limits(
        layout_bytes,
        &RuntimeLayoutLimits::default(),
        open_member,
        cancelled,
    )
}

/// Reconstructs one admitted runtime package using explicit testable limits.
///
/// Explicit limits can only lower the hard defaults.
///
/// # Errors
///
/// Returns [`RuntimeReconstructionError`] for any malformed, missing, changed,
/// unsupported, over-budget, or cancelled input.
pub fn reconstruct_runtime_package_with_limits<R, F, C>(
    layout_bytes: &[u8],
    limits: &RuntimeLayoutLimits,
    mut open_member: F,
    mut cancelled: C,
) -> RuntimeReconstructionResult<ReconstructedRuntimePackage>
where
    R: Read,
    F: FnMut(&rewrite_model::ArtifactSetRelativePath) -> Result<R, MemberOpenError>,
    C: FnMut() -> bool,
{
    if cancelled() {
        return Err(RuntimeReconstructionError::Cancelled);
    }
    let layout = RuntimePackageLayout::parse(layout_bytes, *limits)?;
    let mut set_members = Vec::with_capacity(layout.members().len());
    let mut package_members = Vec::with_capacity(layout.members().len());
    for member in layout.members() {
        if cancelled() {
            return Err(RuntimeReconstructionError::Cancelled);
        }
        let stream = open_member(member.relative_path())
            .map_err(|_| RuntimeReconstructionError::MemberUnavailable)?;
        verify_member(member, stream, &mut cancelled)?;
        let artifact_id = ArtifactId::from_digest(member.digest().clone());
        set_members.push(ArtifactSetMember::new(
            artifact_id.clone(),
            member.byte_size(),
            member.relative_path().clone(),
        ));
        package_members.push(RuntimePackageMember::new(
            artifact_id,
            member.byte_size(),
            member.relative_path().clone(),
            member.roles().to_vec(),
            member.load_policy(),
        ));
    }
    let artifact_set = ArtifactSetManifest::new(set_members)
        .map_err(|_| RuntimeReconstructionError::RuntimeContract)?;
    let runtime_package = RuntimePackageManifest::new(
        &artifact_set,
        layout.runtime_family(),
        layout.reported_version(),
        Some(layout.build_revision().to_owned()),
        layout.target(),
        layout.source().clone(),
        layout.transformation().clone(),
        package_members,
    )
    .map_err(|_| RuntimeReconstructionError::RuntimeContract)?;
    Ok(ReconstructedRuntimePackage {
        layout,
        artifact_set,
        runtime_package,
    })
}

fn verify_member<R, C>(
    member: &RuntimePackageLayoutMember,
    mut stream: R,
    cancelled: &mut C,
) -> RuntimeReconstructionResult<()>
where
    R: Read,
    C: FnMut() -> bool,
{
    let mut hasher = Sha256::new();
    let mut remaining = member.byte_size();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(RuntimeReconstructionError::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RuntimeReconstructionError::InputRead)?;
        if read == 0 {
            break;
        }
        let read_bytes = u64::try_from(read).map_err(|_| RuntimeReconstructionError::InputRead)?;
        if read_bytes > remaining {
            return Err(RuntimeReconstructionError::MemberSizeMismatch);
        }
        hasher.update(&buffer[..read]);
        remaining -= read_bytes;
    }
    if remaining != 0 {
        return Err(RuntimeReconstructionError::MemberSizeMismatch);
    }
    let digest = rewrite_types::Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeReconstructionError::InputRead)?;
    if &digest != member.digest() {
        return Err(RuntimeReconstructionError::MemberDigestMismatch);
    }
    Ok(())
}
