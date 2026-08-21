use std::{collections::BTreeMap, fs};

use rewrite_app::{
    ArtifactRepository, ArtifactSetImportLimits, InstalledOllamaModelSource,
    OllamaModelImportLimits, OllamaModelImportResult, OllamaModelReference,
};
use rewrite_model::RuntimeIdentity;
use rewrite_ollama::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
};
use rewrite_ollama_package::ReconstructionLimits;
use rewrite_types::{CancellationToken, Digest};
use tempfile::TempDir;

use crate::{
    LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaModelPlan, LocalOllamaPreflightMode,
    LocalOllamaPreflightPlan, LocalOllamaPreflightReport,
    local_ollama_preflight::local_ollama_preflight_report,
};

pub(super) struct ImportedFixture {
    pub(super) _root: TempDir,
    pub(super) result: OllamaModelImportResult,
    pub(super) reference: OllamaModelReference,
    pub(super) manifest_digest: Digest,
    pub(super) inventory_size: u64,
    pub(super) model_digest: Digest,
    pub(super) explicit_template_digest: Digest,
    pub(super) embedded_template_digest: Digest,
    pub(super) license_digest: Digest,
}

pub(super) fn import_fixture(
    parameter_value: f64,
    explicit_template: &[u8],
    embedded_template: &[u8],
) -> ImportedFixture {
    let root = tempfile::tempdir().expect("temporary fixture root");
    let package = package_fixture(parameter_value, explicit_template, embedded_template);
    let models = root.path().join("models");
    let manifest_path = models.join("manifests/registry.ollama.ai/library/qwen3/latest");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest hierarchy");
    fs::write(&manifest_path, &package.manifest).expect("write package manifest");
    let blobs_root = models.join("blobs");
    fs::create_dir(&blobs_root).expect("create blobs root");
    for (digest, bytes) in &package.blobs {
        fs::write(blobs_root.join(format!("sha256-{digest}")), bytes).expect("write package blob");
    }
    let reference = OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "latest")
        .expect("fixture reference");
    let selection = InstalledOllamaModelSource::new(models, reference.clone())
        .expect("fixture source selection");
    let repository = ArtifactRepository::new(root.path().join("data")).expect("repository");
    let result = repository
        .import_installed_ollama_model(
            &selection,
            OllamaModelImportLimits {
                reconstruction: ReconstructionLimits::default(),
                artifact_set: ArtifactSetImportLimits {
                    maximum_members: 6,
                    maximum_member_bytes: 1024 * 1024,
                    maximum_total_bytes: 2 * 1024 * 1024,
                    maximum_tree_entries: 16,
                    maximum_storage_entries: 8,
                    maximum_staging_entries: 8,
                },
            },
            &CancellationToken::new(),
        )
        .expect("fixture import");
    ImportedFixture {
        _root: root,
        result,
        reference,
        manifest_digest: Digest::sha256(&package.manifest),
        inventory_size: package.blobs.values().map(Vec::len).sum::<usize>() as u64,
        model_digest: package.model_digest,
        explicit_template_digest: Digest::sha256(explicit_template),
        embedded_template_digest: Digest::sha256(embedded_template),
        license_digest: package.license_digest,
    }
}

pub(super) fn verified_preflight(
    fixture: &ImportedFixture,
    inventory_digest: Digest,
    inventory_size: u64,
    template_digest: Digest,
) -> (LocalOllamaPreflightPlan, LocalOllamaPreflightReport) {
    let details = OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "qwen3".to_owned(),
        quantization: "F32".to_owned(),
        capabilities: vec!["completion".to_owned()],
        license_digest: fixture.license_digest.clone(),
        template_digest,
        metadata_digest: Digest::sha256(b"frozen model metadata"),
    };
    let reference = fixture.reference.runtime_reference();
    let plan = LocalOllamaPreflightPlan {
        schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
        plan_id: "fixture".to_owned(),
        mode: LocalOllamaPreflightMode::Verify,
        endpoint: "http://127.0.0.1:11434".to_owned(),
        expected_runtime_version: "0.32.15".to_owned(),
        require_idle: true,
        models: vec![LocalOllamaModelPlan {
            reference: reference.clone(),
            inventory_digest: inventory_digest.clone(),
            expected_details: Some(details.clone()),
        }],
    };
    let observed = OllamaPreflight {
        runtime: RuntimeIdentity {
            backend: "ollama_native".to_owned(),
            version: "0.32.15".to_owned(),
            digest: None,
        },
        inventory: vec![OllamaInventoryEntry {
            reference: reference.clone(),
            inventory_digest: inventory_digest.clone(),
            byte_size: inventory_size,
        }],
        bindings: vec![OllamaPreflightBinding {
            reference,
            inventory_digest,
            details,
        }],
        running: Vec::new(),
    };
    let report = local_ollama_preflight_report(&plan, observed).expect("valid fixture report");
    (plan, report)
}

struct PackageFixture {
    manifest: Vec<u8>,
    blobs: BTreeMap<String, Vec<u8>>,
    model_digest: Digest,
    license_digest: Digest,
}

fn package_fixture(
    parameter_value: f64,
    explicit_template: &[u8],
    embedded_template: &[u8],
) -> PackageFixture {
    const LICENSE: &[u8] = b"Fixture license text\n";
    let parameters = serde_json::to_vec(&serde_json::json!({"temperature": parameter_value}))
        .expect("parameters serialize");
    let model = tiny_gguf(embedded_template);
    let model_digest = Digest::sha256(&model);
    let template_digest = Digest::sha256(explicit_template);
    let license_digest = Digest::sha256(LICENSE);
    let parameters_digest = Digest::sha256(&parameters);
    let config = config(
        &model_digest,
        &template_digest,
        &license_digest,
        &parameters_digest,
    );
    let config_digest = Digest::sha256(&config);
    let descriptors: [(&str, &[u8], &Digest); 4] = [
        ("application/vnd.ollama.image.model", &model, &model_digest),
        (
            "application/vnd.ollama.image.template",
            explicit_template,
            &template_digest,
        ),
        (
            "application/vnd.ollama.image.license",
            LICENSE,
            &license_digest,
        ),
        (
            "application/vnd.ollama.image.params",
            &parameters,
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
    .expect("manifest serializes");
    let blobs = [
        (config_digest.as_str().to_owned(), config),
        (model_digest.as_str().to_owned(), model),
        (
            template_digest.as_str().to_owned(),
            explicit_template.to_vec(),
        ),
        (license_digest.as_str().to_owned(), LICENSE.to_vec()),
        (parameters_digest.as_str().to_owned(), parameters),
    ]
    .into_iter()
    .collect();
    PackageFixture {
        manifest,
        blobs,
        model_digest,
        license_digest,
    }
}

fn config(model: &Digest, template: &Digest, license: &Digest, parameters: &Digest) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "model_format": "gguf",
        "model_family": "qwen3",
        "model_families": ["qwen3"],
        "model_type": "fixture",
        "file_type": "F32",
        "architecture": "amd64",
        "os": "windows",
        "rootfs": {
            "type": "layers",
            "diff_ids": [prefixed(model), prefixed(template), prefixed(license), prefixed(parameters)]
        }
    }))
    .expect("config serializes")
}

fn tiny_gguf(template: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"GGUF");
    output.extend_from_slice(&3_u32.to_le_bytes());
    output.extend_from_slice(&1_u64.to_le_bytes());
    output.extend_from_slice(&9_u64.to_le_bytes());
    push_string_metadata(&mut output, b"general.architecture", b"qwen3");
    push_u32_metadata(&mut output, b"general.alignment", 32);
    push_u32_metadata(&mut output, b"general.file_type", 0);
    push_u32_metadata(&mut output, b"general.quantization_version", 2);
    push_u64_metadata(&mut output, b"general.parameter_count", 16);
    push_u32_metadata(&mut output, b"qwen3.block_count", 1);
    push_string_metadata(&mut output, b"tokenizer.ggml.model", b"bpe");
    push_string_array(&mut output, b"tokenizer.ggml.tokens", &[b"a", b"b"]);
    push_string_metadata(&mut output, b"tokenizer.chat_template", template);
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

fn push_string_metadata(output: &mut Vec<u8>, key: &[u8], value: &[u8]) {
    push_string(output, key);
    output.extend_from_slice(&8_u32.to_le_bytes());
    push_string(output, value);
}

fn push_u32_metadata(output: &mut Vec<u8>, key: &[u8], value: u32) {
    push_string(output, key);
    output.extend_from_slice(&4_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64_metadata(output: &mut Vec<u8>, key: &[u8], value: u64) {
    push_string(output, key);
    output.extend_from_slice(&10_u32.to_le_bytes());
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_string_array(output: &mut Vec<u8>, key: &[u8], values: &[&[u8]]) {
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
