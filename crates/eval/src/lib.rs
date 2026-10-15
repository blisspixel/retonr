//! Deterministic evaluation fixtures and reporting for fidelity gates.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub use rewrite_model::{EvaluationCase, ExpectedOutput, ReferenceJudgment};

mod active_generation_qualification_subject;
mod baseline;
mod candidate_deterministic_compiler;
mod candidate_judge_preparation;
mod claim_shadow_calibration;
mod editorial_corpus;
mod generation_case_material;
mod generation_qualification_preregistration;
mod hybrid_scorecard;
pub mod lint;
mod local_judge_execution;
mod local_ollama_attested_preflight;
mod local_ollama_bound_preflight;
mod local_ollama_managed_preflight;
mod local_ollama_model_binding;
mod local_ollama_preflight;
mod suite;
mod verified_candidate_batch;
mod verified_candidate_batch_set;
mod verified_generation_qualification;
mod watermark_research;
mod writing_sample_library;

pub use baseline::{
    BASELINE_SCHEMA_VERSION, BaselineCaseError, BaselineCaseResult, BaselineDefinition,
    BaselineError, BaselineInferencePolicy, BaselineKind, BaselineReport, BaselineStatusCounts,
    MAX_BASELINE_DEFINITION_BYTES, parse_baseline_definition, run_attached_baseline, run_baseline,
    run_offline_baseline,
};
pub use candidate_deterministic_compiler::{
    CandidateDeterministicCompilerError, CandidateDeterministicCompilerRelationship,
    CandidateDeterministicCompilerSide, MAX_CANDIDATE_DETERMINISTIC_COMPILER_CASES,
    MAX_CANDIDATE_DETERMINISTIC_PROJECTED_SUITE_BYTES, compile_candidate_deterministic_evaluation,
};
pub use candidate_judge_preparation::{
    CandidateJudgePreparationError, CandidateJudgePreparationInput,
    CandidateJudgePreparationOutcome, CandidateJudgePreparationRelationship,
    CandidateJudgePreparationRequestFailure, CandidateJudgePreparationSide,
    CandidateJudgeRunnerHandoff, CandidateJudgeRunnerPairingError,
    CandidateJudgeRunnerPairingRelationship, CandidateJudgeRunnerPairingValidationPhase,
    PairedCandidateJudgeRunnerHandoff, PreparedCandidateJudgeRun,
    pair_candidate_judge_runner_handoffs, prepare_candidate_judge,
};
pub use claim_shadow_calibration::{
    CLAIM_SHADOW_CALIBRATION_SCHEMA_VERSION, ClaimShadowCalibrationCorpus,
    ClaimShadowCalibrationError, ClaimShadowCalibrationFailure, ClaimShadowCalibrationReport,
    ExpectedShadowOutcome, MAX_CLAIM_SHADOW_CALIBRATION_BYTES, MAX_CLAIM_SHADOW_CALIBRATION_CASES,
    parse_claim_shadow_calibration, run_claim_shadow_calibration,
};
pub use editorial_corpus::{
    EDITORIAL_CORPUS_SCHEMA_VERSION, EditorialCase, EditorialCaseKind, EditorialCorpus,
    EditorialCorpusError, EditorialCorpusOrigin, EditorialCorpusSummary,
    EditorialFindingExpectation, MAX_EDITORIAL_CASES, MAX_EDITORIAL_CORPUS_BYTES,
    parse_editorial_corpus,
};
pub use generation_case_material::{
    GENERATION_CASE_MATERIAL_SET_DIGEST_DOMAIN,
    GENERATION_DETERMINISTIC_CASE_CONTRACT_SCHEMA_VERSION, GenerationCaseMaterialLimits,
    GenerationDeterministicCaseContractError, GenerationDeterministicCaseContractV1,
    GenerationDeterministicCaseContractV1Input, MAX_GENERATION_CASE_MATERIAL_CASES,
    MAX_GENERATION_CASE_MATERIAL_IDENTITY_BYTES, MAX_GENERATION_CASE_MATERIAL_TOTAL_SOURCE_BYTES,
    MAX_GENERATION_DETERMINISTIC_CASE_CONTRACT_JSON_BYTES,
    MAX_GENERATION_DETERMINISTIC_CASE_PROTECTED_TERMS,
    MAX_GENERATION_DETERMINISTIC_CASE_RUBRIC_CLAUSES, VerifiedGenerationCaseMaterial,
    VerifiedGenerationCaseMaterialError, VerifiedGenerationCaseMaterialRelationship,
    VerifiedGenerationCaseMaterialTraversalError,
};
pub use generation_qualification_preregistration::{
    ActiveGenerationHumanAdjudicationOutcome, ActiveGenerationQualificationAttemptLedgerClosure,
    ActiveGenerationQualificationAttemptLedgerError,
    ActiveGenerationQualificationAttemptLedgerErrorKind,
    ActiveGenerationQualificationCandidateCloseoutError,
    ActiveGenerationQualificationCandidateCloseoutErrorKind,
    ActiveGenerationQualificationCandidateCloseoutInput,
    ActiveGenerationQualificationCandidateRunError,
    ActiveGenerationQualificationCandidateRunErrorKind,
    ActiveGenerationQualificationCandidateRunInput,
    ActiveGenerationQualificationCandidateRunOutcome,
    ActiveGenerationQualificationCandidateSettlementError,
    ActiveGenerationQualificationCandidateSettlementErrorKind,
    ActiveGenerationQualificationCloseoutError,
    ActiveGenerationQualificationDeterministicSettlementError,
    ActiveGenerationQualificationJudgeRunError, ActiveGenerationQualificationJudgeRunErrorKind,
    ActiveGenerationQualificationJudgeRunInput, ActiveGenerationQualificationJudgeSettlementError,
    ActiveGenerationQualificationOperation, ActiveGenerationQualificationOperationInterruption,
    ActiveGenerationQualificationOperationInterruptionError,
    ActiveGenerationQualificationOperationInterruptionErrorKind,
    ActiveGenerationQualificationReceiptSetError,
    ActiveGenerationQualificationRepeatabilitySettlementError,
    ActiveGenerationQualificationResourceSettlementError,
    FinalizedPhasePolicyRefusedPretrafficGenerationQualification,
    FinalizedRejectedPretrafficGenerationQualification, GenerationQualificationActivationError,
    GenerationQualificationOperationDraft, GenerationQualificationPhasePolicyRefusalError,
    GenerationQualificationPreparationDisposition, GenerationQualificationPreparationError,
    GenerationQualificationPreregistrationOpenError,
    GenerationQualificationPreregistrationRepository,
    GenerationQualificationPretrafficTerminalizationError,
    PreparedGenerationQualificationOperation, ProjectedGenerationQualificationOperation,
};
pub use hybrid_scorecard::{
    HYBRID_SCORECARD_SCHEMA_VERSION, HybridScorecardCasePlan, HybridScorecardError,
    HybridScorecardPlan, HybridScorecardReport, JudgeAuthority, JudgeCaseOutcome, JudgeCaseResult,
    JudgeChoice, JudgeExecution, JudgeObservation, JudgeObservationBatch, JudgeOrderPolicy,
    JudgePresentation, JudgeTriageSummary, LocalJudgePolicy, MAX_HYBRID_SCORECARD_BYTES,
    ReleaseReviewDisposition, hybrid_scorecard_deterministic_policy_digest,
    hybrid_scorecard_plan_digest, hybrid_scorecard_suite_pair_digest, parse_hybrid_scorecard_plan,
    parse_judge_observation_batch, run_hybrid_scorecard,
};
pub use lint::{
    EditorialFinding, EditorialLintEvaluationReport, EditorialLintFailure, RuleCatalog,
    evaluate_corpus_against_linter, lint_text,
};
pub use local_judge_execution::{
    LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION, LocalJudgeExecution, LocalJudgeExecutionError,
    LocalJudgeExecutionEvidenceClass, LocalJudgeExecutionOutcome, LocalJudgeExecutionReceipt,
    LocalJudgeRubric, LocalJudgeRubricClause, LocalJudgeRubricError, MAX_LOCAL_JUDGE_RUBRIC_BYTES,
    MAX_LOCAL_JUDGE_RUBRIC_CLAUSES, local_judge_prompt_contract_digest, local_judge_rubric_digest,
    parse_local_judge_rubric, run_local_judge_execution,
};
pub use local_ollama_attested_preflight::{
    LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_SCHEMA_VERSION,
    LOCAL_OLLAMA_ATTESTED_PREFLIGHT_REPORT_SCHEMA_VERSION, LocalOllamaAttestedPreflightError,
    LocalOllamaAttestedPreflightPlan, LocalOllamaAttestedPreflightReport,
    LocalOllamaProcessEvidenceLevel, MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES,
    parse_local_ollama_attested_preflight_plan, run_local_ollama_attested_preflight,
};
pub use local_ollama_bound_preflight::{
    LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION,
    LOCAL_OLLAMA_BOUND_PREFLIGHT_REPORT_SCHEMA_VERSION, LocalOllamaBoundPreflightError,
    LocalOllamaBoundPreflightPlan, LocalOllamaBoundPreflightReport,
    LocalOllamaBoundProcessEvidenceLevel, MAX_LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_BYTES,
    parse_local_ollama_bound_preflight_plan, run_local_ollama_bound_preflight,
};
pub use local_ollama_managed_preflight::{
    CompletePassedRepeatabilityRelations, FailedManagedCandidateAttempt,
    GenerationQualificationResourcePhaseAuthorityError,
    GenerationQualificationResourcePhaseCompilationError,
    GenerationQualificationResourcePhaseCompiler,
    GenerationQualificationResourcePhaseDerivationError,
    LOCAL_OLLAMA_MANAGED_BUILD_BINDING_SCHEMA_VERSION,
    LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION,
    LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION,
    LocalOllamaEffectiveStateMissingRelationship, LocalOllamaManagedBuildBinding,
    LocalOllamaManagedBuildEvidenceClass, LocalOllamaManagedGenerationError,
    LocalOllamaManagedGenerationEvidence, LocalOllamaManagedGenerationOutcome,
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightLimits,
    LocalOllamaManagedPreflightOutcome, LocalOllamaManagedPreflightReport,
    LocalOllamaManagedProcessEvidenceLevel,
    MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION,
    ManagedCandidateAttemptCleanupFailures, ManagedCandidateAttemptExecutionError,
    ManagedCandidateAttemptExecutionOutcome, ManagedCandidateAttemptFailureRecordError,
    ManagedCandidateAttemptPrimaryFailure, ManagedCandidateAttemptRunInput,
    ManagedCandidateJudgeRunError, ManagedCandidateJudgeRunErrorKind,
    ManagedOllamaGenerationBracketObservationV1, VerifiedCandidateJudgeJoin,
    VerifiedCandidateJudgeJoinRevalidationError, VerifiedCandidateJudgeJoinRevalidationErrorKind,
    VerifiedCompletePassedRepeatabilityJoins, VerifiedCompletePassedRepeatabilityJoinsError,
    VerifiedCompletedManagedCandidateAttempt, VerifiedGenerationQualificationResourcePhase,
    VerifiedPassedRepeatabilityJoins, VerifiedPassedRepeatabilityJoinsError,
    VerifiedPassedRepeatabilityJoinsErrorKind, run_local_ollama_managed_generation,
    run_local_ollama_managed_preflight, run_local_ollama_managed_preflight_with_build_binding,
    verify_complete_passed_repeatability_joins, verify_passed_repeatability_joins,
};
pub use local_ollama_model_binding::{
    LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, LOCAL_OLLAMA_MODEL_BINDING_SCHEMA_VERSION,
    LocalOllamaArtifactSetDisposition, LocalOllamaModelBindingError,
    LocalOllamaModelBindingEvidence, LocalOllamaObservedTemplateMatch,
    LocalOllamaPackageManifestDisposition, bind_imported_ollama_model_to_preflight,
};
pub use local_ollama_preflight::{
    LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION, LOCAL_OLLAMA_PREFLIGHT_REPORT_SCHEMA_VERSION,
    LocalOllamaModelPlan, LocalOllamaPreflightError, LocalOllamaPreflightExecutionOutcome,
    LocalOllamaPreflightExecutionReceipt, LocalOllamaPreflightMode, LocalOllamaPreflightPlan,
    LocalOllamaPreflightReport, MAX_LOCAL_OLLAMA_MODELS, MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES,
    parse_local_ollama_preflight_plan, run_local_ollama_preflight,
    run_local_ollama_preflight_with_receipt,
};
pub use suite::{
    CategoryResult, EVALUATION_SCHEMA_VERSION, EvaluationError, EvaluationFailure,
    EvaluationReport, EvaluationSuite, MAX_EVALUATION_CASES, MAX_EVALUATION_SUITE_BYTES,
    TransformationCoverage, parse_suite, run_suite,
};
pub use verified_candidate_batch::{
    VerifiedCandidateBatch, VerifiedCandidateBatchError, VerifiedCandidateBatchRelationship,
    VerifiedCandidateBatchResourceInput,
};
pub use verified_candidate_batch_set::{
    VerifiedCandidateBatchSet, VerifiedCandidateBatchSetError, VerifiedCandidateBatchSetInput,
    VerifiedCandidateBatchSetRelationship,
};
pub use verified_generation_qualification::{
    VerifiedGenerationQualification, VerifiedGenerationQualificationCompiler,
    VerifiedGenerationQualificationCompilerInput, VerifiedGenerationQualificationError,
};
pub use watermark_research::{
    MAX_WATERMARK_RESEARCH_BYTES, WATERMARK_RESEARCH_SCHEMA_VERSION, WatermarkResearchCorpus,
    WatermarkResearchError, WatermarkResearchOutcome, WatermarkResearchSummary,
    parse_watermark_research_corpus,
};
pub use writing_sample_library::{
    MAX_WRITING_SAMPLE_LIBRARY_BYTES, WRITING_SAMPLE_LIBRARY_SCHEMA_VERSION, WritingSample,
    WritingSampleLibrary, WritingSampleLibraryError, WritingSampleLibrarySummary,
    WritingSampleOrigin, WritingSampleRole, parse_writing_sample_library,
};
