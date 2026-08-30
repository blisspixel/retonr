use rewrite_model::{ArtifactId, NativeMappingClass, RuntimePackageManifestId};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    ExpectedExternalNativeComponent, MAXIMUM_NATIVE_LOADED_COMPONENTS, NativeLoadDiscovery,
    NativeLoadObserverError, expected_key,
};

/// Current external native-component review version.
pub const EXTERNAL_NATIVE_COMPONENT_REVIEW_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded bytes accepted for one external native-component review.
pub const MAXIMUM_EXTERNAL_NATIVE_COMPONENT_REVIEW_JSON_BYTES: usize = 1_048_576;
/// Maximum bytes bound as the separate reviewer-owned evidence record.
pub const MAXIMUM_EXTERNAL_COMPONENT_REVIEW_EVIDENCE_BYTES: usize = 4 * 1_024 * 1_024;

/// Explicit reviewer disposition for one exact discovered native-component set.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalNativeComponentReviewDisposition {
    /// The exact ordered component set is approved for frozen-set compilation.
    Approved,
    /// The exact ordered component set is explicitly blocked.
    Blocked,
}

/// One exact component recorded in an external native-component review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalNativeComponentReviewMember {
    artifact_id: ArtifactId,
    byte_size: u64,
    mapping_class: NativeMappingClass,
}

impl ExternalNativeComponentReviewMember {
    /// Returns the exact reviewed object byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact reviewed object byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the exact reviewed executable mapping class.
    #[must_use]
    pub const fn mapping_class(&self) -> NativeMappingClass {
        self.mapping_class
    }
}

/// Canonical reviewer disposition for one exact non-authoritative discovery.
///
/// This record binds the runtime package, discovery, complete ordered component
/// sequence, disposition, and separate reviewer-owned evidence. It grants no
/// package or generation authority. Only an approved record can be compiled into
/// a frozen set, which still requires independent verification before use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalNativeComponentReview {
    runtime_package_manifest_id: RuntimePackageManifestId,
    discovery_digest: Digest,
    components: Vec<ExternalNativeComponentReviewMember>,
    disposition: ExternalNativeComponentReviewDisposition,
    reviewer_evidence_digest: Digest,
    canonical_json_bytes: Vec<u8>,
    review_digest: Digest,
}

impl ExternalNativeComponentReview {
    /// Compiles one canonical review from discovery and separate reviewer evidence.
    ///
    /// The caller owns reviewer authority. This function binds the exact decision
    /// and bytes but does not establish who performed the review or its adequacy.
    ///
    /// # Errors
    ///
    /// Returns [`ExternalNativeComponentReviewError`] for empty or excessive
    /// reviewer evidence or an invalid discovered component sequence.
    pub fn compile(
        discovery: &NativeLoadDiscovery,
        disposition: ExternalNativeComponentReviewDisposition,
        reviewer_evidence_bytes: &[u8],
    ) -> Result<Self, ExternalNativeComponentReviewError> {
        validate_reviewer_evidence(reviewer_evidence_bytes)?;
        Self::new(
            discovery.runtime_package_manifest_id().clone(),
            discovery.discovery_digest().clone(),
            review_members(discovery),
            disposition,
            Digest::sha256(reviewer_evidence_bytes),
        )
    }

    /// Independently verifies canonical review, discovery, and reviewer evidence bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ExternalNativeComponentReviewError`] for malformed,
    /// noncanonical, excessive, reordered, digest-drifting, or inconsistently
    /// bound input.
    pub fn verify(
        bytes: &[u8],
        discovery_bytes: &[u8],
        reviewer_evidence_bytes: &[u8],
        expected_package_id: &RuntimePackageManifestId,
    ) -> Result<Self, ExternalNativeComponentReviewError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_EXTERNAL_NATIVE_COMPONENT_REVIEW_JSON_BYTES {
            return Err(ExternalNativeComponentReviewError::LimitExceeded);
        }
        validate_reviewer_evidence(reviewer_evidence_bytes)?;
        let discovery = NativeLoadDiscovery::from_json_bytes(discovery_bytes, expected_package_id)
            .map_err(ExternalNativeComponentReviewError::Discovery)?;
        let wire: ReviewWire = serde_json::from_slice(bytes)
            .map_err(|_| ExternalNativeComponentReviewError::InvalidEncoding)?;
        if wire.schema_version != EXTERNAL_NATIVE_COMPONENT_REVIEW_SCHEMA_VERSION
            || &wire.runtime_package_manifest_id != expected_package_id
            || wire.discovery_digest != *discovery.discovery_digest()
            || wire.reviewer_evidence_digest != Digest::sha256(reviewer_evidence_bytes)
        {
            return Err(ExternalNativeComponentReviewError::InvalidBinding);
        }
        let verified = Self::new(
            wire.runtime_package_manifest_id,
            wire.discovery_digest,
            wire.components
                .into_iter()
                .map(|component| ExternalNativeComponentReviewMember {
                    artifact_id: component.artifact_id,
                    byte_size: component.byte_size,
                    mapping_class: component.mapping_class,
                })
                .collect(),
            wire.disposition,
            wire.reviewer_evidence_digest,
        )?;
        if verified.canonical_json_bytes != bytes {
            return Err(ExternalNativeComponentReviewError::InvalidEncoding);
        }
        if !same_review_members(&verified.components, discovery.external_components()) {
            return Err(ExternalNativeComponentReviewError::InvalidComponents);
        }
        Ok(verified)
    }

    fn new(
        runtime_package_manifest_id: RuntimePackageManifestId,
        discovery_digest: Digest,
        components: Vec<ExternalNativeComponentReviewMember>,
        disposition: ExternalNativeComponentReviewDisposition,
        reviewer_evidence_digest: Digest,
    ) -> Result<Self, ExternalNativeComponentReviewError> {
        validate_members(&components)?;
        let component_json = components
            .iter()
            .map(|component| {
                serde_json::json!({
                    "artifact_id": component.artifact_id,
                    "byte_size": component.byte_size,
                    "mapping_class": component.mapping_class
                })
            })
            .collect::<Vec<_>>();
        let canonical_json_bytes = serde_json::to_vec(&serde_json::json!({
            "components": component_json,
            "discovery_digest": discovery_digest,
            "disposition": disposition,
            "reviewer_evidence_digest": reviewer_evidence_digest,
            "runtime_package_manifest_id": runtime_package_manifest_id,
            "schema_version": EXTERNAL_NATIVE_COMPONENT_REVIEW_SCHEMA_VERSION
        }))
        .map_err(|_| ExternalNativeComponentReviewError::InvalidEncoding)?;
        if canonical_json_bytes.len() > MAXIMUM_EXTERNAL_NATIVE_COMPONENT_REVIEW_JSON_BYTES {
            return Err(ExternalNativeComponentReviewError::LimitExceeded);
        }
        let review_digest = Digest::sha256(&canonical_json_bytes);
        Ok(Self {
            runtime_package_manifest_id,
            discovery_digest,
            components,
            disposition,
            reviewer_evidence_digest,
            canonical_json_bytes,
            review_digest,
        })
    }

    /// Returns the exact runtime package reviewed by this record.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact non-authoritative discovery digest reviewed.
    #[must_use]
    pub const fn discovery_digest(&self) -> &Digest {
        &self.discovery_digest
    }

    /// Returns the exact reviewed components in canonical identity order.
    #[must_use]
    pub fn components(&self) -> &[ExternalNativeComponentReviewMember] {
        &self.components
    }

    /// Returns the explicit reviewer disposition.
    #[must_use]
    pub const fn disposition(&self) -> ExternalNativeComponentReviewDisposition {
        self.disposition
    }

    /// Returns the digest of the separate reviewer-owned evidence bytes.
    #[must_use]
    pub const fn reviewer_evidence_digest(&self) -> &Digest {
        &self.reviewer_evidence_digest
    }

    /// Returns canonical content-free review JSON.
    #[must_use]
    pub fn canonical_json_bytes(&self) -> &[u8] {
        &self.canonical_json_bytes
    }

    /// Returns the digest defining this complete canonical review.
    #[must_use]
    pub const fn review_digest(&self) -> &Digest {
        &self.review_digest
    }
}

/// External native-component review validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ExternalNativeComponentReviewError {
    /// One encoded input was empty or exceeded its fixed ceiling.
    #[error("external native-component review limit was exceeded")]
    LimitExceeded,
    /// Review JSON was malformed, ambiguous, or noncanonical.
    #[error("external native-component review encoding is invalid")]
    InvalidEncoding,
    /// Package, discovery, or reviewer-evidence identity did not match.
    #[error("external native-component review binding is invalid")]
    InvalidBinding,
    /// Components were invalid, reordered, duplicated, or differed from discovery.
    #[error("external native-component review components are invalid")]
    InvalidComponents,
    /// The bound discovery report was invalid.
    #[error("external native-component discovery is invalid: {0}")]
    Discovery(NativeLoadObserverError),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewWire {
    components: Vec<ComponentWire>,
    discovery_digest: Digest,
    disposition: ExternalNativeComponentReviewDisposition,
    reviewer_evidence_digest: Digest,
    runtime_package_manifest_id: RuntimePackageManifestId,
    schema_version: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentWire {
    artifact_id: ArtifactId,
    byte_size: u64,
    mapping_class: NativeMappingClass,
}

pub(super) fn validate_reviewer_evidence(
    reviewer_evidence_bytes: &[u8],
) -> Result<(), ExternalNativeComponentReviewError> {
    if reviewer_evidence_bytes.is_empty()
        || reviewer_evidence_bytes.len() > MAXIMUM_EXTERNAL_COMPONENT_REVIEW_EVIDENCE_BYTES
    {
        Err(ExternalNativeComponentReviewError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn review_members(discovery: &NativeLoadDiscovery) -> Vec<ExternalNativeComponentReviewMember> {
    discovery
        .external_components()
        .iter()
        .map(|component| ExternalNativeComponentReviewMember {
            artifact_id: component.artifact_id().clone(),
            byte_size: component.byte_size(),
            mapping_class: component.mapping_class(),
        })
        .collect()
}

fn validate_members(
    components: &[ExternalNativeComponentReviewMember],
) -> Result<(), ExternalNativeComponentReviewError> {
    if components.len() > MAXIMUM_NATIVE_LOADED_COMPONENTS
        || components.iter().any(|component| {
            component.byte_size == 0
                || component.mapping_class != NativeMappingClass::ExecutableMapped
        })
        || components.windows(2).any(|pair| {
            review_key(&pair[0]) >= review_key(&pair[1])
                || pair[0].artifact_id.digest().as_str() >= pair[1].artifact_id.digest().as_str()
        })
    {
        Err(ExternalNativeComponentReviewError::InvalidComponents)
    } else {
        Ok(())
    }
}

fn review_key(component: &ExternalNativeComponentReviewMember) -> Vec<u8> {
    expected_key(&ExpectedExternalNativeComponent::new(
        component.artifact_id.clone(),
        component.byte_size,
        component.mapping_class,
    ))
}

fn same_review_members(
    reviewed: &[ExternalNativeComponentReviewMember],
    discovered: &[super::DiscoveredExternalNativeComponent],
) -> bool {
    reviewed.len() == discovered.len()
        && reviewed
            .iter()
            .zip(discovered)
            .all(|(reviewed, discovered)| {
                reviewed.artifact_id() == discovered.artifact_id()
                    && reviewed.byte_size() == discovered.byte_size()
                    && reviewed.mapping_class() == discovered.mapping_class()
            })
}
