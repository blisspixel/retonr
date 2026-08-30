use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

/// Domain for one generation-system record identity.
pub const GENERATION_SYSTEM_ID_DOMAIN: &[u8] = b"retonr:generation-system-record:v1\0";
/// Domain for one generation-cluster record identity.
pub const GENERATION_CLUSTER_ID_DOMAIN: &[u8] = b"retonr:generation-cluster:v1\0";
/// Domain for one generation-case manifest identity.
pub const GENERATION_CASE_ID_DOMAIN: &[u8] = b"retonr:generation-case-manifest:v1\0";
/// Domain for one generation-suite manifest identity.
pub const GENERATION_SUITE_MANIFEST_ID_DOMAIN: &[u8] = b"retonr:generation-suite-manifest:v1\0";
/// Domain for one generation-repetition record identity.
pub const GENERATION_REPETITION_ID_DOMAIN: &[u8] = b"retonr:generation-repetition:v1\0";
/// Domain for one generation-qualification plan identity.
pub const GENERATION_QUALIFICATION_PLAN_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-plan:v1\0";
/// Domain for one planned candidate-attempt identity.
pub const PLANNED_CANDIDATE_ATTEMPT_ID_DOMAIN: &[u8] = b"retonr:planned-candidate-attempt:v1\0";
/// Domain for one candidate-attempt precursor identity.
pub const CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-attempt-precursor:v1\0";
/// Domain for one managed Ollama candidate-generation evidence identity.
pub const MANAGED_OLLAMA_CANDIDATE_EVIDENCE_ID_DOMAIN: &[u8] =
    b"retonr:managed-ollama-candidate-generation-evidence:v2\0";
/// Domain for one candidate-generation cleanup identity.
pub const CANDIDATE_GENERATION_CLEANUP_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-cleanup:v1\0";
/// Domain for one candidate-generation evidence-bundle identity.
pub const CANDIDATE_GENERATION_EVIDENCE_BUNDLE_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-evidence-bundle:v1\0";
/// Domain for one candidate-generation evidence readback identity.
pub const CANDIDATE_GENERATION_EVIDENCE_READBACK_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-evidence-readback:v1\0";
/// Domain for one exact retained candidate identity.
pub const CANDIDATE_EVIDENCE_ID_DOMAIN: &[u8] = b"retonr:candidate-evidence:v1\0";
/// Domain for one candidate-generation receipt identity.
pub const CANDIDATE_GENERATION_RECEIPT_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-receipt:v1\0";
/// Domain for one completed-or-failed candidate attempt record identity.
pub const CANDIDATE_GENERATION_ATTEMPT_RECORD_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-attempt-record:v1\0";
/// Domain for one candidate-selection policy identity.
pub const CANDIDATE_SELECTION_POLICY_ID_DOMAIN: &[u8] = b"retonr:candidate-selection-policy:v1\0";
/// Domain for one candidate-generation receipt-set identity.
pub const CANDIDATE_GENERATION_RECEIPT_SET_ID_DOMAIN: &[u8] =
    b"retonr:candidate-generation-receipt-set:v1\0";
/// Domain for one ordered pair of complete candidate receipt sets.
pub const CANDIDATE_RECEIPT_PAIR_SET_ID_DOMAIN: &[u8] = b"retonr:candidate-receipt-pair-set:v1\0";
/// Domain for one deterministic candidate-evaluation record identity.
pub const CANDIDATE_DETERMINISTIC_EVALUATION_ID_DOMAIN: &[u8] =
    b"retonr:candidate-deterministic-evaluation:v1\0";
/// Domain for one pre-output candidate-judge plan.
pub const CANDIDATE_JUDGE_PLAN_ID_DOMAIN: &[u8] = b"retonr:candidate-judge-plan:v1\0";
/// Domain for one exact two-order candidate-judge schedule.
pub const CANDIDATE_JUDGE_SCHEDULE_ID_DOMAIN: &[u8] = b"retonr:candidate-judge-schedule:v1\0";
/// Domain for one exact managed local-judge receipt record.
pub const MANAGED_LOCAL_JUDGE_RECEIPT_ID_DOMAIN: &[u8] = b"retonr:managed-local-judge-receipt:v1\0";
/// Domain for one ordered aggregate of exact judge requests.
pub const CANDIDATE_JUDGE_REQUEST_AGGREGATE_ID_DOMAIN: &[u8] =
    b"retonr:candidate-judge-request-aggregate:v1\0";
/// Domain for one ordered aggregate of exact judge responses.
pub const CANDIDATE_JUDGE_RESPONSE_AGGREGATE_ID_DOMAIN: &[u8] =
    b"retonr:candidate-judge-response-aggregate:v1\0";
/// Domain for one schedule-indexed exact judge response.
pub const CANDIDATE_JUDGE_RESPONSE_ID_DOMAIN: &[u8] = b"retonr:candidate-judge-response:v1\0";
/// Domain for one normalized schedule-bound judge observation.
pub const CANDIDATE_JUDGE_OBSERVATION_ID_DOMAIN: &[u8] = b"retonr:candidate-judge-observation:v1\0";
/// Domain for one ordered batch of normalized judge observations.
pub const CANDIDATE_JUDGE_OBSERVATION_BATCH_ID_DOMAIN: &[u8] =
    b"retonr:candidate-judge-observation-batch:v1\0";
/// Domain for one exact candidate-to-judge relationship join.
pub const CANDIDATE_JUDGE_JOIN_ID_DOMAIN: &[u8] = b"retonr:candidate-judge-join:v1\0";

macro_rules! typed_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(pub(super) Digest);

        impl $name {
            /// Returns the digest that defines this portable identity.
            #[must_use]
            pub const fn digest(&self) -> &Digest {
                &self.0
            }
        }
    };
}

macro_rules! owner_derived_id {
    ($name:ident, $doc:literal) => {
        typed_id!($name, $doc);

        impl $name {
            /// Wraps a digest derived by the named owning boundary.
            ///
            /// The returned value is inert. It does not prove that the owner
            /// observed the relationship and grants no authority.
            #[must_use]
            pub const fn from_derived_digest(digest: Digest) -> Self {
                Self(digest)
            }
        }
    };
}

typed_id!(
    GenerationSystemId,
    "Content identity of one generation-system record."
);
typed_id!(
    GenerationClusterId,
    "Content identity of one generation-cluster record."
);
typed_id!(
    GenerationCaseId,
    "Content identity of one generation-case manifest."
);
typed_id!(
    GenerationSuiteManifestId,
    "Content identity of one generation-suite manifest."
);
typed_id!(
    GenerationRepetitionId,
    "Content identity of one generation-repetition record."
);
typed_id!(
    GenerationQualificationPlanId,
    "Content identity of one generation-qualification plan."
);
typed_id!(
    PlannedCandidateAttemptId,
    "Content identity of one planned candidate attempt."
);
typed_id!(
    CandidateGenerationAttemptPrecursorId,
    "Content identity of one candidate-attempt precursor."
);
typed_id!(
    ManagedOllamaCandidateGenerationEvidenceV2Id,
    "Content identity of managed Ollama candidate evidence V2."
);
typed_id!(
    CandidateGenerationCleanupId,
    "Content identity of one candidate-attempt cleanup record."
);
typed_id!(
    CandidateGenerationEvidenceBundleId,
    "Content identity of one candidate evidence-bundle manifest."
);
typed_id!(
    CandidateGenerationEvidenceBundleReadbackId,
    "Content identity of one candidate evidence-bundle readback."
);
typed_id!(
    CandidateEvidenceId,
    "Content identity of one exact retained candidate."
);
typed_id!(
    CandidateGenerationReceiptId,
    "Content identity of one candidate-generation receipt."
);
typed_id!(
    CandidateGenerationAttemptRecordId,
    "Content identity of one completed-or-failed candidate attempt."
);
typed_id!(
    CandidateSelectionPolicyId,
    "Content identity of one executable candidate-selection policy."
);
typed_id!(
    CandidateGenerationReceiptSetId,
    "Content identity of one semantic-order candidate receipt set."
);
typed_id!(
    CandidateReceiptPairSetId,
    "Content identity of one ordered pair of complete candidate receipt sets."
);
typed_id!(
    CandidateDeterministicEvaluationId,
    "Content identity of one deterministic candidate-evaluation record."
);
typed_id!(
    CandidateJudgePlanId,
    "Content identity of one pre-output candidate-judge plan."
);
typed_id!(
    CandidateJudgeScheduleId,
    "Content identity of one exact two-order candidate-judge schedule."
);
typed_id!(
    ManagedLocalJudgeReceiptId,
    "Content identity of one exact managed local-judge receipt record."
);
typed_id!(
    CandidateJudgeRequestAggregateId,
    "Content identity of ordered exact candidate-judge requests."
);
typed_id!(
    CandidateJudgeResponseAggregateId,
    "Content identity of ordered exact candidate-judge responses."
);
typed_id!(
    CandidateJudgeResponseId,
    "Content identity of one schedule-indexed exact candidate-judge response."
);
typed_id!(
    CandidateJudgeObservationId,
    "Content identity of one normalized schedule-bound judge observation."
);
typed_id!(
    CandidateJudgeObservationBatchId,
    "Content identity of ordered normalized candidate-judge observations."
);
typed_id!(
    CandidateJudgeJoinId,
    "Content identity of one exact candidate-to-judge relationship join."
);

owner_derived_id!(
    RuntimeAdmissionJoinId,
    "Inert typed form of an app-derived runtime-admission join digest."
);
owner_derived_id!(
    ManagedGenerationPathId,
    "Inert typed form of an app-derived managed generation-path digest."
);
owner_derived_id!(
    FrozenExternalComponentSetId,
    "Inert typed form of an attestor-derived frozen component-set digest."
);
owner_derived_id!(
    ManagedOllamaEffectiveRuntimeStateJoinId,
    "Inert typed form of the app-owned managed Ollama live-state join digest."
);
owner_derived_id!(
    GenerationRequestBindingId,
    "Inert typed form of a provider-neutral generation-request binding."
);
owner_derived_id!(
    StructuredCompletionRequestBindingId,
    "Inert typed form of a structured completion-request binding."
);
owner_derived_id!(
    OllamaRetainedSessionResponseId,
    "Inert typed form of an Ollama retained-session response binding."
);
owner_derived_id!(
    ManagedOllamaGenerationBracketObservationV1Id,
    "Inert typed form of a managed Ollama bracket V1 binding."
);
