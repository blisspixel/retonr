use rewrite_app::{OllamaModelImportResult, OllamaModelReference};
use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, EmbeddedModelComponentPurpose,
    ModelPackageManifest, ModelPackageManifestId, ModelPackageMemberRole, ModelWeightLayout,
    PackageSourceId, PackageSourceKind, PackageTransformation,
};
use rewrite_ollama::OllamaInventoryEntry;
use rewrite_types::Digest;

use super::{
    CONFIG_PATH, LICENSE_PATH, LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION,
    LocalOllamaModelBindingError, MODEL_PATH, PARAMETERS_PATH, PROVENANCE_PATH, TEMPLATE_PATH,
};
use crate::{
    LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION, LocalOllamaPreflightExecutionReceipt,
    LocalOllamaPreflightMode, LocalOllamaPreflightPlan, LocalOllamaPreflightReport,
    local_ollama_preflight::{local_ollama_preflight_report, validate_local_ollama_preflight_plan},
};

pub(super) fn validate_preflight(
    reference: &OllamaModelReference,
    plan: &LocalOllamaPreflightPlan,
    report: &LocalOllamaPreflightReport,
    receipt: &LocalOllamaPreflightExecutionReceipt,
) -> Result<(), LocalOllamaModelBindingError> {
    if !receipt
        .validates(plan, report)
        .map_err(|_| LocalOllamaModelBindingError::InvalidPreflight)?
    {
        return Err(LocalOllamaModelBindingError::InvalidPreflight);
    }
    validate_local_ollama_preflight_plan(plan)
        .map_err(|_| LocalOllamaModelBindingError::InvalidPreflight)?;
    if plan.models.len() != 1 || plan.models[0].reference != reference.runtime_reference() {
        return Err(LocalOllamaModelBindingError::ReferenceMismatch);
    }
    if plan.mode != LocalOllamaPreflightMode::Verify
        || !plan.require_idle
        || report.schema_version != LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION
        || report.qualified
        || report.mode != LocalOllamaPreflightMode::Verify
        || !report.observed.running.is_empty()
        || report.observed.bindings.len() != 1
        || report.observed.runtime.backend != "ollama_native"
        || report.observed.runtime.version != LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION
        || report.observed.runtime.digest.is_some()
        || !strictly_ordered_inventory(&report.observed.inventory)
    {
        return Err(LocalOllamaModelBindingError::InvalidPreflight);
    }
    let expected = local_ollama_preflight_report(plan, report.observed.clone())
        .map_err(|_| LocalOllamaModelBindingError::InvalidPreflight)?;
    if &expected != report
        || report.observed.bindings[0].reference != plan.models[0].reference
        || report.observed.bindings[0].inventory_digest != plan.models[0].inventory_digest
        || plan.models[0].expected_details.as_ref() != Some(&report.observed.bindings[0].details)
    {
        return Err(LocalOllamaModelBindingError::InvalidPreflight);
    }
    Ok(())
}

fn strictly_ordered_inventory(inventory: &[OllamaInventoryEntry]) -> bool {
    !inventory.is_empty()
        && inventory
            .windows(2)
            .all(|pair| pair[0].reference.as_bytes() < pair[1].reference.as_bytes())
}

pub(super) fn unique_inventory<'a>(
    report: &'a LocalOllamaPreflightReport,
    runtime_reference: &str,
) -> Result<&'a OllamaInventoryEntry, LocalOllamaModelBindingError> {
    let mut matches = report
        .observed
        .inventory
        .iter()
        .filter(|entry| entry.reference == runtime_reference);
    let entry = matches
        .next()
        .ok_or(LocalOllamaModelBindingError::InventoryMismatch)?;
    if matches.next().is_some() {
        return Err(LocalOllamaModelBindingError::AmbiguousObservation);
    }
    Ok(entry)
}

pub(super) struct ValidatedPackage {
    pub(super) model_package_manifest_id: ModelPackageManifestId,
    pub(super) artifact_set_id: ArtifactSetId,
    pub(super) package_source_id: PackageSourceId,
    pub(super) inventory_byte_size: u64,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) model_byte_size: u64,
    pub(super) provenance_artifact_id: ArtifactId,
    pub(super) provenance_byte_size: u64,
    pub(super) license_digest: Digest,
    pub(super) explicit_template_digest: Digest,
    pub(super) embedded_template_digest: Digest,
    pub(super) transformation_evidence_digest: Digest,
}

pub(super) fn validate_import(
    import: &OllamaModelImportResult,
    reference: &OllamaModelReference,
) -> Result<ValidatedPackage, LocalOllamaModelBindingError> {
    let artifact_set = import.evidence.artifact_set();
    let package = import.evidence.model_package();
    if import.artifact_set_key.artifact_set_id() != &artifact_set.artifact_set_id()
        || import
            .evidence
            .rootfs_comparison()
            .matches_by_position()
            .len()
            != 4
    {
        return Err(LocalOllamaModelBindingError::InvalidImport);
    }
    validate_static_package(artifact_set, package, reference)
}

pub(super) fn validate_static_package(
    artifact_set: &ArtifactSetManifest,
    package: &ModelPackageManifest,
    reference: &OllamaModelReference,
) -> Result<ValidatedPackage, LocalOllamaModelBindingError> {
    package
        .validate_against(artifact_set)
        .map_err(|_| LocalOllamaModelBindingError::InvalidImport)?;
    if package.format_contract_id() != "ollama-manifest-v2"
        || package.format_contract_schema_version() != 1
        || package.source().kind() != PackageSourceKind::LocalArchive
        || package.source().locator()
            != format!(
                "{}/{}/{}",
                reference.registry(),
                reference.namespace(),
                reference.model()
            )
    {
        return Err(LocalOllamaModelBindingError::InvalidImport);
    }
    validate_exact_member_shape(package)?;
    let member = |path: &str| {
        package
            .members()
            .iter()
            .find(|member| member.relative_path().as_str() == path)
            .ok_or(LocalOllamaModelBindingError::InvalidImport)
    };
    let provenance = member(PROVENANCE_PATH)?;
    let model = member(MODEL_PATH)?;
    let license = member(LICENSE_PATH)?;
    let explicit_template = member(TEMPLATE_PATH)?;
    let provenance_digest = provenance.artifact_id().digest();
    if package.source().provenance_digest() != provenance_digest
        || package.source().revision() != format!("sha256:{}", provenance_digest.as_str())
    {
        return Err(LocalOllamaModelBindingError::InvalidImport);
    }
    let transformation_evidence_digest = match package.transformation() {
        PackageTransformation::Untransformed { evidence_digest } => evidence_digest.clone(),
        PackageTransformation::Transformed { .. } => {
            return Err(LocalOllamaModelBindingError::InvalidImport);
        }
    };
    let embedded_templates = package
        .embedded_components()
        .iter()
        .filter(|component| component.purpose() == EmbeddedModelComponentPurpose::PromptTemplate)
        .collect::<Vec<_>>();
    if embedded_templates.len() != 1
        || embedded_templates[0].container_path().as_str() != MODEL_PATH
        || embedded_templates[0].extraction_contract_id() != "gguf-metadata"
        || embedded_templates[0].extraction_contract_schema_version() != 1
        || embedded_templates[0].selector() != "tokenizer.chat_template"
    {
        return Err(LocalOllamaModelBindingError::InvalidImport);
    }
    let inventory_byte_size = artifact_set
        .members()
        .iter()
        .filter(|member| member.relative_path().as_str() != PROVENANCE_PATH)
        .try_fold(0_u64, |total, member| total.checked_add(member.byte_size()))
        .ok_or(LocalOllamaModelBindingError::InvalidImport)?;
    Ok(ValidatedPackage {
        model_package_manifest_id: package.model_package_manifest_id(),
        artifact_set_id: artifact_set.artifact_set_id(),
        package_source_id: package.source().package_source_id(),
        inventory_byte_size,
        model_artifact_id: model.artifact_id().clone(),
        model_byte_size: model.byte_size(),
        provenance_artifact_id: provenance.artifact_id().clone(),
        provenance_byte_size: provenance.byte_size(),
        license_digest: license.artifact_id().digest().clone(),
        explicit_template_digest: explicit_template.artifact_id().digest().clone(),
        embedded_template_digest: embedded_templates[0].value_digest().clone(),
        transformation_evidence_digest,
    })
}

fn validate_exact_member_shape(
    package: &ModelPackageManifest,
) -> Result<(), LocalOllamaModelBindingError> {
    let expected = [
        (CONFIG_PATH, ModelPackageMemberRole::AuxiliaryData),
        (
            PARAMETERS_PATH,
            ModelPackageMemberRole::GenerationConfiguration,
        ),
        (LICENSE_PATH, ModelPackageMemberRole::LicenseText),
        (MODEL_PATH, ModelPackageMemberRole::ModelWeights),
        (TEMPLATE_PATH, ModelPackageMemberRole::PromptTemplate),
        (PROVENANCE_PATH, ModelPackageMemberRole::ProvenanceRecord),
    ];
    if package.members().len() != expected.len()
        || !package
            .members()
            .iter()
            .zip(expected)
            .all(|(member, (path, role))| {
                member.relative_path().as_str() == path && member.roles() == [role]
            })
        || !matches!(
            package.weight_layout(),
            ModelWeightLayout::Single { member } if member.as_str() == MODEL_PATH
        )
    {
        return Err(LocalOllamaModelBindingError::InvalidImport);
    }
    Ok(())
}
