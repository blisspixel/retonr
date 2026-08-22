use serde::Deserialize;

use rewrite_types::Digest;

use crate::error::{ReconstructionError, ReconstructionResult};
use crate::gguf::GgufLimits;
use crate::json::validate_unique_json;

/// Exact supported Ollama manifest media type.
pub const MANIFEST_MEDIA_TYPE: &str = "application/vnd.docker.distribution.manifest.v2+json";
/// Exact supported Ollama configuration media type.
pub const CONFIG_MEDIA_TYPE: &str = "application/vnd.docker.container.image.v1+json";
/// Exact supported Ollama model-layer media type.
pub const MODEL_MEDIA_TYPE: &str = "application/vnd.ollama.image.model";
/// Exact supported Ollama explicit-template media type.
pub const TEMPLATE_MEDIA_TYPE: &str = "application/vnd.ollama.image.template";
/// Exact supported Ollama license-layer media type.
pub const LICENSE_MEDIA_TYPE: &str = "application/vnd.ollama.image.license";
/// Exact supported Ollama parameters-layer media type.
pub const PARAMS_MEDIA_TYPE: &str = "application/vnd.ollama.image.params";

const DEFAULT_MANIFEST_BYTES: usize = 128 * 1024;
const DEFAULT_CONFIG_BYTES: u64 = 1024 * 1024;
const DEFAULT_MODEL_BYTES: u64 = 128 * 1024 * 1024 * 1024;
const DEFAULT_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// Fixed ceilings applied before package bytes are allocated or parsed.
///
/// Explicit values may lower the defaults. Values above a default are rejected
/// and cannot relax the crate's absolute hard limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconstructionLimits {
    /// Maximum raw manifest JSON bytes.
    pub manifest_bytes: usize,
    /// Maximum configuration JSON bytes.
    pub config_bytes: u64,
    /// Maximum GGUF model bytes.
    pub model_bytes: u64,
    /// Maximum bytes in each template, license, or parameters layer.
    pub text_layer_bytes: u64,
    /// GGUF structural parser limits.
    pub gguf: GgufLimits,
}

impl Default for ReconstructionLimits {
    fn default() -> Self {
        Self {
            manifest_bytes: DEFAULT_MANIFEST_BYTES,
            config_bytes: DEFAULT_CONFIG_BYTES,
            model_bytes: DEFAULT_MODEL_BYTES,
            text_layer_bytes: DEFAULT_TEXT_BYTES,
            gguf: GgufLimits::default(),
        }
    }
}

impl ReconstructionLimits {
    /// Validates that every configured ceiling is nonzero and no greater than
    /// the crate's fixed defaults.
    ///
    /// Call this before using a ceiling to allocate or read caller-controlled
    /// input outside this crate.
    ///
    /// # Errors
    ///
    /// Returns [`ReconstructionError::LimitExceeded`] when any ceiling is zero,
    /// exceeds its fixed default, or contains invalid GGUF limits.
    pub fn validate(self) -> ReconstructionResult<Self> {
        validate_limits(&self)?;
        Ok(self)
    }
}

/// One exact content-addressed Ollama blob descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlobDescriptor {
    media_type: &'static str,
    digest: Digest,
    size: u64,
}

impl BlobDescriptor {
    /// Returns the exact supported media type.
    #[must_use]
    pub const fn media_type(&self) -> &'static str {
        self.media_type
    }

    /// Returns the descriptor's lowercase SHA-256 digest without its algorithm prefix.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Returns the exact declared blob length.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }
}

/// Strict plan for one supported Ollama manifest-v2 package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaManifestPlan {
    raw_manifest_digest: Digest,
    raw_manifest_size: u64,
    config: BlobDescriptor,
    model: BlobDescriptor,
    template: BlobDescriptor,
    license: BlobDescriptor,
    parameters: BlobDescriptor,
}

impl OllamaManifestPlan {
    /// Returns the exact raw manifest identity retained as provenance.
    #[must_use]
    pub const fn raw_manifest_digest(&self) -> &Digest {
        &self.raw_manifest_digest
    }

    /// Returns the exact raw manifest byte length.
    #[must_use]
    pub const fn raw_manifest_size(&self) -> u64 {
        self.raw_manifest_size
    }

    /// Returns the exact configuration descriptor.
    #[must_use]
    pub const fn config(&self) -> &BlobDescriptor {
        &self.config
    }

    /// Returns the exact GGUF model descriptor.
    #[must_use]
    pub const fn model(&self) -> &BlobDescriptor {
        &self.model
    }

    /// Returns the exact explicit-template descriptor.
    #[must_use]
    pub const fn template(&self) -> &BlobDescriptor {
        &self.template
    }

    /// Returns the exact license descriptor.
    #[must_use]
    pub const fn license(&self) -> &BlobDescriptor {
        &self.license
    }

    /// Returns the exact parameters descriptor.
    #[must_use]
    pub const fn parameters(&self) -> &BlobDescriptor {
        &self.parameters
    }

    pub(crate) fn ordered_descriptors(&self) -> [&BlobDescriptor; 5] {
        [
            &self.config,
            &self.model,
            &self.template,
            &self.license,
            &self.parameters,
        ]
    }

    pub(crate) fn layer_descriptors(&self) -> [&BlobDescriptor; 4] {
        [&self.model, &self.template, &self.license, &self.parameters]
    }
}

/// Parses a strict, byte-bounded Ollama manifest-v2 document.
///
/// Exactly one model, template, license, and parameters layer is supported, in
/// that order. Descriptors must use lowercase `sha256:` identities.
///
/// # Errors
///
/// Returns [`ReconstructionError`] for malformed, ambiguous, oversized, or
/// unsupported input.
pub fn parse_manifest_v2(
    bytes: &[u8],
    limits: &ReconstructionLimits,
) -> ReconstructionResult<OllamaManifestPlan> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct DescriptorWire {
        media_type: String,
        digest: String,
        size: u64,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "camelCase")]
    struct ManifestWire {
        schema_version: u32,
        media_type: String,
        config: DescriptorWire,
        layers: Vec<DescriptorWire>,
    }

    limits.validate()?;
    if bytes.len() > limits.manifest_bytes {
        return Err(ReconstructionError::ManifestTooLarge);
    }
    validate_unique_json(bytes).map_err(|()| ReconstructionError::InvalidManifest)?;
    let wire: ManifestWire =
        serde_json::from_slice(bytes).map_err(|_| ReconstructionError::InvalidManifest)?;
    if wire.schema_version != 2 || wire.media_type != MANIFEST_MEDIA_TYPE || wire.layers.len() != 4
    {
        return Err(ReconstructionError::UnsupportedManifest);
    }

    let config = descriptor(
        &wire.config.media_type,
        &wire.config.digest,
        wire.config.size,
        CONFIG_MEDIA_TYPE,
        limits.config_bytes,
    )?;
    let mut layers = wire.layers.into_iter();
    let next = layers
        .next()
        .ok_or(ReconstructionError::UnsupportedManifest)?;
    let model = descriptor(
        &next.media_type,
        &next.digest,
        next.size,
        MODEL_MEDIA_TYPE,
        limits.model_bytes,
    )?;
    let next = layers
        .next()
        .ok_or(ReconstructionError::UnsupportedManifest)?;
    let template = descriptor(
        &next.media_type,
        &next.digest,
        next.size,
        TEMPLATE_MEDIA_TYPE,
        limits.text_layer_bytes,
    )?;
    let next = layers
        .next()
        .ok_or(ReconstructionError::UnsupportedManifest)?;
    let license = descriptor(
        &next.media_type,
        &next.digest,
        next.size,
        LICENSE_MEDIA_TYPE,
        limits.text_layer_bytes,
    )?;
    let next = layers
        .next()
        .ok_or(ReconstructionError::UnsupportedManifest)?;
    let parameters = descriptor(
        &next.media_type,
        &next.digest,
        next.size,
        PARAMS_MEDIA_TYPE,
        limits.text_layer_bytes,
    )?;
    Ok(OllamaManifestPlan {
        raw_manifest_digest: Digest::sha256(bytes),
        raw_manifest_size: u64::try_from(bytes.len())
            .map_err(|_| ReconstructionError::LimitExceeded)?,
        config,
        model,
        template,
        license,
        parameters,
    })
}

fn validate_limits(limits: &ReconstructionLimits) -> ReconstructionResult<()> {
    if limits.manifest_bytes == 0
        || limits.manifest_bytes > DEFAULT_MANIFEST_BYTES
        || limits.config_bytes == 0
        || limits.config_bytes > DEFAULT_CONFIG_BYTES
        || limits.model_bytes == 0
        || limits.model_bytes > DEFAULT_MODEL_BYTES
        || limits.text_layer_bytes == 0
        || limits.text_layer_bytes > DEFAULT_TEXT_BYTES
        || !limits.gguf.within_hard_limits()
    {
        Err(ReconstructionError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn descriptor(
    media_type: &str,
    encoded_digest: &str,
    size: u64,
    expected_media_type: &'static str,
    maximum_size: u64,
) -> ReconstructionResult<BlobDescriptor> {
    if media_type != expected_media_type || size == 0 || size > maximum_size {
        return Err(ReconstructionError::InvalidDescriptor);
    }
    let hex = encoded_digest
        .strip_prefix("sha256:")
        .ok_or(ReconstructionError::InvalidDescriptor)?;
    let digest = Digest::from_sha256_hex(hex.to_owned())
        .map_err(|_| ReconstructionError::InvalidDescriptor)?;
    Ok(BlobDescriptor {
        media_type: expected_media_type,
        digest,
        size,
    })
}
