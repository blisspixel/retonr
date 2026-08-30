use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::{
    CandidateArtifactEntryV1, CandidateGenerationEvidenceBundleEntryV1,
    CandidateGenerationEvidenceBundleRoleV1,
};
use crate::generation_qualification::{
    CandidateEvidenceId, CandidateGenerationAttemptPrecursorId, CandidateGenerationCleanupId,
    GenerationQualificationContractError, ManagedOllamaCandidateGenerationEvidenceV2Id,
    OllamaRetainedSessionResponseId,
};
use crate::{ArtifactId, ArtifactSetRelativePath};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EntryWire {
    relative_path: String,
    role: CandidateGenerationEvidenceBundleRoleV1,
    artifact_id: ArtifactId,
    byte_size: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CandidateWire {
    candidate_evidence_id: CandidateEvidenceId,
    ordinal: u8,
    byte_count: u64,
    artifact_id: ArtifactId,
    relative_path: String,
    candidate_digest: Digest,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidenceBundleWire {
    schema_version: u32,
    precursor_id: CandidateGenerationAttemptPrecursorId,
    managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    response_id: OllamaRetainedSessionResponseId,
    cleanup_id: CandidateGenerationCleanupId,
    entries: Vec<EntryWire>,
    candidate_artifacts: Vec<CandidateWire>,
    entry_count: u32,
    aggregate_byte_count: u64,
}

pub(super) struct EvidenceBundleWireParts {
    pub(super) schema_version: u32,
    pub(super) precursor_id: CandidateGenerationAttemptPrecursorId,
    pub(super) managed_evidence_id: ManagedOllamaCandidateGenerationEvidenceV2Id,
    pub(super) response_id: OllamaRetainedSessionResponseId,
    pub(super) cleanup_id: CandidateGenerationCleanupId,
    pub(super) entries: Vec<CandidateGenerationEvidenceBundleEntryV1>,
    pub(super) candidates: Vec<CandidateArtifactEntryV1>,
    pub(super) derived_counts: (u32, u64),
}

impl EvidenceBundleWire {
    pub(super) const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub(super) fn into_parts(
        self,
    ) -> Result<EvidenceBundleWireParts, GenerationQualificationContractError> {
        let entries = self
            .entries
            .into_iter()
            .map(|entry| {
                let path = ArtifactSetRelativePath::new(entry.relative_path).map_err(|_| {
                    GenerationQualificationContractError::InvalidEvidenceBundleEntry
                })?;
                Ok(CandidateGenerationEvidenceBundleEntryV1::new(
                    path,
                    entry.role,
                    entry.artifact_id,
                    entry.byte_size,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let candidates = self
            .candidate_artifacts
            .into_iter()
            .map(|candidate| {
                let relative_path = ArtifactSetRelativePath::new(candidate.relative_path)
                    .map_err(|_| GenerationQualificationContractError::InvalidCandidateArtifact)?;
                Ok(CandidateArtifactEntryV1 {
                    candidate_evidence_id: candidate.candidate_evidence_id,
                    ordinal: candidate.ordinal,
                    byte_count: candidate.byte_count,
                    artifact_id: candidate.artifact_id,
                    relative_path,
                    candidate_digest: candidate.candidate_digest,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EvidenceBundleWireParts {
            schema_version: self.schema_version,
            precursor_id: self.precursor_id,
            managed_evidence_id: self.managed_evidence_id,
            response_id: self.response_id,
            cleanup_id: self.cleanup_id,
            entries,
            candidates,
            derived_counts: (self.entry_count, self.aggregate_byte_count),
        })
    }
}
