//! Application-owned live attestation for one managed runtime process.

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use rewrite_model::{
    EffectiveRuntimeState, EffectiveRuntimeStateInput, RuntimeBuildIdentity,
    RuntimeBuildIdentityInput, RuntimeBuildMode,
};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::artifact_storage::is_indirect;

mod contract;

pub use contract::{
    ManagedRuntimeAttestationRequest, ManagedRuntimeIdentityFacts, ManagedRuntimeStateFacts,
    RuntimeAttestationError, RuntimeAttestationLimits, RuntimeAttestationPersistence,
    RuntimeAttestationResult, host_runtime_target,
};
pub use rewrite_model_store::WriteDisposition;

const HASH_BUFFER_BYTES: usize = 1024 * 1024;
/// Service that attests one managed entrypoint without granting a role.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeAttestationService;

impl RuntimeAttestationService {
    /// Hashes a live managed entrypoint and builds inert runtime evidence.
    ///
    /// The resulting records do not activate an artifact, authorize a role, or
    /// qualify claim extraction. Persistence is optional and fail-closed.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAttestationError`] when limits, the entrypoint, identity
    /// facts, state facts, cancellation, or persistence fail closed.
    pub fn attest_managed(
        request: &ManagedRuntimeAttestationRequest,
        limits: RuntimeAttestationLimits,
        store: Option<&mut ArtifactStateStore>,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeAttestationResult, RuntimeAttestationError> {
        ensure_not_cancelled(cancellation)?;
        if limits.maximum_entrypoint_bytes == 0 {
            return Err(RuntimeAttestationError::InvalidLimits);
        }
        let first = hash_entrypoint(&request.entrypoint, limits, cancellation)?;
        if let Some(expected) = &request.expected_entrypoint_digest
            && expected != &first.digest
        {
            return Err(RuntimeAttestationError::DigestMismatch);
        }
        let confirm = hash_entrypoint(&request.entrypoint, limits, cancellation)?;
        if confirm.digest != first.digest || confirm.byte_size != first.byte_size {
            return Err(RuntimeAttestationError::EntrypointChanged);
        }
        let build = RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
            mode: RuntimeBuildMode::ManagedProcess,
            runtime_family: request.identity.runtime_family.clone(),
            reported_version: request.identity.reported_version.clone(),
            build_revision: request.identity.build_revision.clone(),
            target: request.identity.target,
            package_manifest_digest: request.identity.package_manifest_digest.clone(),
            entrypoint_digest: first.digest.clone(),
            packaged_dependencies_digest: request.identity.packaged_dependencies_digest.clone(),
            build_configuration_digest: request.identity.build_configuration_digest.clone(),
        })
        .map_err(RuntimeAttestationError::InvalidIdentity)?;
        let state = EffectiveRuntimeState::new(
            &build,
            EffectiveRuntimeStateInput {
                provider_snapshot_contract: request.state.provider_snapshot_contract.clone(),
                provider_snapshot_schema_version: request.state.provider_snapshot_schema_version,
                provider_snapshot_digest: request.state.provider_snapshot_digest.clone(),
                launch_policy_digest: request.state.launch_policy_digest.clone(),
                loaded_components_digest: request.state.loaded_components_digest.clone(),
                effective_configuration_digest: request
                    .state
                    .effective_configuration_digest
                    .clone(),
                platform_digest: request.state.platform_digest.clone(),
                execution_class_digest: request.state.execution_class_digest.clone(),
                isolation_policy_digest: request.state.isolation_policy_digest.clone(),
                effective_context_tokens: request.state.effective_context_tokens,
                compute_backend: request.state.compute_backend,
                placement: request.state.placement,
            },
        )
        .map_err(RuntimeAttestationError::InvalidState)?;
        let persistence = persist_records(store, &build, &state)?;
        Ok(RuntimeAttestationResult {
            build,
            state,
            entrypoint_bytes: first.byte_size,
            persistence,
        })
    }
}

struct EntrypointHash {
    digest: Digest,
    byte_size: u64,
}

fn hash_entrypoint(
    path: &Path,
    limits: RuntimeAttestationLimits,
    cancellation: &CancellationToken,
) -> Result<EntrypointHash, RuntimeAttestationError> {
    ensure_not_cancelled(cancellation)?;
    let listed = fs::symlink_metadata(path).map_err(RuntimeAttestationError::EntrypointIo)?;
    if is_indirect(&listed) {
        return Err(RuntimeAttestationError::IndirectEntrypoint);
    }
    if !listed.is_file() {
        return Err(RuntimeAttestationError::EntrypointNotFile);
    }
    if listed.len() > limits.maximum_entrypoint_bytes {
        return Err(RuntimeAttestationError::EntrypointTooLarge {
            actual: listed.len(),
            maximum: limits.maximum_entrypoint_bytes,
        });
    }
    let mut file = File::open(path).map_err(RuntimeAttestationError::EntrypointIo)?;
    let opened = file
        .metadata()
        .map_err(RuntimeAttestationError::EntrypointIo)?;
    if is_indirect(&opened) || !opened.is_file() {
        return Err(RuntimeAttestationError::EntrypointNotFile);
    }
    if opened.len() > limits.maximum_entrypoint_bytes {
        return Err(RuntimeAttestationError::EntrypointTooLarge {
            actual: opened.len(),
            maximum: limits.maximum_entrypoint_bytes,
        });
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        ensure_not_cancelled(cancellation)?;
        let read = file
            .read(&mut buffer)
            .map_err(RuntimeAttestationError::EntrypointIo)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or(RuntimeAttestationError::EntrypointTooLarge {
                actual: u64::MAX,
                maximum: limits.maximum_entrypoint_bytes,
            })?;
        if total > limits.maximum_entrypoint_bytes {
            return Err(RuntimeAttestationError::EntrypointTooLarge {
                actual: total,
                maximum: limits.maximum_entrypoint_bytes,
            });
        }
        hasher.update(&buffer[..read]);
    }
    if total != opened.len() {
        return Err(RuntimeAttestationError::EntrypointChanged);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeAttestationError::EntrypointChanged)?;
    Ok(EntrypointHash {
        digest,
        byte_size: total,
    })
}

fn persist_records(
    store: Option<&mut ArtifactStateStore>,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
) -> Result<RuntimeAttestationPersistence, RuntimeAttestationError> {
    let Some(store) = store else {
        return Ok(RuntimeAttestationPersistence::NotRequested);
    };
    let build_disposition = store
        .put_runtime_build_identity(build)
        .map_err(RuntimeAttestationError::Persistence)?;
    let state_disposition = store
        .put_effective_runtime_state(state)
        .map_err(RuntimeAttestationError::Persistence)?;
    let stored_build = store
        .runtime_build_identity(&build.runtime_build_id())
        .map_err(RuntimeAttestationError::Persistence)?
        .ok_or(RuntimeAttestationError::PersistenceMismatch)?;
    let stored_state = store
        .effective_runtime_state(&state.effective_runtime_state_id())
        .map_err(RuntimeAttestationError::Persistence)?
        .ok_or(RuntimeAttestationError::PersistenceMismatch)?;
    if stored_build != *build || stored_state != *state {
        return Err(RuntimeAttestationError::PersistenceMismatch);
    }
    Ok(RuntimeAttestationPersistence::Stored {
        build: build_disposition,
        state: state_disposition,
    })
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), RuntimeAttestationError> {
    if cancellation.is_cancelled() {
        Err(RuntimeAttestationError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
