use std::collections::BTreeSet;

use sha2::{Digest as _, Sha256};

use rewrite_types::Digest;

use super::{
    CANDIDATE_CONTENT_DIGEST_DOMAIN, CandidateArtifactEntryV1,
    CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationEvidenceBundleManifestV1Relations,
    CandidateGenerationEvidenceBundleRoleV1, EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH,
};
use crate::generation_qualification::{
    CANDIDATE_EVIDENCE_ID_DOMAIN, CandidateEvidenceId, CandidateGenerationAttemptPrecursorId,
    CandidateGenerationAttemptPrecursorV1, GenerationCaseId, GenerationCaseManifestV1,
    GenerationQualificationContractError, MAX_GENERATION_CANDIDATES_PER_COMPLETION,
    PlannedCandidateAttemptV1,
};

pub(super) fn validate_candidate_relationship(
    precursor: &CandidateGenerationAttemptPrecursorV1,
    planned: &PlannedCandidateAttemptV1,
    case: &GenerationCaseManifestV1,
    ordinal: u8,
    bytes: &[u8],
) -> Result<(), GenerationQualificationContractError> {
    if precursor.planned_attempt_id() != planned.planned_attempt_id()
        || planned.case_id() != case.case_id()
        || ordinal >= planned.output_ceilings().candidate_count()
        || ordinal >= MAX_GENERATION_CANDIDATES_PER_COMPLETION
        || std::str::from_utf8(bytes).is_err()
        || u64::try_from(bytes.len()).map_or(true, |count| {
            count > planned.output_ceilings().maximum_candidate_bytes()
        })
    {
        return Err(GenerationQualificationContractError::InvalidCandidateArtifact);
    }
    Ok(())
}

pub(super) fn candidate_content_digest(bytes: &[u8]) -> Digest {
    digest_parts(&[CANDIDATE_CONTENT_DIGEST_DOMAIN, bytes])
}

pub(super) fn candidate_evidence_id(
    precursor_id: &CandidateGenerationAttemptPrecursorId,
    case_id: &GenerationCaseId,
    ordinal: u8,
    byte_count: u64,
    bytes: &[u8],
) -> CandidateEvidenceId {
    let ordinal_bytes = [ordinal];
    let byte_count_bytes = byte_count.to_be_bytes();
    let exact_length = u64::try_from(bytes.len())
        .expect("candidate bytes fit u64")
        .to_be_bytes();
    CandidateEvidenceId(digest_parts(&[
        CANDIDATE_EVIDENCE_ID_DOMAIN,
        precursor_id.digest().as_str().as_bytes(),
        case_id.digest().as_str().as_bytes(),
        &ordinal_bytes,
        &byte_count_bytes,
        &exact_length,
        bytes,
    ]))
}

fn digest_parts(parts: &[&[u8]]) -> Digest {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 formatter is canonical")
}

pub(super) fn validate_bundle(
    relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
    entries: &[CandidateGenerationEvidenceBundleEntryV1],
    candidates: &[CandidateArtifactEntryV1],
) -> Result<u64, GenerationQualificationContractError> {
    validate_upstream(relations)?;
    let limits = relations.qualification_plan.limits();
    if entries.is_empty()
        || entries.len()
            > usize::try_from(limits.maximum_evidence_bundle_entries()).unwrap_or(usize::MAX)
        || candidates.len()
            != usize::from(
                relations
                    .planned_attempt
                    .output_ceilings()
                    .candidate_count(),
            )
    {
        return Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure);
    }
    let mut aggregate = 0_u64;
    let mut previous_path: Option<&str> = None;
    let mut role_counts = [0_u32; 7];
    let mut folded_paths = BTreeSet::new();
    for entry in entries {
        let path = entry.relative_path().as_str();
        let folded = path.to_ascii_lowercase();
        if previous_path.is_some_and(|previous| previous.as_bytes() >= path.as_bytes())
            || path.len()
                > usize::try_from(limits.maximum_evidence_relative_path_bytes())
                    .unwrap_or(usize::MAX)
            || !folded_paths.insert(folded)
            || reserved_manifest_collision(path)
        {
            return Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry);
        }
        previous_path = Some(path);
        aggregate = aggregate
            .checked_add(entry.byte_size())
            .ok_or(GenerationQualificationContractError::EncodingOverflow)?;
        role_counts[usize::from(super::role_tag(entry.role()))] += 1;
    }
    for folded in &folded_paths {
        let mut prefix_end = 0;
        while let Some(separator) = folded[prefix_end..].find('/') {
            prefix_end += separator;
            if folded_paths.contains(&folded[..prefix_end]) {
                return Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry);
            }
            prefix_end += 1;
        }
    }
    if aggregate > limits.maximum_evidence_bundle_bytes() {
        return Err(GenerationQualificationContractError::InvalidEvidenceBundleEntry);
    }
    let candidate_count = u32::try_from(candidates.len())
        .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
    if role_counts[..5] != [1, 1, 1, 1, 1] || role_counts[5] != candidate_count {
        return Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure);
    }
    validate_required_artifacts(relations, entries)?;
    validate_candidates(relations, entries, candidates)?;
    Ok(aggregate)
}

fn reserved_manifest_collision(path: &str) -> bool {
    let folded = path.to_ascii_lowercase();
    folded == EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH
        || folded
            .strip_prefix(EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn validate_required_artifacts(
    relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
    entries: &[CandidateGenerationEvidenceBundleEntryV1],
) -> Result<(), GenerationQualificationContractError> {
    let records = [
        (
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            serde_json::to_vec(relations.planned_attempt),
        ),
        (
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            serde_json::to_vec(relations.precursor),
        ),
        (
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            serde_json::to_vec(relations.managed_evidence),
        ),
        (
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            serde_json::to_vec(relations.cleanup),
        ),
    ];
    for (role, encoded) in records {
        let encoded = encoded.map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let byte_size = u64::try_from(encoded.len())
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        require_artifact(
            entries,
            role,
            &crate::ArtifactId::from_digest(Digest::sha256(&encoded)),
            byte_size,
        )?;
    }
    require_artifact(
        entries,
        CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
        relations.structured_response_artifact.artifact_id(),
        relations.structured_response_artifact.byte_size(),
    )
}

fn require_artifact(
    entries: &[CandidateGenerationEvidenceBundleEntryV1],
    role: CandidateGenerationEvidenceBundleRoleV1,
    artifact_id: &crate::ArtifactId,
    byte_size: u64,
) -> Result<(), GenerationQualificationContractError> {
    if entries.iter().any(|entry| {
        entry.role() == role && entry.artifact_id() == artifact_id && entry.byte_size() == byte_size
    }) {
        Ok(())
    } else {
        Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure)
    }
}

fn validate_upstream(
    relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    let ordinal = usize::try_from(relations.planned_attempt.attempt_ordinal())
        .map_err(|_| GenerationQualificationContractError::EvidenceBundleRelationshipMismatch)?;
    if relations.precursor.qualification_plan_id()
        != relations.qualification_plan.qualification_plan_id()
        || relations.precursor.planned_attempt_id()
            != relations.planned_attempt.planned_attempt_id()
        || relations
            .qualification_plan
            .planned_attempt_ids()
            .get(ordinal)
            != Some(relations.planned_attempt.planned_attempt_id())
        || relations.managed_evidence.precursor_id() != relations.precursor.precursor_id()
        || relations.cleanup.precursor_id() != relations.precursor.precursor_id()
        || relations.cleanup.managed_evidence_id()
            != relations.managed_evidence.managed_evidence_v2_id()
    {
        return Err(GenerationQualificationContractError::EvidenceBundleRelationshipMismatch);
    }
    Ok(())
}

fn validate_candidates(
    relations: CandidateGenerationEvidenceBundleManifestV1Relations<'_>,
    entries: &[CandidateGenerationEvidenceBundleEntryV1],
    candidates: &[CandidateArtifactEntryV1],
) -> Result<(), GenerationQualificationContractError> {
    let mut aggregate = 0_u64;
    for (expected_ordinal, candidate) in candidates.iter().enumerate() {
        if usize::from(candidate.ordinal()) != expected_ordinal
            || candidate.byte_count()
                > relations
                    .planned_attempt
                    .output_ceilings()
                    .maximum_candidate_bytes()
        {
            return Err(GenerationQualificationContractError::InvalidCandidateArtifact);
        }
        aggregate = aggregate
            .checked_add(candidate.byte_count())
            .ok_or(GenerationQualificationContractError::EncodingOverflow)?;
        let matching = entries
            .iter()
            .filter(|entry| {
                entry.role() == CandidateGenerationEvidenceBundleRoleV1::Candidate
                    && entry.relative_path() == candidate.relative_path()
                    && entry.artifact_id() == candidate.artifact_id()
                    && entry.byte_size() == candidate.byte_count()
            })
            .count();
        if matching != 1 {
            return Err(GenerationQualificationContractError::InvalidEvidenceBundleRoleClosure);
        }
    }
    if aggregate
        > relations
            .planned_attempt
            .output_ceilings()
            .maximum_aggregate_candidate_bytes()
    {
        return Err(GenerationQualificationContractError::InvalidCandidateArtifact);
    }
    Ok(())
}
