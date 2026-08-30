use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::codec::{append_count, append_digest, append_u32, validate_canonical_json};
use super::{
    CANDIDATE_SELECTION_POLICY_ID_DOMAIN, CandidateSelectionPolicyId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId,
    GenerationQualificationContractError, GenerationQualificationPlanV1, GenerationSuiteManifestId,
    GenerationSuiteManifestV1, PlannedCandidateAttemptV1,
};

/// Maximum JSON bytes accepted for one candidate-selection policy.
pub const MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_CANDIDATE_SELECTION_POLICY_CANONICAL_BYTES: usize = 64 * 1_024;

/// One pre-output candidate selection for one suite case.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSelectionPolicyEntryV1 {
    case_id: GenerationCaseId,
    selected_ordinal: u8,
}

impl CandidateSelectionPolicyEntryV1 {
    /// Returns the exact case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }

    /// Returns the preselected request-local candidate ordinal.
    #[must_use]
    pub const fn selected_ordinal(&self) -> u8 {
        self.selected_ordinal
    }
}

/// Inert executable selection policy fixed before generation output exists.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSelectionPolicyV1 {
    schema_version: u32,
    suite_manifest_id: GenerationSuiteManifestId,
    entries: Vec<CandidateSelectionPolicyEntryV1>,
    entry_count: u32,
    #[serde(skip)]
    id: CandidateSelectionPolicyId,
}

impl CandidateSelectionPolicyV1 {
    /// Creates one selection entry for every case in semantic suite order.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] when the ordinal array
    /// does not have complete suite closure or the identity exceeds its ceiling.
    pub fn new(
        suite: &GenerationSuiteManifestV1,
        selected_ordinals: &[u8],
    ) -> Result<Self, GenerationQualificationContractError> {
        if selected_ordinals.len() != suite.case_ids().len() {
            return Err(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch);
        }
        let entries = suite
            .case_ids()
            .iter()
            .cloned()
            .zip(selected_ordinals.iter().copied())
            .map(
                |(case_id, selected_ordinal)| CandidateSelectionPolicyEntryV1 {
                    case_id,
                    selected_ordinal,
                },
            )
            .collect();
        Self::from_wire(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            suite,
            suite.suite_manifest_id().clone(),
            entries,
            u32::try_from(selected_ordinals.len())
                .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?,
        )
    }

    fn from_wire(
        schema_version: u32,
        suite: &GenerationSuiteManifestV1,
        suite_manifest_id: GenerationSuiteManifestId,
        entries: Vec<CandidateSelectionPolicyEntryV1>,
        entry_count: u32,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        let derived_count = u32::try_from(entries.len())
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        if &suite_manifest_id != suite.suite_manifest_id()
            || entry_count != derived_count
            || entries.len() != suite.case_ids().len()
            || entries
                .iter()
                .zip(suite.case_ids())
                .any(|(entry, case_id)| entry.case_id() != case_id)
        {
            return Err(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch);
        }
        let canonical = canonical_bytes(schema_version, &suite_manifest_id, &entries, entry_count)?;
        if canonical.len() > MAX_CANDIDATE_SELECTION_POLICY_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            suite_manifest_id,
            entries,
            entry_count,
            id: CandidateSelectionPolicyId(rewrite_types::Digest::sha256(&canonical)),
        })
    }

    /// Parses a bounded canonical policy and reloads its exact suite order.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unknown, duplicate, trailing, future-schema, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        suite: &GenerationSuiteManifestV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            suite_manifest_id: GenerationSuiteManifestId,
            entries: Vec<CandidateSelectionPolicyEntryV1>,
            entry_count: u32,
        }

        if bytes.len() > MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            suite,
            wire.suite_manifest_id,
            wire.entries,
            wire.entry_count,
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates the policy against one exact plan and its complete attempts.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless plan identity,
    /// attempt closure, and every candidate-count bound match exactly.
    pub fn validate_against_plan(
        &self,
        plan: &GenerationQualificationPlanV1,
        planned_attempts: &[PlannedCandidateAttemptV1],
    ) -> Result<(), GenerationQualificationContractError> {
        if plan.suite_manifest_id() != self.suite_manifest_id()
            || plan.selection_policy_digest() != self.selection_policy_id().digest()
            || planned_attempts.len() != plan.planned_attempt_ids().len()
            || planned_attempts
                .iter()
                .zip(plan.planned_attempt_ids())
                .any(|(attempt, id)| attempt.planned_attempt_id() != id)
        {
            return Err(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch);
        }
        for attempt in planned_attempts {
            let entry = self
                .entries
                .iter()
                .find(|entry| entry.case_id() == attempt.case_id())
                .ok_or(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch)?;
            if attempt.suite_manifest_id() != self.suite_manifest_id()
                || entry.selected_ordinal() >= attempt.output_ceilings().candidate_count()
            {
                return Err(
                    GenerationQualificationContractError::SelectionPolicyRelationshipMismatch,
                );
            }
        }
        Ok(())
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact suite identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }

    /// Returns selections in semantic suite order.
    #[must_use]
    pub fn entries(&self) -> &[CandidateSelectionPolicyEntryV1] {
        &self.entries
    }

    /// Returns the array-derived entry count.
    #[must_use]
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }

    /// Returns the content-derived policy identity.
    #[must_use]
    pub const fn selection_policy_id(&self) -> &CandidateSelectionPolicyId {
        &self.id
    }
}

impl fmt::Debug for CandidateSelectionPolicyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateSelectionPolicyV1")
            .field("schema_version", &self.schema_version)
            .field("selection_policy_id", &self.id)
            .field("entry_count", &self.entry_count)
            .finish_non_exhaustive()
    }
}

fn canonical_bytes(
    schema_version: u32,
    suite_manifest_id: &GenerationSuiteManifestId,
    entries: &[CandidateSelectionPolicyEntryV1],
    entry_count: u32,
) -> Result<Vec<u8>, GenerationQualificationContractError> {
    let mut output = CANDIDATE_SELECTION_POLICY_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    append_digest(&mut output, suite_manifest_id.digest());
    append_count(&mut output, entries.len())?;
    for entry in entries {
        append_digest(&mut output, entry.case_id().digest());
        output.push(entry.selected_ordinal());
    }
    append_u32(&mut output, entry_count);
    Ok(output)
}
