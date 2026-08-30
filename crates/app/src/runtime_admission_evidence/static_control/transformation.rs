use rewrite_model::ArtifactSetId;
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

/// Fixed transformation adjudication procedure identity.
pub const RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID: &str =
    "retonr:runtime-admission:transformation:procedure";
/// Fixed transformation adjudication procedure version.
pub const RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_VERSION: u32 = 1;

const CONTROL_DOMAIN: &[u8] = b"retonr:runtime-admission:transformation-control:v1";
const PRIMARY_TRANSFORMATION_PATH: &str = "attempts/primary/transformation.json";
const REBUILD_TRANSFORMATION_PATH: &str = "attempts/rebuild/transformation.json";
const BUILD_STEPS: [&str; 8] = [
    "extract-frozen-inputs",
    "verify-self-contained-tools",
    "apply-ollama-compatibility-patch",
    "configure-llama-cpp-cpu",
    "build-llama-cpp-cpu",
    "install-llama-cpp-cpu",
    "build-ollama-go-entrypoint",
    "assemble-runtime-package",
];

/// Domain-separated identity of one canonical transformation control.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionTransformationControlId(Digest);

impl RuntimeAdmissionTransformationControlId {
    /// Returns the digest defining this control identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Canonical transformation publication material without admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRuntimeAdmissionTransformationControl {
    canonical_bytes: Vec<u8>,
    control_id: RuntimeAdmissionTransformationControlId,
}

impl CompiledRuntimeAdmissionTransformationControl {
    /// Returns exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated publication identity.
    #[must_use]
    pub const fn control_id(&self) -> &RuntimeAdmissionTransformationControlId {
        &self.control_id
    }
}

/// Independently verified passed transformation result.
///
/// This value is inert and cannot be converted into runtime admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPassedRuntimeAdmissionTransformationControl {
    control_id: RuntimeAdmissionTransformationControlId,
    foundation_id: super::super::RuntimeAdmissionEvidenceFoundationId,
}

impl VerifiedPassedRuntimeAdmissionTransformationControl {
    #[cfg(test)]
    pub(crate) fn test_fixture(
        foundation_id: super::super::RuntimeAdmissionEvidenceFoundationId,
    ) -> Self {
        Self {
            control_id: RuntimeAdmissionTransformationControlId(Digest::sha256(
                b"transformation control",
            )),
            foundation_id,
        }
    }

    /// Returns the verified control identity.
    #[must_use]
    pub const fn control_id(&self) -> &RuntimeAdmissionTransformationControlId {
        &self.control_id
    }

    /// Returns the exact verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &super::super::RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }
}

/// Compiler for inert transformation publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionTransformationControlCompiler;

impl RuntimeAdmissionTransformationControlCompiler {
    /// Compiles one reviewer-approved deterministic transformation record.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionStaticControlError`] for invalid review bytes,
    /// evidence drift, a nonidentical build, or any transformation mismatch.
    pub fn compile(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        reviewer_bytes: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<CompiledRuntimeAdmissionTransformationControl, RuntimeAdmissionStaticControlError>
    {
        let review: TransformationReviewWire = parse_canonical(
            reviewer_bytes,
            MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
        )?;
        compile_view(foundation, evidence, review, cancellation)
    }
}

/// Independent verifier for canonical transformation publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionTransformationControlVerifier;

impl RuntimeAdmissionTransformationControlVerifier {
    /// Verifies canonical control bytes against the same retained foundation.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionStaticControlError`] for malformed,
    /// noncanonical, mismatched, changing, or insufficient evidence.
    pub fn verify(
        bytes: &[u8],
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        cancellation: &CancellationToken,
    ) -> Result<
        VerifiedPassedRuntimeAdmissionTransformationControl,
        RuntimeAdmissionStaticControlError,
    > {
        verify_view(bytes, foundation, evidence, cancellation)
    }
}

pub(super) fn compile_view<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    review: TransformationReviewWire,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeAdmissionTransformationControl, RuntimeAdmissionStaticControlError> {
    let wire = derive_wire(foundation, evidence, review, cancellation)?;
    let canonical_bytes = encode(&wire)?;
    finish(foundation, evidence, cancellation)?;
    let control_id =
        RuntimeAdmissionTransformationControlId(digest(CONTROL_DOMAIN, &canonical_bytes));
    Ok(CompiledRuntimeAdmissionTransformationControl {
        canonical_bytes,
        control_id,
    })
}

pub(super) fn verify_view<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    bytes: &[u8],
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    cancellation: &CancellationToken,
) -> Result<VerifiedPassedRuntimeAdmissionTransformationControl, RuntimeAdmissionStaticControlError>
{
    let wire: TransformationControlWire =
        parse_canonical(bytes, MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES)?;
    let expected = derive_wire(foundation, evidence, wire.review.clone(), cancellation)?;
    if wire != expected || encode(&expected)? != bytes {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    finish(foundation, evidence, cancellation)?;
    Ok(VerifiedPassedRuntimeAdmissionTransformationControl {
        control_id: RuntimeAdmissionTransformationControlId(digest(CONTROL_DOMAIN, bytes)),
        foundation_id: foundation.foundation_id().clone(),
    })
}

fn derive_wire<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    review: TransformationReviewWire,
    cancellation: &CancellationToken,
) -> Result<TransformationControlWire, RuntimeAdmissionStaticControlError> {
    let facts = ensure_same_foundation(foundation, evidence, cancellation)?;
    if review.disposition != ReviewDisposition::Approved
        || review.procedure_id != RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID
        || review.procedure_version != RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_VERSION
        || review.schema_version != 1
        || review.approved_build_program_digest != facts.build_program_digest
    {
        return Err(RuntimeAdmissionStaticControlError::ReviewRequired);
    }
    let patches = facts
        .components
        .iter()
        .filter(|component| {
            component
                .roles
                .contains(&RuntimeSourceBuildInputRole::SourcePatch)
        })
        .map(|component| PatchWire {
            digest: component.digest.clone(),
            relative_path: component.relative_path.as_str().to_owned(),
            revision: component.revision.clone(),
        })
        .collect::<Vec<_>>();
    if patches.is_empty()
        || patches
            .windows(2)
            .any(|pair| pair[0].relative_path >= pair[1].relative_path)
        || review.approved_patch_digests
            != patches
                .iter()
                .map(|patch| patch.digest.clone())
                .collect::<Vec<_>>()
    {
        return Err(RuntimeAdmissionStaticControlError::ReviewRequired);
    }
    let parameters = find_role(&facts, RuntimeSourceBuildInputRole::BuildParameters)?;
    let tools = find_role(&facts, RuntimeSourceBuildInputRole::ToolEvidence)?;
    let primary_bytes =
        evidence.read_evidence(&fixed_path(PRIMARY_TRANSFORMATION_PATH)?, cancellation)?;
    let rebuild_bytes =
        evidence.read_evidence(&fixed_path(REBUILD_TRANSFORMATION_PATH)?, cancellation)?;
    let primary: TransformationEvidenceWire = parse_canonical(
        &primary_bytes,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    let rebuild: TransformationEvidenceWire = parse_canonical(
        &rebuild_bytes,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    if primary != rebuild
        || primary.schema_version != 1
        || primary.accelerator != Accelerator::CpuOnly
        || primary.network_access != NetworkAccess::Denied
        || primary
            .build_steps
            .iter()
            .map(String::as_str)
            .ne(BUILD_STEPS)
        || primary.parameters_digest != parameters.digest
        || primary.source_inputs_id != foundation.snapshot.source_build_inputs_id
        || primary.tool_evidence_digest != tools.digest
    {
        return Err(RuntimeAdmissionStaticControlError::EvidenceInsufficient);
    }
    Ok(TransformationControlWire {
        build_program: BuildProgramWire {
            digest: facts.build_program_digest.clone(),
            relative_path: facts.build_program_path.as_str().to_owned(),
        },
        control: TransformationControl::Transformation,
        evidence: TransformationDigestsWire {
            parameters: parameters.digest.clone(),
            primary_transformation: Digest::sha256(&primary_bytes),
            rebuild_transformation: Digest::sha256(&rebuild_bytes),
            source_report: foundation.snapshot.source_report_digest.clone(),
            tool_evidence: tools.digest.clone(),
        },
        foundation_id: foundation.foundation_id().digest().clone(),
        patches,
        procedure_id: RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID.to_owned(),
        procedure_version: RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_VERSION,
        review,
        schema_version: 1,
        status: PassedStatus::Passed,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TransformationControlWire {
    build_program: BuildProgramWire,
    control: TransformationControl,
    evidence: TransformationDigestsWire,
    foundation_id: Digest,
    patches: Vec<PatchWire>,
    procedure_id: String,
    procedure_version: u32,
    review: TransformationReviewWire,
    schema_version: u32,
    status: PassedStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildProgramWire {
    digest: Digest,
    relative_path: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TransformationControl {
    Transformation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TransformationDigestsWire {
    #[serde(rename = "parameters_digest")]
    parameters: Digest,
    #[serde(rename = "primary_transformation_digest")]
    primary_transformation: Digest,
    #[serde(rename = "rebuild_transformation_digest")]
    rebuild_transformation: Digest,
    #[serde(rename = "source_report_digest")]
    source_report: Digest,
    #[serde(rename = "tool_evidence_digest")]
    tool_evidence: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PatchWire {
    digest: Digest,
    relative_path: String,
    revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TransformationReviewWire {
    approved_build_program_digest: Digest,
    approved_patch_digests: Vec<Digest>,
    disposition: ReviewDisposition,
    procedure_id: String,
    procedure_version: u32,
    schema_version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewDisposition {
    Approved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassedStatus {
    Passed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Accelerator {
    CpuOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum NetworkAccess {
    Denied,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct TransformationEvidenceWire {
    accelerator: Accelerator,
    build_steps: Vec<String>,
    network_access: NetworkAccess,
    parameters_digest: Digest,
    schema_version: u32,
    source_inputs_id: ArtifactSetId,
    tool_evidence_digest: Digest,
}
