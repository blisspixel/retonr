use std::{collections::BTreeMap, fs, path::Path};

use rewrite_ollama_package::ReconstructionLimits;
use rewrite_types::Digest;

use super::super::{InstalledOllamaModelSource, OllamaModelImportLimits, OllamaModelReference};

pub(super) struct InstalledFixture {
    pub(super) selection: InstalledOllamaModelSource,
    #[cfg(unix)]
    pub(super) manifest_path: std::path::PathBuf,
    pub(super) blob_paths: Vec<std::path::PathBuf>,
}

pub(super) fn import_limits() -> OllamaModelImportLimits {
    OllamaModelImportLimits {
        reconstruction: ReconstructionLimits::default(),
        artifact_set: crate::ArtifactSetImportLimits {
            maximum_members: 6,
            maximum_member_bytes: 1024 * 1024,
            maximum_total_bytes: 2 * 1024 * 1024,
            maximum_tree_entries: 16,
            maximum_storage_entries: 8,
            maximum_staging_entries: 8,
        },
    }
}

pub(super) fn write_installed_fixture(root: &Path) -> InstalledFixture {
    let package = package_fixture();
    let models = root.join("models");
    let manifest_path = models.join("manifests/registry.ollama.ai/library/qwen3/latest");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest hierarchy");
    fs::write(&manifest_path, &package.manifest).expect("write package manifest");
    let blobs_root = models.join("blobs");
    fs::create_dir(&blobs_root).expect("create blobs root");
    let mut blob_paths = Vec::new();
    for (digest, bytes) in package.blobs {
        let path = blobs_root.join(format!("sha256-{digest}"));
        fs::write(&path, bytes).expect("write package blob");
        blob_paths.push(path);
    }
    blob_paths.sort();
    let reference = OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "latest")
        .expect("fixture reference");
    let selection =
        InstalledOllamaModelSource::new(models, reference).expect("fixture source selection");
    InstalledFixture {
        selection,
        #[cfg(unix)]
        manifest_path,
        blob_paths,
    }
}

struct PackageFixture {
    manifest: Vec<u8>,
    blobs: BTreeMap<String, Vec<u8>>,
}

fn package_fixture() -> PackageFixture {
    const TEMPLATE: &[u8] = b"{{ range .Messages }}{{ .Content }}{{ end }}";
    const LICENSE: &[u8] = b"Fixture license text\n";
    const PARAMETERS: &[u8] = br#"{"temperature":0.2}"#;
    let model = tiny_gguf();
    let model_digest = Digest::sha256(&model);
    let template_digest = Digest::sha256(TEMPLATE);
    let license_digest = Digest::sha256(LICENSE);
    let parameters_digest = Digest::sha256(PARAMETERS);
    let config = serde_json::to_vec(&serde_json::json!({
        "model_format": "gguf",
        "model_family": "qwen3",
        "model_families": ["qwen3"],
        "model_type": "fixture",
        "file_type": "F32",
        "architecture": "amd64",
        "os": "windows",
        "rootfs": {
            "type": "layers",
            "diff_ids": [
                prefixed(&model_digest),
                prefixed(&Digest::sha256(b"informational mismatch")),
                prefixed(&license_digest),
                prefixed(&parameters_digest)
            ]
        }
    }))
    .expect("fixture config serializes");
    let config_digest = Digest::sha256(&config);
    let descriptors: [(&str, &[u8], &Digest); 4] = [
        ("application/vnd.ollama.image.model", &model, &model_digest),
        (
            "application/vnd.ollama.image.template",
            TEMPLATE,
            &template_digest,
        ),
        (
            "application/vnd.ollama.image.license",
            LICENSE,
            &license_digest,
        ),
        (
            "application/vnd.ollama.image.params",
            PARAMETERS,
            &parameters_digest,
        ),
    ];
    let layers = descriptors
        .iter()
        .map(|(media_type, bytes, digest)| {
            serde_json::json!({
                "mediaType": media_type,
                "digest": prefixed(digest),
                "size": bytes.len()
            })
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "config": {
            "mediaType": "application/vnd.docker.container.image.v1+json",
            "digest": prefixed(&config_digest),
            "size": config.len()
        },
        "layers": layers
    }))
    .expect("fixture manifest serializes");
    let blobs = [
        (config_digest.as_str().to_owned(), config),
        (model_digest.as_str().to_owned(), model),
        (template_digest.as_str().to_owned(), TEMPLATE.to_vec()),
        (license_digest.as_str().to_owned(), LICENSE.to_vec()),
        (parameters_digest.as_str().to_owned(), PARAMETERS.to_vec()),
    ]
    .into_iter()
    .collect();
    PackageFixture { manifest, blobs }
}

fn tiny_gguf() -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"GGUF");
    output.extend_from_slice(&3_u32.to_le_bytes());
    output.extend_from_slice(&1_u64.to_le_bytes());
    output.extend_from_slice(&9_u64.to_le_bytes());
    push_metadata_string(&mut output, b"general.architecture", b"qwen3");
    push_metadata_u32(&mut output, b"general.alignment", 32);
    push_metadata_u32(&mut output, b"general.file_type", 0);
    push_metadata_u32(&mut output, b"general.quantization_version", 2);
    push_metadata_u64(&mut output, b"general.parameter_count", 16);
    push_metadata_u32(&mut output, b"qwen3.block_count", 1);
    push_metadata_string(&mut output, b"tokenizer.ggml.model", b"bpe");
    push_metadata_string_array(&mut output, b"tokenizer.ggml.tokens", &[b"a", b"b"]);
    push_metadata_string(&mut output, b"tokenizer.chat_template", b"{{ .Messages }}");
    push_string(&mut output, b"output.weight");
    output.extend_from_slice(&1_u32.to_le_bytes());
    output.extend_from_slice(&4_u64.to_le_bytes());
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&0_u64.to_le_bytes());
    while output.len() % 32 != 0 {
        output.push(0);
    }
    output.resize(output.len() + 16, 0x5a);
    output
}

fn push_metadata_string(output: &mut Vec<u8>, key: &[u8], value: &[u8]) {
    push_string(output, key);
    output.extend_from_slice(&8_u32.to_le_bytes());
    push_string(output, value);
}

fn push_metadata_u32(output: &mut Vec<u8>, key: &[u8], value: u32) {
    push_string(output, key);
    output.extend_from_slice(&4_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_metadata_u64(output: &mut Vec<u8>, key: &[u8], value: u64) {
    push_string(output, key);
    output.extend_from_slice(&10_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_metadata_string_array(output: &mut Vec<u8>, key: &[u8], values: &[&[u8]]) {
    push_string(output, key);
    output.extend_from_slice(&9_u32.to_le_bytes());
    output.extend_from_slice(&8_u32.to_le_bytes());
    output.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for value in values {
        push_string(output, value);
    }
}

fn push_string(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
}

fn prefixed(digest: &Digest) -> String {
    format!("sha256:{}", digest.as_str())
}
