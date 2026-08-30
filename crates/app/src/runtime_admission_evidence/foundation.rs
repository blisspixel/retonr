use rewrite_model::{ArtifactSetId, RuntimePackageManifestId};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use super::contract::{RuntimeAdmissionEvidenceContractError, domain_separated_digest};

/// Current canonical inert-foundation contract version.
pub const RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_SCHEMA_VERSION: u32 = 1;
/// Maximum canonical JSON bytes accepted for one inert foundation.
pub const MAX_RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_JSON_BYTES: usize = 65_536;

const FOUNDATION_ID_DOMAIN: &[u8] = b"retonr:runtime-admission-evidence-foundation:v1";

/// Domain-separated identity of one exact canonical inert foundation.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionEvidenceFoundationId(Digest);

impl RuntimeAdmissionEvidenceFoundationId {
    /// Returns the digest defining this foundation identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Typed immutable subjects available before control and execution closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidenceFoundationInput {
    source_build_evidence_bundle_id: ArtifactSetId,
    source_build_inputs_id: ArtifactSetId,
    source_manifest_digest: Digest,
    build_plan_digest: Digest,
    source_report_digest: Digest,
    runtime_package_manifest_id: RuntimePackageManifestId,
}

impl RuntimeAdmissionEvidenceFoundationInput {
    /// Creates one inert subject-binding input.
    #[must_use]
    pub fn new(
        source_build_evidence_bundle_id: ArtifactSetId,
        source_build_inputs_id: ArtifactSetId,
        source_manifest_digest: Digest,
        build_plan_digest: Digest,
        source_report_digest: Digest,
        runtime_package_manifest_id: RuntimePackageManifestId,
    ) -> Self {
        Self {
            source_build_evidence_bundle_id,
            source_build_inputs_id,
            source_manifest_digest,
            build_plan_digest,
            source_report_digest,
            runtime_package_manifest_id,
        }
    }
}

/// Canonical subject binding that explicitly confers no admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidenceFoundation {
    wire: FoundationWire,
    canonical_bytes: Vec<u8>,
    foundation_id: RuntimeAdmissionEvidenceFoundationId,
}

impl RuntimeAdmissionEvidenceFoundation {
    /// Compiles typed immutable subjects into canonical bounded JSON.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError`] if canonical encoding
    /// exceeds the hard foundation ceiling.
    pub fn compile(
        input: RuntimeAdmissionEvidenceFoundationInput,
    ) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        let wire = FoundationWire {
            authority: FoundationAuthority::None,
            build_plan_digest: input.build_plan_digest,
            runtime_package_manifest_id: input.runtime_package_manifest_id,
            schema_version: RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_SCHEMA_VERSION,
            source_build_evidence_bundle_id: input.source_build_evidence_bundle_id,
            source_build_inputs_id: input.source_build_inputs_id,
            source_manifest_digest: input.source_manifest_digest,
            source_report_digest: input.source_report_digest,
        };
        Self::from_wire(wire)
    }

    /// Parses an exact canonical inert-foundation encoding.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError`] for malformed,
    /// noncanonical, overlong, or unsupported input.
    pub fn from_canonical_bytes(
        bytes: &[u8],
    ) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        if bytes.is_empty() || bytes.len() > MAX_RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_JSON_BYTES {
            return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
        }
        let wire: FoundationWire = serde_json::from_slice(bytes)
            .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidEncoding)?;
        if wire.schema_version != RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_SCHEMA_VERSION {
            return Err(RuntimeAdmissionEvidenceContractError::InvalidBinding);
        }
        let compiled = Self::from_wire(wire)?;
        if compiled.canonical_bytes != bytes {
            return Err(RuntimeAdmissionEvidenceContractError::InvalidEncoding);
        }
        Ok(compiled)
    }

    /// Returns the exact canonical JSON bytes defining this foundation.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated identity of the canonical bytes.
    #[must_use]
    pub const fn foundation_id(&self) -> &RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }

    /// Returns the durable controlled-build evidence identity.
    #[must_use]
    pub const fn source_build_evidence_bundle_id(&self) -> &ArtifactSetId {
        &self.wire.source_build_evidence_bundle_id
    }

    /// Returns the frozen controlled-build input identity.
    #[must_use]
    pub const fn source_build_inputs_id(&self) -> &ArtifactSetId {
        &self.wire.source_build_inputs_id
    }

    /// Returns the complete canonical controlled-build manifest digest.
    #[must_use]
    pub const fn source_manifest_digest(&self) -> &Digest {
        &self.wire.source_manifest_digest
    }

    /// Returns the controlled-build execution-plan digest.
    #[must_use]
    pub const fn build_plan_digest(&self) -> &Digest {
        &self.wire.build_plan_digest
    }

    /// Returns the verified two-attempt source-build report digest.
    #[must_use]
    pub const fn source_report_digest(&self) -> &Digest {
        &self.wire.source_report_digest
    }

    /// Returns the reconstructed runtime-package manifest identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.wire.runtime_package_manifest_id
    }

    fn from_wire(wire: FoundationWire) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        if wire.schema_version != RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_SCHEMA_VERSION {
            return Err(RuntimeAdmissionEvidenceContractError::InvalidBinding);
        }
        let canonical_bytes = serde_json::to_vec(&wire)
            .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidEncoding)?;
        if canonical_bytes.len() > MAX_RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_JSON_BYTES {
            return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
        }
        let foundation_id = RuntimeAdmissionEvidenceFoundationId(domain_separated_digest(
            FOUNDATION_ID_DOMAIN,
            &canonical_bytes,
        ));
        Ok(Self {
            wire,
            canonical_bytes,
            foundation_id,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FoundationAuthority {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FoundationWire {
    authority: FoundationAuthority,
    build_plan_digest: Digest,
    runtime_package_manifest_id: RuntimePackageManifestId,
    schema_version: u32,
    source_build_evidence_bundle_id: ArtifactSetId,
    source_build_inputs_id: ArtifactSetId,
    source_manifest_digest: Digest,
    source_report_digest: Digest,
}
