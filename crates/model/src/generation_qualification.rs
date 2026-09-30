//! Portable inert contracts for generation qualification planning.
//!
//! These records bind bounded identities and cross-record relationships. They do
//! not prove that an attempt ran and grant no qualification or live-use authority.

mod attempt_record;
mod candidate_judge;
mod case;
mod case_request_profile;
mod cleanup;
mod cluster;
mod codec;
mod deterministic_case;
mod deterministic_evaluation;
mod error;
mod evidence_bundle;
mod generation_system;
mod ids;
mod managed_evidence;
mod operation_contracts;
mod phase_evidence;
mod plan;
mod planned_attempt;
mod precursor;
mod qualification_invalidation;
mod qualification_record;
mod qualification_selection;
mod readback;
mod receipt;
mod receipt_set;
mod repetition;
mod selection_policy;
mod suite;

pub use attempt_record::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptDispositionV1,
    CandidateGenerationAttemptFailureCategoryV1, CandidateGenerationAttemptFailurePhaseV1,
    CandidateGenerationAttemptFailureV1Input, CandidateGenerationAttemptOutcomeV1,
    CandidateGenerationAttemptRecordV1, CandidateGenerationAttemptRecordV1Preflight,
    MAX_CANDIDATE_GENERATION_ATTEMPT_RECORD_JSON_BYTES,
};
pub use candidate_judge::{
    CANDIDATE_JUDGE_ATTEMPT_SEED_DOMAIN, CANDIDATE_JUDGE_PRESENTATION_ORDER_DOMAIN,
    CANDIDATE_JUDGE_SCHEMA_VERSION, CANDIDATE_JUDGE_TRIAGE_REPORT_DOMAIN, CandidateJudgeCaseV1,
    CandidateJudgeChoiceV1, CandidateJudgeEvidenceClassV1, CandidateJudgeJoinRecordV1,
    CandidateJudgeJoinRecordV1Relations, CandidateJudgeLimitsV1, CandidateJudgeObservationBatchV1,
    CandidateJudgeObservationV1, CandidateJudgeOrderPolicyV1, CandidateJudgePlanV1,
    CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations, CandidateJudgePresentationV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1, CandidateJudgeResponseV1,
    CandidateJudgeScheduleEntryV1, CandidateJudgeScheduleV1,
    CandidateJudgeTriageReportRelationshipV1,
    MANAGED_LOCAL_JUDGE_CONNECTION_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_EFFECTIVE_RUNTIME_STATE_JOIN_DOMAIN,
    MANAGED_LOCAL_JUDGE_EFFECTIVE_STATE_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_NATIVE_LOAD_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_PROCESS_OBSERVATION_AGGREGATE_DOMAIN,
    MANAGED_LOCAL_JUDGE_RESIDENCY_AGGREGATE_DOMAIN, MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES, MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES, MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_TRIAGE_REPORT_JSON_BYTES, MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES,
    ManagedLocalJudgeEvidenceClassV1, ManagedLocalJudgeReceiptRecordV1,
    ManagedLocalJudgeReceiptRecordV1Input, ManagedLocalJudgeReceiptRecordV1Relations,
    ManagedLocalJudgeReceiptSuccessStatusV1,
};
pub use case::{
    GenerationCaseManifestV1, GenerationCaseManifestV1Input, MAX_GENERATION_CASE_JSON_BYTES,
};
pub use case_request_profile::{
    GENERATION_CASE_FORMAT_DIGEST_DOMAIN, GENERATION_CASE_LANGUAGE_DIGEST_DOMAIN,
    GENERATION_CASE_REQUEST_PROFILE_SCHEMA_VERSION, GENERATION_CASE_REWRITE_MODE_DIGEST_DOMAIN,
    GenerationCaseAtomicityV1, GenerationCaseFormatV1, GenerationCaseLanguageV1,
    GenerationCaseProtectionPolicyV1, GenerationCaseRequestProfileError,
    GenerationCaseRequestProfileV1, GenerationCaseStylePolicyV1, GenerationCaseUnitPolicyV1,
    MAX_GENERATION_CASE_REQUEST_PROFILE_JSON_BYTES,
};
pub use cleanup::{
    CandidateGenerationCleanupFailureCategoryV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationCleanupRecordV1Input, CandidateGenerationPackageRevalidationStatusV1,
    CandidateGenerationProcessCleanupStatusV1, MAX_CANDIDATE_GENERATION_CLEANUP_JSON_BYTES,
};
pub use cluster::{GenerationClusterRecordV1, MAX_GENERATION_CLUSTER_JSON_BYTES};
pub use deterministic_case::{
    GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION,
    GenerationDeterministicCaseContractError, GenerationDeterministicCaseContractV1,
    GenerationDeterministicCaseContractV1Input,
    MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
    MAX_GENERATION_DETERMINISTIC_CASE_PROTECTED_TERMS,
    MAX_GENERATION_DETERMINISTIC_CASE_RUBRIC_CLAUSES,
};
pub use deterministic_evaluation::{
    CANDIDATE_DETERMINISTIC_POLICY_DOMAIN, CANDIDATE_DETERMINISTIC_REPORT_DIGEST_DOMAIN,
    CANDIDATE_DETERMINISTIC_REPORT_PAIR_DIGEST_DOMAIN,
    CANDIDATE_DETERMINISTIC_SUITE_PAIR_DIGEST_DOMAIN, CandidateDeterministicEvaluationRecordV1,
    CandidateDeterministicEvaluationRecordV1Input, CandidateDeterministicEvaluationStatusV1,
    CandidateDeterministicReportRelationshipV1, CandidateDeterministicReportSummaryV1,
    CandidateDeterministicTransformationCoverageV1,
    MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES,
    MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES, candidate_deterministic_policy_digest,
};
pub use error::GenerationQualificationContractError;
pub use evidence_bundle::{
    CANDIDATE_CONTENT_DIGEST_DOMAIN, CandidateArtifactEntryV1,
    CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Preflight,
    CandidateGenerationEvidenceBundleManifestV1Relations, CandidateGenerationEvidenceBundleRoleV1,
    EVIDENCE_BUNDLE_MANIFEST_RELATIVE_PATH, MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_JSON_BYTES,
    StructuredResponseArtifactV1Input,
};
pub use generation_system::{
    GenerationSystemRecordV1, GenerationSystemRecordV1Input, GenerationSystemRecordV1Relations,
    MAX_GENERATION_SYSTEM_JSON_BYTES,
};
pub use ids::{
    CANDIDATE_DETERMINISTIC_EVALUATION_ID_DOMAIN, CANDIDATE_EVIDENCE_ID_DOMAIN,
    CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_ID_DOMAIN,
    CANDIDATE_GENERATION_ATTEMPT_RECORD_ID_DOMAIN, CANDIDATE_GENERATION_CLEANUP_ID_DOMAIN,
    CANDIDATE_GENERATION_EVIDENCE_BUNDLE_ID_DOMAIN,
    CANDIDATE_GENERATION_EVIDENCE_READBACK_ID_DOMAIN, CANDIDATE_GENERATION_RECEIPT_ID_DOMAIN,
    CANDIDATE_GENERATION_RECEIPT_SET_ID_DOMAIN, CANDIDATE_JUDGE_JOIN_ID_DOMAIN,
    CANDIDATE_JUDGE_OBSERVATION_BATCH_ID_DOMAIN, CANDIDATE_JUDGE_OBSERVATION_ID_DOMAIN,
    CANDIDATE_JUDGE_PLAN_ID_DOMAIN, CANDIDATE_JUDGE_REQUEST_AGGREGATE_ID_DOMAIN,
    CANDIDATE_JUDGE_RESPONSE_AGGREGATE_ID_DOMAIN, CANDIDATE_JUDGE_RESPONSE_ID_DOMAIN,
    CANDIDATE_JUDGE_SCHEDULE_ID_DOMAIN, CANDIDATE_RECEIPT_PAIR_SET_ID_DOMAIN,
    CANDIDATE_SELECTION_POLICY_ID_DOMAIN, CandidateDeterministicEvaluationId, CandidateEvidenceId,
    CandidateGenerationAttemptPrecursorId, CandidateGenerationAttemptRecordId,
    CandidateGenerationCleanupId, CandidateGenerationEvidenceBundleId,
    CandidateGenerationEvidenceBundleReadbackId, CandidateGenerationReceiptId,
    CandidateGenerationReceiptSetId, CandidateJudgeJoinId, CandidateJudgeObservationBatchId,
    CandidateJudgeObservationId, CandidateJudgePlanId, CandidateJudgeRequestAggregateId,
    CandidateJudgeResponseAggregateId, CandidateJudgeResponseId, CandidateJudgeScheduleId,
    CandidateReceiptPairSetId, CandidateSelectionPolicyId, FrozenExternalComponentSetId,
    GENERATION_CASE_ID_DOMAIN, GENERATION_CLUSTER_ID_DOMAIN,
    GENERATION_QUALIFICATION_PLAN_ID_DOMAIN, GENERATION_REPETITION_ID_DOMAIN,
    GENERATION_SUITE_MANIFEST_ID_DOMAIN, GENERATION_SYSTEM_ID_DOMAIN, GenerationCaseId,
    GenerationClusterId, GenerationQualificationPlanId, GenerationRepetitionId,
    GenerationRequestBindingId, GenerationSuiteManifestId, GenerationSystemId,
    MANAGED_LOCAL_JUDGE_RECEIPT_ID_DOMAIN, MANAGED_OLLAMA_CANDIDATE_EVIDENCE_ID_DOMAIN,
    ManagedGenerationPathId, ManagedLocalJudgeReceiptId,
    ManagedOllamaCandidateGenerationEvidenceV2Id, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, OllamaRetainedSessionResponseId,
    PLANNED_CANDIDATE_ATTEMPT_ID_DOMAIN, PlannedCandidateAttemptId, RuntimeAdmissionJoinId,
    StructuredCompletionRequestBindingId,
};
pub use managed_evidence::{
    MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_SCHEMA_VERSION,
    MAX_MANAGED_OLLAMA_CANDIDATE_GENERATION_EVIDENCE_V2_JSON_BYTES,
    ManagedOllamaCandidateGenerationEvidenceClassV2, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations,
};
pub use operation_contracts::*;
pub use phase_evidence::{
    GENERATION_ATTEMPT_LEDGER_MANIFEST_ID_DOMAIN, GENERATION_ATTEMPT_LEDGER_ROOT_DOMAIN,
    GENERATION_HUMAN_ADJUDICATION_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_HUMAN_ADJUDICATION_EVIDENCE_ROOT_DOMAIN,
    GENERATION_HUMAN_ADJUDICATION_POLICY_DENIAL_RECORD_ID_DOMAIN,
    GENERATION_REPEATABILITY_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_REPEATABILITY_EVIDENCE_ROOT_DOMAIN, GENERATION_REPEATABILITY_RESULT_ID_DOMAIN,
    GENERATION_RESOURCE_ATTEMPT_RESULT_ID_DOMAIN, GENERATION_RESOURCE_EVIDENCE_MANIFEST_ID_DOMAIN,
    GENERATION_RESOURCE_EVIDENCE_ROOT_DOMAIN, GENERATION_RESOURCE_POLICY_DENIAL_RECORD_ID_DOMAIN,
    GenerationAttemptLedgerManifestId, GenerationAttemptLedgerManifestV1,
    GenerationAttemptLedgerManifestV1Relations, GenerationHumanAdjudicationEvidenceManifestId,
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationHumanAdjudicationPolicyDenialRecordId,
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations, GenerationPhasePolicyDenialReasonV1,
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationRepeatabilityEvidenceManifestId,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultId, GenerationRepeatabilityResultRecordV1,
    GenerationRepeatabilityResultRecordV1Relations, GenerationRepeatabilityTerminalStageV1,
    GenerationResourceAttemptResultId, GenerationResourceAttemptResultRecordV1,
    GenerationResourceAttemptResultRecordV1Input, GenerationResourceAttemptResultRecordV1Relations,
    GenerationResourceEvidenceManifestId, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations, GenerationResourceExceededLimitV1,
    GenerationResourceObservationProfileV1, GenerationResourcePolicyDenialRecordId,
    GenerationResourcePolicyDenialRecordV1, GenerationResourcePolicyDenialRecordV1Relations,
    MAX_GENERATION_PHASE_POLICY_DENIAL_RECORD_JSON_BYTES, MAX_GENERATION_QUALIFICATION_PHASE_ITEMS,
    MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
    MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
    MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_JSON_BYTES,
};
pub use plan::{
    GenerationQualificationPlanLimitsV1, GenerationQualificationPlanV1,
    GenerationQualificationPlanV1Input, GenerationQualificationRetryPolicyV1,
    MAX_GENERATION_CANDIDATES_PER_COMPLETION, MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES, MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
    MAX_GENERATION_PLAN_JSON_BYTES, MAX_GENERATION_RETAINED_INPUT_BYTES,
    MAX_GENERATION_SYSTEMS_PER_PLAN, MAX_PLANNED_GENERATION_ATTEMPTS,
};
pub use planned_attempt::{
    CandidateOutputCeilingsV1, MAX_PLANNED_CANDIDATE_ATTEMPT_JSON_BYTES, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
};
pub use precursor::{
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationAttemptPrecursorV1Input,
    MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES,
};
pub use qualification_invalidation::{
    GENERATION_QUALIFICATION_INVALIDATION_ID_DOMAIN, GenerationQualificationInvalidationId,
    GenerationQualificationInvalidationV1, GenerationQualificationInvalidationV1Error,
    GenerationQualificationInvalidationV1Relations,
    MAX_GENERATION_QUALIFICATION_INVALIDATION_CANONICAL_BYTES,
    MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES,
};
pub use qualification_record::{
    GENERATION_QUALIFICATION_RECORD_ID_DOMAIN, GenerationQualificationId,
    GenerationQualificationRecordV1, GenerationQualificationRecordV1Error,
    GenerationQualificationRecordV1Relations, GenerationQualificationStatusV1,
    MAX_GENERATION_QUALIFICATION_RECORD_CANONICAL_BYTES,
    MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES,
};
pub use qualification_selection::{
    GENERATION_QUALIFICATION_SELECTION_ID_DOMAIN, GenerationQualificationSelectionId,
    GenerationQualificationSelectionV1, GenerationQualificationSelectionV1Error,
    GenerationQualificationSelectionV1Relations,
    MAX_GENERATION_QUALIFICATION_SELECTION_CANONICAL_BYTES,
    MAX_GENERATION_QUALIFICATION_SELECTION_JSON_BYTES,
};
pub use readback::{
    CandidateGenerationEvidenceBundlePublicationModeV1,
    CandidateGenerationEvidenceBundleReadbackStatusV1, CandidateGenerationEvidenceBundleReadbackV1,
    MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES,
};
pub use receipt::{
    CandidateGenerationReceiptEvidenceClassV1, CandidateGenerationReceiptV1,
    CandidateGenerationReceiptV1Preflight, CandidateGenerationReceiptV1Relations,
    CandidateGenerationUsageObservationV1, MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES,
};
pub use receipt_set::{
    CandidateGenerationReceiptSetEntryV1, CandidateGenerationReceiptSetV1,
    CandidateGenerationReceiptSetV1Relations, MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES,
};
pub use repetition::{GenerationRepetitionRecordV1, MAX_GENERATION_REPETITION_JSON_BYTES};
pub use selection_policy::{
    CandidateSelectionPolicyEntryV1, CandidateSelectionPolicyV1,
    MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES,
};
pub use suite::{
    GenerationSuiteManifestV1, MAX_GENERATION_MACHINE_KEY_BYTES, MAX_GENERATION_SUITE_CASES,
    MAX_GENERATION_SUITE_JSON_BYTES,
};

pub(crate) const GENERATION_QUALIFICATION_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests;
