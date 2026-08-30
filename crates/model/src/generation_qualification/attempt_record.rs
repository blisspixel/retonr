use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{append_digest, append_u32, validate_canonical_json};
use super::{
    CANDIDATE_GENERATION_ATTEMPT_RECORD_ID_DOMAIN, CandidateGenerationAttemptPrecursorId,
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationAttemptRecordId,
    CandidateGenerationReceiptId, CandidateGenerationReceiptV1,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationContractError,
    PlannedCandidateAttemptId, PlannedCandidateAttemptV1,
};

/// Maximum JSON bytes accepted for one final candidate-attempt record.
pub const MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES: usize = 16_384;
const MAX_ATTEMPT_RECORD_CANONICAL_BYTES: usize = 1_024;

/// Safe disposition exposed by an opaque attempt-record encoding preflight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateGenerationAttemptDispositionV1 {
    /// The encoding names a completed attempt.
    Completed,
    /// The encoding names a failed attempt.
    Failed,
}

/// Closed primary failure phase in execution order.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationAttemptFailurePhaseV1 {
    /// Provider-neutral request compilation.
    RequestCompilation,
    /// Execution precursor compilation.
    PrecursorCompilation,
    /// Managed process launch.
    Launch,
    /// Revalidation before any generation traffic.
    PreTrafficRevalidation,
    /// Generation request traffic.
    GenerationTraffic,
    /// Structured response validation.
    ResponseValidation,
    /// Final retained bracket observation.
    FinalObservation,
    /// Managed evidence compilation.
    ManagedEvidenceCompilation,
    /// Process cleanup and final package revalidation.
    Cleanup,
    /// Create-new evidence-bundle publication.
    BundlePublication,
    /// Fresh evidence-bundle readback.
    BundleReadback,
    /// Final receipt compilation.
    ReceiptCompilation,
}

/// Closed primary attempt failure category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationAttemptFailureCategoryV1 {
    /// Cooperative cancellation was observed.
    Cancelled,
    /// The declared deadline elapsed.
    DeadlineExceeded,
    /// Request compilation or validation failed.
    RequestInvalid,
    /// Exact typed relationships differed.
    RelationshipMismatch,
    /// The selected managed path is unsupported.
    UnsupportedPlatform,
    /// Exact retained package bytes changed.
    PackageChanged,
    /// Exact retained package revalidation failed.
    PackageRevalidationFailed,
    /// Managed process launch failed.
    LaunchFailed,
    /// Generation transport failed.
    TransportFailed,
    /// Structured response validation failed.
    ResponseInvalid,
    /// A retained observation differed from the expected state.
    ObservationMismatch,
    /// Managed evidence compilation failed.
    ManagedEvidenceInvalid,
    /// Cleanup or final revalidation failed.
    CleanupFailed,
    /// Evidence-bundle publication failed.
    PublicationFailed,
    /// Evidence-bundle readback failed.
    ReadbackFailed,
    /// Final receipt compilation failed.
    ReceiptInvalid,
}

/// Closed cleanup disposition retained by a failed attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationAttemptCleanupDispositionV1 {
    /// No precursor existed, so managed cleanup was not required.
    NotRequired,
    /// Required managed cleanup completed successfully.
    Succeeded,
    /// Required cleanup or a final package revalidation failed.
    Failed,
}

/// Bounded diagnostic facts for one failed attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateGenerationAttemptFailureV1Input {
    /// Primary failure phase.
    pub failure_phase: CandidateGenerationAttemptFailurePhaseV1,
    /// Primary failure category.
    pub failure_category: CandidateGenerationAttemptFailureCategoryV1,
    /// Whether generation traffic was observed.
    pub traffic_observed: bool,
    /// Whether candidate output was observed.
    pub output_observed: bool,
    /// Final cleanup disposition.
    pub cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1,
}

/// Closed completed-or-failed attempt outcome.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationAttemptOutcomeV1 {
    /// Cleanup-and-readback-complete receipt closure.
    Completed {
        /// Exact planned attempt.
        planned_attempt_id: PlannedCandidateAttemptId,
        /// Exact execution precursor.
        precursor_id: CandidateGenerationAttemptPrecursorId,
        /// Exact final receipt.
        receipt_id: CandidateGenerationReceiptId,
    },
    /// Bounded diagnostic outcome with no response or candidate identities.
    Failed {
        /// Exact planned attempt.
        planned_attempt_id: PlannedCandidateAttemptId,
        /// Exact precursor when compilation reached that layer.
        precursor_id: Option<CandidateGenerationAttemptPrecursorId>,
        /// Primary failure phase.
        failure_phase: CandidateGenerationAttemptFailurePhaseV1,
        /// Primary failure category.
        failure_category: CandidateGenerationAttemptFailureCategoryV1,
        /// Whether generation traffic was observed.
        traffic_observed: bool,
        /// Whether candidate output was observed.
        output_observed: bool,
        /// Final cleanup disposition.
        cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AttemptRecordWire {
    schema_version: u32,
    outcome: CandidateGenerationAttemptOutcomeV1,
}

/// Opaque result of cheap bounded attempt syntax, schema, and canonicality checks.
///
/// The token does not trust any embedded identity or failure fact. It grants no
/// candidate, execution, or qualification authority and must be consumed by the
/// relationship-aware decoder.
///
/// ```compile_fail
/// use rewrite_model::CandidateGenerationAttemptRecordV1Preflight;
///
/// fn require_clone<T: Clone>() {}
///
/// fn clone_preflight() {
///     require_clone::<CandidateGenerationAttemptRecordV1Preflight>();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_model::CandidateGenerationAttemptRecordV1Preflight;
///
/// fn serialize_preflight(value: &CandidateGenerationAttemptRecordV1Preflight) {
///     let _bytes = serde_json::to_vec(value).expect("preflight must not serialize");
/// }
/// ```
pub struct CandidateGenerationAttemptRecordV1Preflight {
    wire: AttemptRecordWire,
    canonical_json: Box<[u8]>,
}

impl CandidateGenerationAttemptRecordV1Preflight {
    /// Returns the already-validated attempt-record schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.wire.schema_version
    }

    /// Returns only the closed top-level attempt disposition.
    #[must_use]
    pub const fn disposition(&self) -> CandidateGenerationAttemptDispositionV1 {
        match self.wire.outcome {
            CandidateGenerationAttemptOutcomeV1::Completed { .. } => {
                CandidateGenerationAttemptDispositionV1::Completed
            }
            CandidateGenerationAttemptOutcomeV1::Failed { .. } => {
                CandidateGenerationAttemptDispositionV1::Failed
            }
        }
    }
}

impl fmt::Debug for CandidateGenerationAttemptRecordV1Preflight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationAttemptRecordV1Preflight")
            .field("schema_version", &self.schema_version())
            .field("disposition", &self.disposition())
            .finish_non_exhaustive()
    }
}

/// Inert final portable record for one completed or failed candidate attempt.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationAttemptRecordV1 {
    schema_version: u32,
    outcome: CandidateGenerationAttemptOutcomeV1,
    #[serde(skip)]
    id: CandidateGenerationAttemptRecordId,
}

impl CandidateGenerationAttemptRecordV1 {
    /// Derives a completed record from the exact receipt closure.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if the receipt names a
    /// different planned attempt or precursor.
    pub fn completed(
        planned_attempt: &PlannedCandidateAttemptV1,
        precursor: &CandidateGenerationAttemptPrecursorV1,
        receipt: &CandidateGenerationReceiptV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if precursor.planned_attempt_id() != planned_attempt.planned_attempt_id()
            || receipt.planned_attempt_id() != planned_attempt.planned_attempt_id()
            || receipt.precursor_id() != precursor.precursor_id()
        {
            return Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch);
        }
        Self::build(CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id: planned_attempt.planned_attempt_id().clone(),
            precursor_id: precursor.precursor_id().clone(),
            receipt_id: receipt.receipt_id().clone(),
        })
    }

    /// Derives a failed diagnostic record with no later evidence identities.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless precursor presence,
    /// traffic, output, phase, category, and cleanup disposition form an exact closure.
    pub fn failed(
        planned_attempt: &PlannedCandidateAttemptV1,
        precursor: Option<&CandidateGenerationAttemptPrecursorV1>,
        input: CandidateGenerationAttemptFailureV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_failure(planned_attempt, precursor, input)?;
        Self::build(CandidateGenerationAttemptOutcomeV1::Failed {
            planned_attempt_id: planned_attempt.planned_attempt_id().clone(),
            precursor_id: precursor.map(|value| value.precursor_id().clone()),
            failure_phase: input.failure_phase,
            failure_category: input.failure_category,
            traffic_observed: input.traffic_observed,
            output_observed: input.output_observed,
            cleanup_disposition: input.cleanup_disposition,
        })
    }

    fn build(
        outcome: CandidateGenerationAttemptOutcomeV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        let mut value = Self {
            schema_version: GENERATION_QUALIFICATION_SCHEMA_VERSION,
            outcome,
            id: CandidateGenerationAttemptRecordId(Digest::sha256(b"uninitialized attempt record")),
        };
        let canonical = value.canonical_bytes();
        if canonical.len() > MAX_ATTEMPT_RECORD_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        value.id = CandidateGenerationAttemptRecordId(Digest::sha256(&canonical));
        Ok(value)
    }

    /// Parses canonical bounded JSON and reloads the exact applicable records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// future-schema, noncanonical, stale, or invalid completed or failure closure.
    pub fn from_json_bytes(
        bytes: &[u8],
        planned_attempt: &PlannedCandidateAttemptV1,
        precursor: Option<&CandidateGenerationAttemptPrecursorV1>,
        receipt: Option<&CandidateGenerationReceiptV1>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let preflight = Self::preflight_json_bytes(bytes)?;
        Self::from_preflight(preflight, planned_attempt, precursor, receipt)
    }

    /// Checks the complete attempt encoding before any external evidence access.
    ///
    /// The hard byte ceiling is enforced before JSON decoding. Success proves only
    /// that the bytes use the supported schema and its one canonical JSON encoding.
    /// Embedded identities and failure facts remain inert and untrusted.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unknown-field-bearing, duplicated, trailing, unsupported, or noncanonical
    /// JSON.
    pub fn preflight_json_bytes(
        bytes: &[u8],
    ) -> Result<CandidateGenerationAttemptRecordV1Preflight, GenerationQualificationContractError>
    {
        if bytes.len() > MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: AttemptRecordWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        validate_canonical_json(bytes, &wire)?;
        Ok(CandidateGenerationAttemptRecordV1Preflight {
            wire,
            canonical_json: bytes.into(),
        })
    }

    /// Consumes preflighted bytes and reloads the exact applicable relationships.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if the planned attempt,
    /// precursor, receipt, or failed-attempt closure does not derive the exact
    /// preflighted bytes.
    pub fn from_preflight(
        preflight: CandidateGenerationAttemptRecordV1Preflight,
        planned_attempt: &PlannedCandidateAttemptV1,
        precursor: Option<&CandidateGenerationAttemptPrecursorV1>,
        receipt: Option<&CandidateGenerationReceiptV1>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let CandidateGenerationAttemptRecordV1Preflight {
            wire,
            canonical_json,
        } = preflight;
        let expected = match wire.outcome {
            CandidateGenerationAttemptOutcomeV1::Completed { .. } => Self::completed(
                planned_attempt,
                precursor.ok_or(
                    GenerationQualificationContractError::AttemptRecordRelationshipMismatch,
                )?,
                receipt.ok_or(
                    GenerationQualificationContractError::AttemptRecordRelationshipMismatch,
                )?,
            )?,
            CandidateGenerationAttemptOutcomeV1::Failed {
                failure_phase,
                failure_category,
                traffic_observed,
                output_observed,
                cleanup_disposition,
                ..
            } => {
                if receipt.is_some() {
                    return Err(
                        GenerationQualificationContractError::AttemptRecordRelationshipMismatch,
                    );
                }
                Self::failed(
                    planned_attempt,
                    precursor,
                    CandidateGenerationAttemptFailureV1Input {
                        failure_phase,
                        failure_category,
                        traffic_observed,
                        output_observed,
                        cleanup_disposition,
                    },
                )?
            }
        };
        if serde_json::to_vec(&expected)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?
            != canonical_json.as_ref()
        {
            return Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch);
        }
        Ok(expected)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the closed inert outcome.
    #[must_use]
    pub const fn outcome(&self) -> &CandidateGenerationAttemptOutcomeV1 {
        &self.outcome
    }
    /// Returns the content-derived final attempt-record identity.
    #[must_use]
    pub const fn attempt_record_id(&self) -> &CandidateGenerationAttemptRecordId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = CANDIDATE_GENERATION_ATTEMPT_RECORD_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        match &self.outcome {
            CandidateGenerationAttemptOutcomeV1::Completed {
                planned_attempt_id,
                precursor_id,
                receipt_id,
            } => {
                output.push(0);
                append_digest(&mut output, planned_attempt_id.digest());
                append_digest(&mut output, precursor_id.digest());
                append_digest(&mut output, receipt_id.digest());
            }
            CandidateGenerationAttemptOutcomeV1::Failed {
                planned_attempt_id,
                precursor_id,
                failure_phase,
                failure_category,
                traffic_observed,
                output_observed,
                cleanup_disposition,
            } => {
                output.push(1);
                append_digest(&mut output, planned_attempt_id.digest());
                match precursor_id {
                    Some(value) => {
                        output.push(1);
                        append_digest(&mut output, value.digest());
                    }
                    None => output.push(0),
                }
                output.push(phase_tag(*failure_phase));
                output.push(category_tag(*failure_category));
                output.extend([
                    u8::from(*traffic_observed),
                    u8::from(*output_observed),
                    cleanup_tag(*cleanup_disposition),
                ]);
            }
        }
        output
    }
}

fn validate_failure(
    planned: &PlannedCandidateAttemptV1,
    precursor: Option<&CandidateGenerationAttemptPrecursorV1>,
    input: CandidateGenerationAttemptFailureV1Input,
) -> Result<(), GenerationQualificationContractError> {
    if precursor.is_some_and(|value| value.planned_attempt_id() != planned.planned_attempt_id()) {
        return Err(GenerationQualificationContractError::AttemptRecordRelationshipMismatch);
    }
    let pre_precursor = phase_tag(input.failure_phase)
        <= phase_tag(CandidateGenerationAttemptFailurePhaseV1::PrecursorCompilation);
    if precursor.is_some() == pre_precursor
        || (phase_tag(input.failure_phase)
            < phase_tag(CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic)
            && input.traffic_observed)
        || (input.output_observed && !input.traffic_observed)
        || ((input.cleanup_disposition
            == CandidateGenerationAttemptCleanupDispositionV1::NotRequired)
            != pre_precursor)
        || (input.failure_phase == CandidateGenerationAttemptFailurePhaseV1::Cleanup
            && (input.failure_category
                != CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
                || input.cleanup_disposition
                    != CandidateGenerationAttemptCleanupDispositionV1::Failed))
        || (input.failure_category == CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
            && input.cleanup_disposition != CandidateGenerationAttemptCleanupDispositionV1::Failed)
    {
        return Err(GenerationQualificationContractError::InvalidAttemptFailure);
    }
    Ok(())
}

const fn phase_tag(value: CandidateGenerationAttemptFailurePhaseV1) -> u8 {
    match value {
        CandidateGenerationAttemptFailurePhaseV1::RequestCompilation => 0,
        CandidateGenerationAttemptFailurePhaseV1::PrecursorCompilation => 1,
        CandidateGenerationAttemptFailurePhaseV1::Launch => 2,
        CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation => 3,
        CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic => 4,
        CandidateGenerationAttemptFailurePhaseV1::ResponseValidation => 5,
        CandidateGenerationAttemptFailurePhaseV1::FinalObservation => 6,
        CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation => 7,
        CandidateGenerationAttemptFailurePhaseV1::Cleanup => 8,
        CandidateGenerationAttemptFailurePhaseV1::BundlePublication => 9,
        CandidateGenerationAttemptFailurePhaseV1::BundleReadback => 10,
        CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation => 11,
    }
}
const fn category_tag(value: CandidateGenerationAttemptFailureCategoryV1) -> u8 {
    match value {
        CandidateGenerationAttemptFailureCategoryV1::Cancelled => 0,
        CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded => 1,
        CandidateGenerationAttemptFailureCategoryV1::RequestInvalid => 2,
        CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch => 3,
        CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform => 4,
        CandidateGenerationAttemptFailureCategoryV1::PackageChanged => 5,
        CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed => 6,
        CandidateGenerationAttemptFailureCategoryV1::LaunchFailed => 7,
        CandidateGenerationAttemptFailureCategoryV1::TransportFailed => 8,
        CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid => 9,
        CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch => 10,
        CandidateGenerationAttemptFailureCategoryV1::ManagedEvidenceInvalid => 11,
        CandidateGenerationAttemptFailureCategoryV1::CleanupFailed => 12,
        CandidateGenerationAttemptFailureCategoryV1::PublicationFailed => 13,
        CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed => 14,
        CandidateGenerationAttemptFailureCategoryV1::ReceiptInvalid => 15,
    }
}
const fn cleanup_tag(value: CandidateGenerationAttemptCleanupDispositionV1) -> u8 {
    match value {
        CandidateGenerationAttemptCleanupDispositionV1::NotRequired => 0,
        CandidateGenerationAttemptCleanupDispositionV1::Succeeded => 1,
        CandidateGenerationAttemptCleanupDispositionV1::Failed => 2,
    }
}

impl fmt::Debug for CandidateGenerationAttemptRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let outcome = match self.outcome {
            CandidateGenerationAttemptOutcomeV1::Completed { .. } => "completed",
            CandidateGenerationAttemptOutcomeV1::Failed { .. } => "failed",
        };
        formatter
            .debug_struct("CandidateGenerationAttemptRecordV1")
            .field("attempt_record_id", &self.id)
            .field("outcome", &outcome)
            .finish_non_exhaustive()
    }
}
