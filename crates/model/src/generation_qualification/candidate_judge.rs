//! Portable inert candidate-to-judge records.
//!
//! These records bind exact bounded relationships. They do not prove that a
//! judge ran, that its output is correct, or that any candidate is qualified.

use rewrite_types::Digest;

use super::GenerationQualificationContractError;

mod aggregate;
mod join;
mod managed_receipt;
mod observation;
mod plan;
mod response;
mod schedule;

pub use aggregate::{CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1};
pub use join::{
    CandidateJudgeEvidenceClassV1, CandidateJudgeJoinRecordV1, CandidateJudgeJoinRecordV1Relations,
};
pub use managed_receipt::{
    ManagedLocalJudgeEvidenceClassV1, ManagedLocalJudgeReceiptRecordV1,
    ManagedLocalJudgeReceiptRecordV1Input, ManagedLocalJudgeReceiptRecordV1Relations,
    ManagedLocalJudgeReceiptSuccessStatusV1,
};
pub use observation::{
    CandidateJudgeChoiceV1, CandidateJudgeObservationBatchV1, CandidateJudgeObservationV1,
};
pub use plan::{
    CandidateJudgeCaseV1, CandidateJudgeLimitsV1, CandidateJudgeOrderPolicyV1,
    CandidateJudgePlanV1, CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations,
};
pub use response::CandidateJudgeResponseV1;
pub use schedule::{
    CandidateJudgePresentationV1, CandidateJudgeScheduleEntryV1, CandidateJudgeScheduleV1,
};

/// Current portable candidate-judge schema version.
pub const CANDIDATE_JUDGE_SCHEMA_VERSION: u32 = 1;
/// Maximum JSON bytes accepted for one candidate-judge plan.
pub const MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES: usize = 4 * 1_024 * 1_024;
/// Maximum JSON bytes accepted for one exact candidate-judge schedule.
pub const MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES: usize = 256 * 1_024;
/// Maximum JSON bytes accepted for either request or response aggregate.
pub const MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES: usize = 256 * 1_024;
/// Maximum JSON bytes accepted for either request or response aggregate.
pub const MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES: usize = 256 * 1_024;
/// Maximum JSON bytes accepted for one normalized observation batch.
pub const MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES: usize = 2 * 1_024 * 1_024;
/// Maximum JSON bytes accepted for one managed local-judge receipt record.
pub const MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES: usize = 256 * 1_024;
/// Maximum JSON bytes accepted for one candidate-judge join record.
pub const MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES: usize = 256 * 1_024;
/// Maximum canonical compatibility report bytes accepted for inert triage framing.
pub const MAX_CANDIDATE_JUDGE_TRIAGE_REPORT_JSON_BYTES: usize = 4 * 1_024 * 1_024;

/// Domain for deterministic presentation-order selection.
pub const CANDIDATE_JUDGE_PRESENTATION_ORDER_DOMAIN: &[u8] =
    b"retonr:candidate-judge-presentation-order:v1\0";
/// Domain for one deterministic candidate-judge attempt seed.
pub const CANDIDATE_JUDGE_ATTEMPT_SEED_DOMAIN: &[u8] = b"retonr:candidate-judge-attempt-seed:v1\0";
/// Domain for one inert compatibility-shaped candidate-judge triage report.
pub const CANDIDATE_JUDGE_TRIAGE_REPORT_DOMAIN: &[u8] =
    b"retonr:candidate-judge-triage-report:v1\0";
/// Domain for ordered residency-receipt bindings in one managed judge bracket.
pub const MANAGED_LOCAL_JUDGE_RESIDENCY_AGGREGATE_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-residency-aggregate:v1\0";
/// Domain for ordered process-observation bindings in one managed judge bracket.
pub const MANAGED_LOCAL_JUDGE_PROCESS_OBSERVATION_AGGREGATE_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-process-observation-aggregate:v1\0";
/// Domain for ordered native-load bindings in one managed judge bracket.
pub const MANAGED_LOCAL_JUDGE_NATIVE_LOAD_OBSERVATION_AGGREGATE_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-native-load-observation-aggregate:v1\0";
/// Domain for ordered connection bindings in one managed judge bracket.
pub const MANAGED_LOCAL_JUDGE_CONNECTION_OBSERVATION_AGGREGATE_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-connection-observation-aggregate:v1\0";
/// Domain for ordered effective-state bindings in one managed judge bracket.
pub const MANAGED_LOCAL_JUDGE_EFFECTIVE_STATE_OBSERVATION_AGGREGATE_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-effective-state-observation-aggregate:v1\0";
/// Domain for the app-owned complete managed judge effective-state join.
pub const MANAGED_LOCAL_JUDGE_EFFECTIVE_RUNTIME_STATE_JOIN_DOMAIN: &[u8] =
    b"retonr:managed-local-judge-effective-runtime-state-join:v1\0";

/// Inert framing relationship for canonical compatibility triage-report bytes.
///
/// This type checks only nonempty bounded framing. The eval-owned compiler must
/// validate the exact `HybridScorecardReport` structure and canonical encoding
/// before constructing an authoritative judge join.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateJudgeTriageReportRelationshipV1 {
    digest: Digest,
}

impl CandidateJudgeTriageReportRelationshipV1 {
    /// Frames one bounded canonical report without claiming semantic validation.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for empty or excessive
    /// bytes, or if the byte count cannot be represented in the fixed framing.
    pub fn new(bytes: &[u8]) -> Result<Self, GenerationQualificationContractError> {
        if bytes.is_empty() || bytes.len() > MAX_CANDIDATE_JUDGE_TRIAGE_REPORT_JSON_BYTES {
            return Err(GenerationQualificationContractError::InvalidCandidateJudgeTriageReport);
        }
        let length = u64::try_from(bytes.len())
            .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?;
        let mut canonical = Vec::with_capacity(
            CANDIDATE_JUDGE_TRIAGE_REPORT_DOMAIN
                .len()
                .checked_add(8)
                .and_then(|value| value.checked_add(bytes.len()))
                .ok_or(GenerationQualificationContractError::EncodingOverflow)?,
        );
        canonical.extend_from_slice(CANDIDATE_JUDGE_TRIAGE_REPORT_DOMAIN);
        canonical.extend_from_slice(&length.to_be_bytes());
        canonical.extend_from_slice(bytes);
        Ok(Self {
            digest: Digest::sha256(&canonical),
        })
    }

    /// Returns the inert framed report digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }
}
