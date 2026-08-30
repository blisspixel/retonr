use rewrite_model::{
    ArtifactId, NativeLoadEvidenceClass, NativeMappingClass, RuntimePackageManifest,
    RuntimePackageManifestId,
};
#[cfg(target_os = "linux")]
use rewrite_model::{NativeLoadOrigin, NativeLoadedComponent};
use rewrite_types::Digest;
use serde::Deserialize;

use super::{
    ExpectedExternalNativeComponent, MAXIMUM_NATIVE_LOADED_COMPONENTS, expected_key,
    validate_subject,
};
use crate::{NativeLoadObservationLimits, NativeLoadObserverError, RetainedNativePackageMember};

/// Current external native-component discovery report version.
pub const NATIVE_LOAD_DISCOVERY_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded bytes accepted for one discovery proposal.
pub const MAXIMUM_NATIVE_LOAD_DISCOVERY_JSON_BYTES: usize = 1_048_576;

const MAXIMUM_OBSERVATION_CONTRACT_ID_BYTES: usize = 64;

/// Complete caller input for one non-authoritative external-component discovery.
pub struct NativeLoadDiscoveryRequest<'a> {
    /// Exact typed runtime package used to classify packaged mappings.
    pub package: &'a RuntimePackageManifest,
    /// Expected content-derived identity of `package`.
    pub expected_package_id: &'a RuntimePackageManifestId,
    /// Exact retained file objects for every packaged code member.
    pub retained_package_members: &'a [RetainedNativePackageMember],
    /// Hard caller-selected resource ceilings.
    pub limits: NativeLoadObservationLimits,
}

impl NativeLoadDiscoveryRequest<'_> {
    pub(crate) fn validate(&self) -> Result<NativeLoadObservationLimits, NativeLoadObserverError> {
        validate_subject(
            self.package,
            self.expected_package_id,
            self.retained_package_members,
            self.limits,
        )
    }
}

/// One external executable object proposed by non-authoritative discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredExternalNativeComponent {
    artifact_id: ArtifactId,
    byte_size: u64,
    mapping_class: NativeMappingClass,
}

impl DiscoveredExternalNativeComponent {
    /// Returns the exact observed object byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact observed object byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the observed executable mapping class.
    #[must_use]
    pub const fn mapping_class(&self) -> NativeMappingClass {
        self.mapping_class
    }
}

/// Content-free, explicitly non-authoritative external-component proposal.
///
/// This report cannot be used as a native-load allowlist. A reviewer must create a
/// distinct frozen policy before the existing verification observer can consume it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLoadDiscovery {
    runtime_package_manifest_id: RuntimePackageManifestId,
    process_evidence_digest: Digest,
    external_components: Vec<DiscoveredExternalNativeComponent>,
    canonical_json_bytes: Vec<u8>,
    discovery_digest: Digest,
}

impl NativeLoadDiscovery {
    #[cfg(target_os = "linux")]
    pub(crate) fn from_observed_components(
        package: &RuntimePackageManifest,
        process_evidence_digest: Digest,
        evidence_class: NativeLoadEvidenceClass,
        observation_contract_id: &str,
        observation_contract_schema_version: u32,
        components: &[NativeLoadedComponent],
    ) -> Result<Self, NativeLoadObserverError> {
        let external_components = components
            .iter()
            .filter(|component| {
                matches!(
                    component.origin(),
                    NativeLoadOrigin::ExternalPlatformComponent
                )
            })
            .map(|component| DiscoveredExternalNativeComponent {
                artifact_id: component.artifact_id().clone(),
                byte_size: component.byte_size(),
                mapping_class: component.mapping_class(),
            })
            .collect::<Vec<_>>();
        Self::new(
            package.runtime_package_manifest_id(),
            process_evidence_digest,
            evidence_class,
            observation_contract_id,
            observation_contract_schema_version,
            external_components,
        )
    }

    fn new(
        runtime_package_manifest_id: RuntimePackageManifestId,
        process_evidence_digest: Digest,
        evidence_class: NativeLoadEvidenceClass,
        observation_contract_id: &str,
        observation_contract_schema_version: u32,
        external_components: Vec<DiscoveredExternalNativeComponent>,
    ) -> Result<Self, NativeLoadObserverError> {
        if evidence_class != NativeLoadEvidenceClass::LinuxProcMapFiles
            || observation_contract_schema_version == 0
            || !valid_contract_id(observation_contract_id)
            || external_components.len() > MAXIMUM_NATIVE_LOADED_COMPONENTS
            || external_components.iter().any(|component| {
                component.byte_size == 0
                    || component.mapping_class != NativeMappingClass::ExecutableMapped
            })
            || external_components.windows(2).any(|pair| {
                discovered_key(&pair[0]) >= discovered_key(&pair[1])
                    || pair[0].artifact_id.digest().as_str()
                        >= pair[1].artifact_id.digest().as_str()
            })
        {
            return Err(NativeLoadObserverError::InvalidObservation);
        }
        let external_json = external_components
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
            "authority": "none",
            "evidence_class": evidence_class,
            "external_components": external_json,
            "observation_contract_id": observation_contract_id,
            "observation_contract_schema_version": observation_contract_schema_version,
            "process_evidence_digest": process_evidence_digest,
            "runtime_package_manifest_id": runtime_package_manifest_id,
            "schema_version": NATIVE_LOAD_DISCOVERY_SCHEMA_VERSION,
            "status": "proposed"
        }))
        .map_err(|_| NativeLoadObserverError::InvalidObservation)?;
        let discovery_digest = Digest::sha256(&canonical_json_bytes);
        Ok(Self {
            runtime_package_manifest_id,
            process_evidence_digest,
            external_components,
            canonical_json_bytes,
            discovery_digest,
        })
    }

    /// Parses one canonical content-free proposal without granting it authority.
    ///
    /// # Errors
    ///
    /// Returns [`NativeLoadObserverError::InvalidObservation`] for excessive,
    /// malformed, noncanonical, reordered, unsupported, or inconsistently bound
    /// proposal bytes.
    pub fn from_json_bytes(
        bytes: &[u8],
        expected_package_id: &RuntimePackageManifestId,
    ) -> Result<Self, NativeLoadObserverError> {
        if bytes.is_empty() || bytes.len() > MAXIMUM_NATIVE_LOAD_DISCOVERY_JSON_BYTES {
            return Err(NativeLoadObserverError::InvalidObservation);
        }
        let wire: DiscoveryWire = serde_json::from_slice(bytes)
            .map_err(|_| NativeLoadObserverError::InvalidObservation)?;
        if wire.schema_version != NATIVE_LOAD_DISCOVERY_SCHEMA_VERSION
            || &wire.runtime_package_manifest_id != expected_package_id
        {
            return Err(NativeLoadObserverError::InvalidObservation);
        }
        let Authority::None = wire.authority;
        let Status::Proposed = wire.status;
        let parsed = Self::new(
            wire.runtime_package_manifest_id,
            wire.process_evidence_digest,
            wire.evidence_class,
            &wire.observation_contract_id,
            wire.observation_contract_schema_version,
            wire.external_components
                .into_iter()
                .map(|component| DiscoveredExternalNativeComponent {
                    artifact_id: component.artifact_id,
                    byte_size: component.byte_size,
                    mapping_class: component.mapping_class,
                })
                .collect(),
        )?;
        if parsed.canonical_json_bytes != bytes {
            return Err(NativeLoadObserverError::InvalidObservation);
        }
        Ok(parsed)
    }

    /// Returns the exact runtime-package identity used for classification.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the retained process-evidence binding.
    #[must_use]
    pub const fn process_evidence_digest(&self) -> &Digest {
        &self.process_evidence_digest
    }

    /// Returns proposed external objects in canonical identity order.
    #[must_use]
    pub fn external_components(&self) -> &[DiscoveredExternalNativeComponent] {
        &self.external_components
    }

    /// Returns the canonical content-free proposal JSON.
    #[must_use]
    pub fn canonical_json_bytes(&self) -> &[u8] {
        &self.canonical_json_bytes
    }

    /// Returns the SHA-256 digest of the canonical proposal JSON.
    #[must_use]
    pub const fn discovery_digest(&self) -> &Digest {
        &self.discovery_digest
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoveryWire {
    authority: Authority,
    evidence_class: NativeLoadEvidenceClass,
    external_components: Vec<ComponentWire>,
    observation_contract_id: String,
    observation_contract_schema_version: u32,
    process_evidence_digest: Digest,
    runtime_package_manifest_id: RuntimePackageManifestId,
    schema_version: u32,
    status: Status,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Authority {
    None,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Proposed,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentWire {
    artifact_id: ArtifactId,
    byte_size: u64,
    mapping_class: NativeMappingClass,
}

fn discovered_key(component: &DiscoveredExternalNativeComponent) -> Vec<u8> {
    let expected = ExpectedExternalNativeComponent::new(
        component.artifact_id.clone(),
        component.byte_size,
        component.mapping_class,
    );
    expected_key(&expected)
}

fn valid_contract_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_OBSERVATION_CONTRACT_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}
