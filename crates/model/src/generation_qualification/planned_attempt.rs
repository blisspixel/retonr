use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use rewrite_types::Digest;

use super::codec::{append_digest, append_u32, append_u64, validate_canonical_json};
use super::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId, GenerationCaseManifestV1,
    GenerationClusterId, GenerationClusterRecordV1, GenerationQualificationContractError,
    GenerationRepetitionId, GenerationRepetitionRecordV1, GenerationRequestBindingId,
    GenerationSuiteManifestId, GenerationSuiteManifestV1, GenerationSystemId,
    GenerationSystemRecordV1, MAX_GENERATION_CANDIDATES_PER_COMPLETION,
    MAX_PLANNED_GENERATION_ATTEMPTS, PLANNED_CANDIDATE_ATTEMPT_ID_DOMAIN,
    PlannedCandidateAttemptId,
};
use crate::ArtifactId;

mod wire;

use wire::AttemptWire;

/// Maximum JSON bytes accepted for one planned candidate attempt.
pub const MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES: usize = 16_384;
const MAX_PLANNED_CANDIDATE_ATTEMPT_CANONICAL_BYTES: usize = 4_096;
const MAX_JSON_BYTES_PER_DECODED_BYTE: u64 = 6;
const MAX_ENVELOPE_FRAMING_BYTES: u64 = 16;
const MAX_CANDIDATE_FRAMING_BYTES: u64 = 12;
const RAW_ENVELOPE_ALLOWANCE_BYTES: u64 = 256;

/// Exact count and byte ceilings for one predeclared candidate envelope.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateOutputCeilingsV1 {
    candidate_count: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_envelope_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateOutputCeilingsV1Wire {
    candidate_count: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_envelope_bytes: u64,
}

impl<'de> Deserialize<'de> for CandidateOutputCeilingsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = CandidateOutputCeilingsV1Wire::deserialize(deserializer)?;
        Self::from_wire(
            wire.candidate_count,
            wire.maximum_candidate_bytes,
            wire.maximum_aggregate_candidate_bytes,
            wire.maximum_envelope_bytes,
        )
        .map_err(|_error| D::Error::custom("invalid candidate output ceilings"))
    }
}

impl CandidateOutputCeilingsV1 {
    /// Creates a provider-neutral policy and derives its exact envelope ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError::InvalidCandidateOutputPolicy`]
    /// for zero or excessive count, zero byte ceilings, a useless aggregate ceiling,
    /// or checked envelope-ceiling arithmetic overflow.
    pub fn new(
        candidate_count: u8,
        maximum_candidate_bytes: u64,
        maximum_aggregate_candidate_bytes: u64,
    ) -> Result<Self, GenerationQualificationContractError> {
        let maximum_envelope_bytes = derive_envelope_bytes(
            candidate_count,
            maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes,
        )?;
        let value = Self {
            candidate_count,
            maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes,
            maximum_envelope_bytes,
        };
        if value.valid() {
            Ok(value)
        } else {
            Err(GenerationQualificationContractError::InvalidCandidateOutputPolicy)
        }
    }

    pub(super) fn from_wire(
        candidate_count: u8,
        maximum_candidate_bytes: u64,
        maximum_aggregate_candidate_bytes: u64,
        maximum_envelope_bytes: u64,
    ) -> Result<Self, GenerationQualificationContractError> {
        let value = Self::new(
            candidate_count,
            maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes,
        )?;
        if value.maximum_envelope_bytes == maximum_envelope_bytes {
            Ok(value)
        } else {
            Err(GenerationQualificationContractError::InvalidCandidateOutputPolicy)
        }
    }

    fn valid(self) -> bool {
        derive_envelope_bytes(
            self.candidate_count,
            self.maximum_candidate_bytes,
            self.maximum_aggregate_candidate_bytes,
        ) == Ok(self.maximum_envelope_bytes)
    }

    /// Returns the exact candidate count.
    #[must_use]
    pub const fn candidate_count(self) -> u8 {
        self.candidate_count
    }
    /// Returns the per-candidate UTF-8 byte ceiling.
    #[must_use]
    pub const fn maximum_candidate_bytes(self) -> u64 {
        self.maximum_candidate_bytes
    }
    /// Returns the aggregate candidate UTF-8 byte ceiling.
    #[must_use]
    pub const fn maximum_aggregate_candidate_bytes(self) -> u64 {
        self.maximum_aggregate_candidate_bytes
    }
    /// Returns the exact raw envelope byte ceiling.
    #[must_use]
    pub const fn maximum_envelope_bytes(self) -> u64 {
        self.maximum_envelope_bytes
    }
}

/// Attempt-specific facts not already owned by exact manifest and system records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedCandidateAttemptV1Input {
    /// Zero-based immutable plan attempt ordinal.
    pub attempt_ordinal: u32,
    /// Exact predeclared request seed.
    pub declared_seed: u64,
    /// Digest of the exact grounded request bytes and construction contract.
    pub grounded_request_digest: Digest,
    /// Provider-neutral generation-request binding.
    pub generation_request_binding_id: GenerationRequestBindingId,
    /// Digest of the exact candidate output contract.
    pub candidate_output_contract_digest: Digest,
    /// Exact candidate count and byte ceilings.
    pub output_ceilings: CandidateOutputCeilingsV1,
}

/// Inert portable declaration of one exact candidate-generation attempt.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedCandidateAttemptV1 {
    schema_version: u32,
    suite_manifest_id: GenerationSuiteManifestId,
    case_id: GenerationCaseId,
    cluster_id: GenerationClusterId,
    repetition_id: GenerationRepetitionId,
    attempt_ordinal: u32,
    declared_seed: u64,
    generation_system_id: GenerationSystemId,
    source_artifact_id: ArtifactId,
    source_digest: Digest,
    source_byte_count: u64,
    case_contract_digest: Digest,
    grounded_request_digest: Digest,
    generation_request_binding_id: GenerationRequestBindingId,
    candidate_output_contract_digest: Digest,
    candidate_count: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_envelope_bytes: u64,
    #[serde(skip)]
    id: PlannedCandidateAttemptId,
}

/// Exact manifest and system records required to validate one planned attempt.
#[derive(Clone, Copy)]
pub struct PlannedCandidateAttemptV1Relations<'a> {
    /// Exact suite containing the selected case.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact selected case.
    pub case: &'a GenerationCaseManifestV1,
    /// Exact cluster named by the case.
    pub cluster: &'a GenerationClusterRecordV1,
    /// Exact repetition of the selected suite.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Exact generation system selected for the attempt.
    pub generation_system: &'a GenerationSystemRecordV1,
}

impl PlannedCandidateAttemptV1 {
    /// Creates one attempt from exact manifests and a stable generation system.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for any manifest, source,
    /// selector, ordinal, or candidate-policy mismatch.
    pub fn new(
        relations: PlannedCandidateAttemptV1Relations<'_>,
        input: PlannedCandidateAttemptV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            relations,
            input,
            None,
        )
    }

    fn build(
        schema_version: u32,
        relations: PlannedCandidateAttemptV1Relations<'_>,
        input: PlannedCandidateAttemptV1Input,
        wire: Option<&AttemptWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        validate_relations(relations)?;
        if usize::try_from(input.attempt_ordinal)
            .map_or(true, |ordinal| ordinal >= MAX_PLANNED_GENERATION_ATTEMPTS)
        {
            return Err(GenerationQualificationContractError::AttemptOrdinalMismatch);
        }
        if !input.output_ceilings.valid() {
            return Err(GenerationQualificationContractError::InvalidCandidateOutputPolicy);
        }
        if wire.is_some_and(|wire| !wire.matches(relations, &input)) {
            return Err(GenerationQualificationContractError::PlannedAttemptMismatch);
        }
        let mut record = Self {
            schema_version,
            suite_manifest_id: relations.suite.suite_manifest_id().clone(),
            case_id: relations.case.case_id().clone(),
            cluster_id: relations.cluster.cluster_id().clone(),
            repetition_id: relations.repetition.repetition_id().clone(),
            attempt_ordinal: input.attempt_ordinal,
            declared_seed: input.declared_seed,
            generation_system_id: relations.generation_system.generation_system_id().clone(),
            source_artifact_id: relations.case.source_artifact_id().clone(),
            source_digest: relations.case.source_digest().clone(),
            source_byte_count: relations.case.source_byte_count(),
            case_contract_digest: relations.case.case_contract_digest().clone(),
            grounded_request_digest: input.grounded_request_digest,
            generation_request_binding_id: input.generation_request_binding_id,
            candidate_output_contract_digest: input.candidate_output_contract_digest,
            candidate_count: input.output_ceilings.candidate_count,
            maximum_candidate_bytes: input.output_ceilings.maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes: input
                .output_ceilings
                .maximum_aggregate_candidate_bytes,
            maximum_envelope_bytes: input.output_ceilings.maximum_envelope_bytes,
            id: PlannedCandidateAttemptId(Digest::sha256(b"uninitialized planned attempt")),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_PLANNED_CANDIDATE_ATTEMPT_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        record.id = PlannedCandidateAttemptId(Digest::sha256(&canonical));
        Ok(record)
    }

    /// Parses canonical bounded JSON and reloads every manifest relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// noncanonical, unsupported, or relationship-inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: PlannedCandidateAttemptV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: AttemptWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        let input = wire.input()?;
        let record = Self::build(wire.schema_version, relations, input, Some(&wire))?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this record against exact manifests and generation system.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if any relationship or
    /// serialized attempt fact differs from a fresh checked derivation.
    pub fn validate_against(
        &self,
        relations: PlannedCandidateAttemptV1Relations<'_>,
    ) -> Result<(), GenerationQualificationContractError> {
        let expected = Self::new(
            relations,
            PlannedCandidateAttemptV1Input {
                attempt_ordinal: self.attempt_ordinal,
                declared_seed: self.declared_seed,
                grounded_request_digest: self.grounded_request_digest.clone(),
                generation_request_binding_id: self.generation_request_binding_id.clone(),
                candidate_output_contract_digest: self.candidate_output_contract_digest.clone(),
                output_ceilings: self.output_ceilings(),
            },
        )?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationContractError::PlannedAttemptMismatch)
        }
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact suite identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the exact case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }
    /// Returns the exact cluster identity.
    #[must_use]
    pub const fn cluster_id(&self) -> &GenerationClusterId {
        &self.cluster_id
    }
    /// Returns the exact repetition identity.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }
    /// Returns the zero-based plan attempt ordinal.
    #[must_use]
    pub const fn attempt_ordinal(&self) -> u32 {
        self.attempt_ordinal
    }
    /// Returns the exact predeclared seed.
    #[must_use]
    pub const fn declared_seed(&self) -> u64 {
        self.declared_seed
    }
    /// Returns the exact stable generation-system identity.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.generation_system_id
    }
    /// Returns the exact source artifact identity.
    #[must_use]
    pub const fn source_artifact_id(&self) -> &ArtifactId {
        &self.source_artifact_id
    }
    /// Returns the exact source digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        &self.source_digest
    }
    /// Returns the exact source byte count.
    #[must_use]
    pub const fn source_byte_count(&self) -> u64 {
        self.source_byte_count
    }
    /// Returns the exact case-contract digest.
    #[must_use]
    pub const fn case_contract_digest(&self) -> &Digest {
        &self.case_contract_digest
    }
    /// Returns the exact grounded-request digest.
    #[must_use]
    pub const fn grounded_request_digest(&self) -> &Digest {
        &self.grounded_request_digest
    }
    /// Returns the provider-neutral request binding.
    #[must_use]
    pub const fn generation_request_binding_id(&self) -> &GenerationRequestBindingId {
        &self.generation_request_binding_id
    }
    /// Returns the candidate-output contract digest.
    #[must_use]
    pub const fn candidate_output_contract_digest(&self) -> &Digest {
        &self.candidate_output_contract_digest
    }
    /// Returns exact candidate output ceilings.
    #[must_use]
    pub const fn output_ceilings(&self) -> CandidateOutputCeilingsV1 {
        CandidateOutputCeilingsV1 {
            candidate_count: self.candidate_count,
            maximum_candidate_bytes: self.maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes: self.maximum_aggregate_candidate_bytes,
            maximum_envelope_bytes: self.maximum_envelope_bytes,
        }
    }
    /// Returns the content-derived planned-attempt identity.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = PLANNED_CANDIDATE_ATTEMPT_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.suite_manifest_id.digest(),
            self.case_id.digest(),
            self.cluster_id.digest(),
            self.repetition_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        append_u32(&mut output, self.attempt_ordinal);
        append_u64(&mut output, self.declared_seed);
        for digest in [
            self.generation_system_id.digest(),
            self.source_artifact_id.digest(),
            &self.source_digest,
        ] {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.source_byte_count);
        for digest in [
            &self.case_contract_digest,
            &self.grounded_request_digest,
            self.generation_request_binding_id.digest(),
            &self.candidate_output_contract_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output.push(self.candidate_count);
        append_u64(&mut output, self.maximum_candidate_bytes);
        append_u64(&mut output, self.maximum_aggregate_candidate_bytes);
        append_u64(&mut output, self.maximum_envelope_bytes);
        output
    }
}

impl fmt::Debug for PlannedCandidateAttemptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlannedCandidateAttemptV1")
            .field("schema_version", &self.schema_version)
            .field("planned_attempt_id", &self.id)
            .field("attempt_ordinal", &self.attempt_ordinal)
            .field("candidate_count", &self.candidate_count)
            .finish_non_exhaustive()
    }
}

fn validate_relations(
    relations: PlannedCandidateAttemptV1Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    if relations
        .suite
        .case_ids()
        .iter()
        .filter(|id| *id == relations.case.case_id())
        .count()
        != 1
    {
        return Err(GenerationQualificationContractError::PlannedCaseMismatch);
    }
    if relations.case.cluster_id() != relations.cluster.cluster_id() {
        return Err(GenerationQualificationContractError::ClusterMismatch);
    }
    if relations.repetition.suite_manifest_id() != relations.suite.suite_manifest_id() {
        return Err(GenerationQualificationContractError::RepetitionMismatch);
    }
    if relations.case.language_digest() != relations.generation_system.language_digest()
        || relations.case.mode_digest() != relations.generation_system.mode_digest()
        || relations.case.format_digest() != relations.generation_system.format_digest()
    {
        return Err(GenerationQualificationContractError::GenerationSystemMismatch);
    }
    Ok(())
}

fn derive_envelope_bytes(
    candidate_count: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
) -> Result<u64, GenerationQualificationContractError> {
    if candidate_count == 0
        || candidate_count > MAX_GENERATION_CANDIDATES_PER_COMPLETION
        || maximum_candidate_bytes == 0
        || maximum_aggregate_candidate_bytes == 0
        || maximum_candidate_bytes
            .checked_mul(u64::from(candidate_count))
            .is_none_or(|useful| maximum_aggregate_candidate_bytes > useful)
    {
        return Err(GenerationQualificationContractError::InvalidCandidateOutputPolicy);
    }
    maximum_aggregate_candidate_bytes
        .checked_mul(MAX_JSON_BYTES_PER_DECODED_BYTE)
        .and_then(|escaped| {
            let framing = u64::from(candidate_count) * MAX_CANDIDATE_FRAMING_BYTES
                + MAX_ENVELOPE_FRAMING_BYTES
                + RAW_ENVELOPE_ALLOWANCE_BYTES;
            escaped.checked_add(framing)
        })
        .ok_or(GenerationQualificationContractError::InvalidCandidateOutputPolicy)
}
