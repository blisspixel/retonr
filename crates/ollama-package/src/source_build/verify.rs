use std::{collections::BTreeMap, io::Read};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::program_lineage::{declares_lineage, retained_bytes_limit, verify_lineage};
use super::{
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputError, RuntimeSourceBuildInputLimits,
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
    SelfContainedLinuxExecutable, VerifiedRuntimeSourceBuildInputs,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Verifies every content-addressed component in a controlled-build input manifest.
///
/// The caller-supplied opener receives validated portable paths and retains authority
/// over where the frozen input bundle is stored. This function performs no path,
/// filesystem, or network discovery.
///
/// # Errors
///
/// Returns [`RuntimeSourceBuildInputError`] for malformed declarations and every
/// missing, changed, unreadable, oversized, or cancelled component stream.
pub fn verify_runtime_source_build_inputs<R, F, C>(
    manifest_bytes: &[u8],
    limits: RuntimeSourceBuildInputLimits,
    mut open_component: F,
    mut cancelled: C,
) -> Result<VerifiedRuntimeSourceBuildInputs, RuntimeSourceBuildInputError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    let manifest = RuntimeSourceBuildInputManifest::parse(manifest_bytes, limits)?;
    let lineage_declared = declares_lineage(&manifest);
    let mut retained = BTreeMap::new();
    for component in manifest.components() {
        if cancelled() {
            return Err(RuntimeSourceBuildInputError::Cancelled);
        }
        let stream = open_component(component.relative_path())
            .map_err(|_| RuntimeSourceBuildInputError::ComponentUnavailable)?;
        let retained_role = lineage_declared
            .then(|| {
                component
                    .roles()
                    .iter()
                    .copied()
                    .find(|role| retained_bytes_limit(*role).is_some())
            })
            .flatten();
        let mut retained_bytes = retained_role.map(|_| Vec::new());
        if let Some(role) = retained_role {
            let maximum = retained_bytes_limit(role)
                .ok_or(RuntimeSourceBuildInputError::InvalidProgramLineage)?;
            if component.byte_size() > maximum {
                return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
            }
        }
        verify_component(component, stream, &mut cancelled, retained_bytes.as_mut())?;
        if let (Some(role), Some(bytes)) = (retained_role, retained_bytes)
            && retained.insert(role, bytes).is_some()
        {
            return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
        }
    }
    let program_lineage = lineage_declared
        .then(|| verify_lineage(&manifest, &retained))
        .transpose()?;
    Ok(VerifiedRuntimeSourceBuildInputs {
        manifest,
        program_lineage,
    })
}

fn verify_component<R, C>(
    component: &RuntimeSourceBuildInputComponent,
    mut stream: R,
    cancelled: &mut C,
    mut retained: Option<&mut Vec<u8>>,
) -> Result<(), RuntimeSourceBuildInputError>
where
    R: Read,
    C: FnMut() -> bool,
{
    let mut hasher = Sha256::new();
    let mut remaining = component.byte_size();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let inspect_executable = component.roles().iter().any(|role| {
        matches!(
            role,
            RuntimeSourceBuildInputRole::BuildScript
                | RuntimeSourceBuildInputRole::IsolationHelper
                | RuntimeSourceBuildInputRole::SourceArchivePreparationTool
                | RuntimeSourceBuildInputRole::SourceManifestPreparationTool
        )
    });
    if inspect_executable
        && component.byte_size()
            > u64::try_from(SelfContainedLinuxExecutable::MAXIMUM_BYTES).unwrap_or(u64::MAX)
    {
        return Err(RuntimeSourceBuildInputError::NonSelfContainedExecutable);
    }
    let executable_capacity = inspect_executable
        .then(|| usize::try_from(component.byte_size()))
        .transpose()
        .map_err(|_| RuntimeSourceBuildInputError::NonSelfContainedExecutable)?;
    let mut executable_bytes = executable_capacity.map(Vec::with_capacity);
    loop {
        if cancelled() {
            return Err(RuntimeSourceBuildInputError::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RuntimeSourceBuildInputError::ComponentRead)?;
        if read == 0 {
            break;
        }
        let read_bytes =
            u64::try_from(read).map_err(|_| RuntimeSourceBuildInputError::ComponentRead)?;
        if read_bytes > remaining {
            return Err(RuntimeSourceBuildInputError::ComponentSizeMismatch);
        }
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
        if let Some(bytes) = &mut executable_bytes {
            bytes.extend_from_slice(&buffer[..read]);
        }
        remaining -= read_bytes;
    }
    if remaining != 0 {
        return Err(RuntimeSourceBuildInputError::ComponentSizeMismatch);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeSourceBuildInputError::ComponentRead)?;
    if &digest != component.digest() {
        return Err(RuntimeSourceBuildInputError::ComponentDigestMismatch);
    }
    if let Some(bytes) = executable_bytes {
        SelfContainedLinuxExecutable::verify(&bytes)?;
    }
    Ok(())
}
