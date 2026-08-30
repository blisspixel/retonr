use rewrite_ollama_package::RuntimeSourceBuildInputRole;
use rewrite_types::{CancellationToken, Digest};
use serde::{Deserialize, Serialize};

use super::super::VerifiedRuntimeAdmissionFoundationBinding;
use super::common::{
    MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES,
    MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES, RuntimeAdmissionStaticControlError,
    RuntimeAdmissionStaticEvidenceView, digest, encode, ensure_same_foundation, find_role, finish,
    fixed_path, parse_canonical,
};
use crate::RuntimeSourceBuildEvidenceBundleLease;

mod inventory;

/// Fixed license adjudication procedure identity.
pub const RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID: &str =
    "retonr:runtime-admission:license:procedure";
/// Fixed license adjudication procedure version.
pub const RUNTIME_ADMISSION_LICENSE_PROCEDURE_VERSION: u32 = 2;

const CONTROL_DOMAIN: &[u8] = b"retonr:runtime-admission:license-control:v2";
const LICENSE_INVENTORY_PATH: &str = "legal/licenses.json";
const CARGO_LOCK_PATH: &str = "lineage/source/Cargo.lock";

/// Domain-separated identity of one canonical license control.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionLicenseControlId(Digest);

impl RuntimeAdmissionLicenseControlId {
    /// Returns the digest defining this control identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Canonical license publication material without admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRuntimeAdmissionLicenseControl {
    canonical_bytes: Vec<u8>,
    control_id: RuntimeAdmissionLicenseControlId,
}

impl CompiledRuntimeAdmissionLicenseControl {
    /// Returns exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated publication identity.
    #[must_use]
    pub const fn control_id(&self) -> &RuntimeAdmissionLicenseControlId {
        &self.control_id
    }
}

/// Independently verified passed license result.
///
/// This value is inert and cannot be converted into runtime admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPassedRuntimeAdmissionLicenseControl {
    control_id: RuntimeAdmissionLicenseControlId,
    foundation_id: super::super::RuntimeAdmissionEvidenceFoundationId,
}

impl VerifiedPassedRuntimeAdmissionLicenseControl {
    #[cfg(test)]
    pub(crate) fn test_fixture(
        foundation_id: super::super::RuntimeAdmissionEvidenceFoundationId,
    ) -> Self {
        Self {
            control_id: RuntimeAdmissionLicenseControlId(Digest::sha256(b"license control")),
            foundation_id,
        }
    }

    /// Returns the verified control identity.
    #[must_use]
    pub const fn control_id(&self) -> &RuntimeAdmissionLicenseControlId {
        &self.control_id
    }

    /// Returns the exact verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &super::super::RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }
}

/// Compiler for inert license publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionLicenseControlCompiler;

impl RuntimeAdmissionLicenseControlCompiler {
    /// Compiles an approved complete license review against retained evidence.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionStaticControlError`] for invalid review bytes,
    /// pending or incomplete dispositions, evidence drift, or identity mismatch.
    pub fn compile(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        reviewer_bytes: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<CompiledRuntimeAdmissionLicenseControl, RuntimeAdmissionStaticControlError> {
        let review: LicenseReviewWire = parse_canonical(
            reviewer_bytes,
            MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
        )?;
        compile_view(foundation, evidence, review, cancellation)
    }
}

/// Independent verifier for canonical license publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionLicenseControlVerifier;

impl RuntimeAdmissionLicenseControlVerifier {
    /// Verifies canonical control bytes against the same retained foundation.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionStaticControlError`] for malformed,
    /// noncanonical, mismatched, changing, pending, or incomplete evidence.
    pub fn verify(
        bytes: &[u8],
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        cancellation: &CancellationToken,
    ) -> Result<VerifiedPassedRuntimeAdmissionLicenseControl, RuntimeAdmissionStaticControlError>
    {
        verify_view(bytes, foundation, evidence, cancellation)
    }
}

pub(super) fn compile_view<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    review: LicenseReviewWire,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeAdmissionLicenseControl, RuntimeAdmissionStaticControlError> {
    let wire = derive_wire(foundation, evidence, review, cancellation)?;
    let canonical_bytes = encode(&wire)?;
    finish(foundation, evidence, cancellation)?;
    let control_id = RuntimeAdmissionLicenseControlId(digest(CONTROL_DOMAIN, &canonical_bytes));
    Ok(CompiledRuntimeAdmissionLicenseControl {
        canonical_bytes,
        control_id,
    })
}

pub(super) fn verify_view<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    bytes: &[u8],
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    cancellation: &CancellationToken,
) -> Result<VerifiedPassedRuntimeAdmissionLicenseControl, RuntimeAdmissionStaticControlError> {
    let wire: LicenseControlWire =
        parse_canonical(bytes, MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES)?;
    let expected = derive_wire(foundation, evidence, wire.review.clone(), cancellation)?;
    if wire != expected || encode(&expected)? != bytes {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    finish(foundation, evidence, cancellation)?;
    Ok(VerifiedPassedRuntimeAdmissionLicenseControl {
        control_id: RuntimeAdmissionLicenseControlId(digest(CONTROL_DOMAIN, bytes)),
        foundation_id: foundation.foundation_id().clone(),
    })
}

fn derive_wire<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    review: LicenseReviewWire,
    cancellation: &CancellationToken,
) -> Result<LicenseControlWire, RuntimeAdmissionStaticControlError> {
    let facts = ensure_same_foundation(foundation, evidence, cancellation)?;
    if review.disposition != ReviewDisposition::Approved
        || review.procedure_id != RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID
        || review.procedure_version != RUNTIME_ADMISSION_LICENSE_PROCEDURE_VERSION
        || review.schema_version != 2
    {
        return Err(RuntimeAdmissionStaticControlError::ReviewRequired);
    }
    let inventory_path = fixed_path(LICENSE_INVENTORY_PATH)?;
    let inventory_bytes = evidence.read_source_input_with_limit(
        &inventory_path,
        u64::try_from(inventory::MAXIMUM_INVENTORY_BYTES).unwrap_or(u64::MAX),
        cancellation,
    )?;
    let inventory_component = find_role(&facts, RuntimeSourceBuildInputRole::LicenseEvidence)?;
    if inventory_component.relative_path != inventory_path
        || inventory_component.byte_size != u64::try_from(inventory_bytes.len()).unwrap_or(u64::MAX)
        || inventory_component.digest != Digest::sha256(&inventory_bytes)
    {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    let lock_path = fixed_path(CARGO_LOCK_PATH)?;
    let lock_bytes = evidence.read_source_input(&lock_path, cancellation)?;
    let lock_component = find_role(&facts, RuntimeSourceBuildInputRole::CargoLockfile)?;
    let lock_digest = Digest::sha256(&lock_bytes);
    if lock_component.relative_path != lock_path
        || lock_component.byte_size != u64::try_from(lock_bytes.len()).unwrap_or(u64::MAX)
        || lock_component.digest != lock_digest
    {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    let verified = inventory::verify(&facts, &inventory_bytes, &lock_bytes)?;
    if review.inventory_digest != verified.digest
        || review.subjects.len() != verified.subjects.len()
        || review
            .subjects
            .iter()
            .zip(&verified.subjects)
            .any(|(reviewed, retained)| {
                reviewed.subject_key != retained.key
                    || reviewed.inventory_subject_id != retained.identity
            })
        || facts.license_members.is_empty()
        || facts
            .license_members
            .windows(2)
            .any(|pair| pair[0].relative_path.as_str() >= pair[1].relative_path.as_str())
        || review.runtime_license_members.len() != facts.license_members.len()
        || review
            .runtime_license_members
            .iter()
            .zip(&facts.license_members)
            .any(|(reviewed, retained)| {
                reviewed.relative_path != retained.relative_path.as_str()
                    || reviewed.byte_size != retained.byte_size
                    || reviewed.digest != retained.digest
            })
    {
        return Err(RuntimeAdmissionStaticControlError::ReviewRequired);
    }
    Ok(LicenseControlWire {
        control: LicenseControl::License,
        evidence: LicenseEvidenceWire {
            cargo_lock_digest: lock_digest,
            license_inventory_digest: verified.digest,
            material_byte_count: verified.material_bytes,
            material_count: verified.material_count,
            runtime_package_manifest_id: facts.runtime_package_manifest_id,
            subject_count: verified.subjects.len(),
        },
        foundation_id: foundation.foundation_id().digest().clone(),
        procedure_id: RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID.to_owned(),
        procedure_version: RUNTIME_ADMISSION_LICENSE_PROCEDURE_VERSION,
        review,
        schema_version: 2,
        status: PassedStatus::Passed,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LicenseControlWire {
    control: LicenseControl,
    evidence: LicenseEvidenceWire,
    foundation_id: Digest,
    procedure_id: String,
    procedure_version: u32,
    review: LicenseReviewWire,
    schema_version: u32,
    status: PassedStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LicenseControl {
    License,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LicenseEvidenceWire {
    cargo_lock_digest: Digest,
    license_inventory_digest: Digest,
    material_byte_count: usize,
    material_count: usize,
    runtime_package_manifest_id: rewrite_model::RuntimePackageManifestId,
    subject_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LicenseReviewWire {
    disposition: ReviewDisposition,
    inventory_digest: Digest,
    procedure_id: String,
    procedure_version: u32,
    runtime_license_members: Vec<LicenseMemberWire>,
    schema_version: u32,
    subjects: Vec<LicenseSubjectReviewWire>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewDisposition {
    Approved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LicenseSubjectReviewWire {
    disposition: SubjectDisposition,
    inventory_subject_id: Digest,
    subject_key: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SubjectDisposition {
    ApprovedBuildOnly,
    ApprovedRuntimeDistribution,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LicenseMemberWire {
    byte_size: u64,
    digest: Digest,
    relative_path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassedStatus {
    Passed,
}
