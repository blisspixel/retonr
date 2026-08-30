use rewrite_model::{
    ArtifactId, FrozenExternalComponentSetId, NativeMappingClass, RuntimePackageManifestId,
};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    ExpectedExternalNativeComponent, ExternalNativeComponentReview,
    ExternalNativeComponentReviewDisposition, ExternalNativeComponentReviewError,
    MAXIMUM_NATIVE_LOADED_COMPONENTS, NativeLoadDiscovery, NativeLoadObserverError, expected_key,
};

/// Current frozen external native-component set version.
pub const FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded bytes accepted for one frozen component set.
pub const MAXIMUM_FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_JSON_BYTES: usize = 1_048_576;

/// Content-derived identifier for one canonical frozen external-component set.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FrozenExternalNativeComponentSetId(Digest);

impl FrozenExternalNativeComponentSetId {
    /// Returns the digest defining this frozen set.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrozenExternalNativeComponentSet {
    frozen_set_id: FrozenExternalNativeComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    discovery_digest: Digest,
    external_component_review_digest: Digest,
    reviewer_evidence_digest: Digest,
    components: Vec<ExpectedExternalNativeComponent>,
    canonical_json_bytes: Vec<u8>,
}

/// Canonical frozen set compiled from an approved typed review.
///
/// This value is publication material only. It deliberately does not expose the
/// native observation allowlist. Independent verification of its canonical bytes,
/// discovery, review, and reviewer evidence is required first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledFrozenExternalNativeComponentSet {
    frozen: FrozenExternalNativeComponentSet,
}

impl CompiledFrozenExternalNativeComponentSet {
    /// Compiles canonical frozen-set bytes from one discovery and approved review.
    ///
    /// # Errors
    ///
    /// Returns [`FrozenExternalNativeComponentSetError`] when review authority is
    /// blocked or any package, discovery, component, or limit binding is invalid.
    pub fn compile(
        discovery: &NativeLoadDiscovery,
        review: &ExternalNativeComponentReview,
    ) -> Result<Self, FrozenExternalNativeComponentSetError> {
        validate_review_binding(discovery, review)?;
        if review.disposition() != ExternalNativeComponentReviewDisposition::Approved {
            return Err(FrozenExternalNativeComponentSetError::ReviewBlocked);
        }
        let frozen = FrozenExternalNativeComponentSet::new(
            discovery.runtime_package_manifest_id().clone(),
            discovery.discovery_digest().clone(),
            review.review_digest().clone(),
            review.reviewer_evidence_digest().clone(),
            expected_components(discovery),
        )?;
        Ok(Self { frozen })
    }

    /// Returns the content-derived identity of the canonical frozen set.
    #[must_use]
    pub const fn frozen_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen.frozen_set_id
    }

    /// Returns the exact runtime package bound by this compiled set.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.frozen.runtime_package_manifest_id
    }

    /// Returns the bound non-authoritative discovery-report digest.
    #[must_use]
    pub const fn discovery_digest(&self) -> &Digest {
        &self.frozen.discovery_digest
    }

    /// Returns the digest of the complete canonical external-component review.
    #[must_use]
    pub const fn external_component_review_digest(&self) -> &Digest {
        &self.frozen.external_component_review_digest
    }

    /// Returns the digest of the separate reviewer-owned evidence bytes.
    #[must_use]
    pub const fn reviewer_evidence_digest(&self) -> &Digest {
        &self.frozen.reviewer_evidence_digest
    }

    /// Returns canonical content-free frozen-set JSON for publication.
    #[must_use]
    pub fn canonical_json_bytes(&self) -> &[u8] {
        &self.frozen.canonical_json_bytes
    }
}

/// Independently verified frozen set consumable by native-load observation.
///
/// Only this type exposes the exact expected component slice. Construction
/// reparses and revalidates the canonical frozen set, discovery, typed review,
/// and separate reviewer evidence from bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFrozenExternalNativeComponentSet {
    frozen: FrozenExternalNativeComponentSet,
}

impl VerifiedFrozenExternalNativeComponentSet {
    /// Independently verifies frozen-set, discovery, review, and reviewer evidence bytes.
    ///
    /// # Errors
    ///
    /// Returns [`FrozenExternalNativeComponentSetError`] for malformed,
    /// noncanonical, excessive, reordered, blocked, digest-drifting, or
    /// inconsistently bound input.
    pub fn verify(
        bytes: &[u8],
        discovery_bytes: &[u8],
        review_bytes: &[u8],
        reviewer_evidence_bytes: &[u8],
        expected_package_id: &RuntimePackageManifestId,
    ) -> Result<Self, FrozenExternalNativeComponentSetError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_JSON_BYTES
        {
            return Err(FrozenExternalNativeComponentSetError::LimitExceeded);
        }
        let discovery = NativeLoadDiscovery::from_json_bytes(discovery_bytes, expected_package_id)
            .map_err(FrozenExternalNativeComponentSetError::Discovery)?;
        let review = ExternalNativeComponentReview::verify(
            review_bytes,
            discovery_bytes,
            reviewer_evidence_bytes,
            expected_package_id,
        )
        .map_err(FrozenExternalNativeComponentSetError::Review)?;
        validate_review_binding(&discovery, &review)?;
        if review.disposition() != ExternalNativeComponentReviewDisposition::Approved {
            return Err(FrozenExternalNativeComponentSetError::ReviewBlocked);
        }
        let wire: FrozenWire = serde_json::from_slice(bytes)
            .map_err(|_| FrozenExternalNativeComponentSetError::InvalidEncoding)?;
        let FrozenStatus::Frozen = wire.status;
        if wire.schema_version != FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_SCHEMA_VERSION
            || &wire.runtime_package_manifest_id != expected_package_id
            || wire.discovery_digest != *discovery.discovery_digest()
            || wire.external_component_review_digest != *review.review_digest()
            || wire.reviewer_evidence_digest != *review.reviewer_evidence_digest()
        {
            return Err(FrozenExternalNativeComponentSetError::InvalidBinding);
        }
        let frozen = FrozenExternalNativeComponentSet::new(
            wire.runtime_package_manifest_id,
            wire.discovery_digest,
            wire.external_component_review_digest,
            wire.reviewer_evidence_digest,
            wire.components
                .into_iter()
                .map(|component| {
                    ExpectedExternalNativeComponent::new(
                        component.artifact_id,
                        component.byte_size,
                        component.mapping_class,
                    )
                })
                .collect(),
        )?;
        if frozen.canonical_json_bytes != bytes {
            return Err(FrozenExternalNativeComponentSetError::InvalidEncoding);
        }
        if !same_components(&frozen.components, discovery.external_components()) {
            return Err(FrozenExternalNativeComponentSetError::InvalidComponents);
        }
        Ok(Self { frozen })
    }

    /// Returns the content-derived identity of the canonical frozen set.
    #[must_use]
    pub const fn frozen_set_id(&self) -> &FrozenExternalNativeComponentSetId {
        &self.frozen.frozen_set_id
    }

    /// Returns the inert portable identity of this exact frozen component set.
    ///
    /// This wraps the already-derived attestor digest without rehashing. The
    /// portable identity does not carry component-review or observation authority.
    #[must_use]
    pub fn frozen_external_component_set_id(&self) -> FrozenExternalComponentSetId {
        FrozenExternalComponentSetId::from_derived_digest(self.frozen_set_id().digest().clone())
    }

    /// Returns the exact runtime package this verified set was reviewed for.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.frozen.runtime_package_manifest_id
    }

    /// Returns the bound non-authoritative discovery-report digest.
    #[must_use]
    pub const fn discovery_digest(&self) -> &Digest {
        &self.frozen.discovery_digest
    }

    /// Returns the digest of the complete canonical external-component review.
    #[must_use]
    pub const fn external_component_review_digest(&self) -> &Digest {
        &self.frozen.external_component_review_digest
    }

    /// Returns the digest of the separate reviewer-owned evidence bytes.
    #[must_use]
    pub const fn reviewer_evidence_digest(&self) -> &Digest {
        &self.frozen.reviewer_evidence_digest
    }

    /// Returns the exact canonical allowlist consumed by native-load verification.
    #[must_use]
    pub fn expected_components(&self) -> &[ExpectedExternalNativeComponent] {
        &self.frozen.components
    }

    /// Returns canonical content-free frozen-set JSON.
    #[must_use]
    pub fn canonical_json_bytes(&self) -> &[u8] {
        &self.frozen.canonical_json_bytes
    }
}

impl FrozenExternalNativeComponentSet {
    fn new(
        runtime_package_manifest_id: RuntimePackageManifestId,
        discovery_digest: Digest,
        external_component_review_digest: Digest,
        reviewer_evidence_digest: Digest,
        components: Vec<ExpectedExternalNativeComponent>,
    ) -> Result<Self, FrozenExternalNativeComponentSetError> {
        validate_components(&components)?;
        let component_json = components
            .iter()
            .map(|component| {
                serde_json::json!({
                    "artifact_id": component.artifact_id(),
                    "byte_size": component.byte_size(),
                    "mapping_class": component.mapping_class()
                })
            })
            .collect::<Vec<_>>();
        let canonical_json_bytes = serde_json::to_vec(&serde_json::json!({
            "components": component_json,
            "discovery_digest": discovery_digest,
            "external_component_review_digest": external_component_review_digest,
            "reviewer_evidence_digest": reviewer_evidence_digest,
            "runtime_package_manifest_id": runtime_package_manifest_id,
            "schema_version": FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_SCHEMA_VERSION,
            "status": "frozen"
        }))
        .map_err(|_| FrozenExternalNativeComponentSetError::InvalidEncoding)?;
        if canonical_json_bytes.len() > MAXIMUM_FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_JSON_BYTES {
            return Err(FrozenExternalNativeComponentSetError::LimitExceeded);
        }
        let frozen_set_id =
            FrozenExternalNativeComponentSetId(Digest::sha256(&canonical_json_bytes));
        Ok(Self {
            frozen_set_id,
            runtime_package_manifest_id,
            discovery_digest,
            external_component_review_digest,
            reviewer_evidence_digest,
            components,
            canonical_json_bytes,
        })
    }
}

/// Frozen external-component set validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FrozenExternalNativeComponentSetError {
    /// One encoded input was empty or exceeded its fixed ceiling.
    #[error("frozen external native-component set limit was exceeded")]
    LimitExceeded,
    /// Frozen-set JSON was malformed, ambiguous, or noncanonical.
    #[error("frozen external native-component set encoding is invalid")]
    InvalidEncoding,
    /// Package, discovery, review, or reviewer-evidence identity did not match.
    #[error("frozen external native-component set binding is invalid")]
    InvalidBinding,
    /// Components were invalid, reordered, duplicated, or differed from discovery.
    #[error("frozen external native-component set is invalid")]
    InvalidComponents,
    /// The exact external-component review blocked the discovered set.
    #[error("external native-component review blocked frozen-set compilation")]
    ReviewBlocked,
    /// The bound discovery report was invalid.
    #[error("frozen external native-component discovery is invalid: {0}")]
    Discovery(NativeLoadObserverError),
    /// The bound typed external-component review was invalid.
    #[error("frozen external native-component review is invalid: {0}")]
    Review(ExternalNativeComponentReviewError),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenWire {
    components: Vec<ComponentWire>,
    discovery_digest: Digest,
    external_component_review_digest: Digest,
    reviewer_evidence_digest: Digest,
    runtime_package_manifest_id: RuntimePackageManifestId,
    schema_version: u32,
    status: FrozenStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FrozenStatus {
    Frozen,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentWire {
    artifact_id: ArtifactId,
    byte_size: u64,
    mapping_class: NativeMappingClass,
}

fn validate_review_binding(
    discovery: &NativeLoadDiscovery,
    review: &ExternalNativeComponentReview,
) -> Result<(), FrozenExternalNativeComponentSetError> {
    if review.runtime_package_manifest_id() != discovery.runtime_package_manifest_id()
        || review.discovery_digest() != discovery.discovery_digest()
    {
        return Err(FrozenExternalNativeComponentSetError::InvalidBinding);
    }
    if review.components().len() != discovery.external_components().len()
        || review
            .components()
            .iter()
            .zip(discovery.external_components())
            .any(|(reviewed, discovered)| {
                reviewed.artifact_id() != discovered.artifact_id()
                    || reviewed.byte_size() != discovered.byte_size()
                    || reviewed.mapping_class() != discovered.mapping_class()
            })
    {
        return Err(FrozenExternalNativeComponentSetError::InvalidComponents);
    }
    Ok(())
}

fn expected_components(discovery: &NativeLoadDiscovery) -> Vec<ExpectedExternalNativeComponent> {
    discovery
        .external_components()
        .iter()
        .map(|component| {
            ExpectedExternalNativeComponent::new(
                component.artifact_id().clone(),
                component.byte_size(),
                component.mapping_class(),
            )
        })
        .collect()
}

fn validate_components(
    components: &[ExpectedExternalNativeComponent],
) -> Result<(), FrozenExternalNativeComponentSetError> {
    if components.len() > MAXIMUM_NATIVE_LOADED_COMPONENTS
        || components.iter().any(|component| {
            component.byte_size() == 0
                || component.mapping_class() != NativeMappingClass::ExecutableMapped
        })
        || components.windows(2).any(|pair| {
            expected_key(&pair[0]) >= expected_key(&pair[1])
                || pair[0].artifact_id().digest().as_str()
                    >= pair[1].artifact_id().digest().as_str()
        })
    {
        Err(FrozenExternalNativeComponentSetError::InvalidComponents)
    } else {
        Ok(())
    }
}

fn same_components(
    frozen: &[ExpectedExternalNativeComponent],
    discovered: &[super::DiscoveredExternalNativeComponent],
) -> bool {
    frozen.len() == discovered.len()
        && frozen.iter().zip(discovered).all(|(frozen, discovered)| {
            frozen.artifact_id() == discovered.artifact_id()
                && frozen.byte_size() == discovered.byte_size()
                && frozen.mapping_class() == discovered.mapping_class()
        })
}
