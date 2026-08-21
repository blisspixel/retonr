use sha2::{Digest as _, Sha256};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout, PackageSource,
    PackageTransformation,
};
use rewrite_types::Digest;

use crate::error::{ReconstructionError, ReconstructionResult};
use crate::gguf::GgufObservation;
use crate::manifest::{BlobDescriptor, OllamaManifestPlan};

const CONFIG_PATH: &str = "config/ollama-config.json";
const PARAMETERS_PATH: &str = "config/parameters.json";
const LICENSE_PATH: &str = "legal/license.txt";
const MODEL_PATH: &str = "model/model.gguf";
const TEMPLATE_PATH: &str = "prompts/template.go.tmpl";
const PROVENANCE_PATH: &str = "provenance/ollama-manifest-v2.json";

const SOURCE_MAPPINGS: [(&str, &str); 5] = [
    (super::super::manifest::CONFIG_MEDIA_TYPE, CONFIG_PATH),
    (super::super::manifest::MODEL_MEDIA_TYPE, MODEL_PATH),
    (super::super::manifest::TEMPLATE_MEDIA_TYPE, TEMPLATE_PATH),
    (super::super::manifest::LICENSE_MEDIA_TYPE, LICENSE_PATH),
    (super::super::manifest::PARAMS_MEDIA_TYPE, PARAMETERS_PATH),
];

pub(super) fn artifact_set(plan: &OllamaManifestPlan) -> ReconstructionResult<ArtifactSetManifest> {
    ArtifactSetManifest::new(vec![
        artifact_member(plan.config(), CONFIG_PATH)?,
        artifact_member(plan.parameters(), PARAMETERS_PATH)?,
        artifact_member(plan.license(), LICENSE_PATH)?,
        artifact_member(plan.model(), MODEL_PATH)?,
        artifact_member(plan.template(), TEMPLATE_PATH)?,
        ArtifactSetMember::new(
            ArtifactId::from_digest(plan.raw_manifest_digest().clone()),
            plan.raw_manifest_size(),
            path(PROVENANCE_PATH)?,
        ),
    ])
    .map_err(|_| ReconstructionError::ModelContract)
}

pub(super) fn model_package(
    artifact_set: &ArtifactSetManifest,
    plan: &OllamaManifestPlan,
    source: PackageSource,
    transformation: PackageTransformation,
    gguf: &GgufObservation,
) -> ReconstructionResult<ModelPackageManifest> {
    let members = vec![
        package_member(
            plan.config(),
            CONFIG_PATH,
            ModelPackageMemberRole::AuxiliaryData,
        )?,
        package_member(
            plan.parameters(),
            PARAMETERS_PATH,
            ModelPackageMemberRole::GenerationConfiguration,
        )?,
        package_member(
            plan.license(),
            LICENSE_PATH,
            ModelPackageMemberRole::LicenseText,
        )?,
        package_member(
            plan.model(),
            MODEL_PATH,
            ModelPackageMemberRole::ModelWeights,
        )?,
        package_member(
            plan.template(),
            TEMPLATE_PATH,
            ModelPackageMemberRole::PromptTemplate,
        )?,
        ModelPackageMember::new(
            ArtifactId::from_digest(plan.raw_manifest_digest().clone()),
            plan.raw_manifest_size(),
            path(PROVENANCE_PATH)?,
            vec![ModelPackageMemberRole::ProvenanceRecord],
        ),
    ];
    let model_path = path(MODEL_PATH)?;
    let digests = gguf.component_digests();
    let embedded = vec![
        EmbeddedModelComponent::new(
            model_path.clone(),
            EmbeddedModelComponentPurpose::ModelConfiguration,
            "gguf-metadata",
            1,
            "model-load-configuration",
            digests.model_configuration().clone(),
        )
        .map_err(|_| ReconstructionError::ModelContract)?,
        EmbeddedModelComponent::new(
            model_path.clone(),
            EmbeddedModelComponentPurpose::Tokenizer,
            "gguf-metadata",
            1,
            "tokenizer-without-chat-template",
            digests.tokenizer().clone(),
        )
        .map_err(|_| ReconstructionError::ModelContract)?,
        EmbeddedModelComponent::new(
            model_path.clone(),
            EmbeddedModelComponentPurpose::PromptTemplate,
            "gguf-metadata",
            1,
            "tokenizer.chat_template",
            digests.prompt_template().clone(),
        )
        .map_err(|_| ReconstructionError::ModelContract)?,
    ];
    ModelPackageManifest::new(
        artifact_set,
        "ollama-manifest-v2",
        1,
        source,
        transformation,
        members,
        ModelWeightLayout::Single { member: model_path },
        embedded,
    )
    .map_err(|_| ReconstructionError::ModelContract)
}

pub(super) fn logical_binding_digest(plan: &OllamaManifestPlan) -> ReconstructionResult<Digest> {
    let mut hasher = Sha256::new();
    hasher.update(b"retonr:ollama-logical-binding:v1\0");
    append_digest(&mut hasher, plan.raw_manifest_digest());
    hasher.update(5_u64.to_be_bytes());
    for (descriptor, (media_type, logical_path)) in
        plan.ordered_descriptors().into_iter().zip(SOURCE_MAPPINGS)
    {
        if descriptor.media_type() != media_type {
            return Err(ReconstructionError::ModelContract);
        }
        append_text(&mut hasher, media_type)?;
        append_digest(&mut hasher, descriptor.digest());
        hasher.update(descriptor.size().to_be_bytes());
        append_text(&mut hasher, logical_path)?;
        append_digest(&mut hasher, descriptor.digest());
        hasher.update(descriptor.size().to_be_bytes());
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| ReconstructionError::ModelContract)
}

fn artifact_member(
    descriptor: &BlobDescriptor,
    relative_path: &str,
) -> ReconstructionResult<ArtifactSetMember> {
    Ok(ArtifactSetMember::new(
        ArtifactId::from_digest(descriptor.digest().clone()),
        descriptor.size(),
        path(relative_path)?,
    ))
}

fn package_member(
    descriptor: &BlobDescriptor,
    relative_path: &str,
    role: ModelPackageMemberRole,
) -> ReconstructionResult<ModelPackageMember> {
    Ok(ModelPackageMember::new(
        ArtifactId::from_digest(descriptor.digest().clone()),
        descriptor.size(),
        path(relative_path)?,
        vec![role],
    ))
}

fn path(value: &str) -> ReconstructionResult<ArtifactSetRelativePath> {
    ArtifactSetRelativePath::new(value).map_err(|_| ReconstructionError::ModelContract)
}

fn append_text(hasher: &mut Sha256, value: &str) -> ReconstructionResult<()> {
    hasher.update(
        u64::try_from(value.len())
            .map_err(|_| ReconstructionError::ModelContract)?
            .to_be_bytes(),
    );
    hasher.update(value.as_bytes());
    Ok(())
}

fn append_digest(hasher: &mut Sha256, digest: &Digest) {
    hasher.update(digest.as_str().as_bytes());
}
