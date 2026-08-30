use thiserror::Error;

/// Portable generation-qualification contract validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationContractError {
    /// Encoded JSON exceeds the fixed pre-decode ceiling for its record.
    #[error("encoded generation-qualification record exceeds its limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, incomplete, duplicated, trailing, or otherwise invalid.
    #[error("generation-qualification record encoding is invalid")]
    InvalidEncoding,
    /// JSON is valid but differs from the one canonical encoding.
    #[error("generation-qualification record encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The portable record schema version is unsupported.
    #[error("unsupported generation-qualification schema {0}")]
    UnsupportedSchema(u32),
    /// A machine key is empty, noncanonical, or exceeds its fixed ceiling.
    #[error("generation-qualification machine key is invalid")]
    InvalidMachineKey,
    /// A source identity, digest, or byte count is inconsistent.
    #[error("generation case source binding is invalid")]
    InvalidSourceBinding,
    /// A count or byte ceiling is zero, over its hard maximum, or inconsistent.
    #[error("generation-qualification limits are invalid")]
    InvalidLimits,
    /// A required semantic array is empty or exceeds its hard item ceiling.
    #[error("generation-qualification collection size is invalid")]
    InvalidCollectionSize,
    /// A semantic array contains a duplicate identity or machine key.
    #[error("generation-qualification collection contains a duplicate")]
    DuplicateEntry,
    /// A digest-sorted set is reordered or contains an equal adjacent identity.
    #[error("generation-qualification digest set is not strictly ordered")]
    NonCanonicalSetOrder,
    /// A case names a different cluster than the supplied cluster record.
    #[error("generation case cluster relationship does not match")]
    ClusterMismatch,
    /// A suite case identity or order differs from the supplied case records.
    #[error("generation suite case relationship does not match")]
    CaseMismatch,
    /// A repetition or plan names a different suite from the supplied suite record.
    #[error("generation suite relationship does not match")]
    SuiteMismatch,
    /// A runtime build names a different runtime-package manifest.
    #[error("generation system runtime package relationship does not match")]
    RuntimePackageMismatch,
    /// An effective runtime state names a different runtime build.
    #[error("generation system runtime build relationship does not match")]
    RuntimeBuildMismatch,
    /// A model-package manifest names or describes a different artifact set.
    #[error("generation system model package relationship does not match")]
    ModelPackageMismatch,
    /// The selected model artifact is absent from the effective generation closure.
    #[error("generation system selected model artifact relationship does not match")]
    ModelArtifactMismatch,
    /// Effective-package evidence names a different package, build, or runtime state.
    #[error("generation system effective-package relationship does not match")]
    EffectivePackageMismatch,
    /// A planned attempt names a case outside its exact suite.
    #[error("planned candidate attempt case relationship does not match")]
    PlannedCaseMismatch,
    /// A planned attempt names a different repetition record.
    #[error("planned candidate attempt repetition relationship does not match")]
    RepetitionMismatch,
    /// A planned attempt names a different generation system.
    #[error("planned candidate attempt generation-system relationship does not match")]
    GenerationSystemMismatch,
    /// A planned attempt ordinal is out of range or not contiguous in its plan.
    #[error("planned candidate attempt ordinal is invalid")]
    AttemptOrdinalMismatch,
    /// Candidate count or exact candidate-envelope ceilings are invalid.
    #[error("planned candidate output policy is invalid")]
    InvalidCandidateOutputPolicy,
    /// A precursor does not name the selected attempt exactly once at its plan ordinal.
    #[error("candidate-attempt precursor plan relationship does not match")]
    PlannedAttemptMismatch,
    /// A runtime or model installation generation is zero.
    #[error("candidate-attempt precursor installation generation is invalid")]
    InvalidInstallationGeneration,
    /// Managed generation evidence does not close over the supplied exact records.
    #[error("managed candidate-generation evidence relationship does not match")]
    ManagedEvidenceRelationshipMismatch,
    /// Managed generation evidence makes an unsupported positive claim.
    #[error("managed candidate-generation evidence claims are invalid")]
    InvalidManagedEvidenceClaims,
    /// Cleanup names another precursor or managed-evidence record.
    #[error("candidate-generation cleanup relationship does not match")]
    CleanupRelationshipMismatch,
    /// Cleanup statuses and their complete failure categories disagree.
    #[error("candidate-generation cleanup status closure is invalid")]
    InvalidCleanupStatusClosure,
    /// A candidate artifact is malformed, over limit, or bound to another attempt.
    #[error("candidate artifact binding is invalid")]
    InvalidCandidateArtifact,
    /// An evidence-bundle member path, size, or ordering is invalid.
    #[error("candidate evidence-bundle entry is invalid")]
    InvalidEvidenceBundleEntry,
    /// Evidence-bundle role multiplicities or candidate membership are incomplete.
    #[error("candidate evidence-bundle role closure is invalid")]
    InvalidEvidenceBundleRoleClosure,
    /// An evidence-bundle record names different upstream records.
    #[error("candidate evidence-bundle relationship does not match")]
    EvidenceBundleRelationshipMismatch,
    /// A readback record names a different bundle or unsupported fixed status.
    #[error("candidate evidence-bundle readback relationship does not match")]
    ReadbackRelationshipMismatch,
    /// A receipt names stale or substituted upstream records or candidates.
    #[error("candidate-generation receipt relationship does not match")]
    ReceiptRelationshipMismatch,
    /// A receipt was requested without successful cleanup and verified readback.
    #[error("candidate-generation receipt postconditions are incomplete")]
    InvalidReceiptPostconditions,
    /// A final attempt record names another planned attempt, precursor, or receipt.
    #[error("candidate-generation attempt relationship does not match")]
    AttemptRecordRelationshipMismatch,
    /// A failed-attempt phase, observation, category, and cleanup closure is invalid.
    #[error("candidate-generation failed-attempt closure is invalid")]
    InvalidAttemptFailure,
    /// A selection policy does not close over its suite, plan, or planned attempts.
    #[error("candidate-selection policy relationship does not match")]
    SelectionPolicyRelationshipMismatch,
    /// A receipt set does not close over its exact completed attempt records.
    #[error("candidate-generation receipt-set relationship does not match")]
    ReceiptSetRelationshipMismatch,
    /// An ordered receipt-set pair does not share one complete evaluation scope.
    #[error("candidate receipt-pair relationship does not match")]
    ReceiptPairRelationshipMismatch,
    /// Deterministic report bytes or their bounded aggregate facts are invalid.
    #[error("candidate deterministic report relationship is invalid")]
    InvalidDeterministicReportRelationship,
    /// A deterministic evaluation names substituted scope, report, or result facts.
    #[error("candidate deterministic evaluation relationship does not match")]
    DeterministicEvaluationRelationshipMismatch,
    /// A candidate-judge plan names substituted scope, systems, cases, or policy.
    #[error("candidate-judge plan relationship does not match")]
    CandidateJudgePlanRelationshipMismatch,
    /// A candidate-judge schedule is incomplete, reordered, or inconsistently seeded.
    #[error("candidate-judge schedule relationship does not match")]
    CandidateJudgeScheduleRelationshipMismatch,
    /// A request, response, or observation aggregate does not close over its schedule.
    #[error("candidate-judge aggregate relationship does not match")]
    CandidateJudgeAggregateRelationshipMismatch,
    /// A normalized judge observation is malformed or names another scheduled attempt.
    #[error("candidate-judge observation relationship does not match")]
    CandidateJudgeObservationRelationshipMismatch,
    /// Managed local-judge receipt fields do not close over the supplied exact records.
    #[error("managed local-judge receipt relationship does not match")]
    ManagedLocalJudgeReceiptRelationshipMismatch,
    /// A managed local-judge receipt lacks a required successful postcondition.
    #[error("managed local-judge receipt postconditions are incomplete")]
    InvalidManagedLocalJudgeReceiptPostconditions,
    /// Triage report bytes are empty or exceed their fixed inert framing bound.
    #[error("candidate-judge triage report relationship is invalid")]
    InvalidCandidateJudgeTriageReport,
    /// A candidate-judge join names substituted scope, evidence, or result facts.
    #[error("candidate-judge join relationship does not match")]
    CandidateJudgeJoinRelationshipMismatch,
    /// Checked count or identity-encoding arithmetic overflowed.
    #[error("generation-qualification identity encoding overflowed")]
    EncodingOverflow,
    /// Canonical identity bytes exceed the fixed record ceiling.
    #[error("generation-qualification canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}
