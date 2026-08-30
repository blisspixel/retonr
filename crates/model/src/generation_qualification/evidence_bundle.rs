use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{
    append_count, append_digest, append_text, append_u32, append_u64, validate_canonical_json,
};
use super::{
    CANDIDATE_GENERATION_EVIDENCE_BUNDLE_ID_DOMAIN, CandidateEvidenceId,
    CandidateGenerationAttemptPrecursorId, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationCleanupId, CandidateGenerationCleanupRecordV1,
    CandidateGenerationEvidenceBundleId, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GenerationCaseManifestV1, GenerationQualificationContractError, GenerationQualificationPlanV1,
    ManagedOllamaCandidateGenerationEvidenceV2, ManagedOllamaCandidateGenerationEvidenceV2Id,
    OllamaRetainedSessionResponseId, PlannedCandidateAttemptV1,
};
use crate::{ArtifactId, ArtifactSetRelativePath};

mod validation;
mod wire;

use validation::{validate_bundle, validate_candidate_relationship};
use wire::{EvidenceBundleWire, EvidenceBundleWireParts};

/// Domain separating a candidate content digest from a raw artifact digest.
pub const CANDIDATE_CONTENT_DIGEST_DOMAIN: &[u8] = b"retonr:candidate-content:v1\0";
/// Reserved root path for the canonical manifest stored outside its entry list.
pub const EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH: &str = "bundle-manifest.json";
/// Maximum JSON bytes accepted for one evidence-bundle manifest.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES: usize = 4 * 1_024 * 1_024;

/// Opaque bounded parse of one canonical evidence-bundle manifest.
///
/// The token retains inert, explicitly untrusted candidate metadata. It proves
/// only bounded syntax, schema, canonical encoding, and portable path syntax.
/// Raw candidate bytes still need independent bundle reacquisition before the
/// metadata can be trusted as an exact candidate binding.
///
/// ```compile_fail
/// use rewrite_model::CandidateGenerationEvidenceBundleManifestV1Preflight;
///
/// fn require_clone<T: Clone>() {}
///
/// fn clone_preflight() {
///     require_clone::<CandidateGenerationEvidenceBundleManifestV1Preflight>();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_model::CandidateGenerationEvidenceBundleManifestV1Preflight;
///
/// fn serialize_preflight(value: &CandidateGenerationEvidenceBundleManifestV1Preflight) {
///     let _bytes = serde_json::to_vec(value).expect("preflight must not serialize");
/// }
/// ```
pub struct CandidateGenerationEvidenceBundleManifestV1Preflight {
    schema_version: u32,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    response_id: OllamaRetainedSessionResponseId,
    cleanup_id: CandidateGenerationCleanupId,
    entries: Vec<CandidateGenerationEvidenceBundleEntryV1>,
    candidates: Vec<CandidateArtifactEntryV1>,
    derived_counts: (u32, u64),
    canonical_json: Box<[u8]>,
}

impl fmt::Debug for CandidateGenerationEvidenceBundleManifestV1Preflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationEvidenceBundleManifestV1Preflight")
            .field("schema_version", &self.schema_version)
            .field("entry_count", &self.entries.len())
            .field("candidate_count", &self.candidates.len())
            .finish_non_exhaustive()
    }
}

impl CandidateGenerationEvidenceBundleManifestV1Preflight {
    /// Returns the unique inert structured-response artifact metadata.
    ///
    /// This value comes only from canonical manifest metadata. An upper layer must
    /// still reacquire and hash the exact response bytes before trusting it.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless exactly one
    /// nonempty structured-response entry is present.
    pub fn structured_response_artifact(
        &self,
    ) -> Result<StructuredResponseArtifactV1Input, GenerationQualificationContractError> {
        let mut matches = self.entries.iter().filter(|entry| {
            entry.role() == CandidateGenerationEvidenceBundleRoleV1::StructuredResponse
        });
        let entry = matches
            .next()
            .ok_or(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure)?;
        if matches.next().is_some() {
            return Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure);
        }
        StructuredResponseArtifactV1Input::new(entry.artifact_id().clone(), entry.byte_size())
    }
}

/// Closed role for one logical evidence-bundle member.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationEvidenceBundleRoleV1 {
    /// Canonical planned-attempt record.
    PlannedAttempt,
    /// Canonical execution precursor.
    AttemptPrecursor,
    /// Canonical managed generation evidence.
    ManagedGenerationEvidence,
    /// Retained structured response observation.
    StructuredResponse,
    /// Canonical cleanup record.
    CleanupRecord,
    /// Exact candidate bytes.
    Candidate,
    /// Additional plan-bounded observation.
    AuxiliaryObservation,
}

/// One artifact-bound logical evidence-bundle member.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationEvidenceBundleEntryV1 {
    relative_path: ArtifactSetRelativePath,
    role: CandidateGenerationEvidenceBundleRoleV1,
    artifact_id: ArtifactId,
    byte_size: u64,
}

impl CandidateGenerationEvidenceBundleEntryV1 {
    /// Creates inert metadata for one logical bundle member.
    #[must_use]
    pub const fn new(
        relative_path: ArtifactSetRelativePath,
        role: CandidateGenerationEvidenceBundleRoleV1,
        artifact_id: ArtifactId,
        byte_size: u64,
    ) -> Self {
        Self {
            relative_path,
            role,
            artifact_id,
            byte_size,
        }
    }

    /// Returns the canonical relative path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }
    /// Returns the closed evidence role.
    #[must_use]
    pub const fn role(&self) -> CandidateGenerationEvidenceBundleRoleV1 {
        self.role
    }
    /// Returns the exact content artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }
    /// Returns the exact byte size.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// One exact candidate artifact and its attempt-local identity.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateArtifactEntryV1 {
    candidate_evidence_id: CandidateEvidenceId,
    ordinal: u8,
    byte_count: u64,
    artifact_id: ArtifactId,
    relative_path: ArtifactSetRelativePath,
    candidate_digest: Digest,
}

impl CandidateArtifactEntryV1 {
    /// Derives one candidate entry from exact UTF-8 bytes and upstream records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for a foreign case or
    /// precursor, a noncontiguous-range ordinal, invalid UTF-8, or excessive bytes.
    pub fn new(
        precursor: &CandidateGenerationAttemptPrecursorV1,
        planned_attempt: &PlannedCandidateAttemptV1,
        case: &GenerationCaseManifestV1,
        ordinal: u8,
        relative_path: ArtifactSetRelativePath,
        bytes: &[u8],
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_candidate_relationship(precursor, planned_attempt, case, ordinal, bytes)?;
        let byte_count = u64::try_from(bytes.len())
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        let artifact_id = ArtifactId::from_digest(Digest::sha256(bytes));
        let candidate_digest = validation::candidate_content_digest(bytes);
        let candidate_evidence_id = validation::candidate_evidence_id(
            precursor.precursor_id(),
            case.case_id(),
            ordinal,
            byte_count,
            bytes,
        );
        Ok(Self {
            candidate_evidence_id,
            ordinal,
            byte_count,
            artifact_id,
            relative_path,
            candidate_digest,
        })
    }

    /// Returns the candidate evidence identity.
    #[must_use]
    pub const fn candidate_evidence_id(&self) -> &CandidateEvidenceId {
        &self.candidate_evidence_id
    }
    /// Returns the request-local ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        self.ordinal
    }
    /// Returns the UTF-8 byte count.
    #[must_use]
    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }
    /// Returns the raw-byte artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }
    /// Returns the canonical bundle path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }
    /// Returns the domain-separated candidate content digest.
    #[must_use]
    pub const fn candidate_digest(&self) -> &Digest {
        &self.candidate_digest
    }
}

impl fmt::Debug for CandidateArtifactEntryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateArtifactEntryV1")
            .field("candidate_evidence_id", &self.candidate_evidence_id)
            .field("ordinal", &self.ordinal)
            .field("byte_count", &self.byte_count)
            .finish_non_exhaustive()
    }
}

/// Exact upstream records required to validate one evidence-bundle manifest.
#[derive(Clone, Copy)]
pub struct CandidateGenerationEvidenceBundleManifestV1Relations<'a> {
    /// Exact qualification plan.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Exact selected planned attempt.
    pub planned_attempt: &'a PlannedCandidateAttemptV1,
    /// Exact execution precursor.
    pub precursor: &'a CandidateGenerationAttemptPrecursorV1,
    /// Exact managed generation evidence.
    pub managed_evidence: &'a ManagedOllamaCandidateGenerationEvidenceV2,
    /// Exact cleanup record.
    pub cleanup: &'a CandidateGenerationCleanupRecordV1,
    /// Exact retained-response artifact binding derived by the upper layer.
    pub structured_response_artifact: &'a StructuredResponseArtifactV1Input,
}

/// Inert exact-byte artifact binding for the retained structured response.
///
/// The model layer cannot load response bytes from its typed response identity.
/// An upper-layer compiler derives this input from the exact retained bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredResponseArtifactV1Input {
    artifact_id: ArtifactId,
    byte_size: u64,
}

impl StructuredResponseArtifactV1Input {
    /// Creates one inert response artifact binding from upper-layer derived facts.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for an empty response.
    pub fn new(
        artifact_id: ArtifactId,
        byte_size: u64,
    ) -> Result<Self, GenerationQualificationContractError> {
        if byte_size == 0 {
            Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry)
        } else {
            Ok(Self {
                artifact_id,
                byte_size,
            })
        }
    }

    /// Returns the exact response artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }
    /// Returns the exact response byte size.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// Inert portable manifest for one bounded generation evidence bundle.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationEvidenceBundleManifestV1 {
    schema_version: u32,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    response_id: OllamaRetainedSessionResponseId,
    cleanup_id: CandidateGenerationCleanupId,
    entries: Vec<CandidateGenerationEvidenceBundleEntryV1>,
    candidate_artifacts: Vec<CandidateArtifactEntryV1>,
    entry_count: u32,
    aggregate_byte_count: u64,
    #[serde(skip)]
    id: CandidateGenerationEvidenceBundleId,
}

impl CandidateGenerationEvidenceBundleManifestV1 {
    /// Creates an inert manifest from exact upstream records and logical entries.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless every relationship,
    /// limit, role multiplicity, path order, and candidate mapping closes exactly.
    pub fn new(
        relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
        entries: Vec<CandidateGenerationEvidenceBundleEntryV1>,
        candidate_artifacts: Vec<CandidateArtifactEntryV1>,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            relations,
            entries,
            candidate_artifacts,
            None,
        )
    }

    fn build(
        schema_version: u32,
        relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
        entries: Vec<CandidateGenerationEvidenceBundleEntryV1>,
        candidate_artifacts: Vec<CandidateArtifactEntryV1>,
        wire_derived: Option<(u32, u64)>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        let aggregate_byte_count = validate_bundle(relations, &entries, &candidate_artifacts)?;
        let entry_count = u32::try_from(entries.len())
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        if wire_derived.is_some_and(|values| values != (entry_count, aggregate_byte_count)) {
            return Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry);
        }
        let mut value = Self {
            schema_version,
            precursor_id: relations.precursor.precursor_id().clone(),
            managed_evidence_id: relations.managed_evidence.managed_evidence_v2_id().clone(),
            response_id: relations.managed_evidence.response_id().clone(),
            cleanup_id: relations.cleanup.cleanup_id().clone(),
            entries,
            candidate_artifacts,
            entry_count,
            aggregate_byte_count,
            id: CandidateGenerationEvidenceBundleId(Digest::sha256(
                b"uninitialized evidence bundle",
            )),
        };
        value.to_canonical_json_bytes()?;
        value.id = value.rederive_evidence_bundle_id()?;
        Ok(value)
    }

    /// Parses canonical bounded JSON and reloads exact relationships and candidates.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// future-schema, noncanonical, substituted, or structurally incomplete input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
        expected_candidates: &[CandidateArtifactEntryV1],
    ) -> Result<Self, GenerationQualificationContractError> {
        let preflight = Self::preflight_json_bytes(bytes)?;
        if preflight.candidates != expected_candidates {
            return Err(GenerationQualificationContractError::InvalidCandidateArtifact);
        }
        Self::from_preflight(preflight, relations)
    }

    /// Checks a complete manifest before external evidence access.
    ///
    /// The hard byte ceiling is enforced before JSON decoding. Success does not
    /// validate upstream relationships or candidate bytes and grants no authority.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// future-schema, noncanonical, or invalid portable metadata.
    pub fn preflight_json_bytes(
        bytes: &[u8],
    ) -> Result<
        CandidateGenerationEvidenceBundleManifestV1Preflight,
        GenerationQualificationContractError,
    > {
        if bytes.len() > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: EvidenceBundleWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version() != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version(),
            ));
        }
        validate_canonical_json(bytes, &wire)?;
        let EvidenceBundleWireParts {
            schema_version,
            precursor_id,
            managed_evidence_id,
            response_id,
            cleanup_id,
            entries,
            candidates,
            derived_counts,
        } = wire.into_parts()?;
        Ok(CandidateGenerationEvidenceBundleManifestV1Preflight {
            schema_version,
            precursor_id,
            managed_evidence_id,
            response_id,
            cleanup_id,
            entries,
            candidates,
            derived_counts,
            canonical_json: bytes.into(),
        })
    }

    /// Consumes a bounded manifest parse and reloads its portable relationships.
    ///
    /// Candidate metadata remains inert until an upper layer reacquires the exact
    /// bundle bytes and rederives every candidate entry.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for substituted upstream
    /// records, invalid bundle structure, or a nonmatching canonical encoding.
    pub fn from_preflight(
        preflight: CandidateGenerationEvidenceBundleManifestV1Preflight,
        relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let CandidateGenerationEvidenceBundleManifestV1Preflight {
            schema_version,
            precursor_id,
            managed_evidence_id,
            response_id,
            cleanup_id,
            entries,
            candidates,
            derived_counts,
            canonical_json,
        } = preflight;
        if precursor_id != *relations.precursor.precursor_id()
            || managed_evidence_id != *relations.managed_evidence.managed_evidence_v2_id()
            || response_id != *relations.managed_evidence.response_id()
            || cleanup_id != *relations.cleanup.cleanup_id()
        {
            return Err(GenerationQualificationContractError::EvidenceBundleRelationshipMismatch);
        }
        let value = Self::build(
            schema_version,
            relations,
            entries,
            candidates,
            Some(derived_counts),
        )?;
        validate_canonical_json(&canonical_json, &value)?;
        Ok(value)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the precursor identity.
    #[must_use]
    pub const fn precursor_id(&self) -> &CandidateGenerationAttemptPrecursorId {
        &self.precursor_id
    }
    /// Returns the managed-evidence identity.
    #[must_use]
    pub const fn managed_evidence_id(&self) -> &ManagedOllamaCandidateGenerationEvidenceV2Id {
        &self.managed_evidence_id
    }
    /// Returns the retained-response identity.
    #[must_use]
    pub const fn response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.response_id
    }
    /// Returns the cleanup identity.
    #[must_use]
    pub const fn cleanup_id(&self) -> &CandidateGenerationCleanupId {
        &self.cleanup_id
    }
    /// Returns entries in strict canonical path order.
    #[must_use]
    pub fn entries(&self) -> &[CandidateGenerationEvidenceBundleEntryV1] {
        &self.entries
    }
    /// Returns candidate artifacts in contiguous ordinal order.
    #[must_use]
    pub fn candidate_artifacts(&self) -> &[CandidateArtifactEntryV1] {
        &self.candidate_artifacts
    }
    /// Returns the derived entry count.
    #[must_use]
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }
    /// Returns the derived aggregate byte count.
    #[must_use]
    pub const fn aggregate_byte_count(&self) -> u64 {
        self.aggregate_byte_count
    }
    /// Returns the content-derived manifest identity.
    #[must_use]
    pub const fn evidence_bundle_id(&self) -> &CandidateGenerationEvidenceBundleId {
        &self.id
    }

    /// Encodes the manifest as its one canonical JSON representation.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if encoding fails or
    /// exceeds the manifest JSON ceiling.
    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if bytes.len() > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES {
            Err(GenerationQualificationContractError::CanonicalEncodingTooLarge)
        } else {
            Ok(bytes)
        }
    }

    /// Rederives the content identity from every manifest field.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if an encoded collection
    /// length cannot be represented by the canonical identity codec.
    pub fn rederive_evidence_bundle_id(
        &self,
    ) -> Result<CandidateGenerationEvidenceBundleId, GenerationQualificationContractError> {
        Ok(CandidateGenerationEvidenceBundleId(Digest::sha256(
            &self.canonical_bytes()?,
        )))
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationContractError> {
        let mut output = CANDIDATE_GENERATION_EVIDENCE_BUNDLE_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.precursor_id.digest(),
            self.managed_evidence_id.digest(),
            self.response_id.digest(),
            self.cleanup_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        append_count(&mut output, self.entries.len())?;
        for entry in &self.entries {
            append_text(&mut output, entry.relative_path.as_str());
            output.push(role_tag(entry.role));
            append_digest(&mut output, entry.artifact_id.digest());
            append_u64(&mut output, entry.byte_size);
        }
        append_count(&mut output, self.candidate_artifacts.len())?;
        for candidate in &self.candidate_artifacts {
            append_digest(&mut output, candidate.candidate_evidence_id.digest());
            output.push(candidate.ordinal);
            append_u64(&mut output, candidate.byte_count);
            append_digest(&mut output, candidate.artifact_id.digest());
            append_text(&mut output, candidate.relative_path.as_str());
            append_digest(&mut output, &candidate.candidate_digest);
        }
        append_u32(&mut output, self.entry_count);
        append_u64(&mut output, self.aggregate_byte_count);
        Ok(output)
    }
}

impl fmt::Debug for CandidateGenerationEvidenceBundleManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationEvidenceBundleManifestV1")
            .field("evidence_bundle_id", &self.id)
            .field("entry_count", &self.entry_count)
            .field("candidate_count", &self.candidate_artifacts.len())
            .field("aggregate_byte_count", &self.aggregate_byte_count)
            .finish_non_exhaustive()
    }
}

pub(super) const fn role_tag(role: CandidateGenerationEvidenceBundleRoleV1) -> u8 {
    match role {
        CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt => 0,
        CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor => 1,
        CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence => 2,
        CandidateGenerationEvidenceBundleRoleV1::StructuredResponse => 3,
        CandidateGenerationEvidenceBundleRoleV1::CleanupRecord => 4,
        CandidateGenerationEvidenceBundleRoleV1::Candidate => 5,
        CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation => 6,
    }
}
