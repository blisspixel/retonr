#[cfg(target_os = "linux")]
use std::{fs::File, io::Read, path::Path};

use rewrite_model::RuntimeBuildIdentity;
use rewrite_runtime_attestor::ManagedGenerationWorkerNativeLoadEvidence;
use rewrite_types::CancellationToken;
#[cfg(any(target_os = "linux", test))]
use rewrite_types::Digest;

#[cfg(any(target_os = "linux", test))]
use super::contract::{
    EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION, PlatformDriverEvidenceClass,
};
use super::contract::{EffectiveRuntimeStateObservationError, LinuxPlatformFrameworkEvidence};
#[cfg(target_os = "linux")]
use super::validation::valid_runtime_profile;
#[cfg(any(target_os = "linux", test))]
use super::validation::{domain_digest, push_bytes};

#[cfg(any(target_os = "linux", test))]
const MAX_KERNEL_FIELD_BYTES: usize = 4_096;
#[cfg(any(target_os = "linux", test))]
const KERNEL_FIELDS: [&str; 3] = [
    "/proc/sys/kernel/ostype",
    "/proc/sys/kernel/osrelease",
    "/proc/sys/kernel/version",
];

#[cfg(not(target_os = "linux"))]
pub(super) fn observe(
    _runtime_build: &RuntimeBuildIdentity,
    _worker_native_load: &ManagedGenerationWorkerNativeLoadEvidence,
    _cancellation: &CancellationToken,
) -> Result<LinuxPlatformFrameworkEvidence, EffectiveRuntimeStateObservationError> {
    Err(EffectiveRuntimeStateObservationError::UnsupportedProfile)
}

#[cfg(target_os = "linux")]
pub(super) fn observe(
    runtime_build: &RuntimeBuildIdentity,
    worker_native_load: &ManagedGenerationWorkerNativeLoadEvidence,
    cancellation: &CancellationToken,
) -> Result<LinuxPlatformFrameworkEvidence, EffectiveRuntimeStateObservationError> {
    if !valid_runtime_profile(runtime_build) {
        return Err(EffectiveRuntimeStateObservationError::UnsupportedProfile);
    }
    if worker_native_load.component_count() == 0 {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    let first = observe_kernel(cancellation)?;
    let second = observe_kernel(cancellation)?;
    if first != second {
        return Err(EffectiveRuntimeStateObservationError::PlatformObservationChanged);
    }
    build_platform(PlatformRecordInput {
        runtime_build_id: runtime_build.runtime_build_id(),
        target: runtime_build.target(),
        kernel_fields: first,
        worker_portable_closure_digest: worker_native_load.portable_closure_digest().clone(),
        worker_native_load_digest: worker_native_load.observation_digest().clone(),
        worker_native_component_count: worker_native_load.component_count(),
    })
}

#[cfg(any(target_os = "linux", test))]
struct PlatformRecordInput {
    runtime_build_id: rewrite_model::RuntimeBuildId,
    target: rewrite_model::RuntimeTarget,
    kernel_fields: Vec<Vec<u8>>,
    worker_portable_closure_digest: Digest,
    worker_native_load_digest: Digest,
    worker_native_component_count: u32,
}

#[cfg(any(target_os = "linux", test))]
fn build_platform(
    input: PlatformRecordInput,
) -> Result<LinuxPlatformFrameworkEvidence, EffectiveRuntimeStateObservationError> {
    if input.worker_native_component_count == 0 || !valid_kernel_fields(&input.kernel_fields) {
        return Err(EffectiveRuntimeStateObservationError::InvalidPlatformObservation);
    }
    let mut kernel_material = Vec::new();
    push_bytes(&mut kernel_material, b"linux/kernel-identity/v1");
    for field in &input.kernel_fields {
        push_bytes(&mut kernel_material, field);
    }
    let kernel_observation_digest = Digest::sha256(&kernel_material);
    let platform_digest = domain_digest(
        b"ollama/v0.32.15/linux-platform-framework/v1",
        &[
            input.runtime_build_id.digest(),
            &input.worker_portable_closure_digest,
        ],
        &[
            operating_system_code(input.target.operating_system()),
            architecture_code(input.target.architecture()),
            abi_code(input.target.abi()),
            u64::from(PlatformDriverEvidenceClass::NotApplicableForReviewedNativeCpuProfile as u8),
        ],
    );
    Ok(LinuxPlatformFrameworkEvidence {
        schema_version: EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION,
        runtime_build_id: input.runtime_build_id,
        target: input.target,
        kernel_observation_digest,
        worker_portable_closure_digest: input.worker_portable_closure_digest,
        worker_native_load_digest: input.worker_native_load_digest,
        driver_evidence: PlatformDriverEvidenceClass::NotApplicableForReviewedNativeCpuProfile,
        platform_digest,
    })
}

#[cfg(target_os = "linux")]
fn observe_kernel(
    cancellation: &CancellationToken,
) -> Result<Vec<Vec<u8>>, EffectiveRuntimeStateObservationError> {
    KERNEL_FIELDS
        .iter()
        .map(|path| read_kernel_field(Path::new(path), cancellation))
        .collect()
}

#[cfg(target_os = "linux")]
fn read_kernel_field(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, EffectiveRuntimeStateObservationError> {
    if cancellation.is_cancelled() {
        return Err(EffectiveRuntimeStateObservationError::Cancelled);
    }
    let file = File::open(path).map_err(EffectiveRuntimeStateObservationError::PlatformIo)?;
    let mut bytes = Vec::with_capacity(MAX_KERNEL_FIELD_BYTES + 1);
    file.take((MAX_KERNEL_FIELD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(EffectiveRuntimeStateObservationError::PlatformIo)?;
    if cancellation.is_cancelled() {
        return Err(EffectiveRuntimeStateObservationError::Cancelled);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if !valid_kernel_field(&bytes) {
        return Err(EffectiveRuntimeStateObservationError::InvalidPlatformObservation);
    }
    Ok(bytes)
}

#[cfg(any(target_os = "linux", test))]
fn valid_kernel_fields(fields: &[Vec<u8>]) -> bool {
    fields.len() == KERNEL_FIELDS.len() && fields.iter().all(|field| valid_kernel_field(field))
}

#[cfg(any(target_os = "linux", test))]
fn valid_kernel_field(field: &[u8]) -> bool {
    !field.is_empty()
        && field.len() <= MAX_KERNEL_FIELD_BYTES
        && field
            .iter()
            .all(|byte| byte.is_ascii() && !byte.is_ascii_control())
}

#[cfg(any(target_os = "linux", test))]
const fn operating_system_code(value: rewrite_model::RuntimeOperatingSystem) -> u64 {
    match value {
        rewrite_model::RuntimeOperatingSystem::Windows => 0,
        rewrite_model::RuntimeOperatingSystem::MacOs => 1,
        rewrite_model::RuntimeOperatingSystem::Linux => 2,
    }
}

#[cfg(any(target_os = "linux", test))]
const fn architecture_code(value: rewrite_model::RuntimeArchitecture) -> u64 {
    match value {
        rewrite_model::RuntimeArchitecture::X86_64 => 0,
        rewrite_model::RuntimeArchitecture::Aarch64 => 1,
    }
}

#[cfg(any(target_os = "linux", test))]
const fn abi_code(value: rewrite_model::RuntimeAbi) -> u64 {
    match value {
        rewrite_model::RuntimeAbi::WindowsMsvc => 0,
        rewrite_model::RuntimeAbi::WindowsGnu => 1,
        rewrite_model::RuntimeAbi::LinuxGnuLibc => 2,
        rewrite_model::RuntimeAbi::LinuxMusl => 3,
        rewrite_model::RuntimeAbi::Darwin => 4,
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use rewrite_model::RuntimeBuildIdentity;
    use rewrite_types::Digest;

    use super::{PlatformRecordInput, build_platform, valid_kernel_field, valid_kernel_fields};
    use crate::effective_runtime_state_observation::{
        EffectiveRuntimeStateObservationError, LinuxPlatformFrameworkEvidence,
    };

    pub(crate) fn fields_valid(fields: &[Vec<u8>]) -> bool {
        valid_kernel_fields(fields)
    }

    pub(crate) fn field_valid(field: &[u8]) -> bool {
        valid_kernel_field(field)
    }

    pub(crate) fn build_fixture(
        runtime_build: &RuntimeBuildIdentity,
        fields: Vec<Vec<u8>>,
        portable_closure_digest: Digest,
        native_load_digest: Digest,
        component_count: u32,
    ) -> Result<LinuxPlatformFrameworkEvidence, EffectiveRuntimeStateObservationError> {
        build_platform(PlatformRecordInput {
            runtime_build_id: runtime_build.runtime_build_id(),
            target: runtime_build.target(),
            kernel_fields: fields,
            worker_portable_closure_digest: portable_closure_digest,
            worker_native_load_digest: native_load_digest,
            worker_native_component_count: component_count,
        })
    }
}
