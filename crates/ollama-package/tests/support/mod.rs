use std::collections::HashMap;

use rewrite_types::Digest;

pub const TEMPLATE: &[u8] = b"{{ range .Messages }}{{ .Content }}{{ end }}";
pub const LICENSE: &[u8] = b"Fixture license text\n";
pub const PARAMETERS: &[u8] = br#"{"temperature":0.2}"#;

#[derive(Clone)]
pub enum MetadataValue {
    U32(u32),
    String(Vec<u8>),
    StringArray(Vec<Vec<u8>>),
    Raw { kind: u32, bytes: Vec<u8> },
}

#[derive(Clone)]
pub struct Tensor {
    pub name: Vec<u8>,
    pub dimensions: Vec<u64>,
    pub kind: u32,
    pub offset: u64,
}

pub fn base_metadata() -> Vec<(Vec<u8>, MetadataValue)> {
    vec![
        (
            b"general.architecture".to_vec(),
            MetadataValue::String(b"qwen3".to_vec()),
        ),
        (b"general.alignment".to_vec(), MetadataValue::U32(32)),
        (b"general.file_type".to_vec(), MetadataValue::U32(7)),
        (
            b"general.quantization_version".to_vec(),
            MetadataValue::U32(2),
        ),
        (
            b"general.parameter_count".to_vec(),
            MetadataValue::Raw {
                kind: 10,
                bytes: 600_000_000_u64.to_le_bytes().to_vec(),
            },
        ),
        (b"qwen3.block_count".to_vec(), MetadataValue::U32(28)),
        (
            b"tokenizer.ggml.model".to_vec(),
            MetadataValue::String(b"bpe".to_vec()),
        ),
        (
            b"tokenizer.ggml.tokens".to_vec(),
            MetadataValue::StringArray(vec![b"a".to_vec(), b"b".to_vec()]),
        ),
        (
            b"tokenizer.chat_template".to_vec(),
            MetadataValue::String(b"{{ .Messages }}".to_vec()),
        ),
    ]
}

pub fn base_tensors() -> Vec<Tensor> {
    vec![
        Tensor {
            name: b"blk.0.weight".to_vec(),
            dimensions: vec![2, 2],
            kind: 0,
            offset: 0,
        },
        Tensor {
            name: b"output.weight".to_vec(),
            dimensions: vec![2, 2],
            kind: 2,
            offset: 32,
        },
    ]
}

pub fn build_gguf(
    metadata: &[(Vec<u8>, MetadataValue)],
    tensors: &[Tensor],
    data_bytes: usize,
) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"GGUF");
    output.extend_from_slice(&3_u32.to_le_bytes());
    output.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
    output.extend_from_slice(&(metadata.len() as u64).to_le_bytes());
    for (key, value) in metadata {
        push_string(&mut output, key);
        match value {
            MetadataValue::U32(value) => {
                output.extend_from_slice(&4_u32.to_le_bytes());
                output.extend_from_slice(&value.to_le_bytes());
            }
            MetadataValue::String(value) => {
                output.extend_from_slice(&8_u32.to_le_bytes());
                push_string(&mut output, value);
            }
            MetadataValue::StringArray(values) => {
                output.extend_from_slice(&9_u32.to_le_bytes());
                output.extend_from_slice(&8_u32.to_le_bytes());
                output.extend_from_slice(&(values.len() as u64).to_le_bytes());
                for value in values {
                    push_string(&mut output, value);
                }
            }
            MetadataValue::Raw { kind, bytes } => {
                output.extend_from_slice(&kind.to_le_bytes());
                output.extend_from_slice(bytes);
            }
        }
    }
    for tensor in tensors {
        push_string(&mut output, &tensor.name);
        output.extend_from_slice(
            &u32::try_from(tensor.dimensions.len())
                .expect("fixture dimension count fits u32")
                .to_le_bytes(),
        );
        for dimension in &tensor.dimensions {
            output.extend_from_slice(&dimension.to_le_bytes());
        }
        output.extend_from_slice(&tensor.kind.to_le_bytes());
        output.extend_from_slice(&tensor.offset.to_le_bytes());
    }
    while output.len() % 32 != 0 {
        output.push(0);
    }
    output.resize(output.len() + data_bytes, 0x5a);
    output
}

pub fn tiny_gguf() -> Vec<u8> {
    build_gguf(&base_metadata(), &base_tensors(), 64)
}

pub struct PackageFixture {
    pub manifest: Vec<u8>,
    pub blobs: HashMap<String, Vec<u8>>,
}

pub fn package_fixture() -> PackageFixture {
    let model = tiny_gguf();
    let model_digest = Digest::sha256(&model);
    let template_digest = Digest::sha256(TEMPLATE);
    let license_digest = Digest::sha256(LICENSE);
    let parameters_digest = Digest::sha256(PARAMETERS);
    let mismatched_template = Digest::sha256(b"informational mismatch");
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
                prefixed(&mismatched_template),
                prefixed(&license_digest),
                prefixed(&parameters_digest)
            ]
        }
    }))
    .expect("fixture config serializes");
    let config_digest = Digest::sha256(&config);
    let descriptors: [(&str, &[u8], &Digest); 4] = [
        (
            "application/vnd.ollama.image.model",
            model.as_slice(),
            &model_digest,
        ),
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

fn push_string(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value);
}

fn prefixed(digest: &Digest) -> String {
    format!("sha256:{}", digest.as_str())
}
