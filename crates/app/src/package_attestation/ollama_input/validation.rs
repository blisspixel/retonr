use std::{fs::File, marker::PhantomData};

use rewrite_model::{
    ArtifactId, ArtifactSetId, ModelPackageManifest, ModelPackageManifestId,
    ModelPackageMemberRole, ModelWeightLayout, PackageSourceKind, PackageTransformation,
};
use rewrite_ollama_package::{
    BlobDescriptor, OllamaManifestPlan, ReconstructionLimits, ollama_logical_binding_digest,
};
use rewrite_types::{CancellationToken, Digest};

use crate::OllamaModelReference;

use super::{
    ExactModelWeightSource, ManagedOllamaInputError, OrderedInputMember,
    RetainedModelPackageIsolationSource, target_path,
};

const CONFIG_PATH: &str = "config/ollama-config.json";
const PARAMETERS_PATH: &str = "config/parameters.json";
const LICENSE_PATH: &str = "legal/license.txt";
const MODEL_PATH: &str = "model/model.gguf";
const TEMPLATE_PATH: &str = "prompts/template.go.tmpl";
const PROVENANCE_PATH: &str = "provenance/ollama-manifest-v2.json";
const FORMAT_CONTRACT_ID: &str = "ollama-manifest-v2";
const FORMAT_CONTRACT_VERSION: u32 = 1;

const EXPECTED_MEMBERS: [ExpectedMember; 6] = [
    ExpectedMember::new(
        CONFIG_PATH,
        ModelPackageMemberRole::AuxiliaryData,
        MemberKind::Config,
    ),
    ExpectedMember::new(
        PARAMETERS_PATH,
        ModelPackageMemberRole::GenerationConfiguration,
        MemberKind::Parameters,
    ),
    ExpectedMember::new(
        LICENSE_PATH,
        ModelPackageMemberRole::LicenseText,
        MemberKind::License,
    ),
    ExpectedMember::new(
        MODEL_PATH,
        ModelPackageMemberRole::ModelWeights,
        MemberKind::Model,
    ),
    ExpectedMember::new(
        TEMPLATE_PATH,
        ModelPackageMemberRole::PromptTemplate,
        MemberKind::Template,
    ),
    ExpectedMember::new(
        PROVENANCE_PATH,
        ModelPackageMemberRole::ProvenanceRecord,
        MemberKind::Provenance,
    ),
];

pub(super) struct PreparedInput<'lease> {
    pub(super) artifact_set_id: ArtifactSetId,
    pub(super) model_package_manifest_id: ModelPackageManifestId,
    pub(super) installation_generation: u64,
    pub(super) reference_digest: Digest,
    pub(super) mapping_digest: Digest,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) model_target_path: String,
    pub(super) model_target_digest: Digest,
    pub(super) model_weight_source: ExactModelWeightSource<'lease>,
    pub(super) members: Vec<OrderedInputMember>,
}

pub(super) fn prepare<'lease>(
    source: RetainedModelPackageIsolationSource<'lease>,
    package: &ModelPackageManifest,
    reference: &OllamaModelReference,
    reconstruction_limits: &ReconstructionLimits,
    cancellation: &CancellationToken,
) -> Result<PreparedInput<'lease>, ManagedOllamaInputError> {
    ensure_not_cancelled(cancellation)?;
    reconstruction_limits
        .validate()
        .map_err(ManagedOllamaInputError::Reconstruction)?;
    source.revalidate(cancellation).map_err(map_package_error)?;
    validate_package_identity(&source, package, reference)?;
    validate_members(&source, package)?;

    let provenance = source
        .members
        .get(5)
        .ok_or(ManagedOllamaInputError::RelationshipMismatch)?;
    let manifest_bytes = read_manifest(
        &provenance.file,
        provenance.byte_size,
        reconstruction_limits.manifest_bytes,
        cancellation,
    )?;
    let plan = rewrite_ollama_package::parse_manifest_v2(&manifest_bytes, reconstruction_limits)
        .map_err(ManagedOllamaInputError::Reconstruction)?;
    validate_source(package, reference, &plan)?;
    validate_transformation(package, &plan)?;
    validate_descriptors(&source, &plan)?;
    ensure_not_cancelled(cancellation)?;

    build_prepared(source, reference)
}

fn validate_package_identity(
    source: &RetainedModelPackageIsolationSource<'_>,
    package: &ModelPackageManifest,
    reference: &OllamaModelReference,
) -> Result<(), ManagedOllamaInputError> {
    let expected_locator = source_locator(reference);
    let relationship_matches = source.artifact_set_id() == package.artifact_set_id()
        && source.model_package_manifest_id() == &package.model_package_manifest_id()
        && package.format_contract_id() == FORMAT_CONTRACT_ID
        && package.format_contract_schema_version() == FORMAT_CONTRACT_VERSION
        && package.source().kind() == PackageSourceKind::LocalArchive
        && package.source().locator() == expected_locator
        && matches!(
            package.transformation(),
            PackageTransformation::Untransformed { .. }
        )
        && matches!(
            package.weight_layout(),
            ModelWeightLayout::Single { member } if member.as_str() == MODEL_PATH
        );
    if relationship_matches {
        Ok(())
    } else {
        Err(ManagedOllamaInputError::RelationshipMismatch)
    }
}

fn validate_members(
    source: &RetainedModelPackageIsolationSource<'_>,
    package: &ModelPackageManifest,
) -> Result<(), ManagedOllamaInputError> {
    if source.members.len() != EXPECTED_MEMBERS.len()
        || package.members().len() != EXPECTED_MEMBERS.len()
    {
        return Err(ManagedOllamaInputError::RelationshipMismatch);
    }
    for ((retained, declared), expected) in source
        .members
        .iter()
        .zip(package.members())
        .zip(EXPECTED_MEMBERS)
    {
        if retained.relative_path.as_str() != expected.path
            || declared.relative_path().as_str() != expected.path
            || retained.roles.as_slice() != [expected.role]
            || declared.roles() != [expected.role]
            || retained.artifact_id != *declared.artifact_id()
            || retained.byte_size != declared.byte_size()
        {
            return Err(ManagedOllamaInputError::RelationshipMismatch);
        }
    }
    Ok(())
}

fn validate_source(
    package: &ModelPackageManifest,
    reference: &OllamaModelReference,
    plan: &OllamaManifestPlan,
) -> Result<(), ManagedOllamaInputError> {
    let expected_revision = format!("sha256:{}", plan.raw_manifest_digest().as_str());
    if package.source().locator() != source_locator(reference)
        || package.source().revision() != expected_revision
        || package.source().provenance_digest() != plan.raw_manifest_digest()
    {
        Err(ManagedOllamaInputError::RelationshipMismatch)
    } else {
        Ok(())
    }
}

fn validate_transformation(
    package: &ModelPackageManifest,
    plan: &OllamaManifestPlan,
) -> Result<(), ManagedOllamaInputError> {
    let expected =
        ollama_logical_binding_digest(plan).map_err(ManagedOllamaInputError::Reconstruction)?;
    match package.transformation() {
        PackageTransformation::Untransformed { evidence_digest }
            if evidence_digest == &expected =>
        {
            Ok(())
        }
        PackageTransformation::Untransformed { .. } | PackageTransformation::Transformed { .. } => {
            Err(ManagedOllamaInputError::RelationshipMismatch)
        }
    }
}

fn validate_descriptors(
    source: &RetainedModelPackageIsolationSource<'_>,
    plan: &OllamaManifestPlan,
) -> Result<(), ManagedOllamaInputError> {
    for (retained, expected) in source.members.iter().zip(EXPECTED_MEMBERS) {
        let matches = match expected.kind {
            MemberKind::Config => descriptor_matches(retained, plan.config()),
            MemberKind::Parameters => descriptor_matches(retained, plan.parameters()),
            MemberKind::License => descriptor_matches(retained, plan.license()),
            MemberKind::Model => descriptor_matches(retained, plan.model()),
            MemberKind::Template => descriptor_matches(retained, plan.template()),
            MemberKind::Provenance => {
                retained.artifact_id.digest() == plan.raw_manifest_digest()
                    && retained.byte_size == plan.raw_manifest_size()
            }
        };
        if !matches {
            return Err(ManagedOllamaInputError::RelationshipMismatch);
        }
    }
    Ok(())
}

fn descriptor_matches(
    member: &super::super::verification::RetainedModelMember,
    descriptor: &BlobDescriptor,
) -> bool {
    member.artifact_id.digest() == descriptor.digest() && member.byte_size == descriptor.size()
}

fn build_prepared<'lease>(
    source: RetainedModelPackageIsolationSource<'lease>,
    reference: &OllamaModelReference,
) -> Result<PreparedInput<'lease>, ManagedOllamaInputError> {
    let artifact_set_id = source.artifact_set_id;
    let model_package_manifest_id = source.model_package_manifest_id;
    let installation_generation = source.installation_generation;
    let reference_digest = reference_digest(reference);
    let manifest_alias = manifest_alias(reference);
    let mut model_artifact_id = None;
    let mut model_alias = None;
    let mut model_weight_source = None;
    let mut members = Vec::with_capacity(EXPECTED_MEMBERS.len());
    for (retained, expected) in source.members.into_iter().zip(EXPECTED_MEMBERS) {
        let alias = if expected.kind == MemberKind::Provenance {
            manifest_alias.clone()
        } else {
            format!("blobs/sha256-{}", retained.artifact_id.digest().as_str())
        };
        if expected.kind == MemberKind::Model {
            model_artifact_id = Some(retained.artifact_id.clone());
            model_alias = Some(alias.clone());
            model_weight_source = Some(ExactModelWeightSource {
                artifact_id: retained.artifact_id.clone(),
                byte_size: retained.byte_size,
                file: retained.file.try_clone().map_err(|error| {
                    ManagedOllamaInputError::Package(super::PackageAttestationError::MemberIo(
                        error,
                    ))
                })?,
                _lease: PhantomData,
            });
        }
        members.push(OrderedInputMember {
            alias,
            digest: retained.artifact_id.digest().clone(),
            byte_size: retained.byte_size,
            file: retained.file,
        });
    }
    members.sort_by(|left, right| left.alias.cmp(&right.alias));
    members = deduplicate_cas_members(members)?;
    let mapping_digest = mapping_digest(&members, &reference_digest);
    let model_alias = model_alias.ok_or(ManagedOllamaInputError::RelationshipMismatch)?;
    let model_target_path = target_path(&model_alias);
    let model_target_digest = Digest::sha256(model_target_path.as_bytes());
    Ok(PreparedInput {
        artifact_set_id,
        model_package_manifest_id,
        installation_generation,
        reference_digest,
        mapping_digest,
        model_artifact_id: model_artifact_id
            .ok_or(ManagedOllamaInputError::RelationshipMismatch)?,
        model_target_path,
        model_target_digest,
        model_weight_source: model_weight_source
            .ok_or(ManagedOllamaInputError::RelationshipMismatch)?,
        members,
    })
}

fn deduplicate_cas_members(
    members: Vec<OrderedInputMember>,
) -> Result<Vec<OrderedInputMember>, ManagedOllamaInputError> {
    let mut deduplicated: Vec<OrderedInputMember> = Vec::with_capacity(members.len());
    for member in members {
        if let Some(previous) = deduplicated.last()
            && previous.alias == member.alias
        {
            if previous.digest != member.digest || previous.byte_size != member.byte_size {
                return Err(ManagedOllamaInputError::RelationshipMismatch);
            }
            continue;
        }
        deduplicated.push(member);
    }
    Ok(deduplicated)
}

fn read_manifest(
    file: &File,
    byte_size: u64,
    maximum_bytes: usize,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, ManagedOllamaInputError> {
    let byte_size = usize::try_from(byte_size)
        .ok()
        .filter(|size| *size <= maximum_bytes)
        .ok_or(ManagedOllamaInputError::RelationshipMismatch)?;
    let mut output = vec![0_u8; byte_size];
    let mut offset = 0_usize;
    while offset < output.len() {
        ensure_not_cancelled(cancellation)?;
        let count = read_positioned(
            file,
            &mut output[offset..],
            u64::try_from(offset).map_err(|_| ManagedOllamaInputError::RelationshipMismatch)?,
        )
        .map_err(|error| {
            ManagedOllamaInputError::Package(super::PackageAttestationError::MemberIo(error))
        })?;
        if count == 0 {
            return Err(ManagedOllamaInputError::RelationshipMismatch);
        }
        offset = offset
            .checked_add(count)
            .ok_or(ManagedOllamaInputError::RelationshipMismatch)?;
    }
    ensure_not_cancelled(cancellation)?;
    let mut trailing = [0_u8; 1];
    if read_positioned(
        file,
        &mut trailing,
        u64::try_from(offset).map_err(|_| ManagedOllamaInputError::RelationshipMismatch)?,
    )
    .map_err(|error| {
        ManagedOllamaInputError::Package(super::PackageAttestationError::MemberIo(error))
    })? != 0
    {
        return Err(ManagedOllamaInputError::RelationshipMismatch);
    }
    Ok(output)
}

fn source_locator(reference: &OllamaModelReference) -> String {
    format!(
        "{}/{}/{}",
        reference.registry(),
        reference.namespace(),
        reference.model()
    )
}

fn manifest_alias(reference: &OllamaModelReference) -> String {
    format!(
        "manifests/{}/{}/{}/{}",
        reference.registry(),
        reference.namespace(),
        reference.model(),
        reference.tag()
    )
}

fn reference_digest(reference: &OllamaModelReference) -> Digest {
    let mut material = b"retonr:managed-ollama-reference:v1\0".to_vec();
    for component in [
        reference.registry(),
        reference.namespace(),
        reference.model(),
        reference.tag(),
    ] {
        append_text(&mut material, component);
    }
    Digest::sha256(&material)
}

fn mapping_digest(members: &[OrderedInputMember], reference_digest: &Digest) -> Digest {
    let mut material = b"retonr:managed-ollama-input-mapping:v1\0".to_vec();
    material.extend_from_slice(reference_digest.as_str().as_bytes());
    material.extend_from_slice(&(members.len() as u64).to_be_bytes());
    for member in members {
        append_text(&mut material, &member.alias);
        material.extend_from_slice(member.digest.as_str().as_bytes());
        material.extend_from_slice(&member.byte_size.to_be_bytes());
    }
    Digest::sha256(&material)
}

fn append_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), ManagedOllamaInputError> {
    if cancellation.is_cancelled() {
        Err(ManagedOllamaInputError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_package_error(error: super::PackageAttestationError) -> ManagedOllamaInputError {
    if matches!(error, super::PackageAttestationError::Cancelled) {
        ManagedOllamaInputError::Cancelled
    } else {
        ManagedOllamaInputError::Package(error)
    }
}

#[cfg(unix)]
fn read_positioned(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt as _;

    file.read_at(buffer, offset)
}

#[cfg(windows)]
fn read_positioned(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt as _;

    file.seek_read(buffer, offset)
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ExpectedMember {
    path: &'static str,
    role: ModelPackageMemberRole,
    kind: MemberKind,
}

impl ExpectedMember {
    const fn new(path: &'static str, role: ModelPackageMemberRole, kind: MemberKind) -> Self {
        Self { path, role, kind }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MemberKind {
    Config,
    Parameters,
    License,
    Model,
    Template,
    Provenance,
}
