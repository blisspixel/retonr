use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use super::super::GenerationQualificationOperationContractError;
use super::super::common::{append_u32, append_u64};
use crate::generation_qualification::{
    CandidateOutputCeilingsV1, MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    MAX_GENERATION_RETAINED_INPUT_BYTES, MAX_PLANNED_GENERATION_ATTEMPTS,
};

/// Hard V1 source and complete-input byte ceiling.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES: u64 =
    MAX_GENERATION_RETAINED_INPUT_BYTES;
/// Hard V1 context-token ceiling.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_CONTEXT_TOKENS: u32 = 131_072;
/// Hard V1 output-token ceiling.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_TOKENS: u32 = 8_192;
/// Hard V1 candidate, aggregate-candidate, envelope, and evidence byte ceiling.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_BYTES: u64 =
    MAX_GENERATION_EVIDENCE_BUNDLE_BYTES;
/// Exact V1 candidate count per completion.
pub const GENERATION_QUALIFICATION_OPERATION_CANDIDATES_PER_COMPLETION: u8 = 1;
/// Exact V1 peak candidate-attempt concurrency.
pub const GENERATION_QUALIFICATION_OPERATION_CONCURRENT_ATTEMPTS: u32 = 1;

/// Exact common ceilings for one V1 qualification operation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the canonical wire contract keeps every ceiling explicitly named maximum"
)]
pub struct GenerationQualificationOperationLimitsV1 {
    maximum_source_bytes: u64,
    maximum_complete_input_bytes: u64,
    maximum_context_tokens: u32,
    maximum_output_tokens: u32,
    maximum_output_bytes: u64,
    maximum_candidates_per_completion: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_predeclared_attempts: u32,
    maximum_concurrent_attempts: u32,
    maximum_elapsed_milliseconds: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "the wire shape mirrors every explicitly named maximum ceiling"
)]
struct GenerationQualificationOperationLimitsV1Wire {
    maximum_source_bytes: u64,
    maximum_complete_input_bytes: u64,
    maximum_context_tokens: u32,
    maximum_output_tokens: u32,
    maximum_output_bytes: u64,
    maximum_candidates_per_completion: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_predeclared_attempts: u32,
    maximum_concurrent_attempts: u32,
    maximum_elapsed_milliseconds: u32,
}

impl<'de> Deserialize<'de> for GenerationQualificationOperationLimitsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = GenerationQualificationOperationLimitsV1Wire::deserialize(deserializer)?;
        Self::new(
            wire.maximum_source_bytes,
            wire.maximum_complete_input_bytes,
            wire.maximum_context_tokens,
            wire.maximum_output_tokens,
            wire.maximum_output_bytes,
            wire.maximum_candidates_per_completion,
            wire.maximum_candidate_bytes,
            wire.maximum_aggregate_candidate_bytes,
            wire.maximum_predeclared_attempts,
            wire.maximum_concurrent_attempts,
            wire.maximum_elapsed_milliseconds,
        )
        .map_err(|_error| D::Error::custom("invalid generation qualification operation limits"))
    }
}

impl GenerationQualificationOperationLimitsV1 {
    /// Creates one bounded set of common operation limits.
    ///
    /// The output-envelope ceiling is derived from the exact single-candidate
    /// ceilings and cannot be supplied independently with a different value.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationOperationContractError::InvalidLimits`]
    /// if a limit is zero, exceeds a V1 hard cap, or violates a derived relation.
    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments mirror the frozen wire contract"
    )]
    pub fn new(
        maximum_source_bytes: u64,
        maximum_complete_input_bytes: u64,
        maximum_context_tokens: u32,
        maximum_output_tokens: u32,
        maximum_output_bytes: u64,
        maximum_candidates_per_completion: u8,
        maximum_candidate_bytes: u64,
        maximum_aggregate_candidate_bytes: u64,
        maximum_predeclared_attempts: u32,
        maximum_concurrent_attempts: u32,
        maximum_elapsed_milliseconds: u32,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        let value = Self {
            maximum_source_bytes,
            maximum_complete_input_bytes,
            maximum_context_tokens,
            maximum_output_tokens,
            maximum_output_bytes,
            maximum_candidates_per_completion,
            maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes,
            maximum_predeclared_attempts,
            maximum_concurrent_attempts,
            maximum_elapsed_milliseconds,
        };
        value.validate()?;
        Ok(value)
    }

    pub(super) fn validate(self) -> Result<(), GenerationQualificationOperationContractError> {
        let envelope = CandidateOutputCeilingsV1::new(
            self.maximum_candidates_per_completion,
            self.maximum_candidate_bytes,
            self.maximum_aggregate_candidate_bytes,
        )
        .map_err(|_| GenerationQualificationOperationContractError::InvalidLimits)?;
        let valid = self.maximum_source_bytes > 0
            && self.maximum_source_bytes <= MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES
            && self.maximum_complete_input_bytes >= self.maximum_source_bytes
            && self.maximum_complete_input_bytes
                <= MAX_GENERATION_QUALIFICATION_OPERATION_INPUT_BYTES
            && self.maximum_context_tokens > 0
            && self.maximum_context_tokens <= MAX_GENERATION_QUALIFICATION_OPERATION_CONTEXT_TOKENS
            && self.maximum_output_tokens > 0
            && self.maximum_output_tokens <= MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_TOKENS
            && self.maximum_output_bytes > 0
            && self.maximum_output_bytes <= MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_BYTES
            && self.maximum_candidates_per_completion
                == GENERATION_QUALIFICATION_OPERATION_CANDIDATES_PER_COMPLETION
            && self.maximum_candidate_bytes > 0
            && self.maximum_candidate_bytes <= MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_BYTES
            && self.maximum_aggregate_candidate_bytes > 0
            && self.maximum_aggregate_candidate_bytes
                <= MAX_GENERATION_QUALIFICATION_OPERATION_OUTPUT_BYTES
            && self.maximum_output_bytes == envelope.maximum_envelope_bytes()
            && self.maximum_predeclared_attempts > 0
            && usize::try_from(self.maximum_predeclared_attempts)
                .is_ok_and(|value| value <= MAX_PLANNED_GENERATION_ATTEMPTS)
            && self.maximum_concurrent_attempts
                == GENERATION_QUALIFICATION_OPERATION_CONCURRENT_ATTEMPTS
            && self.maximum_elapsed_milliseconds > 0;
        if valid {
            Ok(())
        } else {
            Err(GenerationQualificationOperationContractError::InvalidLimits)
        }
    }

    /// Returns the maximum retained source bytes.
    #[must_use]
    pub const fn maximum_source_bytes(self) -> u64 {
        self.maximum_source_bytes
    }
    /// Returns the maximum complete provider-neutral input bytes.
    #[must_use]
    pub const fn maximum_complete_input_bytes(self) -> u64 {
        self.maximum_complete_input_bytes
    }
    /// Returns the maximum requested context tokens.
    #[must_use]
    pub const fn maximum_context_tokens(self) -> u32 {
        self.maximum_context_tokens
    }
    /// Returns the maximum requested output tokens.
    #[must_use]
    pub const fn maximum_output_tokens(self) -> u32 {
        self.maximum_output_tokens
    }
    /// Returns the exact derived response-envelope byte ceiling.
    #[must_use]
    pub const fn maximum_output_bytes(self) -> u64 {
        self.maximum_output_bytes
    }
    /// Returns the exact candidate count per completion.
    #[must_use]
    pub const fn maximum_candidates_per_completion(self) -> u8 {
        self.maximum_candidates_per_completion
    }
    /// Returns the maximum bytes in one candidate.
    #[must_use]
    pub const fn maximum_candidate_bytes(self) -> u64 {
        self.maximum_candidate_bytes
    }
    /// Returns the maximum aggregate candidate bytes.
    #[must_use]
    pub const fn maximum_aggregate_candidate_bytes(self) -> u64 {
        self.maximum_aggregate_candidate_bytes
    }
    /// Returns the exact predeclared attempt count.
    #[must_use]
    pub const fn maximum_predeclared_attempts(self) -> u32 {
        self.maximum_predeclared_attempts
    }
    /// Returns the exact peak concurrent attempt count.
    #[must_use]
    pub const fn maximum_concurrent_attempts(self) -> u32 {
        self.maximum_concurrent_attempts
    }
    /// Returns the absolute operation deadline in milliseconds.
    #[must_use]
    pub const fn maximum_elapsed_milliseconds(self) -> u32 {
        self.maximum_elapsed_milliseconds
    }
}

pub(super) fn append_limits(
    output: &mut Vec<u8>,
    limits: GenerationQualificationOperationLimitsV1,
) {
    append_u64(output, limits.maximum_source_bytes());
    append_u64(output, limits.maximum_complete_input_bytes());
    append_u32(output, limits.maximum_context_tokens());
    append_u32(output, limits.maximum_output_tokens());
    append_u64(output, limits.maximum_output_bytes());
    output.push(limits.maximum_candidates_per_completion());
    append_u64(output, limits.maximum_candidate_bytes());
    append_u64(output, limits.maximum_aggregate_candidate_bytes());
    append_u32(output, limits.maximum_predeclared_attempts());
    append_u32(output, limits.maximum_concurrent_attempts());
    append_u32(output, limits.maximum_elapsed_milliseconds());
}
