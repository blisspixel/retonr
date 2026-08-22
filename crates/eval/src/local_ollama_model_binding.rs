//! Inert binding from one offline Ollama import to one verified API inventory entry.

use rewrite_app::{
    ArtifactSetImportDisposition, OllamaModelImportResult, OllamaModelReference,
    PackageManifestWriteDisposition,
};
use rewrite_model::{ArtifactId, ArtifactSetId, ModelPackageManifestId, PackageSourceId};
use rewrite_types::Digest;
use serde::Serialize;
use thiserror::Error;

use crate::{
    LocalOllamaPreflightExecutionReceipt, LocalOllamaPreflightPlan, LocalOllamaPreflightReport,
};

mod validation;

use validation::{unique_inventory, validate_import, validate_preflight};

/// Current offline-import-to-inventory binding evidence version.
pub const LOCAL_OLLAMA_MODEL_BINDING_SCHEMA_VERSION: u32 = 1;
/// Exact Ollama runtime version whose inventory-size implementation was reviewed.
pub const LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION: &str = "0.32.15";

const OLLAMA_INVENTORY_SOURCE_REVISION: &str = "b7871fc0d1d82fe109536efa3e0e8e411c766c75";

const CONFIG_PATH: &str = "config/ollama-config.json";
const PARAMETERS_PATH: &str = "config/parameters.json";
const LICENSE_PATH: &str = "legal/license.txt";
const MODEL_PATH: &str = "model/model.gguf";
const TEMPLATE_PATH: &str = "prompts/template.go.tmpl";
const PROVENANCE_PATH: &str = "provenance/ollama-manifest-v2.json";

/// Managed byte-publication outcome retained by the inert binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaArtifactSetDisposition {
    /// This import published and registered a new managed tree.
    Imported,
    /// This import registered exact bytes that were already published.
    RegisteredExisting,
    /// The exact managed bytes and state were already present.
    AlreadyPresent,
}

/// Model-package persistence outcome retained by the inert binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaPackageManifestDisposition {
    /// The immutable model-package manifest was inserted.
    Inserted,
    /// The exact immutable model-package manifest was already present.
    AlreadyPresent,
}

/// Which unique package template candidate matched the verified Ollama details.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalOllamaObservedTemplateMatch {
    /// The explicit template layer matched.
    ExplicitLayer,
    /// The exact embedded GGUF chat-template bytes matched.
    EmbeddedGguf,
}

/// Content-free evidence for one exact static package and mutable inventory address.
///
/// This record proves only the checked static relationship. It does not prove
/// model residency, request handling, generation, effective identity, or qualification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the inert evidence exposes distinct positive and negative security claims"
)]
pub struct LocalOllamaModelBindingEvidence {
    /// Evidence contract version.
    pub schema_version: u32,
    /// Domain-separated digest over every evidence field except this digest.
    binding_digest: Digest,
    /// Exact verified preflight plan identity.
    pub preflight_plan_digest: Digest,
    /// Digest of the complete stable API observation without retaining its strings.
    pub preflight_observation_digest: Digest,
    /// Exact imported semantic model-package identity.
    pub model_package_manifest_id: ModelPackageManifestId,
    /// Exact imported six-member byte-set identity.
    pub artifact_set_id: ArtifactSetId,
    /// Exact positive repository installation generation.
    pub artifact_set_installation_generation: u64,
    /// Exact package-source identity.
    pub package_source_id: PackageSourceId,
    /// Digest of the exact canonical Ollama API reference.
    pub runtime_reference_digest: Digest,
    /// Raw manifest digest reported by Ollama inventory.
    pub inventory_digest: Digest,
    /// Sum of the five exact descriptor blob sizes reported by Ollama inventory.
    pub inventory_byte_size: u64,
    /// Reviewed version-scoped rule that binds inventory size to config plus layers.
    pub inventory_size_contract_digest: Digest,
    /// Exact GGUF model blob identity.
    pub model_artifact_id: ArtifactId,
    /// Exact GGUF model blob size.
    pub model_byte_size: u64,
    /// Exact retained raw manifest provenance identity.
    pub provenance_artifact_id: ArtifactId,
    /// Exact retained raw manifest length.
    pub provenance_byte_size: u64,
    /// Unique package template candidate matching the frozen runtime details.
    pub observed_template_match: LocalOllamaObservedTemplateMatch,
    /// Digest of the full frozen model-details record.
    pub model_details_digest: Digest,
    /// Exact untransformed package relationship evidence.
    pub transformation_evidence_digest: Digest,
    /// Managed byte-publication outcome.
    pub artifact_set_disposition: LocalOllamaArtifactSetDisposition,
    /// Immutable model-package persistence outcome.
    pub model_package_disposition: LocalOllamaPackageManifestDisposition,
    /// Informational config rootfs and layer cardinality comparison.
    pub rootfs_same_cardinality: bool,
    /// Informational positional config rootfs and layer comparisons.
    pub rootfs_matches_by_position: Vec<bool>,
    /// True only for a successfully checked static package-to-inventory relationship.
    pub static_package_inventory_relationship_verified: bool,
    /// Always false. Static and read-only evidence does not prove model residency.
    pub model_loaded_proven: bool,
    /// Always false. Static and read-only evidence does not prove model use.
    pub model_used_proven: bool,
    /// Always false. This evidence does not identify an application handler.
    pub application_handler_proven: bool,
    /// Always false. This evidence is not an effective runtime identity.
    pub effective_identity_proven: bool,
    /// Always false. Family, quantization, capabilities, and metadata are not
    /// reconstructed into fields directly comparable with Ollama `/api/show`.
    pub complete_model_details_reconstructed_proven: bool,
    /// Always false. This relationship cannot qualify a model or runtime.
    pub qualified: bool,
}

impl LocalOllamaModelBindingEvidence {
    /// Returns the domain-separated digest binding every evidence field.
    #[must_use]
    pub const fn binding_digest(&self) -> &Digest {
        &self.binding_digest
    }
}

/// Failure to establish the exact static package-to-inventory relationship.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LocalOllamaModelBindingError {
    /// The supplied import evidence is internally inconsistent or unsupported.
    #[error("installed Ollama import evidence is invalid for inventory binding")]
    InvalidImport,
    /// The logical model reference does not match the imported source or verified plan.
    #[error("Ollama model reference does not match the import and preflight")]
    ReferenceMismatch,
    /// The supplied plan and report do not form one exact frozen idle preflight.
    #[error("Ollama preflight evidence is invalid for model-package binding")]
    InvalidPreflight,
    /// Runtime inventory digest or size differs from the reconstructed package.
    #[error("Ollama inventory does not match the reconstructed package")]
    InventoryMismatch,
    /// The report contains an ambiguous inventory or details relationship.
    #[error("Ollama inventory or details relationship is ambiguous")]
    AmbiguousObservation,
    /// Runtime details disagree with package license, format, or template candidates.
    #[error("Ollama model details do not match the reconstructed package")]
    DetailsMismatch,
    /// More than one package template candidate matches the runtime details.
    #[error("Ollama template candidate relationship is ambiguous")]
    AmbiguousTemplate,
}

/// Binds one inert offline import to one exact verified Ollama inventory entry.
///
/// The plan must contain exactly one model in verify mode and require an idle
/// runtime. The receipt must come from the preflight runner for the exact plan
/// and report and is consumed by this call. The returned evidence is redacted
/// and cannot authorize execution.
///
/// # Errors
///
/// Returns [`LocalOllamaModelBindingError`] for any package, reference, report,
/// digest, size, member, details, ordering, or template-candidate mismatch.
pub fn bind_imported_ollama_model_to_preflight(
    import: &OllamaModelImportResult,
    reference: &OllamaModelReference,
    plan: &LocalOllamaPreflightPlan,
    report: &LocalOllamaPreflightReport,
    receipt: LocalOllamaPreflightExecutionReceipt,
) -> Result<LocalOllamaModelBindingEvidence, LocalOllamaModelBindingError> {
    validate_preflight(reference, plan, report, &receipt)?;
    drop(receipt);
    let package = validate_import(import, reference)?;
    let runtime_reference = reference.runtime_reference();
    let inventory = unique_inventory(report, &runtime_reference)?;
    if inventory.inventory_digest != package.provenance_artifact_id.digest().clone()
        || inventory.byte_size != package.inventory_byte_size
    {
        return Err(LocalOllamaModelBindingError::InventoryMismatch);
    }
    let details = &report.observed.bindings[0].details;
    let observed_template_match = match (
        details.template_digest == package.explicit_template_digest,
        details.template_digest == package.embedded_template_digest,
    ) {
        (true, false) => LocalOllamaObservedTemplateMatch::ExplicitLayer,
        (false, true) => LocalOllamaObservedTemplateMatch::EmbeddedGguf,
        (true, true) => return Err(LocalOllamaModelBindingError::AmbiguousTemplate),
        (false, false) => return Err(LocalOllamaModelBindingError::DetailsMismatch),
    };
    if details.format != "gguf" || details.license_digest != package.license_digest {
        return Err(LocalOllamaModelBindingError::DetailsMismatch);
    }

    let preflight_observation_digest = digest_json(&report.observed)?;
    let model_details_digest = digest_json(details)?;
    let inventory_size_contract_digest = inventory_size_contract_digest();
    let material = BindingMaterial {
        schema_version: LOCAL_OLLAMA_MODEL_BINDING_SCHEMA_VERSION,
        preflight_plan_digest: &report.plan_digest,
        preflight_observation_digest: &preflight_observation_digest,
        model_package_manifest_id: &package.model_package_manifest_id,
        artifact_set_id: &package.artifact_set_id,
        artifact_set_installation_generation: import.artifact_set_key.installation_generation(),
        package_source_id: &package.package_source_id,
        runtime_reference_digest: Digest::sha256(runtime_reference.as_bytes()),
        inventory_digest: &inventory.inventory_digest,
        inventory_byte_size: inventory.byte_size,
        inventory_size_contract_digest: &inventory_size_contract_digest,
        model_artifact_id: &package.model_artifact_id,
        model_byte_size: package.model_byte_size,
        provenance_artifact_id: &package.provenance_artifact_id,
        provenance_byte_size: package.provenance_byte_size,
        observed_template_match,
        model_details_digest: &model_details_digest,
        transformation_evidence_digest: &package.transformation_evidence_digest,
        artifact_set_disposition: import.artifact_set_disposition.into(),
        model_package_disposition: import.model_package_disposition.into(),
        rootfs_same_cardinality: import.evidence.rootfs_comparison().same_cardinality(),
        rootfs_matches_by_position: import.evidence.rootfs_comparison().matches_by_position(),
        static_package_inventory_relationship_verified: true,
        model_loaded_proven: false,
        model_used_proven: false,
        application_handler_proven: false,
        effective_identity_proven: false,
        complete_model_details_reconstructed_proven: false,
        qualified: false,
    };
    let binding_digest = binding_digest(&material)?;
    Ok(LocalOllamaModelBindingEvidence {
        schema_version: material.schema_version,
        binding_digest,
        preflight_plan_digest: material.preflight_plan_digest.clone(),
        preflight_observation_digest: preflight_observation_digest.clone(),
        model_package_manifest_id: material.model_package_manifest_id.clone(),
        artifact_set_id: material.artifact_set_id.clone(),
        artifact_set_installation_generation: material.artifact_set_installation_generation,
        package_source_id: material.package_source_id.clone(),
        runtime_reference_digest: material.runtime_reference_digest,
        inventory_digest: material.inventory_digest.clone(),
        inventory_byte_size: material.inventory_byte_size,
        inventory_size_contract_digest: inventory_size_contract_digest.clone(),
        model_artifact_id: material.model_artifact_id.clone(),
        model_byte_size: material.model_byte_size,
        provenance_artifact_id: material.provenance_artifact_id.clone(),
        provenance_byte_size: material.provenance_byte_size,
        observed_template_match,
        model_details_digest: model_details_digest.clone(),
        transformation_evidence_digest: material.transformation_evidence_digest.clone(),
        artifact_set_disposition: material.artifact_set_disposition,
        model_package_disposition: material.model_package_disposition,
        rootfs_same_cardinality: material.rootfs_same_cardinality,
        rootfs_matches_by_position: material.rootfs_matches_by_position.to_vec(),
        static_package_inventory_relationship_verified: material
            .static_package_inventory_relationship_verified,
        model_loaded_proven: material.model_loaded_proven,
        model_used_proven: material.model_used_proven,
        application_handler_proven: material.application_handler_proven,
        effective_identity_proven: material.effective_identity_proven,
        complete_model_details_reconstructed_proven: material
            .complete_model_details_reconstructed_proven,
        qualified: material.qualified,
    })
}

#[derive(Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the digest material includes every explicit fixed security claim flag"
)]
struct BindingMaterial<'a> {
    schema_version: u32,
    preflight_plan_digest: &'a Digest,
    preflight_observation_digest: &'a Digest,
    model_package_manifest_id: &'a ModelPackageManifestId,
    artifact_set_id: &'a ArtifactSetId,
    artifact_set_installation_generation: u64,
    package_source_id: &'a PackageSourceId,
    runtime_reference_digest: Digest,
    inventory_digest: &'a Digest,
    inventory_byte_size: u64,
    inventory_size_contract_digest: &'a Digest,
    model_artifact_id: &'a ArtifactId,
    model_byte_size: u64,
    provenance_artifact_id: &'a ArtifactId,
    provenance_byte_size: u64,
    observed_template_match: LocalOllamaObservedTemplateMatch,
    model_details_digest: &'a Digest,
    transformation_evidence_digest: &'a Digest,
    artifact_set_disposition: LocalOllamaArtifactSetDisposition,
    model_package_disposition: LocalOllamaPackageManifestDisposition,
    rootfs_same_cardinality: bool,
    rootfs_matches_by_position: &'a [bool],
    static_package_inventory_relationship_verified: bool,
    model_loaded_proven: bool,
    model_used_proven: bool,
    application_handler_proven: bool,
    effective_identity_proven: bool,
    complete_model_details_reconstructed_proven: bool,
    qualified: bool,
}

fn binding_digest(material: &BindingMaterial<'_>) -> Result<Digest, LocalOllamaModelBindingError> {
    let canonical =
        serde_json::to_vec(material).map_err(|_| LocalOllamaModelBindingError::InvalidImport)?;
    let mut bytes = b"retonr:local-ollama-model-binding:v1\0".to_vec();
    bytes.extend_from_slice(&canonical);
    Ok(Digest::sha256(&bytes))
}

pub(crate) fn validate_local_ollama_model_binding_evidence(
    evidence: &LocalOllamaModelBindingEvidence,
) -> bool {
    if evidence.schema_version != LOCAL_OLLAMA_MODEL_BINDING_SCHEMA_VERSION
        || !evidence.static_package_inventory_relationship_verified
        || evidence.model_loaded_proven
        || evidence.model_used_proven
        || evidence.application_handler_proven
        || evidence.effective_identity_proven
        || evidence.complete_model_details_reconstructed_proven
        || evidence.qualified
    {
        return false;
    }
    let material = BindingMaterial {
        schema_version: evidence.schema_version,
        preflight_plan_digest: &evidence.preflight_plan_digest,
        preflight_observation_digest: &evidence.preflight_observation_digest,
        model_package_manifest_id: &evidence.model_package_manifest_id,
        artifact_set_id: &evidence.artifact_set_id,
        artifact_set_installation_generation: evidence.artifact_set_installation_generation,
        package_source_id: &evidence.package_source_id,
        runtime_reference_digest: evidence.runtime_reference_digest.clone(),
        inventory_digest: &evidence.inventory_digest,
        inventory_byte_size: evidence.inventory_byte_size,
        inventory_size_contract_digest: &evidence.inventory_size_contract_digest,
        model_artifact_id: &evidence.model_artifact_id,
        model_byte_size: evidence.model_byte_size,
        provenance_artifact_id: &evidence.provenance_artifact_id,
        provenance_byte_size: evidence.provenance_byte_size,
        observed_template_match: evidence.observed_template_match,
        model_details_digest: &evidence.model_details_digest,
        transformation_evidence_digest: &evidence.transformation_evidence_digest,
        artifact_set_disposition: evidence.artifact_set_disposition,
        model_package_disposition: evidence.model_package_disposition,
        rootfs_same_cardinality: evidence.rootfs_same_cardinality,
        rootfs_matches_by_position: &evidence.rootfs_matches_by_position,
        static_package_inventory_relationship_verified: evidence
            .static_package_inventory_relationship_verified,
        model_loaded_proven: evidence.model_loaded_proven,
        model_used_proven: evidence.model_used_proven,
        application_handler_proven: evidence.application_handler_proven,
        effective_identity_proven: evidence.effective_identity_proven,
        complete_model_details_reconstructed_proven: evidence
            .complete_model_details_reconstructed_proven,
        qualified: evidence.qualified,
    };
    binding_digest(&material).is_ok_and(|digest| digest == evidence.binding_digest)
}

fn digest_json(value: &impl Serialize) -> Result<Digest, LocalOllamaModelBindingError> {
    serde_json::to_vec(value)
        .map(|bytes| Digest::sha256(&bytes))
        .map_err(|_| LocalOllamaModelBindingError::InvalidPreflight)
}

fn inventory_size_contract_digest() -> Digest {
    let mut bytes = b"retonr:ollama-inventory-size-contract:v1\0".to_vec();
    bytes.extend_from_slice(LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(OLLAMA_INVENTORY_SOURCE_REVISION.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(b"manifest-v2-config-plus-layers");
    Digest::sha256(&bytes)
}

impl From<ArtifactSetImportDisposition> for LocalOllamaArtifactSetDisposition {
    fn from(value: ArtifactSetImportDisposition) -> Self {
        match value {
            ArtifactSetImportDisposition::Imported => Self::Imported,
            ArtifactSetImportDisposition::RegisteredExisting => Self::RegisteredExisting,
            ArtifactSetImportDisposition::AlreadyPresent => Self::AlreadyPresent,
        }
    }
}

impl From<PackageManifestWriteDisposition> for LocalOllamaPackageManifestDisposition {
    fn from(value: PackageManifestWriteDisposition) -> Self {
        match value {
            PackageManifestWriteDisposition::Inserted => Self::Inserted,
            PackageManifestWriteDisposition::AlreadyPresent => Self::AlreadyPresent,
        }
    }
}

#[cfg(test)]
#[path = "local_ollama_model_binding/tests.rs"]
pub(crate) mod tests;
