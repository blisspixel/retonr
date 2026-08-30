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

/// Fixed source-lineage adjudication procedure identity.
pub const RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID: &str =
    "retonr:runtime-admission:source-lineage:procedure";
/// Fixed source-lineage adjudication procedure version.
pub const RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_VERSION: u32 = 1;

const CONTROL_DOMAIN: &[u8] = b"retonr:runtime-admission:source-lineage-control:v1";
const SOURCE_PROVENANCE_PATH: &str = "metadata/source-provenance.json";
const PRIMARY_PROVENANCE_PATH: &str = "attempts/primary/provenance.json";
const REBUILD_PROVENANCE_PATH: &str = "attempts/rebuild/provenance.json";

/// Domain-separated identity of one canonical source-lineage control.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionSourceLineageControlId(Digest);

impl RuntimeAdmissionSourceLineageControlId {
    /// Returns the digest defining this control identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Canonical source-lineage publication material without admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRuntimeAdmissionSourceLineageControl {
    canonical_bytes: Vec<u8>,
    control_id: RuntimeAdmissionSourceLineageControlId,
}

impl CompiledRuntimeAdmissionSourceLineageControl {
    /// Returns exact canonical publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated publication identity.
    #[must_use]
    pub const fn control_id(&self) -> &RuntimeAdmissionSourceLineageControlId {
        &self.control_id
    }
}

/// Independently verified passed source-lineage result.
///
/// This value is inert and cannot be converted into runtime admission authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedPassedRuntimeAdmissionSourceLineageControl {
    control_id: RuntimeAdmissionSourceLineageControlId,
    foundation_id: super::super::RuntimeAdmissionEvidenceFoundationId,
}

impl VerifiedPassedRuntimeAdmissionSourceLineageControl {
    #[cfg(test)]
    pub(crate) fn test_fixture(
        foundation_id: super::super::RuntimeAdmissionEvidenceFoundationId,
    ) -> Self {
        Self {
            control_id: RuntimeAdmissionSourceLineageControlId(Digest::sha256(b"lineage control")),
            foundation_id,
        }
    }

    /// Returns the verified control identity.
    #[must_use]
    pub const fn control_id(&self) -> &RuntimeAdmissionSourceLineageControlId {
        &self.control_id
    }

    /// Returns the exact verified foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &super::super::RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }
}

/// Compiler for inert source-lineage publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionSourceLineageControlCompiler;

impl RuntimeAdmissionSourceLineageControlCompiler {
    /// Compiles a reviewer-approved lineage mapping against retained evidence.
    ///
    /// The reviewer record is canonical input owned by the caller. Compilation
    /// binds it but grants no admission or policy authority.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionStaticControlError`] for invalid review bytes,
    /// evidence drift, incomplete lineage, or any identity mismatch.
    pub fn compile(
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        reviewer_bytes: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<CompiledRuntimeAdmissionSourceLineageControl, RuntimeAdmissionStaticControlError>
    {
        let review: LineageReviewWire = parse_canonical(
            reviewer_bytes,
            MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
        )?;
        compile_view(foundation, evidence, review, cancellation)
    }
}

/// Independent verifier for canonical source-lineage publication material.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionSourceLineageControlVerifier;

impl RuntimeAdmissionSourceLineageControlVerifier {
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
        VerifiedPassedRuntimeAdmissionSourceLineageControl,
        RuntimeAdmissionStaticControlError,
    > {
        verify_view(bytes, foundation, evidence, cancellation)
    }
}

pub(super) fn compile_view<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    review: LineageReviewWire,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeAdmissionSourceLineageControl, RuntimeAdmissionStaticControlError> {
    let wire = derive_wire(foundation, evidence, review, cancellation)?;
    let canonical_bytes = encode(&wire)?;
    finish(foundation, evidence, cancellation)?;
    let control_id =
        RuntimeAdmissionSourceLineageControlId(digest(CONTROL_DOMAIN, &canonical_bytes));
    Ok(CompiledRuntimeAdmissionSourceLineageControl {
        canonical_bytes,
        control_id,
    })
}

pub(super) fn verify_view<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    bytes: &[u8],
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    cancellation: &CancellationToken,
) -> Result<VerifiedPassedRuntimeAdmissionSourceLineageControl, RuntimeAdmissionStaticControlError>
{
    let wire: LineageControlWire =
        parse_canonical(bytes, MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES)?;
    let expected = derive_wire(foundation, evidence, wire.review.clone(), cancellation)?;
    if wire != expected || encode(&expected)? != bytes {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    finish(foundation, evidence, cancellation)?;
    Ok(VerifiedPassedRuntimeAdmissionSourceLineageControl {
        control_id: RuntimeAdmissionSourceLineageControlId(digest(CONTROL_DOMAIN, bytes)),
        foundation_id: foundation.foundation_id().clone(),
    })
}

fn derive_wire<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    review: LineageReviewWire,
    cancellation: &CancellationToken,
) -> Result<LineageControlWire, RuntimeAdmissionStaticControlError> {
    let facts = ensure_same_foundation(foundation, evidence, cancellation)?;
    validate_review_header(&review)?;
    let source_path = fixed_path(SOURCE_PROVENANCE_PATH)?;
    let source_bytes = evidence.read_source_input(&source_path, cancellation)?;
    let source: SourceProvenanceWire = parse_canonical(
        &source_bytes,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    if source.schema_version != 1 || review.sources.len() != 2 {
        return Err(RuntimeAdmissionStaticControlError::EvidenceInsufficient);
    }
    validate_source(
        &review.sources[0],
        &source.ollama,
        find_role(&facts, RuntimeSourceBuildInputRole::OllamaSource)?,
        LineageRole::OllamaSource,
    )?;
    validate_source(
        &review.sources[1],
        &source.llama_cpp,
        find_role(&facts, RuntimeSourceBuildInputRole::LlamaCppSource)?,
        LineageRole::LlamaCppSource,
    )?;
    validate_patches(&facts, &source)?;
    let source_component = find_role(&facts, RuntimeSourceBuildInputRole::SourceProvenance)?;
    if source_component.relative_path != source_path
        || source_component.byte_size != u64::try_from(source_bytes.len()).unwrap_or(u64::MAX)
        || source_component.digest != Digest::sha256(&source_bytes)
    {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    let primary_bytes =
        evidence.read_evidence(&fixed_path(PRIMARY_PROVENANCE_PATH)?, cancellation)?;
    let rebuild_bytes =
        evidence.read_evidence(&fixed_path(REBUILD_PROVENANCE_PATH)?, cancellation)?;
    let primary: BuildProvenanceWire = parse_canonical(
        &primary_bytes,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    let rebuild: BuildProvenanceWire = parse_canonical(
        &rebuild_bytes,
        MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES,
    )?;
    if primary != rebuild
        || primary.schema_version != 1
        || primary.build_revision
            != find_role(&facts, RuntimeSourceBuildInputRole::OllamaSource)?.revision
        || primary.builder_digest != facts.build_program_digest
        || primary.source_inputs_id != foundation.snapshot.source_build_inputs_id
        || primary.source_provenance_digest != source_component.digest
        || primary.target != "x86_64-linux-gnu"
    {
        return Err(RuntimeAdmissionStaticControlError::EvidenceInsufficient);
    }
    Ok(LineageControlWire {
        control: LineageControl::SourceLineage,
        evidence: LineageEvidenceWire {
            primary: Digest::sha256(&primary_bytes),
            rebuild: Digest::sha256(&rebuild_bytes),
            source: Digest::sha256(&source_bytes),
        },
        foundation_id: foundation.foundation_id().digest().clone(),
        procedure_id: RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID.to_owned(),
        procedure_version: RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_VERSION,
        review,
        schema_version: 1,
        status: PassedStatus::Passed,
    })
}

fn validate_review_header(
    review: &LineageReviewWire,
) -> Result<(), RuntimeAdmissionStaticControlError> {
    if review.disposition != ReviewDisposition::Approved
        || review.procedure_id != RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID
        || review.procedure_version != RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_VERSION
        || review.schema_version != 1
    {
        return Err(RuntimeAdmissionStaticControlError::ReviewRequired);
    }
    Ok(())
}

fn validate_source(
    reviewed: &ReviewedSourceWire,
    provenance: &ProvenanceSourceWire,
    component: &super::common::StaticInputComponent,
    expected_role: LineageRole,
) -> Result<(), RuntimeAdmissionStaticControlError> {
    if reviewed.role != expected_role
        || reviewed.acquired_archive_byte_size != provenance.acquired_archive.byte_size
        || reviewed.acquired_archive_digest != provenance.acquired_archive.digest
        || reviewed.normalized_component_byte_size != provenance.normalized_component.byte_size
        || reviewed.normalized_component_digest != provenance.normalized_component.digest
        || reviewed.revision != provenance.revision
        || reviewed.source_locator != provenance.acquired_archive.locator
        || reviewed.upstream_tag != provenance.upstream_tag
        || component.byte_size != provenance.normalized_component.byte_size
        || component.digest != provenance.normalized_component.digest
        || component.revision != provenance.revision
        || component.source_locator != provenance.acquired_archive.locator
    {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    Ok(())
}

fn validate_patches(
    facts: &super::common::StaticEvidenceFacts,
    provenance: &SourceProvenanceWire,
) -> Result<(), RuntimeAdmissionStaticControlError> {
    let components = facts
        .components
        .iter()
        .filter(|component| {
            component
                .roles
                .contains(&RuntimeSourceBuildInputRole::SourcePatch)
        })
        .collect::<Vec<_>>();
    if components.is_empty()
        || components.len() != provenance.retained_source_patches.len()
        || components.len() > 64
    {
        return Err(RuntimeAdmissionStaticControlError::EvidenceInsufficient);
    }
    let mut matched = vec![false; components.len()];
    for (index, patch) in provenance.retained_source_patches.iter().enumerate() {
        if patch.name.is_empty()
            || patch.target_revision != provenance.ollama.revision
                && patch.target_revision != provenance.llama_cpp.revision
            || provenance.retained_source_patches[..index]
                .iter()
                .any(|earlier| earlier.name == patch.name)
        {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        let mut candidates = components
            .iter()
            .enumerate()
            .filter(|(candidate, component)| {
                !matched[*candidate]
                    && component.byte_size == patch.normalized_component.byte_size
                    && component.digest == patch.normalized_component.digest
            });
        let (candidate, _) = candidates
            .next()
            .ok_or(RuntimeAdmissionStaticControlError::InvalidBinding)?;
        if candidates.next().is_some() {
            return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
        }
        matched[candidate] = true;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageControlWire {
    control: LineageControl,
    evidence: LineageEvidenceWire,
    foundation_id: Digest,
    procedure_id: String,
    procedure_version: u32,
    review: LineageReviewWire,
    schema_version: u32,
    status: PassedStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LineageControl {
    SourceLineage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PassedStatus {
    Passed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageEvidenceWire {
    #[serde(rename = "primary_provenance_digest")]
    primary: Digest,
    #[serde(rename = "rebuild_provenance_digest")]
    rebuild: Digest,
    #[serde(rename = "source_provenance_digest")]
    source: Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LineageReviewWire {
    disposition: ReviewDisposition,
    procedure_id: String,
    procedure_version: u32,
    schema_version: u32,
    sources: Vec<ReviewedSourceWire>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewDisposition {
    Approved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewedSourceWire {
    acquired_archive_byte_size: u64,
    acquired_archive_digest: Digest,
    normalized_component_byte_size: u64,
    normalized_component_digest: Digest,
    revision: String,
    role: LineageRole,
    source_locator: String,
    upstream_tag: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LineageRole {
    OllamaSource,
    LlamaCppSource,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceProvenanceWire {
    llama_cpp: ProvenanceSourceWire,
    ollama: ProvenanceSourceWire,
    retained_source_patches: Vec<ProvenancePatchWire>,
    schema_version: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceSourceWire {
    acquired_archive: AcquiredArchiveWire,
    normalized_component: MeasurementWire,
    revision: String,
    upstream_tag: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AcquiredArchiveWire {
    byte_size: u64,
    digest: Digest,
    locator: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MeasurementWire {
    byte_size: u64,
    digest: Digest,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ProvenancePatchWire {
    name: String,
    normalized_component: MeasurementWire,
    target_revision: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BuildProvenanceWire {
    build_revision: String,
    builder_digest: Digest,
    reported_version: String,
    schema_version: u32,
    source_inputs_id: ArtifactSetId,
    source_provenance_digest: Digest,
    target: String,
}
