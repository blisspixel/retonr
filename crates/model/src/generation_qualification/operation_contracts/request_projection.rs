//! Complete content-free projection of every preregistered request.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    MAX_PROJECTION_CANONICAL_BYTES, MAX_PROJECTION_JSON_BYTES, OPERATION_SCHEMA_VERSION,
    append_digest, append_u32, append_u64, validate_canonical_json,
};
use super::{
    GENERATION_QUALIFICATION_REQUEST_PROJECTION_ID_DOMAIN,
    GenerationQualificationOperationContractError, GenerationQualificationOperationPolicyId,
    GenerationQualificationOperationPolicyV1, GenerationQualificationRequestProjectionId,
};
use crate::generation_qualification::{
    GenerationQualificationPlanId, GenerationQualificationPlanV1, GenerationSuiteManifestId,
    GenerationSuiteManifestV1, GenerationSystemId, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1,
};
use crate::{GenerationRequestBindingId, StructuredCompletionRequestBindingId};

/// Maximum JSON bytes accepted for one request projection.
pub const MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES: usize =
    MAX_PROJECTION_JSON_BYTES;
/// Maximum canonical identity bytes for one request projection.
pub const MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_CANONICAL_BYTES: usize =
    MAX_PROJECTION_CANONICAL_BYTES;

/// Request facts derived by the upper-layer compiler from one exact raw request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationQualificationRequestProjectionEntryV1Input {
    /// Binding of the internally derived provider-specific structured request.
    pub structured_completion_request_binding_id: StructuredCompletionRequestBindingId,
    /// Complete provider-neutral input bytes, including all retained context.
    pub complete_input_byte_count: u64,
    /// Exact requested context-token ceiling.
    pub context_token_limit: u32,
    /// Exact requested output-token ceiling.
    pub output_token_limit: u32,
}

/// Content-free projection of one exact planned request.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationRequestProjectionEntryV1 {
    planned_attempt_id: PlannedCandidateAttemptId,
    generation_system_id: GenerationSystemId,
    generation_request_binding_id: GenerationRequestBindingId,
    structured_completion_request_binding_id: StructuredCompletionRequestBindingId,
    source_byte_count: u64,
    complete_input_byte_count: u64,
    source_byte_limit: u64,
    complete_input_byte_limit: u64,
    context_token_limit: u32,
    output_token_limit: u32,
    candidate_count: u8,
    candidate_byte_limit: u64,
    aggregate_candidate_byte_limit: u64,
    output_byte_limit: u64,
}

impl GenerationQualificationRequestProjectionEntryV1 {
    /// Returns the exact planned-attempt identity.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }
    /// Returns the exact generation-system identity.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.generation_system_id
    }
    /// Returns the provider-neutral request binding.
    #[must_use]
    pub const fn generation_request_binding_id(&self) -> &GenerationRequestBindingId {
        &self.generation_request_binding_id
    }
    /// Returns the provider-specific structured-request binding.
    #[must_use]
    pub const fn structured_completion_request_binding_id(
        &self,
    ) -> &StructuredCompletionRequestBindingId {
        &self.structured_completion_request_binding_id
    }
    /// Returns the exact source byte count.
    #[must_use]
    pub const fn source_byte_count(&self) -> u64 {
        self.source_byte_count
    }
    /// Returns the exact complete-input byte count.
    #[must_use]
    pub const fn complete_input_byte_count(&self) -> u64 {
        self.complete_input_byte_count
    }
    /// Returns the common source byte ceiling.
    #[must_use]
    pub const fn source_byte_limit(&self) -> u64 {
        self.source_byte_limit
    }
    /// Returns the common complete-input byte ceiling.
    #[must_use]
    pub const fn complete_input_byte_limit(&self) -> u64 {
        self.complete_input_byte_limit
    }
    /// Returns the common context-token ceiling.
    #[must_use]
    pub const fn context_token_limit(&self) -> u32 {
        self.context_token_limit
    }
    /// Returns the common output-token ceiling.
    #[must_use]
    pub const fn output_token_limit(&self) -> u32 {
        self.output_token_limit
    }
    /// Returns the fixed V1 candidate count.
    #[must_use]
    pub const fn candidate_count(&self) -> u8 {
        self.candidate_count
    }
    /// Returns the common per-candidate byte ceiling.
    #[must_use]
    pub const fn candidate_byte_limit(&self) -> u64 {
        self.candidate_byte_limit
    }
    /// Returns the common aggregate-candidate byte ceiling.
    #[must_use]
    pub const fn aggregate_candidate_byte_limit(&self) -> u64 {
        self.aggregate_candidate_byte_limit
    }
    /// Returns the common exact structured-output envelope ceiling.
    #[must_use]
    pub const fn output_byte_limit(&self) -> u64 {
        self.output_byte_limit
    }

    fn append_canonical_bytes(&self, output: &mut Vec<u8>) {
        for digest in [
            self.planned_attempt_id.digest(),
            self.generation_system_id.digest(),
            self.generation_request_binding_id.digest(),
            self.structured_completion_request_binding_id.digest(),
        ] {
            append_digest(output, digest);
        }
        append_u64(output, self.source_byte_count);
        append_u64(output, self.complete_input_byte_count);
        append_u64(output, self.source_byte_limit);
        append_u64(output, self.complete_input_byte_limit);
        append_u32(output, self.context_token_limit);
        append_u32(output, self.output_token_limit);
        output.push(self.candidate_count);
        append_u64(output, self.candidate_byte_limit);
        append_u64(output, self.aggregate_candidate_byte_limit);
        append_u64(output, self.output_byte_limit);
    }
}

/// Exact portable records needed to validate one request projection.
#[derive(Clone, Copy)]
pub struct GenerationQualificationRequestProjectionV1Relations<'a> {
    /// Exact preregistered operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Exact qualification plan named by the policy.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Exact suite named by the plan.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Complete planned attempts in exact plan order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
}

/// Complete content-free request projection for one qualification operation.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationRequestProjectionV1 {
    schema_version: u32,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    entry_count: u64,
    entries: Vec<GenerationQualificationRequestProjectionEntryV1>,
    #[serde(skip)]
    id: GenerationQualificationRequestProjectionId,
}

impl GenerationQualificationRequestProjectionV1 {
    /// Creates the complete plan-order request projection.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless every bounded entry is present once,
    /// in exact plan order, with the common preregistered request ceilings.
    pub fn new(
        relations: GenerationQualificationRequestProjectionV1Relations<'_>,
        entry_inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        Self::build(relations, entry_inputs, None)
    }

    /// Decodes canonical bounded JSON and rederives every entry relationship.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, missing, reordered, duplicated, or substituted input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationQualificationRequestProjectionV1Relations<'_>,
        entry_inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES {
            return Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge);
        }
        let wire: ProjectionWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationOperationContractError::InvalidEncoding)?;
        if wire.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        validate_count(relations, wire.entry_count, wire.entries.len())?;
        let trusted_count = u64::try_from(entry_inputs.len())
            .map_err(|_| GenerationQualificationOperationContractError::EncodingOverflow)?;
        validate_count(relations, trusted_count, entry_inputs.len())?;
        let value = Self::build(relations, entry_inputs, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    fn build(
        relations: GenerationQualificationRequestProjectionV1Relations<'_>,
        entry_inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
        wire: Option<&ProjectionWire>,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        let entry_count = u64::try_from(entry_inputs.len())
            .map_err(|_| GenerationQualificationOperationContractError::EncodingOverflow)?;
        validate_count(relations, entry_count, entry_inputs.len())?;
        validate_scope(relations)?;
        let limits = relations.operation_policy.limits();
        let mut entries = Vec::with_capacity(entry_inputs.len());
        for (attempt, input) in relations.planned_attempts.iter().zip(entry_inputs) {
            let ceilings = attempt.output_ceilings();
            if attempt.source_byte_count() > limits.maximum_source_bytes()
                || input.complete_input_byte_count > limits.maximum_complete_input_bytes()
                || input.context_token_limit != limits.maximum_context_tokens()
                || input.output_token_limit != limits.maximum_output_tokens()
                || ceilings.candidate_count() != limits.maximum_candidates_per_completion()
                || ceilings.maximum_candidate_bytes() != limits.maximum_candidate_bytes()
                || ceilings.maximum_aggregate_candidate_bytes()
                    != limits.maximum_aggregate_candidate_bytes()
                || ceilings.maximum_envelope_bytes() != limits.maximum_output_bytes()
            {
                return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
            }
            entries.push(GenerationQualificationRequestProjectionEntryV1 {
                planned_attempt_id: attempt.planned_attempt_id().clone(),
                generation_system_id: attempt.generation_system_id().clone(),
                generation_request_binding_id: attempt.generation_request_binding_id().clone(),
                structured_completion_request_binding_id: input
                    .structured_completion_request_binding_id
                    .clone(),
                source_byte_count: attempt.source_byte_count(),
                complete_input_byte_count: input.complete_input_byte_count,
                source_byte_limit: limits.maximum_source_bytes(),
                complete_input_byte_limit: limits.maximum_complete_input_bytes(),
                context_token_limit: input.context_token_limit,
                output_token_limit: input.output_token_limit,
                candidate_count: ceilings.candidate_count(),
                candidate_byte_limit: ceilings.maximum_candidate_bytes(),
                aggregate_candidate_byte_limit: ceilings.maximum_aggregate_candidate_bytes(),
                output_byte_limit: ceilings.maximum_envelope_bytes(),
            });
        }
        let mut value = Self {
            schema_version: OPERATION_SCHEMA_VERSION,
            operation_policy_id: relations.operation_policy.operation_policy_id().clone(),
            target_generation_system_id: relations
                .operation_policy
                .target_generation_system_id()
                .clone(),
            baseline_generation_system_id: relations
                .operation_policy
                .baseline_generation_system_id()
                .clone(),
            generation_qualification_plan_id: relations
                .qualification_plan
                .qualification_plan_id()
                .clone(),
            suite_manifest_id: relations.suite.suite_manifest_id().clone(),
            entry_count,
            entries,
            id: GenerationQualificationRequestProjectionId::from_canonical_bytes(
                b"uninitialized qualification request projection",
            ),
        };
        if wire.is_some_and(|wire| !wire.matches(&value)) {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        let canonical = value.canonical_bytes()?;
        value.id = GenerationQualificationRequestProjectionId::from_canonical_bytes(&canonical);
        Ok(value)
    }

    /// Revalidates the complete projection against fresh exact dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when any dependency, entry, order, or derived limit differs.
    pub fn validate_against(
        &self,
        relations: GenerationQualificationRequestProjectionV1Relations<'_>,
        entry_inputs: &[GenerationQualificationRequestProjectionEntryV1Input],
    ) -> Result<(), GenerationQualificationOperationContractError> {
        let expected = Self::new(relations, entry_inputs)?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationOperationContractError::RelationshipMismatch)
        }
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact operation-policy identity.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &GenerationQualificationOperationPolicyId {
        &self.operation_policy_id
    }
    /// Returns the exact target generation system.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }
    /// Returns the exact baseline generation system.
    #[must_use]
    pub const fn baseline_generation_system_id(&self) -> &GenerationSystemId {
        &self.baseline_generation_system_id
    }
    /// Returns the exact qualification plan.
    #[must_use]
    pub const fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.generation_qualification_plan_id
    }
    /// Returns the exact suite manifest.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns every request projection in exact plan order.
    #[must_use]
    pub fn entries(&self) -> &[GenerationQualificationRequestProjectionEntryV1] {
        &self.entries
    }
    /// Returns the checked number of complete projected entries.
    #[must_use]
    pub const fn entry_count(&self) -> u64 {
        self.entry_count
    }
    /// Returns the content-derived projection identity.
    #[must_use]
    pub const fn request_projection_id(&self) -> &GenerationQualificationRequestProjectionId {
        &self.id
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationOperationContractError> {
        let mut output = GENERATION_QUALIFICATION_REQUEST_PROJECTION_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.operation_policy_id.digest(),
            self.target_generation_system_id.digest(),
            self.baseline_generation_system_id.digest(),
            self.generation_qualification_plan_id.digest(),
            self.suite_manifest_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.entry_count);
        for entry in &self.entries {
            entry.append_canonical_bytes(&mut output);
            if output.len() > MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_CANONICAL_BYTES {
                return Err(
                    GenerationQualificationOperationContractError::CanonicalEncodingTooLarge,
                );
            }
        }
        Ok(output)
    }
}

impl fmt::Debug for GenerationQualificationRequestProjectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationRequestProjectionV1")
            .field("request_projection_id", &self.id)
            .field("entry_count", &self.entry_count)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionWire {
    schema_version: u32,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    entry_count: u64,
    entries: Vec<GenerationQualificationRequestProjectionEntryV1>,
}

impl ProjectionWire {
    fn matches(&self, value: &GenerationQualificationRequestProjectionV1) -> bool {
        self.schema_version == value.schema_version
            && self.operation_policy_id == value.operation_policy_id
            && self.target_generation_system_id == value.target_generation_system_id
            && self.baseline_generation_system_id == value.baseline_generation_system_id
            && self.generation_qualification_plan_id == value.generation_qualification_plan_id
            && self.suite_manifest_id == value.suite_manifest_id
            && self.entry_count == value.entry_count
            && self.entries == value.entries
    }
}

fn validate_count(
    relations: GenerationQualificationRequestProjectionV1Relations<'_>,
    declared_count: u64,
    entry_count: usize,
) -> Result<(), GenerationQualificationOperationContractError> {
    let expected_count = relations.planned_attempts.len();
    let policy_count = usize::try_from(
        relations
            .operation_policy
            .limits()
            .maximum_predeclared_attempts(),
    )
    .map_err(|_| GenerationQualificationOperationContractError::EncodingOverflow)?;
    if usize::try_from(declared_count).ok() == Some(entry_count)
        && entry_count == expected_count
        && entry_count == relations.qualification_plan.planned_attempt_ids().len()
        && entry_count == policy_count
        && entry_count > 0
        && entry_count <= super::super::MAX_PLANNED_GENERATION_ATTEMPTS
    {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::InvalidCount)
    }
}

fn validate_scope(
    relations: GenerationQualificationRequestProjectionV1Relations<'_>,
) -> Result<(), GenerationQualificationOperationContractError> {
    let policy = relations.operation_policy;
    let plan = relations.qualification_plan;
    if policy.generation_qualification_plan_id() != plan.qualification_plan_id()
        || policy.suite_manifest_id() != relations.suite.suite_manifest_id()
        || plan.suite_manifest_id() != relations.suite.suite_manifest_id()
        || plan
            .generation_system_ids()
            .iter()
            .filter(|id| {
                *id == policy.target_generation_system_id()
                    || *id == policy.baseline_generation_system_id()
            })
            .count()
            != 2
        || plan
            .planned_attempt_ids()
            .iter()
            .zip(relations.planned_attempts)
            .any(|(id, attempt)| id != attempt.planned_attempt_id())
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "request_projection/tests.rs"]
mod tests;
