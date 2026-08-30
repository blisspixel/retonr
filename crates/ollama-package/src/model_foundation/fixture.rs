use std::io::Cursor;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    EmbeddedModelComponent, ModelPackageManifest, ModelPackageMember, ModelPackageMemberRole,
    PackageSource, PackageTransformation,
};
use rewrite_types::Digest;
use serde_json::json;

use crate::{
    BlobOpenError, CONFIG_MEDIA_TYPE, LICENSE_MEDIA_TYPE, MANIFEST_MEDIA_TYPE, MODEL_MEDIA_TYPE,
    PARAMS_MEDIA_TYPE, ReconstructionLimits, TEMPLATE_MEDIA_TYPE,
    reconstruct_model_package_with_limits,
};

pub(super) struct Fixture {
    pub(super) raw_manifest: Vec<u8>,
    pub(super) blobs: Vec<(Digest, Vec<u8>)>,
    pub(super) artifact_set: ArtifactSetManifest,
    pub(super) package: ModelPackageManifest,
}

pub(super) fn fixture(shared_license_and_template: bool) -> Fixture {
    let config = br#"{"model_format":"gguf","model_family":"fixture","model_families":["fixture"],"model_type":"fixture","file_type":"fixture","architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#.to_vec();
    let model = minimal_gguf();
    let template = b"{{ .Prompt }}".to_vec();
    let license = if shared_license_and_template {
        template.clone()
    } else {
        b"fixture model license".to_vec()
    };
    let parameters = br#"{"temperature":0}"#.to_vec();
    let raw_manifest = serde_json::to_vec(&json!({
        "schemaVersion": 2,
        "mediaType": MANIFEST_MEDIA_TYPE,
        "config": descriptor(CONFIG_MEDIA_TYPE, &config),
        "layers": [
            descriptor(MODEL_MEDIA_TYPE, &model),
            descriptor(TEMPLATE_MEDIA_TYPE, &template),
            descriptor(LICENSE_MEDIA_TYPE, &license),
            descriptor(PARAMS_MEDIA_TYPE, &parameters)
        ]
    }))
    .expect("manifest JSON");
    let mut blobs = vec![
        (Digest::sha256(&config), config),
        (Digest::sha256(&model), model),
        (Digest::sha256(&template), template),
        (Digest::sha256(&license), license),
        (Digest::sha256(&parameters), parameters),
    ];
    blobs.dedup_by(|left, right| left.0 == right.0);
    let reconstructed = reconstruct_model_package_with_limits(
        &raw_manifest,
        "registry.ollama.ai/library/fixture",
        &ReconstructionLimits::default(),
        |digest| open_blob(&blobs, digest),
        || false,
    )
    .expect("reconstruct fixture");
    Fixture {
        raw_manifest,
        blobs,
        artifact_set: reconstructed.artifact_set().clone(),
        package: reconstructed.model_package().clone(),
    }
}

pub(super) fn rebuild_package(
    fixture: &Fixture,
    source: PackageSource,
    transformation: PackageTransformation,
    artifact_set: &ArtifactSetManifest,
    members: Vec<ModelPackageMember>,
    embedded: Vec<EmbeddedModelComponent>,
) -> ModelPackageManifest {
    ModelPackageManifest::new(
        artifact_set,
        fixture.package.format_contract_id(),
        fixture.package.format_contract_schema_version(),
        source,
        transformation,
        members,
        fixture.package.weight_layout().clone(),
        embedded,
    )
    .expect("valid substituted package")
}

pub(super) fn transformed_artifact_set(fixture: &Fixture) -> ArtifactSetManifest {
    let mut members = fixture.artifact_set.members().to_vec();
    members.push(ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(b"transformation evidence")),
        b"transformation evidence".len() as u64,
        artifact_path("review/transformation.json"),
    ));
    ArtifactSetManifest::new(members).expect("transformed artifact set")
}

pub(super) fn transformed_members(fixture: &Fixture) -> Vec<ModelPackageMember> {
    let set = transformed_artifact_set(fixture);
    let mut members = fixture.package.members().to_vec();
    let artifact = set.members().last().expect("transformation member");
    members.push(ModelPackageMember::new(
        artifact.artifact_id().clone(),
        artifact.byte_size(),
        artifact.relative_path().clone(),
        vec![ModelPackageMemberRole::TransformationEvidence],
    ));
    members
}

pub(super) fn transformed_embedded(fixture: &Fixture) -> Vec<EmbeddedModelComponent> {
    fixture.package.embedded_components().to_vec()
}

pub(super) fn open_blob(
    blobs: &[(Digest, Vec<u8>)],
    digest: &Digest,
) -> Result<Cursor<Vec<u8>>, BlobOpenError> {
    blobs
        .iter()
        .find(|(candidate, _bytes)| candidate == digest)
        .map(|(_digest, bytes)| Cursor::new(bytes.clone()))
        .ok_or(BlobOpenError)
}

fn descriptor(media_type: &str, bytes: &[u8]) -> serde_json::Value {
    json!({
        "mediaType": media_type,
        "digest": format!("sha256:{}", Digest::sha256(bytes).as_str()),
        "size": bytes.len()
    })
}

pub(super) fn artifact_path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value.to_owned()).expect("fixture path")
}

fn minimal_gguf() -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"GGUF");
    output.extend_from_slice(&3_u32.to_le_bytes());
    output.extend_from_slice(&1_u64.to_le_bytes());
    output.extend_from_slice(&6_u64.to_le_bytes());
    push_string_metadata(&mut output, "general.architecture", "llama");
    push_u32_metadata(&mut output, "general.file_type", 1);
    push_u32_metadata(&mut output, "general.quantization_version", 2);
    push_u64_metadata(&mut output, "general.parameter_count", 1);
    push_string_metadata(&mut output, "tokenizer.model", "fixture");
    push_string_metadata(&mut output, "tokenizer.chat_template", "{{ .Prompt }}");
    push_text(&mut output, "tensor");
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&1_u64.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&0_u64.to_le_bytes());
    while output.len() % 32 != 0 {
        output.push(0);
    }
    output.push(0);
    output
}

fn push_string_metadata(output: &mut Vec<u8>, key: &str, value: &str) {
    push_text(output, key);
    output.extend_from_slice(&8_u32.to_le_bytes());
    push_text(output, value);
}

fn push_u32_metadata(output: &mut Vec<u8>, key: &str, value: u32) {
    push_text(output, key);
    output.extend_from_slice(&4_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64_metadata(output: &mut Vec<u8>, key: &str, value: u64) {
    push_text(output, key);
    output.extend_from_slice(&10_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}
