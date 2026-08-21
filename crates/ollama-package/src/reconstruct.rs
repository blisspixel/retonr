use std::collections::BTreeSet;
use std::io::Read;

use rewrite_model::{
    ArtifactSetManifest, ModelPackageManifest, PackageSource, PackageSourceKind,
    PackageTransformation,
};
use rewrite_types::Digest;
use serde::Deserialize;

use crate::error::{BlobOpenError, ReconstructionError, ReconstructionResult};
use crate::gguf::{GgufObservation, inspect_gguf_v3};
use crate::json::validate_unique_json;
use crate::manifest::{
    BlobDescriptor, OllamaManifestPlan, ReconstructionLimits, parse_manifest_v2,
};

mod contract;

use contract::{artifact_set, logical_binding_digest, model_package};

/// Informational comparison between config `rootfs.diff_ids` and layer descriptors.
///
/// These booleans never select, open, or authorize a blob. The manifest
/// descriptors remain the sole blob authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootfsDescriptorComparison {
    same_cardinality: bool,
    matches_by_position: Vec<bool>,
}

impl RootfsDescriptorComparison {
    /// Returns whether the rootfs list and manifest layer list have equal lengths.
    #[must_use]
    pub const fn same_cardinality(&self) -> bool {
        self.same_cardinality
    }

    /// Returns one informational equality result per manifest layer position.
    #[must_use]
    pub fn matches_by_position(&self) -> &[bool] {
        &self.matches_by_position
    }

    /// Returns whether cardinality and every positional digest match.
    #[must_use]
    pub fn all_match(&self) -> bool {
        self.same_cardinality && self.matches_by_position.iter().all(|value| *value)
    }
}

/// Canonical manifests and observations reconstructed from exact package bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconstructedModelPackage {
    plan: OllamaManifestPlan,
    artifact_set: ArtifactSetManifest,
    model_package: ModelPackageManifest,
    gguf: GgufObservation,
    rootfs_comparison: RootfsDescriptorComparison,
}

impl ReconstructedModelPackage {
    /// Returns the strict source descriptor plan.
    #[must_use]
    pub const fn plan(&self) -> &OllamaManifestPlan {
        &self.plan
    }

    /// Returns the exact canonical artifact set.
    #[must_use]
    pub const fn artifact_set(&self) -> &ArtifactSetManifest {
        &self.artifact_set
    }

    /// Returns the exact semantic model package.
    #[must_use]
    pub const fn model_package(&self) -> &ModelPackageManifest {
        &self.model_package
    }

    /// Returns bounded GGUF structural and embedded-component evidence.
    #[must_use]
    pub const fn gguf(&self) -> &GgufObservation {
        &self.gguf
    }

    /// Returns the informational rootfs comparison.
    #[must_use]
    pub const fn rootfs_comparison(&self) -> &RootfsDescriptorComparison {
        &self.rootfs_comparison
    }
}

/// Reconstructs one exact supported package using default fixed limits.
///
/// The opener receives only validated descriptor digests. Each returned stream
/// is consumed once, checked for exact size and digest, and then dropped.
///
/// # Errors
///
/// Returns [`ReconstructionError`] for any malformed, missing, changed,
/// unsupported, over-budget, or cancelled input.
pub fn reconstruct_model_package<R, F, C>(
    raw_manifest: &[u8],
    source_locator: &str,
    open_blob: F,
    cancelled: C,
) -> ReconstructionResult<ReconstructedModelPackage>
where
    R: Read,
    F: FnMut(&Digest) -> Result<R, BlobOpenError>,
    C: FnMut() -> bool,
{
    reconstruct_model_package_with_limits(
        raw_manifest,
        source_locator,
        &ReconstructionLimits::default(),
        open_blob,
        cancelled,
    )
}

/// Reconstructs one exact supported package using explicit testable limits.
///
/// Explicit limits can only lower the hard defaults.
///
/// # Errors
///
/// Returns [`ReconstructionError`] for any malformed, missing, changed,
/// unsupported, over-budget, or cancelled input.
pub fn reconstruct_model_package_with_limits<R, F, C>(
    raw_manifest: &[u8],
    source_locator: &str,
    limits: &ReconstructionLimits,
    mut open_blob: F,
    mut cancelled: C,
) -> ReconstructionResult<ReconstructedModelPackage>
where
    R: Read,
    F: FnMut(&Digest) -> Result<R, BlobOpenError>,
    C: FnMut() -> bool,
{
    if cancelled() {
        return Err(ReconstructionError::Cancelled);
    }
    let plan = parse_manifest_v2(raw_manifest, limits)?;

    let config_bytes = read_small_blob(plan.config(), &mut open_blob, &mut cancelled)?;
    let config = parse_config(&config_bytes)?;

    let mut model_stream =
        open_blob(plan.model().digest()).map_err(|_| ReconstructionError::BlobUnavailable)?;
    let gguf = inspect_gguf_v3(
        &mut model_stream,
        plan.model().digest(),
        plan.model().size(),
        &limits.gguf,
        &mut cancelled,
    )?;

    let template_bytes = read_small_blob(plan.template(), &mut open_blob, &mut cancelled)?;
    validate_text(&template_bytes)?;
    let license_bytes = read_small_blob(plan.license(), &mut open_blob, &mut cancelled)?;
    validate_text(&license_bytes)?;
    let parameters_bytes = read_small_blob(plan.parameters(), &mut open_blob, &mut cancelled)?;
    validate_unique_json(&parameters_bytes).map_err(|()| ReconstructionError::InvalidJson)?;

    let artifact_set = artifact_set(&plan)?;
    let source = PackageSource::new(
        PackageSourceKind::LocalArchive,
        source_locator,
        format!("sha256:{}", plan.raw_manifest_digest().as_str()),
        plan.raw_manifest_digest().clone(),
    )
    .map_err(|_| ReconstructionError::ModelContract)?;
    let transformation = PackageTransformation::Untransformed {
        evidence_digest: logical_binding_digest(&plan)?,
    };
    let model_package = model_package(&artifact_set, &plan, source, transformation, &gguf)?;
    let rootfs_comparison = compare_rootfs(&config.rootfs.diff_ids, &plan);
    Ok(ReconstructedModelPackage {
        plan,
        artifact_set,
        model_package,
        gguf,
        rootfs_comparison,
    })
}

fn read_small_blob<R, F, C>(
    descriptor: &BlobDescriptor,
    open_blob: &mut F,
    cancelled: &mut C,
) -> ReconstructionResult<Vec<u8>>
where
    R: Read,
    F: FnMut(&Digest) -> Result<R, BlobOpenError>,
    C: FnMut() -> bool,
{
    let size =
        usize::try_from(descriptor.size()).map_err(|_| ReconstructionError::LimitExceeded)?;
    let mut reader =
        open_blob(descriptor.digest()).map_err(|_| ReconstructionError::BlobUnavailable)?;
    let mut bytes = Vec::with_capacity(size);
    let mut buffer = vec![0; 64 * 1024];
    while bytes.len() < size {
        if cancelled() {
            return Err(ReconstructionError::Cancelled);
        }
        let amount = (size - bytes.len()).min(buffer.len());
        reader.read_exact(&mut buffer[..amount]).map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                ReconstructionError::BlobSizeMismatch
            } else {
                ReconstructionError::InputRead
            }
        })?;
        bytes.extend_from_slice(&buffer[..amount]);
    }
    if cancelled() {
        return Err(ReconstructionError::Cancelled);
    }
    let mut trailing = [0; 1];
    match reader.read(&mut trailing) {
        Ok(0) => {}
        Ok(_) => return Err(ReconstructionError::BlobSizeMismatch),
        Err(_) => return Err(ReconstructionError::InputRead),
    }
    if &Digest::sha256(&bytes) != descriptor.digest() {
        return Err(ReconstructionError::BlobDigestMismatch);
    }
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigWire {
    model_format: String,
    model_family: String,
    model_families: Vec<String>,
    model_type: String,
    file_type: String,
    architecture: String,
    os: String,
    rootfs: RootfsWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootfsWire {
    #[serde(rename = "type")]
    kind: String,
    diff_ids: Vec<String>,
}

fn parse_config(bytes: &[u8]) -> ReconstructionResult<ConfigWire> {
    validate_unique_json(bytes).map_err(|()| ReconstructionError::InvalidJson)?;
    let config: ConfigWire =
        serde_json::from_slice(bytes).map_err(|_| ReconstructionError::UnsupportedConfiguration)?;
    let fields = [
        config.model_family.as_str(),
        config.model_type.as_str(),
        config.file_type.as_str(),
        config.architecture.as_str(),
        config.os.as_str(),
    ];
    let valid_fields = fields.iter().all(|field| {
        !field.is_empty() && field.len() <= 256 && !field.chars().any(char::is_control)
    });
    let unique_families = config.model_families.iter().collect::<BTreeSet<_>>();
    if config.model_format != "gguf"
        || config.rootfs.kind != "layers"
        || !valid_fields
        || config.model_families.is_empty()
        || config.model_families.len() > 16
        || unique_families.len() != config.model_families.len()
        || config.model_families.iter().any(|family| {
            family.is_empty() || family.len() > 256 || family.chars().any(char::is_control)
        })
        || config.rootfs.diff_ids.len() > 64
        || config
            .rootfs
            .diff_ids
            .iter()
            .any(|digest| parse_prefixed_digest(digest).is_err())
    {
        return Err(ReconstructionError::UnsupportedConfiguration);
    }
    Ok(config)
}

fn compare_rootfs(diff_ids: &[String], plan: &OllamaManifestPlan) -> RootfsDescriptorComparison {
    let layers = plan.layer_descriptors();
    let matches_by_position = layers
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            diff_ids.get(index).is_some_and(|value| {
                parse_prefixed_digest(value).is_ok_and(|digest| &digest == descriptor.digest())
            })
        })
        .collect();
    RootfsDescriptorComparison {
        same_cardinality: diff_ids.len() == layers.len(),
        matches_by_position,
    }
}

fn parse_prefixed_digest(value: &str) -> ReconstructionResult<Digest> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or(ReconstructionError::UnsupportedConfiguration)?;
    Digest::from_sha256_hex(hex.to_owned())
        .map_err(|_| ReconstructionError::UnsupportedConfiguration)
}

fn validate_text(bytes: &[u8]) -> ReconstructionResult<()> {
    if bytes.is_empty() || std::str::from_utf8(bytes).is_err() {
        Err(ReconstructionError::InvalidTextLayer)
    } else {
        Ok(())
    }
}
