use std::{fmt, marker::PhantomData};

pub use rewrite_model::ModelPackageFoundationId;
use rewrite_model::{
    ArtifactSetId, ArtifactSetManifest, ModelPackageManifest, ModelPackageManifestId,
    PackageSourceId,
};
use rewrite_ollama_package::{
    OllamaLocalArchiveFoundationError, OllamaLocalArchiveFoundationEvidence, ReconstructionLimits,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use crate::RuntimeArtifactSetLease;

use super::{
    ModelPackageAttestationEvidence, ModelPackageIdentityToken, ModelPackageLease,
    ModelPackageLeaseLimits, PackageAttestationError, PackageAttestationService,
    RetainedModelPackageIsolationSource, clone_retained_model_members,
    isolation_source_binding_digest, recheck_retained_model_package_identities,
    retain_model_package_for_full_verification, revalidate_verified_model_members_once,
};

mod binding;

const FOUNDATION_ID_DOMAIN: &[u8] = b"retonr:managed-ollama-model-package-foundation:v1\0";

/// Failure while constructing or revalidating a verified managed Ollama package.
#[derive(Debug, Error)]
pub enum ManagedOllamaModelPackageError {
    /// Generic retained package verification failed.
    #[error("managed Ollama model package retention failed")]
    Package(#[source] PackageAttestationError),
    /// Full or lightweight Ollama foundation verification failed.
    #[error("managed Ollama model package foundation verification failed")]
    Foundation(#[source] OllamaLocalArchiveFoundationError),
    /// Lightweight revalidation derived different stable foundation facts.
    #[error("managed Ollama model package foundation binding changed")]
    FoundationBindingChanged,
}

/// Noncloneable retained authority for one exact managed Ollama model foundation.
///
/// Construction retains exact package handles and lets the full Ollama
/// local-archive foundation verifier perform their only complete initial content
/// pass before either retained lease is constructed. The value grants no
/// license, launch, model-use, or qualification authority.
pub struct VerifiedManagedOllamaModelPackageLease {
    artifact_set_manifest: ArtifactSetManifest,
    model_package_manifest: ModelPackageManifest,
    foundation: OllamaLocalArchiveFoundationEvidence,
    foundation_id: ModelPackageFoundationId,
    reconstruction_limits: ReconstructionLimits,
    package: ModelPackageLease,
    identity: ModelPackageIdentityToken,
}

/// Borrowed exact foundation facts for adjacent application-owned authorities.
///
/// This view is crate-private so source text, retained logical paths, and
/// installation-instance state cannot escape through the public lease API.
pub(crate) struct VerifiedManagedOllamaModelPackageView<'lease> {
    package: &'lease VerifiedManagedOllamaModelPackageLease,
}

impl<'lease> VerifiedManagedOllamaModelPackageView<'lease> {
    pub(crate) fn installation_generation(&self) -> u64 {
        self.package
            .package
            .installation_key()
            .installation_generation()
    }

    pub(crate) const fn artifact_set_manifest(&self) -> &'lease ArtifactSetManifest {
        &self.package.artifact_set_manifest
    }

    pub(crate) const fn model_package_manifest(&self) -> &'lease ModelPackageManifest {
        &self.package.model_package_manifest
    }

    pub(crate) const fn foundation_evidence(&self) -> &'lease OllamaLocalArchiveFoundationEvidence {
        &self.package.foundation
    }
}

impl VerifiedManagedOllamaModelPackageLease {
    /// Returns the stable verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }

    /// Returns the exact verified artifact-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        self.foundation.artifact_set_id()
    }

    /// Returns the exact verified semantic model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        self.foundation.model_package_manifest_id()
    }

    /// Returns the exact verified local-archive source identity.
    #[must_use]
    pub const fn package_source_id(&self) -> &PackageSourceId {
        self.foundation.package_source_id()
    }

    /// Returns the number of exact retained logical package members.
    #[must_use]
    pub const fn member_count(&self) -> u32 {
        self.package.evidence.member_count()
    }

    /// Returns the checked aggregate retained logical bytes.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.package.evidence.byte_size()
    }

    pub(crate) fn identity_token(&self) -> ModelPackageIdentityToken {
        self.identity.clone()
    }

    pub(crate) fn binds_identity_token(&self, token: &ModelPackageIdentityToken) -> bool {
        self.identity.ptr_eq(token)
    }

    /// Returns the number of logical members carrying license text.
    #[must_use]
    pub fn license_member_count(&self) -> usize {
        self.foundation.license_members().len()
    }

    /// Rehashes every retained member once, then repeats the lightweight raw
    /// manifest, source, descriptor, and logical-binding verification.
    ///
    /// This operation deliberately does not repeat the GGUF structural pass.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaModelPackageError`] on cancellation, retained byte
    /// or tree drift, malformed raw-manifest state, or changed foundation facts.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedOllamaModelPackageError> {
        revalidate_verified_model_members_once(
            &self.package.artifact_set,
            &self.package.retained,
            cancellation,
        )
        .map_err(ManagedOllamaModelPackageError::Package)?;
        let observed = binding::verify_lightweight(
            &self.package.retained,
            &self.artifact_set_manifest,
            &self.model_package_manifest,
            &self.reconstruction_limits,
            cancellation,
        )?;
        if observed == self.foundation {
            Ok(())
        } else {
            Err(ManagedOllamaModelPackageError::FoundationBindingChanged)
        }
    }

    pub(crate) const fn private_view(&self) -> VerifiedManagedOllamaModelPackageView<'_> {
        VerifiedManagedOllamaModelPackageView { package: self }
    }

    pub(super) const fn model_package(&self) -> &ModelPackageManifest {
        &self.model_package_manifest
    }

    pub(super) fn clone_members_for_verified_input(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<RetainedModelPackageIsolationSource<'_>, ManagedOllamaModelPackageError> {
        self.revalidate(cancellation)?;
        let members = clone_retained_model_members(&self.package.retained)
            .map_err(ManagedOllamaModelPackageError::Package)?;
        let evidence = &self.package.evidence;
        let installation_generation = self.package.installation_key().installation_generation();
        Ok(RetainedModelPackageIsolationSource {
            artifact_set_id: evidence.artifact_set_id().clone(),
            model_package_manifest_id: evidence.model_package_manifest_id().clone(),
            installation_generation,
            byte_size: evidence.byte_size(),
            binding_digest: isolation_source_binding_digest(
                evidence.artifact_set_id(),
                evidence.model_package_manifest_id(),
                installation_generation,
            ),
            members,
            _lease: PhantomData,
        })
    }
}

impl fmt::Debug for VerifiedManagedOllamaModelPackageLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedOllamaModelPackageLease")
            .field("foundation_id", &self.foundation_id)
            .field("artifact_set_id", self.artifact_set_id())
            .field(
                "model_package_manifest_id",
                self.model_package_manifest_id(),
            )
            .field("package_source_id", self.package_source_id())
            .field("member_count", &self.member_count())
            .field("byte_size", &self.byte_size())
            .field("license_member_count", &self.license_member_count())
            .finish_non_exhaustive()
    }
}

impl PackageAttestationService {
    /// Constructs a retained verified managed Ollama model-package authority.
    ///
    /// The artifact-set lease is consumed. Exact handles are opened without
    /// issuing authority. Full local-archive reconstruction then verifies each
    /// retained logical member in one complete content pass. Cheap handle and
    /// canonical-name checks plus lightweight raw binding verification complete
    /// before either retained lease is constructed.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaModelPackageError`] for invalid limits,
    /// cancellation, package or byte drift, or incomplete foundation evidence.
    pub fn verify_managed_ollama_model_package(
        artifact_set: RuntimeArtifactSetLease,
        package: &ModelPackageManifest,
        lease_limits: ModelPackageLeaseLimits,
        reconstruction_limits: &ReconstructionLimits,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedManagedOllamaModelPackageLease, ManagedOllamaModelPackageError> {
        let reconstruction_limits = reconstruction_limits
            .validate()
            .map_err(OllamaLocalArchiveFoundationError::from)
            .map_err(ManagedOllamaModelPackageError::Foundation)?;
        let retained = retain_model_package_for_full_verification(
            &artifact_set,
            package,
            lease_limits,
            cancellation,
        )
        .map_err(ManagedOllamaModelPackageError::Package)?;
        let artifact_set_manifest = artifact_set.manifest().clone();
        let model_package_manifest = package.clone();
        let foundation = binding::verify_full(
            &retained,
            &artifact_set_manifest,
            &model_package_manifest,
            &reconstruction_limits,
            cancellation,
        )?;
        recheck_retained_model_package_identities(&artifact_set, &retained, cancellation)
            .map_err(ManagedOllamaModelPackageError::Package)?;
        let observed = binding::verify_lightweight(
            &retained,
            &artifact_set_manifest,
            &model_package_manifest,
            &reconstruction_limits,
            cancellation,
        )?;
        if observed != foundation {
            return Err(ManagedOllamaModelPackageError::FoundationBindingChanged);
        }
        recheck_retained_model_package_identities(&artifact_set, &retained, cancellation)
            .map_err(ManagedOllamaModelPackageError::Package)?;
        let evidence = ModelPackageAttestationEvidence::new(
            artifact_set_manifest.artifact_set_id(),
            model_package_manifest.model_package_manifest_id(),
            u32::try_from(model_package_manifest.members().len())
                .map_err(|_| PackageAttestationError::InvalidModelLimits)
                .map_err(ManagedOllamaModelPackageError::Package)?,
            artifact_set_manifest.total_byte_size(),
        );
        let generic = ModelPackageLease {
            artifact_set,
            retained,
            evidence,
        };
        let foundation_id = derive_foundation_id(&foundation);
        Ok(VerifiedManagedOllamaModelPackageLease {
            artifact_set_manifest,
            model_package_manifest,
            foundation,
            foundation_id,
            reconstruction_limits,
            package: generic,
            identity: ModelPackageIdentityToken::new(),
        })
    }
}

fn derive_foundation_id(
    foundation: &OllamaLocalArchiveFoundationEvidence,
) -> ModelPackageFoundationId {
    let mut material = FOUNDATION_ID_DOMAIN.to_vec();
    push_digest(&mut material, foundation.artifact_set_id().digest());
    push_digest(
        &mut material,
        foundation.model_package_manifest_id().digest(),
    );
    push_digest(&mut material, foundation.package_source_id().digest());
    push_member(&mut material, foundation.provenance_manifest());
    push_digest(&mut material, foundation.descriptor_mapping_digest());
    push_digest(&mut material, foundation.logical_binding_digest());
    material.extend_from_slice(
        &u32::try_from(foundation.license_members().len())
            .expect("verified license member count fits u32")
            .to_be_bytes(),
    );
    for member in foundation.license_members() {
        push_member(&mut material, member);
    }
    ModelPackageFoundationId::from_derived_digest(Digest::sha256(&material))
}

fn push_member(
    material: &mut Vec<u8>,
    member: &rewrite_ollama_package::OllamaLocalArchiveMemberBinding,
) {
    push_text(material, member.relative_path().as_str());
    push_digest(material, member.artifact_id().digest());
    material.extend_from_slice(&member.byte_size().to_be_bytes());
}

fn push_digest(material: &mut Vec<u8>, digest: &Digest) {
    material.extend_from_slice(digest.as_str().as_bytes());
}

fn push_text(material: &mut Vec<u8>, value: &str) {
    material.extend_from_slice(
        &u64::try_from(value.len())
            .expect("verified foundation text length fits u64")
            .to_be_bytes(),
    );
    material.extend_from_slice(value.as_bytes());
}
