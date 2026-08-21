use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::Instant,
};

use rewrite_model::{
    ArtifactId, NativeLoadEvidenceClass, NativeLoadObservation, NativeLoadObservationInput,
    NativeLoadOrigin, NativeLoadVisibilityScope, NativeLoadedComponent, RuntimePackageLoadPolicy,
    RuntimePackageManifest,
};
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use crate::{
    ExpectedExternalNativeComponent, NativeLoadObservationLimits, NativeLoadObserverError,
    ensure_native_active, native_load::expected_key,
};

const HASH_BUFFER_BYTES: usize = 1024 * 1024;

pub(super) struct HashBudget {
    remaining: u64,
}

impl HashBudget {
    pub(super) const fn new(limit: u64) -> Self {
        Self { remaining: limit }
    }

    pub(super) fn reserve(&mut self, bytes: u64) -> Result<(), NativeLoadObserverError> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(NativeLoadObserverError::ResourceLimit)?;
        Ok(())
    }
}

pub(super) fn hash_file(
    file: &mut File,
    expected_bytes: u64,
    budget: &mut HashBudget,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
) -> Result<ArtifactId, NativeLoadObserverError> {
    if expected_bytes == 0 {
        return Err(NativeLoadObserverError::InvalidMappedObject);
    }
    budget.reserve(expected_bytes)?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_error| NativeLoadObserverError::MappedObjectUnavailable)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        ensure_native_active(cancellation, started, limits)?;
        let read = file
            .read(&mut buffer)
            .map_err(|_error| NativeLoadObserverError::MappedObjectUnavailable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(
                u64::try_from(read).map_err(|_error| NativeLoadObserverError::ResourceLimit)?,
            )
            .ok_or(NativeLoadObserverError::ResourceLimit)?;
        if total > expected_bytes {
            return Err(NativeLoadObserverError::ObservationChanged);
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_bytes {
        return Err(NativeLoadObserverError::ObservationChanged);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| NativeLoadObserverError::PlatformObservationFailed)?;
    Ok(ArtifactId::from_digest(digest))
}

pub(super) fn finish_observation(
    package: &RuntimePackageManifest,
    expected_external: &[ExpectedExternalNativeComponent],
    evidence_class: NativeLoadEvidenceClass,
    contract_id: &str,
    process_evidence_digest: &Digest,
    mut components: Vec<NativeLoadedComponent>,
) -> Result<NativeLoadObservation, NativeLoadObserverError> {
    components.sort_by_key(component_key);
    if components.iter().any(|component| {
        package.members().iter().any(|member| {
            member.load_policy() == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
                && member.artifact_id() == component.artifact_id()
        })
    }) {
        return Err(NativeLoadObserverError::ComponentPolicyMismatch);
    }
    validate_external_set(&components, expected_external)?;
    NativeLoadObservation::new(
        package,
        NativeLoadObservationInput {
            evidence_class,
            visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
            process_evidence_digest: process_evidence_digest.clone(),
            observation_contract_id: contract_id.to_owned(),
            observation_contract_schema_version: 1,
            components,
        },
    )
    .map_err(|_error| NativeLoadObserverError::InvalidObservation)
}

fn validate_external_set(
    components: &[NativeLoadedComponent],
    expected: &[ExpectedExternalNativeComponent],
) -> Result<(), NativeLoadObserverError> {
    let observed = components
        .iter()
        .filter(|component| {
            matches!(
                component.origin(),
                NativeLoadOrigin::ExternalPlatformComponent
            )
        })
        .map(|component| {
            let expected = ExpectedExternalNativeComponent::new(
                component.artifact_id().clone(),
                component.byte_size(),
                component.mapping_class(),
            );
            expected_key(&expected)
        })
        .collect::<Vec<_>>();
    let frozen = expected.iter().map(expected_key).collect::<Vec<_>>();
    if observed != frozen {
        return Err(NativeLoadObserverError::ComponentPolicyMismatch);
    }
    Ok(())
}

fn component_key(component: &NativeLoadedComponent) -> Vec<u8> {
    let mut key = Vec::with_capacity(256);
    match component.origin() {
        NativeLoadOrigin::PackagedMember { relative_path } => {
            key.push(0);
            key.extend_from_slice(relative_path.as_str().as_bytes());
            key.push(0);
        }
        NativeLoadOrigin::ExternalPlatformComponent => key.push(1),
    }
    key.extend_from_slice(component.artifact_id().digest().as_str().as_bytes());
    key.extend_from_slice(&component.byte_size().to_be_bytes());
    key.push(match component.mapping_class() {
        rewrite_model::NativeMappingClass::ExecutableImage => 0,
        rewrite_model::NativeMappingClass::ExecutableMapped => 1,
        rewrite_model::NativeMappingClass::DataMapped => 2,
    });
    key.extend_from_slice(component.object_evidence_digest().as_str().as_bytes());
    key
}

#[cfg(test)]
#[path = "native_load_common/tests.rs"]
mod tests;
