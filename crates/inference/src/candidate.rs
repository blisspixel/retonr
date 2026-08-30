use thiserror::Error;

use crate::{GenerationCandidate, MAX_INFERENCE_CANDIDATES};

mod parser;

const MAX_JSON_BYTES_PER_DECODED_BYTE: u64 = 6;
const MAX_ENVELOPE_FRAMING_BYTES: u64 = 16;
const MAX_CANDIDATE_FRAMING_BYTES: u64 = 12;
const RAW_ENVELOPE_ALLOWANCE_BYTES: u64 = 256;

/// Exact count and byte ceilings for one provider-neutral candidate envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateOutputPolicy {
    expected_count: u8,
    maximum_candidate_bytes: u64,
    maximum_aggregate_candidate_bytes: u64,
    maximum_envelope_bytes: u64,
}

impl CandidateOutputPolicy {
    /// Creates an exact bounded candidate-output policy.
    ///
    /// The aggregate ceiling may be stricter than the exact count multiplied by
    /// the per-candidate ceiling, but it cannot be greater. The raw envelope
    /// ceiling is derived from the aggregate ceiling, worst-case candidate-text
    /// JSON escaping, exact-count framing, and a fixed total raw-envelope
    /// allowance.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateOutputError::InvalidPolicy`] when the expected count is
    /// zero or above the provider-neutral maximum, either byte ceiling is zero,
    /// the aggregate ceiling is not useful, or checked ceiling arithmetic fails.
    pub const fn new(
        expected_count: u8,
        maximum_candidate_bytes: u64,
        maximum_aggregate_candidate_bytes: u64,
    ) -> Result<Self, CandidateOutputError> {
        if expected_count == 0
            || expected_count > MAX_INFERENCE_CANDIDATES
            || maximum_candidate_bytes == 0
            || maximum_aggregate_candidate_bytes == 0
        {
            return Err(CandidateOutputError::InvalidPolicy);
        }
        let Some(useful_aggregate_ceiling) =
            maximum_candidate_bytes.checked_mul(expected_count as u64)
        else {
            return Err(CandidateOutputError::InvalidPolicy);
        };
        if maximum_aggregate_candidate_bytes > useful_aggregate_ceiling {
            return Err(CandidateOutputError::InvalidPolicy);
        }
        let Some(escaped_candidate_bytes) =
            maximum_aggregate_candidate_bytes.checked_mul(MAX_JSON_BYTES_PER_DECODED_BYTE)
        else {
            return Err(CandidateOutputError::InvalidPolicy);
        };
        let framing_bytes = (expected_count as u64) * MAX_CANDIDATE_FRAMING_BYTES
            + MAX_ENVELOPE_FRAMING_BYTES
            + RAW_ENVELOPE_ALLOWANCE_BYTES;
        let Some(maximum_envelope_bytes) = escaped_candidate_bytes.checked_add(framing_bytes)
        else {
            return Err(CandidateOutputError::InvalidPolicy);
        };
        Ok(Self {
            expected_count,
            maximum_candidate_bytes,
            maximum_aggregate_candidate_bytes,
            maximum_envelope_bytes,
        })
    }

    /// Returns the exact required candidate count.
    #[must_use]
    pub const fn expected_count(self) -> u8 {
        self.expected_count
    }

    /// Returns the maximum UTF-8 bytes accepted for each candidate.
    #[must_use]
    pub const fn maximum_candidate_bytes(self) -> u64 {
        self.maximum_candidate_bytes
    }

    /// Returns the maximum checked UTF-8 byte sum across all candidates.
    #[must_use]
    pub const fn maximum_aggregate_candidate_bytes(self) -> u64 {
        self.maximum_aggregate_candidate_bytes
    }

    /// Returns the derived maximum raw JSON envelope bytes.
    #[must_use]
    pub const fn maximum_envelope_bytes(self) -> u64 {
        self.maximum_envelope_bytes
    }
}

/// Provider-neutral candidate envelope validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CandidateOutputError {
    /// Count or byte ceilings do not form a supported policy.
    #[error("candidate output policy is invalid")]
    InvalidPolicy,
    /// The raw JSON envelope exceeds its derived byte ceiling.
    #[error("candidate output envelope exceeds its raw byte ceiling")]
    EnvelopeTooLarge,
    /// Backend output is not valid UTF-8.
    #[error("candidate output is not valid UTF-8")]
    InvalidUtf8,
    /// Backend output is not one complete candidate-envelope JSON value.
    #[error("candidate output envelope is invalid")]
    InvalidEnvelope,
    /// The observed candidate count differs from the exact policy.
    #[error("candidate output count differs from policy")]
    CountMismatch,
    /// One candidate exceeds its UTF-8 byte ceiling.
    #[error("candidate output exceeds its per-candidate byte ceiling")]
    CandidateTooLarge,
    /// Candidate byte-count addition overflowed.
    #[error("candidate output aggregate byte count overflowed")]
    AggregateByteCountOverflow,
    /// The checked candidate byte sum exceeds its ceiling.
    #[error("candidate output exceeds its aggregate byte ceiling")]
    AggregateTooLarge,
}

impl CandidateOutputError {
    /// Returns a stable redacted adapter error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "invalid_candidate_output_policy",
            Self::EnvelopeTooLarge => "candidate_envelope_too_large",
            Self::InvalidUtf8 => "invalid_candidate_output_utf8",
            Self::InvalidEnvelope => "invalid_candidate_envelope",
            Self::CountMismatch => "candidate_count_mismatch",
            Self::CandidateTooLarge => "candidate_too_large",
            Self::AggregateByteCountOverflow => "candidate_byte_count_overflow",
            Self::AggregateTooLarge => "candidate_aggregate_too_large",
        }
    }
}

/// Parses one complete bounded provider-neutral candidate envelope.
///
/// Candidate ordinals are assigned canonically from zero in observed array order.
/// The raw input is bounded before decoding, and decoding retains at most 16
/// candidates.
///
/// # Errors
///
/// Returns [`CandidateOutputError`] for invalid policy, raw size, UTF-8, JSON
/// framing or structure, count mismatch, a per-candidate violation, aggregate
/// overflow, or an aggregate byte violation.
pub fn parse_candidate_output(
    bytes: &[u8],
    policy: CandidateOutputPolicy,
) -> Result<Vec<GenerationCandidate>, CandidateOutputError> {
    parser::parse(bytes, policy)
}

#[cfg(test)]
mod tests;
