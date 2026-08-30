use std::io::Read;

use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetRelativePath, ModelPackageManifest,
    ModelPackageManifestId, ModelPackageMemberRole, PackageSource, PackageSourceId,
    PackageSourceKind, PackageTransformation,
};
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{
    BlobOpenError, OllamaManifestPlan, ReconstructionError, ReconstructionLimits,
    ollama_logical_binding_digest, parse_manifest_v2, reconstruct_model_package_with_limits,
};

const CONFIG_PATH: &str = "config/ollama-config.json";
const PARAMETERS_PATH: &str = "config/parameters.json";
const LICENSE_PATH: &str = "legal/license.txt";
const MODEL_PATH: &str = "model/model.gguf";
const TEMPLATE_PATH: &str = "prompts/template.go.tmpl";
const PROVENANCE_PATH: &str = "provenance/ollama-manifest-v2.json";
const DESCRIPTOR_MAPPING_DOMAIN: &[u8] = b"retonr:ollama-local-archive-descriptor-mapping:v1\0";

/// Exact logical member binding derived from one verified local archive.
///
/// Logical paths remain distinct when multiple members have the same artifact
/// identity. In particular, license bindings are never deduplicated by digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaLocalArchiveMemberBinding {
    relative_path: ArtifactSetRelativePath,
    artifact_id: ArtifactId,
    byte_size: u64,
}

impl OllamaLocalArchiveMemberBinding {
    /// Returns the exact portable logical path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns the complete member byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact member byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// Inert evidence derived from one exactly reconstructed Ollama local archive.
///
/// This value records only relationships established from supplied retained
/// bytes. It does not approve a license, establish upstream authenticity, grant
/// launch authority, or prove that a runtime loaded or used any member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaLocalArchiveFoundationEvidence {
    artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    package_source_id: PackageSourceId,
    provenance_manifest: OllamaLocalArchiveMemberBinding,
    descriptor_mapping_digest: Digest,
    logical_binding_digest: Digest,
    license_members: Vec<OllamaLocalArchiveMemberBinding>,
}

impl OllamaLocalArchiveFoundationEvidence {
    /// Returns the exact reconstructed artifact-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact reconstructed model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the exact reviewed local-archive source identity.
    #[must_use]
    pub const fn package_source_id(&self) -> &PackageSourceId {
        &self.package_source_id
    }

    /// Returns the exact raw manifest retained as provenance and runtime input.
    #[must_use]
    pub const fn provenance_manifest(&self) -> &OllamaLocalArchiveMemberBinding {
        &self.provenance_manifest
    }

    /// Returns the internally derived descriptor-to-logical-path mapping digest.
    #[must_use]
    pub const fn descriptor_mapping_digest(&self) -> &Digest {
        &self.descriptor_mapping_digest
    }

    /// Returns the independently recomputed untransformed logical binding.
    #[must_use]
    pub const fn logical_binding_digest(&self) -> &Digest {
        &self.logical_binding_digest
    }

    /// Returns every logical license member in canonical package path order.
    #[must_use]
    pub fn license_members(&self) -> &[OllamaLocalArchiveMemberBinding] {
        &self.license_members
    }
}

/// Failure while deriving an inert retained-model foundation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OllamaLocalArchiveFoundationError {
    /// The package does not declare the narrowly supported local-archive source.
    #[error("Ollama model foundation requires a local-archive package source")]
    UnsupportedSource,
    /// The package source does not bind the exact retained raw manifest.
    #[error("Ollama model foundation source binding does not match")]
    SourceBindingMismatch,
    /// The package is not untransformed under the recomputed logical binding.
    #[error("Ollama model foundation transformation binding does not match")]
    TransformationBindingMismatch,
    /// The raw manifest and descriptor relationships do not match the package.
    #[error("Ollama model foundation descriptor relationships do not match")]
    DescriptorRelationshipMismatch,
    /// Full reconstruction produced another artifact-set manifest.
    #[error("Ollama model foundation artifact set does not match reconstruction")]
    ArtifactSetMismatch,
    /// Full reconstruction produced another semantic model package.
    #[error("Ollama model foundation package does not match reconstruction")]
    ModelPackageMismatch,
    /// Strict parsing or complete package reconstruction failed.
    #[error(transparent)]
    Reconstruction(#[from] ReconstructionError),
}

/// Verifies one exact retained Ollama local-archive foundation.
///
/// The verifier derives the source locator from `expected_package`, parses the
/// retained raw manifest under fixed limits, checks every fixed descriptor and
/// logical member relationship, recomputes the untransformed logical binding,
/// and then performs complete model-package reconstruction. Both reconstructed
/// manifests must exactly equal the supplied expected manifests.
///
/// The blob opener receives only validated descriptor digests. This function
/// reads no path, performs no network access, and grants no license, launch, or
/// generation authority. A `LocalArchive` result makes no claim that the bytes
/// came from an official registry or named upstream publisher.
///
/// # Errors
///
/// Returns [`OllamaLocalArchiveFoundationError`] for an unsupported source,
/// malformed or drifting manifest, descriptor or source mismatch, substituted
/// transformation evidence, reconstruction failure, or unequal reconstructed
/// manifests.
pub fn verify_ollama_local_archive_foundation<R, F, C>(
    raw_manifest: &[u8],
    expected_artifact_set: &ArtifactSetManifest,
    expected_package: &ModelPackageManifest,
    limits: &ReconstructionLimits,
    open_blob: F,
    cancelled: C,
) -> Result<OllamaLocalArchiveFoundationEvidence, OllamaLocalArchiveFoundationError>
where
    R: Read,
    F: FnMut(&Digest) -> Result<R, BlobOpenError>,
    C: FnMut() -> bool,
{
    let evidence = verify_ollama_local_archive_foundation_bindings(
        raw_manifest,
        expected_artifact_set,
        expected_package,
        limits,
    )?;

    let reconstructed = reconstruct_model_package_with_limits(
        raw_manifest,
        expected_package.source().locator(),
        limits,
        open_blob,
        cancelled,
    )?;
    if reconstructed.artifact_set() != expected_artifact_set {
        return Err(OllamaLocalArchiveFoundationError::ArtifactSetMismatch);
    }
    if reconstructed.model_package() != expected_package {
        return Err(OllamaLocalArchiveFoundationError::ModelPackageMismatch);
    }
    Ok(evidence)
}

/// Rechecks local-archive bindings after all retained members were rehashed.
///
/// This lightweight operation reparses the exact raw manifest, verifies the
/// source, fixed descriptor-to-path relationships, and untransformed logical
/// binding against the expected manifests, and derives the same inert evidence
/// as full verification. It deliberately does not reopen or read descriptor
/// blobs and therefore must not be used for initial foundation verification.
/// A later application lease may call it only after independently rehashing
/// every retained artifact-set member.
///
/// # Errors
///
/// Returns [`OllamaLocalArchiveFoundationError`] for an unsupported source,
/// malformed or drifting raw manifest, descriptor mismatch, or substituted
/// source or transformation binding.
pub fn verify_ollama_local_archive_foundation_bindings(
    raw_manifest: &[u8],
    expected_artifact_set: &ArtifactSetManifest,
    expected_package: &ModelPackageManifest,
    limits: &ReconstructionLimits,
) -> Result<OllamaLocalArchiveFoundationEvidence, OllamaLocalArchiveFoundationError> {
    if expected_package.source().kind() != PackageSourceKind::LocalArchive {
        return Err(OllamaLocalArchiveFoundationError::UnsupportedSource);
    }
    let plan = parse_manifest_v2(raw_manifest, limits)?;
    validate_source(expected_package, &plan)?;
    let logical_binding_digest = ollama_logical_binding_digest(&plan)?;
    validate_transformation(expected_package, &logical_binding_digest)?;
    validate_descriptor_relationships(expected_artifact_set, expected_package, &plan)?;
    let provenance_manifest = binding_for_path(expected_package, PROVENANCE_PATH)?;
    let license_members = expected_package
        .members()
        .iter()
        .filter(|member| {
            member
                .roles()
                .contains(&ModelPackageMemberRole::LicenseText)
        })
        .map(member_binding)
        .collect::<Vec<_>>();
    if license_members.is_empty() {
        return Err(OllamaLocalArchiveFoundationError::DescriptorRelationshipMismatch);
    }
    Ok(OllamaLocalArchiveFoundationEvidence {
        artifact_set_id: expected_artifact_set.artifact_set_id(),
        model_package_manifest_id: expected_package.model_package_manifest_id(),
        package_source_id: expected_package.source().package_source_id(),
        provenance_manifest,
        descriptor_mapping_digest: descriptor_mapping_digest(&plan),
        logical_binding_digest,
        license_members,
    })
}

fn validate_source(
    package: &ModelPackageManifest,
    plan: &OllamaManifestPlan,
) -> Result<(), OllamaLocalArchiveFoundationError> {
    let expected = PackageSource::new(
        PackageSourceKind::LocalArchive,
        package.source().locator(),
        format!("sha256:{}", plan.raw_manifest_digest().as_str()),
        plan.raw_manifest_digest().clone(),
    )
    .map_err(|_error| OllamaLocalArchiveFoundationError::SourceBindingMismatch)?;
    if package.source() == &expected {
        Ok(())
    } else {
        Err(OllamaLocalArchiveFoundationError::SourceBindingMismatch)
    }
}

fn validate_transformation(
    package: &ModelPackageManifest,
    logical_binding_digest: &Digest,
) -> Result<(), OllamaLocalArchiveFoundationError> {
    match package.transformation() {
        PackageTransformation::Untransformed { evidence_digest }
            if evidence_digest == logical_binding_digest =>
        {
            Ok(())
        }
        PackageTransformation::Untransformed { .. } | PackageTransformation::Transformed { .. } => {
            Err(OllamaLocalArchiveFoundationError::TransformationBindingMismatch)
        }
    }
}

fn validate_descriptor_relationships(
    artifact_set: &ArtifactSetManifest,
    package: &ModelPackageManifest,
    plan: &OllamaManifestPlan,
) -> Result<(), OllamaLocalArchiveFoundationError> {
    let expected = [
        ExpectedMember::descriptor(
            CONFIG_PATH,
            ModelPackageMemberRole::AuxiliaryData,
            plan.config(),
        ),
        ExpectedMember::descriptor(
            PARAMETERS_PATH,
            ModelPackageMemberRole::GenerationConfiguration,
            plan.parameters(),
        ),
        ExpectedMember::descriptor(
            LICENSE_PATH,
            ModelPackageMemberRole::LicenseText,
            plan.license(),
        ),
        ExpectedMember::descriptor(
            MODEL_PATH,
            ModelPackageMemberRole::ModelWeights,
            plan.model(),
        ),
        ExpectedMember::descriptor(
            TEMPLATE_PATH,
            ModelPackageMemberRole::PromptTemplate,
            plan.template(),
        ),
        ExpectedMember {
            path: PROVENANCE_PATH,
            role: ModelPackageMemberRole::ProvenanceRecord,
            digest: plan.raw_manifest_digest(),
            byte_size: plan.raw_manifest_size(),
        },
    ];
    if artifact_set.members().len() != expected.len() || package.members().len() != expected.len() {
        return Err(OllamaLocalArchiveFoundationError::DescriptorRelationshipMismatch);
    }
    for ((artifact, member), expected) in artifact_set
        .members()
        .iter()
        .zip(package.members())
        .zip(expected)
    {
        if artifact.relative_path().as_str() != expected.path
            || artifact.artifact_id().digest() != expected.digest
            || artifact.byte_size() != expected.byte_size
            || member.relative_path() != artifact.relative_path()
            || member.artifact_id() != artifact.artifact_id()
            || member.byte_size() != artifact.byte_size()
            || member.roles() != [expected.role]
        {
            return Err(OllamaLocalArchiveFoundationError::DescriptorRelationshipMismatch);
        }
    }
    Ok(())
}

fn binding_for_path(
    package: &ModelPackageManifest,
    path: &str,
) -> Result<OllamaLocalArchiveMemberBinding, OllamaLocalArchiveFoundationError> {
    package
        .members()
        .iter()
        .find(|member| member.relative_path().as_str() == path)
        .map(member_binding)
        .ok_or(OllamaLocalArchiveFoundationError::DescriptorRelationshipMismatch)
}

fn member_binding(member: &rewrite_model::ModelPackageMember) -> OllamaLocalArchiveMemberBinding {
    OllamaLocalArchiveMemberBinding {
        relative_path: member.relative_path().clone(),
        artifact_id: member.artifact_id().clone(),
        byte_size: member.byte_size(),
    }
}

fn descriptor_mapping_digest(plan: &OllamaManifestPlan) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(DESCRIPTOR_MAPPING_DOMAIN);
    append_mapping(
        &mut hasher,
        CONFIG_PATH,
        plan.config().media_type(),
        plan.config().digest(),
        plan.config().size(),
    );
    append_mapping(
        &mut hasher,
        PARAMETERS_PATH,
        plan.parameters().media_type(),
        plan.parameters().digest(),
        plan.parameters().size(),
    );
    append_mapping(
        &mut hasher,
        LICENSE_PATH,
        plan.license().media_type(),
        plan.license().digest(),
        plan.license().size(),
    );
    append_mapping(
        &mut hasher,
        MODEL_PATH,
        plan.model().media_type(),
        plan.model().digest(),
        plan.model().size(),
    );
    append_mapping(
        &mut hasher,
        TEMPLATE_PATH,
        plan.template().media_type(),
        plan.template().digest(),
        plan.template().size(),
    );
    append_mapping(
        &mut hasher,
        PROVENANCE_PATH,
        crate::MANIFEST_MEDIA_TYPE,
        plan.raw_manifest_digest(),
        plan.raw_manifest_size(),
    );
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 output is a valid digest")
}

fn append_mapping(
    hasher: &mut Sha256,
    path: &str,
    media_type: &str,
    digest: &Digest,
    byte_size: u64,
) {
    append_text(hasher, path);
    append_text(hasher, media_type);
    hasher.update(digest.as_str().as_bytes());
    hasher.update(byte_size.to_be_bytes());
}

fn append_text(hasher: &mut Sha256, value: &str) {
    hasher.update(
        u64::try_from(value.len())
            .expect("fixed descriptor text length fits u64")
            .to_be_bytes(),
    );
    hasher.update(value.as_bytes());
}

struct ExpectedMember<'a> {
    path: &'static str,
    role: ModelPackageMemberRole,
    digest: &'a Digest,
    byte_size: u64,
}

impl<'a> ExpectedMember<'a> {
    const fn descriptor(
        path: &'static str,
        role: ModelPackageMemberRole,
        descriptor: &'a crate::BlobDescriptor,
    ) -> Self {
        Self {
            path,
            role,
            digest: descriptor.digest(),
            byte_size: descriptor.size(),
        }
    }
}

#[cfg(test)]
#[path = "model_foundation/tests.rs"]
mod tests;
