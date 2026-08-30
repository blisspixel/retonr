use rewrite_model::{
    ArtifactId, ArtifactSetId, PackageSourceId, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageManifestId, RuntimePackageMemberRole,
};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_runtime_attestor::FrozenExternalNativeComponentSetId;
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use super::{
    MAX_MANAGED_GENERATION_PATH_REVIEW_BYTES, RuntimeAdmissionAuthorityError,
    RuntimeAdmissionEvidenceFoundationId,
};

#[derive(Serialize)]
pub(super) struct AdmittedRuntimeBinding {
    pub(super) cloud_disable_control_digest: Digest,
    pub(super) cloud_disable_version_status: OllamaCloudDisableVersionStatus,
    pub(super) foundation_id: RuntimeAdmissionEvidenceFoundationId,
    pub(super) frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    pub(super) layout_digest: Digest,
    pub(super) license_control_digest: Digest,
    pub(super) managed_startup_control_digest: Digest,
    pub(super) native_closure_control_digest: Digest,
    pub(super) package_source_id: PackageSourceId,
    pub(super) runtime_package_manifest_id: RuntimePackageManifestId,
    pub(super) source_build_inputs_id: ArtifactSetId,
    pub(super) source_lineage_control_digest: Digest,
    pub(super) startup_launch_spec_digest: Digest,
    pub(super) transformation_control_digest: Digest,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ManagedGenerationPathReviewWire {
    pub(super) admitted_runtime_id: Digest,
    pub(super) frozen_external_component_set_id: FrozenExternalNativeComponentSetId,
    pub(super) package_source_id: PackageSourceId,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) reviewed_source_build_inputs_id: ArtifactSetId,
    pub(super) runtime_package_manifest_id: RuntimePackageManifestId,
    pub(super) runtime_version: String,
    pub(super) schema_version: u32,
    pub(super) status: GenerationPathReviewStatus,
    pub(super) worker_artifact_id: ArtifactId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GenerationPathReviewStatus {
    Reviewed,
}

pub(super) fn exact_generation_worker(
    package: &RuntimePackageManifest,
) -> Result<&rewrite_model::RuntimePackageMember, RuntimeAdmissionAuthorityError> {
    let mut workers = package.members().iter().filter(|member| {
        member
            .roles()
            .contains(&RuntimePackageMemberRole::WorkerExecutable)
    });
    let worker = workers
        .next()
        .ok_or(RuntimeAdmissionAuthorityError::GenerationPathBinding)?;
    if workers.next().is_some()
        || worker.roles() != [RuntimePackageMemberRole::WorkerExecutable]
        || worker.load_policy() != RuntimePackageLoadPolicy::BackendConditional
    {
        return Err(RuntimeAdmissionAuthorityError::GenerationPathBinding);
    }
    Ok(worker)
}

pub(super) fn validate_control_bindings(
    foundation_id: &RuntimeAdmissionEvidenceFoundationId,
    control_foundation_ids: [&RuntimeAdmissionEvidenceFoundationId; 6],
    managed_record_matches: bool,
    native_record_matches: bool,
    frozen_set_matches: bool,
) -> Result<(), RuntimeAdmissionAuthorityError> {
    if !managed_record_matches
        || !native_record_matches
        || !frozen_set_matches
        || control_foundation_ids
            .into_iter()
            .any(|control_foundation| control_foundation != foundation_id)
    {
        return Err(RuntimeAdmissionAuthorityError::ControlBinding);
    }
    Ok(())
}

pub(super) fn validate_review_relationships(
    all_passed: bool,
    admitted_disposition: bool,
    declared_package_id: Option<&RuntimePackageManifestId>,
    reconstructed_package_id: Option<&RuntimePackageManifestId>,
    foundation_package_id: &RuntimePackageManifestId,
    reviewed_source_build_inputs_id: &ArtifactSetId,
    foundation_source_build_inputs_id: &ArtifactSetId,
) -> Result<(), RuntimeAdmissionAuthorityError> {
    if !all_passed
        || !admitted_disposition
        || declared_package_id.is_none()
        || reconstructed_package_id.is_none()
    {
        return Err(RuntimeAdmissionAuthorityError::ReviewNotAdmitted);
    }
    if declared_package_id != reconstructed_package_id
        || declared_package_id != Some(foundation_package_id)
        || reviewed_source_build_inputs_id != foundation_source_build_inputs_id
    {
        return Err(RuntimeAdmissionAuthorityError::ReviewBinding);
    }
    Ok(())
}

pub(super) fn parse_canonical(
    bytes: &[u8],
) -> Result<ManagedGenerationPathReviewWire, RuntimeAdmissionAuthorityError> {
    if bytes.is_empty() || bytes.len() > MAX_MANAGED_GENERATION_PATH_REVIEW_BYTES {
        return Err(RuntimeAdmissionAuthorityError::GenerationPathLimit);
    }
    let wire: ManagedGenerationPathReviewWire = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeAdmissionAuthorityError::GenerationPathEncoding)?;
    if encode(&wire)? != bytes {
        return Err(RuntimeAdmissionAuthorityError::GenerationPathEncoding);
    }
    Ok(wire)
}

pub(super) fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, RuntimeAdmissionAuthorityError> {
    serde_json::to_vec(value).map_err(|_| RuntimeAdmissionAuthorityError::Encoding)
}

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> Digest {
    let mut input = Vec::with_capacity(domain.len().saturating_add(bytes.len()));
    input.extend_from_slice(domain);
    input.extend_from_slice(bytes);
    Digest::sha256(&input)
}
