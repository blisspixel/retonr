use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{append_count, append_digest, append_u32, append_u64, validate_canonical_json};
use super::{
    GENERATION_QUALIFICATION_PLAN_ID_DOMAIN, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GenerationQualificationContractError, GenerationQualificationPlanId,
    GenerationRepetitionRecordV1, GenerationSuiteManifestId, GenerationSuiteManifestV1,
    GenerationSystemId, GenerationSystemRecordV1, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1,
};

mod validation;

use validation::{PlanBuildContext, validate_records};

/// Maximum candidate count admitted for one completion.
pub const MAX_GENERATION_CANDIDATES_PER_COMPLETION: u8 = 16;
/// Maximum generation systems named by one qualification plan.
pub const MAX_GENERATION_SYSTEMS_PER_PLAN: usize = 16;
/// Maximum planned generation attempts in one qualification plan.
pub const MAX_PLANNED_GENERATION_ATTEMPTS: usize = 1_024;
/// Maximum complete retained-session UTF-8 input bytes.
pub const MAX_GENERATION_RETAINED_INPUT_BYTES: u64 = 4 * 1_024 * 1_024;
/// Maximum content entries in one generation evidence bundle.
pub const MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES: u32 = 4_096;
/// Maximum bytes in one canonical evidence relative path.
pub const MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES: u32 = 512;
/// Maximum aggregate bytes in one generation evidence bundle.
pub const MAX_GENERATION_EVIDENCE_BUNDLE_BYTES: u64 = 256 * 1_024 * 1_024;
/// Maximum JSON bytes accepted for one generation qualification plan.
pub const MAX_GENERATION_PLAN_JSON_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_GENERATION_PLAN_CANONICAL_BYTES: usize = 128 * 1_024;

/// Closed retry policy for a version 1 generation qualification plan.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationRetryPolicyV1 {
    /// Every attempt is separately predeclared and an attempt is never retried.
    NoRetry,
}

/// Plan-local ceilings that may narrow but never widen the hard contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the canonical wire contract keeps every ceiling explicitly named maximum"
)]
pub struct GenerationQualificationPlanLimitsV1 {
    maximum_candidates_per_completion: u8,
    maximum_predeclared_attempts: u32,
    maximum_retained_input_bytes: u64,
    maximum_evidence_bundle_entries: u32,
    maximum_evidence_relative_path_bytes: u32,
    maximum_evidence_bundle_bytes: u64,
}

impl GenerationQualificationPlanLimitsV1 {
    /// Creates nonzero plan limits no broader than the hard generation contract.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError::InvalidLimits`] if any
    /// limit is zero or exceeds its hard maximum.
    pub fn new(
        maximum_candidates_per_completion: u8,
        maximum_predeclared_attempts: u32,
        maximum_retained_input_bytes: u64,
        maximum_evidence_bundle_entries: u32,
        maximum_evidence_relative_path_bytes: u32,
        maximum_evidence_bundle_bytes: u64,
    ) -> Result<Self, GenerationQualificationContractError> {
        let limits = Self {
            maximum_candidates_per_completion,
            maximum_predeclared_attempts,
            maximum_retained_input_bytes,
            maximum_evidence_bundle_entries,
            maximum_evidence_relative_path_bytes,
            maximum_evidence_bundle_bytes,
        };
        limits.validate()?;
        Ok(limits)
    }

    fn validate(self) -> Result<(), GenerationQualificationContractError> {
        let valid = self.maximum_candidates_per_completion > 0
            && self.maximum_candidates_per_completion <= MAX_GENERATION_CANDIDATES_PER_COMPLETION
            && self.maximum_predeclared_attempts > 0
            && usize::try_from(self.maximum_predeclared_attempts)
                .is_ok_and(|value| value <= MAX_PLANNED_GENERATION_ATTEMPTS)
            && self.maximum_retained_input_bytes > 0
            && self.maximum_retained_input_bytes <= MAX_GENERATION_RETAINED_INPUT_BYTES
            && self.maximum_evidence_bundle_entries > 0
            && self.maximum_evidence_bundle_entries <= MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES
            && self.maximum_evidence_relative_path_bytes > 0
            && self.maximum_evidence_relative_path_bytes
                <= MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES
            && self.maximum_evidence_bundle_bytes > 0
            && self.maximum_evidence_bundle_bytes <= MAX_GENERATION_EVIDENCE_BUNDLE_BYTES;
        if valid {
            Ok(())
        } else {
            Err(GenerationQualificationContractError::InvalidLimits)
        }
    }

    /// Returns the exact candidate-count ceiling.
    #[must_use]
    pub const fn maximum_candidates_per_completion(self) -> u8 {
        self.maximum_candidates_per_completion
    }

    /// Returns the exact predeclared-attempt ceiling.
    #[must_use]
    pub const fn maximum_predeclared_attempts(self) -> u32 {
        self.maximum_predeclared_attempts
    }

    /// Returns the exact retained-input byte ceiling.
    #[must_use]
    pub const fn maximum_retained_input_bytes(self) -> u64 {
        self.maximum_retained_input_bytes
    }

    /// Returns the exact evidence-entry ceiling.
    #[must_use]
    pub const fn maximum_evidence_bundle_entries(self) -> u32 {
        self.maximum_evidence_bundle_entries
    }

    /// Returns the exact evidence-path byte ceiling.
    #[must_use]
    pub const fn maximum_evidence_relative_path_bytes(self) -> u32 {
        self.maximum_evidence_relative_path_bytes
    }

    /// Returns the exact aggregate evidence byte ceiling.
    #[must_use]
    pub const fn maximum_evidence_bundle_bytes(self) -> u64 {
        self.maximum_evidence_bundle_bytes
    }
}

/// Caller-supplied inert facts for one generation qualification plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationQualificationPlanV1Input {
    /// Exact plan-local limits.
    pub limits: GenerationQualificationPlanLimitsV1,
    /// Digest of the candidate selection policy.
    pub selection_policy_digest: Digest,
    /// Digest of the complete attempt failure policy.
    pub failure_policy_digest: Digest,
}

/// Inert portable generation qualification plan.
///
/// Construction reloads exact generation-system and planned-attempt records and
/// validates contiguous ordinals plus complete suite and system closure. It grants
/// no qualification or execution authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationPlanV1 {
    schema_version: u32,
    suite_manifest_id: GenerationSuiteManifestId,
    generation_system_ids: Vec<GenerationSystemId>,
    planned_attempt_ids: Vec<PlannedCandidateAttemptId>,
    limits: GenerationQualificationPlanLimitsV1,
    selection_policy_digest: Digest,
    failure_policy_digest: Digest,
    retry_policy: GenerationQualificationRetryPolicyV1,
    #[serde(skip)]
    id: GenerationQualificationPlanId,
}

impl GenerationQualificationPlanV1 {
    /// Creates one bounded inert plan from exact system and attempt records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for invalid limits,
    /// empty or excessive arrays, duplicate attempts, or a system set that is not
    /// strictly digest-sorted and unique.
    pub fn new(
        suite: &GenerationSuiteManifestV1,
        repetitions: &[GenerationRepetitionRecordV1],
        generation_systems: &[GenerationSystemRecordV1],
        planned_attempts: &[PlannedCandidateAttemptV1],
        input: GenerationQualificationPlanV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::from_wire(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            suite.suite_manifest_id().clone(),
            input,
            GenerationQualificationRetryPolicyV1::NoRetry,
            PlanBuildContext {
                suite,
                repetitions,
                generation_systems,
                planned_attempts,
                wire_ids: None,
            },
        )
    }

    fn from_wire(
        schema_version: u32,
        suite_manifest_id: GenerationSuiteManifestId,
        input: GenerationQualificationPlanV1Input,
        retry_policy: GenerationQualificationRetryPolicyV1,
        context: PlanBuildContext<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if &suite_manifest_id != context.suite.suite_manifest_id() {
            return Err(GenerationQualificationContractError::SuiteMismatch);
        }
        input.limits.validate()?;
        validate_records(
            context.suite,
            context.repetitions,
            context.generation_systems,
            context.planned_attempts,
            input.limits,
        )?;
        let generation_system_ids = context
            .generation_systems
            .iter()
            .map(|system| system.generation_system_id().clone())
            .collect::<Vec<_>>();
        let planned_attempt_ids = context
            .planned_attempts
            .iter()
            .map(|attempt| attempt.planned_attempt_id().clone())
            .collect::<Vec<_>>();
        if context.wire_ids.is_some_and(|(systems, attempts)| {
            systems != generation_system_ids || attempts != planned_attempt_ids
        }) {
            return Err(GenerationQualificationContractError::PlannedAttemptMismatch);
        }
        let canonical = canonical_bytes(
            schema_version,
            &suite_manifest_id,
            &generation_system_ids,
            &planned_attempt_ids,
            &input,
            retry_policy,
        )?;
        if canonical.len() > MAX_GENERATION_PLAN_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            suite_manifest_id,
            generation_system_ids,
            planned_attempt_ids,
            limits: input.limits,
            selection_policy_digest: input.selection_policy_digest,
            failure_policy_digest: input.failure_policy_digest,
            retry_policy,
            id: GenerationQualificationPlanId(Digest::sha256(&canonical)),
        })
    }

    /// Parses one canonical plan and rechecks its exact suite relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for excessive, malformed,
    /// noncanonical, unsupported, cross-suite, duplicate, reordered, or invalid input.
    pub fn from_json_bytes(
        bytes: &[u8],
        suite: &GenerationSuiteManifestV1,
        repetitions: &[GenerationRepetitionRecordV1],
        generation_systems: &[GenerationSystemRecordV1],
        planned_attempts: &[PlannedCandidateAttemptV1],
    ) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            suite_manifest_id: GenerationSuiteManifestId,
            generation_system_ids: Vec<GenerationSystemId>,
            planned_attempt_ids: Vec<PlannedCandidateAttemptId>,
            limits: GenerationQualificationPlanLimitsV1,
            selection_policy_digest: Digest,
            failure_policy_digest: Digest,
            retry_policy: GenerationQualificationRetryPolicyV1,
        }

        if bytes.len() > MAX_GENERATION_PLAN_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        let record = Self::from_wire(
            wire.schema_version,
            wire.suite_manifest_id,
            GenerationQualificationPlanV1Input {
                limits: wire.limits,
                selection_policy_digest: wire.selection_policy_digest,
                failure_policy_digest: wire.failure_policy_digest,
            },
            wire.retry_policy,
            PlanBuildContext {
                suite,
                repetitions,
                generation_systems,
                planned_attempts,
                wire_ids: Some((&wire.generation_system_ids, &wire.planned_attempt_ids)),
            },
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact suite-manifest identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the digest-sorted unique generation-system set.
    #[must_use]
    pub fn generation_system_ids(&self) -> &[GenerationSystemId] {
        &self.generation_system_ids
    }
    /// Returns planned attempts in semantic execution order.
    #[must_use]
    pub fn planned_attempt_ids(&self) -> &[PlannedCandidateAttemptId] {
        &self.planned_attempt_ids
    }
    /// Returns the exact plan-local ceilings.
    #[must_use]
    pub const fn limits(&self) -> GenerationQualificationPlanLimitsV1 {
        self.limits
    }
    /// Returns the candidate selection-policy digest.
    #[must_use]
    pub const fn selection_policy_digest(&self) -> &Digest {
        &self.selection_policy_digest
    }
    /// Returns the complete failure-policy digest.
    #[must_use]
    pub const fn failure_policy_digest(&self) -> &Digest {
        &self.failure_policy_digest
    }
    /// Returns the closed no-retry policy.
    #[must_use]
    pub const fn retry_policy(&self) -> GenerationQualificationRetryPolicyV1 {
        self.retry_policy
    }
    /// Returns the content-derived qualification-plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.id
    }
}

impl fmt::Debug for GenerationQualificationPlanV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationPlanV1")
            .field("schema_version", &self.schema_version)
            .field("qualification_plan_id", &self.id)
            .field("generation_system_count", &self.generation_system_ids.len())
            .field("planned_attempt_count", &self.planned_attempt_ids.len())
            .field("retry_policy", &self.retry_policy)
            .finish_non_exhaustive()
    }
}

fn canonical_bytes(
    schema_version: u32,
    suite_manifest_id: &GenerationSuiteManifestId,
    generation_system_ids: &[GenerationSystemId],
    planned_attempt_ids: &[PlannedCandidateAttemptId],
    input: &GenerationQualificationPlanV1Input,
    retry_policy: GenerationQualificationRetryPolicyV1,
) -> Result<Vec<u8>, GenerationQualificationContractError> {
    let mut output = GENERATION_QUALIFICATION_PLAN_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    append_digest(&mut output, suite_manifest_id.digest());
    append_count(&mut output, generation_system_ids.len())?;
    for id in generation_system_ids {
        append_digest(&mut output, id.digest());
    }
    append_count(&mut output, planned_attempt_ids.len())?;
    for id in planned_attempt_ids {
        append_digest(&mut output, id.digest());
    }
    append_limits(&mut output, input.limits);
    append_digest(&mut output, &input.selection_policy_digest);
    append_digest(&mut output, &input.failure_policy_digest);
    output.push(match retry_policy {
        GenerationQualificationRetryPolicyV1::NoRetry => 0,
    });
    Ok(output)
}

fn append_limits(output: &mut Vec<u8>, limits: GenerationQualificationPlanLimitsV1) {
    output.push(limits.maximum_candidates_per_completion());
    append_u32(output, limits.maximum_predeclared_attempts());
    append_u64(output, limits.maximum_retained_input_bytes());
    append_u32(output, limits.maximum_evidence_bundle_entries());
    append_u32(output, limits.maximum_evidence_relative_path_bytes());
    append_u64(output, limits.maximum_evidence_bundle_bytes());
}
