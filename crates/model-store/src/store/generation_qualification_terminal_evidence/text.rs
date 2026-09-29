use rewrite_model::{
    GenerationQualificationInterruptedPhaseV1, GenerationQualificationLicenseDecisionV1,
    GenerationQualificationLicensePermissionV1, GenerationQualificationLicenseReasonV1,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseCheckpointV1,
    GenerationQualificationPhaseInterruptionReasonV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationPlatformReasonV1, GenerationQualificationPlatformStatusV1,
    GenerationRepeatabilityTerminalStageV1,
};

use crate::{StoreError, StoreResult};

pub(super) const fn phase_status(value: GenerationQualificationPhaseStatusV1) -> &'static str {
    match value {
        GenerationQualificationPhaseStatusV1::Passed => "passed",
        GenerationQualificationPhaseStatusV1::Failed => "failed",
        GenerationQualificationPhaseStatusV1::Skipped => "skipped",
    }
}

pub(super) const fn platform_status(
    value: GenerationQualificationPlatformStatusV1,
) -> &'static str {
    match value {
        GenerationQualificationPlatformStatusV1::Supported => "supported",
        GenerationQualificationPlatformStatusV1::Rejected => "rejected",
    }
}

pub(super) const fn platform_reason(
    value: GenerationQualificationPlatformReasonV1,
) -> &'static str {
    match value {
        GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu => {
            "reviewed_managed_linux_native_cpu"
        }
        GenerationQualificationPlatformReasonV1::UnsupportedOperatingSystem => {
            "unsupported_operating_system"
        }
        GenerationQualificationPlatformReasonV1::UnsupportedArchitecture => {
            "unsupported_architecture"
        }
        GenerationQualificationPlatformReasonV1::UnsupportedAbi => "unsupported_abi",
        GenerationQualificationPlatformReasonV1::UnsupportedExecutionClass => {
            "unsupported_execution_class"
        }
        GenerationQualificationPlatformReasonV1::UnsupportedHardwareEnvelope => {
            "unsupported_hardware_envelope"
        }
        GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied => {
            "assessment_policy_denied"
        }
    }
}

pub(super) const fn license_permission(
    value: GenerationQualificationLicensePermissionV1,
) -> &'static str {
    match value {
        GenerationQualificationLicensePermissionV1::LocalGeneration => "local_generation",
    }
}

pub(super) const fn license_decision(
    value: GenerationQualificationLicenseDecisionV1,
) -> &'static str {
    match value {
        GenerationQualificationLicenseDecisionV1::LocalUseOnly => "local_use_only",
        GenerationQualificationLicenseDecisionV1::Rejected => "rejected",
    }
}

pub(super) const fn license_reason(value: GenerationQualificationLicenseReasonV1) -> &'static str {
    match value {
        GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration => {
            "approved_local_generation"
        }
        GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied => "approval_policy_denied",
    }
}

pub(super) const fn terminal_status(
    value: GenerationQualificationOperationTerminalStatusV1,
) -> &'static str {
    match value {
        GenerationQualificationOperationTerminalStatusV1::Completed => "completed",
        GenerationQualificationOperationTerminalStatusV1::Cancelled => "cancelled",
        GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded => "deadline_exceeded",
        GenerationQualificationOperationTerminalStatusV1::Failed => "failed",
    }
}

pub(super) const fn finalization_status(
    value: GenerationQualificationOperationFinalizationStatusV1,
) -> &'static str {
    match value {
        GenerationQualificationOperationFinalizationStatusV1::NotRequired => "not_required",
        GenerationQualificationOperationFinalizationStatusV1::Passed => "passed",
        GenerationQualificationOperationFinalizationStatusV1::Failed => "failed",
    }
}

pub(super) const fn interrupted_phase(
    value: GenerationQualificationInterruptedPhaseV1,
) -> &'static str {
    match value {
        GenerationQualificationInterruptedPhaseV1::AttemptLedger => "attempt_ledger",
        GenerationQualificationInterruptedPhaseV1::Repeatability => "repeatability",
        GenerationQualificationInterruptedPhaseV1::ResourceEvidence => "resource_evidence",
        GenerationQualificationInterruptedPhaseV1::HumanAdjudication => "human_adjudication",
    }
}

pub(super) const fn checkpoint(value: GenerationQualificationPhaseCheckpointV1) -> &'static str {
    match value {
        GenerationQualificationPhaseCheckpointV1::BeforePhase => "before_phase",
        GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition => "evidence_acquisition",
        GenerationQualificationPhaseCheckpointV1::EvidenceCompilation => "evidence_compilation",
        GenerationQualificationPhaseCheckpointV1::ManifestCompilation => "manifest_compilation",
        GenerationQualificationPhaseCheckpointV1::FinalAuthorityRevalidation => {
            "final_authority_revalidation"
        }
        GenerationQualificationPhaseCheckpointV1::MandatoryFinalization => "mandatory_finalization",
    }
}

pub(super) const fn interruption_reason(
    value: GenerationQualificationPhaseInterruptionReasonV1,
) -> &'static str {
    match value {
        GenerationQualificationPhaseInterruptionReasonV1::Cancelled => "cancelled",
        GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded => "deadline_exceeded",
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift => "authority_drift",
        GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationMissing => {
            "required_observation_missing"
        }
        GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationInvalid => {
            "required_observation_invalid"
        }
        GenerationQualificationPhaseInterruptionReasonV1::MeasurementOverflow => {
            "measurement_overflow"
        }
        GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed => {
            "evidence_compilation_failed"
        }
        GenerationQualificationPhaseInterruptionReasonV1::CleanupFailed => "cleanup_failed",
    }
}

pub(super) fn repeatability_stage(
    value: GenerationRepeatabilityTerminalStageV1,
) -> StoreResult<&'static str> {
    match value {
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed => {
            Ok("candidate_generation_failed")
        }
        GenerationRepeatabilityTerminalStageV1::DeterministicFailed
        | GenerationRepeatabilityTerminalStageV1::JudgeFailed
        | GenerationRepeatabilityTerminalStageV1::Passed => Err(StoreError::CorruptRecord),
    }
}
