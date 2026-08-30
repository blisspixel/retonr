use rewrite_app::effective_runtime_state_observation::EffectiveRuntimeStateObservationError;
use rewrite_app::{
    GenerationEffectivePackageReleaseError, ManagedOllamaLaunchError,
    ManagedOllamaPostAcquisitionFailure, PackageAttestationError,
};
use rewrite_inference::InferenceErrorKind;
use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1,
};

use crate::LocalOllamaManagedPreflightError;
use rewrite_runtime_attestor::ManagedGenerationWorkerError;
use rewrite_runtime_isolation::IsolationError;

use super::super::super::LocalOllamaManagedGenerationError;

pub(super) fn cleanup_disposition(
    release: Option<&GenerationEffectivePackageReleaseError>,
) -> CandidateGenerationAttemptCleanupDispositionV1 {
    if release.is_some_and(release_cleanup_failed) {
        CandidateGenerationAttemptCleanupDispositionV1::Failed
    } else {
        CandidateGenerationAttemptCleanupDispositionV1::Succeeded
    }
}

pub(super) fn release_cleanup_failed(error: &GenerationEffectivePackageReleaseError) -> bool {
    error.cleanup().is_some() || error.model().is_some() || error.runtime().is_some()
}

pub(super) fn generation_failure_phase(
    observed: CandidateGenerationAttemptFailurePhaseV1,
    error: &LocalOllamaManagedGenerationError,
) -> CandidateGenerationAttemptFailurePhaseV1 {
    match error {
        LocalOllamaManagedGenerationError::Launch(
            ManagedOllamaLaunchError::PostAcquisitionFailure { finalization, .. },
        ) => post_acquisition_failure_phase(finalization.has_failures()),
        LocalOllamaManagedGenerationError::Cleanup(_)
        | LocalOllamaManagedGenerationError::RuntimePackageAfterCleanup(_)
        | LocalOllamaManagedGenerationError::RuntimePackageAfterCleanupFailure { .. } => {
            CandidateGenerationAttemptFailurePhaseV1::Cleanup
        }
        LocalOllamaManagedGenerationError::Launch(_) => {
            CandidateGenerationAttemptFailurePhaseV1::Launch
        }
        LocalOllamaManagedGenerationError::CleanupAfterFailure { operation, .. }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterOperationFailure {
            operation,
            ..
        }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterOperationAndCleanupFailure {
            operation,
            ..
        } => generation_failure_phase(observed, operation),
        LocalOllamaManagedGenerationError::ResponseValidation(_) => {
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation
        }
        LocalOllamaManagedGenerationError::EffectiveState(_)
        | LocalOllamaManagedGenerationError::Worker(_)
        | LocalOllamaManagedGenerationError::InvalidCandidateAttemptRelationship => {
            CandidateGenerationAttemptFailurePhaseV1::FinalObservation
        }
        LocalOllamaManagedGenerationError::Managed(error) => {
            preflight_failure_phase(observed, error)
        }
        LocalOllamaManagedGenerationError::InvalidGenerationAuthority
        | LocalOllamaManagedGenerationError::Session(_) => observed,
    }
}

pub(super) fn generation_failure_category(
    error: &LocalOllamaManagedGenerationError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        LocalOllamaManagedGenerationError::Managed(error) => preflight_failure_category(error),
        LocalOllamaManagedGenerationError::Launch(error) => launch_failure_category(error),
        LocalOllamaManagedGenerationError::Worker(error) => worker_failure_category(*error),
        LocalOllamaManagedGenerationError::EffectiveState(error) => {
            effective_state_failure_category(error)
        }
        LocalOllamaManagedGenerationError::InvalidGenerationAuthority
        | LocalOllamaManagedGenerationError::InvalidCandidateAttemptRelationship => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        LocalOllamaManagedGenerationError::Session(error) => match error.kind {
            InferenceErrorKind::Cancelled => CandidateGenerationAttemptFailureCategoryV1::Cancelled,
            InferenceErrorKind::Deadline => {
                CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded
            }
            InferenceErrorKind::Compatibility => {
                CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform
            }
            InferenceErrorKind::Policy => {
                CandidateGenerationAttemptFailureCategoryV1::RequestInvalid
            }
            InferenceErrorKind::MalformedResponse => {
                CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid
            }
            InferenceErrorKind::Retryable | InferenceErrorKind::Permanent => {
                CandidateGenerationAttemptFailureCategoryV1::TransportFailed
            }
        },
        LocalOllamaManagedGenerationError::Cleanup(_)
        | LocalOllamaManagedGenerationError::RuntimePackageAfterCleanup(_)
        | LocalOllamaManagedGenerationError::RuntimePackageAfterCleanupFailure { .. } => {
            CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
        }
        LocalOllamaManagedGenerationError::CleanupAfterFailure { operation, .. }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterOperationFailure {
            operation,
            ..
        }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterOperationAndCleanupFailure {
            operation,
            ..
        } => generation_failure_category(operation),
        LocalOllamaManagedGenerationError::ResponseValidation(_) => {
            CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid
        }
    }
}

fn preflight_failure_category(
    error: &LocalOllamaManagedPreflightError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        LocalOllamaManagedPreflightError::InvalidInput => {
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid
        }
        LocalOllamaManagedPreflightError::InvalidPackageBinding
        | LocalOllamaManagedPreflightError::InvalidHelperBinding => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        LocalOllamaManagedPreflightError::Package(error) => package_failure_category(error),
        LocalOllamaManagedPreflightError::Isolation(error) => {
            isolation_failure_category(error, false)
        }
        LocalOllamaManagedPreflightError::CloudDisableFeatureUnavailable => {
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform
        }
        LocalOllamaManagedPreflightError::Cleanup(_) => {
            CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
        }
        LocalOllamaManagedPreflightError::CleanupAfterFailure { operation, .. } => {
            preflight_failure_category(operation)
        }
        LocalOllamaManagedPreflightError::Preflight(_) => {
            CandidateGenerationAttemptFailureCategoryV1::TransportFailed
        }
        LocalOllamaManagedPreflightError::InvalidEvidenceBinding
        | LocalOllamaManagedPreflightError::TruncatedStartupOutput
        | LocalOllamaManagedPreflightError::CloudDisable(_)
        | LocalOllamaManagedPreflightError::Witness(_)
        | LocalOllamaManagedPreflightError::NativeLoad(_)
        | LocalOllamaManagedPreflightError::BoundObservation(_)
        | LocalOllamaManagedPreflightError::ReportEncoding => {
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch
        }
    }
}

fn launch_failure_category(
    error: &ManagedOllamaLaunchError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        ManagedOllamaLaunchError::Package(error) => package_failure_category(error),
        ManagedOllamaLaunchError::RelationshipMismatch
        | ManagedOllamaLaunchError::UnauthorizedLaunch
        | ManagedOllamaLaunchError::ModelAuthority(_)
        | ManagedOllamaLaunchError::IsolationInputMismatch
        | ManagedOllamaLaunchError::CapabilityBusy => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        ManagedOllamaLaunchError::Isolation(error) => isolation_failure_category(error, true),
        ManagedOllamaLaunchError::PostAcquisitionFailure {
            primary,
            finalization,
        } => post_acquisition_failure_category(*primary, finalization.has_failures()),
    }
}

pub(super) const fn post_acquisition_failure_phase(
    has_finalizer_failures: bool,
) -> CandidateGenerationAttemptFailurePhaseV1 {
    if has_finalizer_failures {
        CandidateGenerationAttemptFailurePhaseV1::Cleanup
    } else {
        CandidateGenerationAttemptFailurePhaseV1::Launch
    }
}

pub(super) const fn post_acquisition_failure_category(
    primary: ManagedOllamaPostAcquisitionFailure,
    has_finalizer_failures: bool,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    if has_finalizer_failures {
        return CandidateGenerationAttemptFailureCategoryV1::CleanupFailed;
    }
    match primary {
        ManagedOllamaPostAcquisitionFailure::InputBindingMismatch => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        ManagedOllamaPostAcquisitionFailure::Cancelled => {
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        }
        ManagedOllamaPostAcquisitionFailure::DeadlineExceeded => {
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded
        }
    }
}

fn package_failure_category(
    error: &PackageAttestationError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        PackageAttestationError::Cancelled => {
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        }
        PackageAttestationError::MemberBytesConflict
        | PackageAttestationError::MemberIdentityChanged
        | PackageAttestationError::RuntimeRelationship(_)
        | PackageAttestationError::ModelRelationship(_) => {
            CandidateGenerationAttemptFailureCategoryV1::PackageChanged
        }
        PackageAttestationError::ArtifactSet(_) | PackageAttestationError::MemberIo(_) => {
            CandidateGenerationAttemptFailureCategoryV1::PackageRevalidationFailed
        }
        PackageAttestationError::InvalidLimits
        | PackageAttestationError::InvalidModelLimits
        | PackageAttestationError::TooManyCodeMembers { .. }
        | PackageAttestationError::CodeMemberTooLarge { .. }
        | PackageAttestationError::CodeBytesTooLarge { .. }
        | PackageAttestationError::TooManyModelMembers { .. }
        | PackageAttestationError::ModelMemberTooLarge { .. }
        | PackageAttestationError::ModelBytesTooLarge { .. } => {
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid
        }
    }
}

fn worker_failure_category(
    error: ManagedGenerationWorkerError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        ManagedGenerationWorkerError::InvalidLimits
        | ManagedGenerationWorkerError::InvalidRequest
        | ManagedGenerationWorkerError::InvalidNativeLoadRequest => {
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid
        }
        ManagedGenerationWorkerError::Cancelled => {
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        }
        ManagedGenerationWorkerError::DeadlineExceeded => {
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded
        }
        ManagedGenerationWorkerError::Unsupported => {
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform
        }
        ManagedGenerationWorkerError::ServerChanged
        | ManagedGenerationWorkerError::ProcessVisibilityInsufficient
        | ManagedGenerationWorkerError::WorkerCountMismatch
        | ManagedGenerationWorkerError::WorkerChanged
        | ManagedGenerationWorkerError::ParentChainMismatch
        | ManagedGenerationWorkerError::NamespaceMismatch
        | ManagedGenerationWorkerError::PrivilegeMismatch
        | ManagedGenerationWorkerError::ExecutableMismatch
        | ManagedGenerationWorkerError::CommandMismatch
        | ManagedGenerationWorkerError::UnverifiableExecutableMapping
        | ManagedGenerationWorkerError::NativeClosureMismatch
        | ManagedGenerationWorkerError::ForbiddenServerCodeMapped
        | ManagedGenerationWorkerError::AcceleratorLibraryMapped
        | ManagedGenerationWorkerError::ModelMappingMismatch
        | ManagedGenerationWorkerError::IncompleteObservation
        | ManagedGenerationWorkerError::ResourceLimit
        | ManagedGenerationWorkerError::ObservationChanged
        | ManagedGenerationWorkerError::PlatformObservationFailed => {
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch
        }
    }
}

fn effective_state_failure_category(
    error: &EffectiveRuntimeStateObservationError,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        EffectiveRuntimeStateObservationError::Cancelled => {
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        }
        EffectiveRuntimeStateObservationError::UnsupportedProfile => {
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform
        }
        EffectiveRuntimeStateObservationError::RelationshipMismatch => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        EffectiveRuntimeStateObservationError::PlatformIo(_)
        | EffectiveRuntimeStateObservationError::InvalidPlatformObservation
        | EffectiveRuntimeStateObservationError::PlatformObservationChanged
        | EffectiveRuntimeStateObservationError::LiveReobservation(_)
        | EffectiveRuntimeStateObservationError::InvalidEffectiveState(_) => {
            CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch
        }
    }
}

fn isolation_failure_category(
    error: &IsolationError,
    during_launch: bool,
) -> CandidateGenerationAttemptFailureCategoryV1 {
    match error {
        IsolationError::Cancelled => CandidateGenerationAttemptFailureCategoryV1::Cancelled,
        IsolationError::UnsupportedPlatform => {
            CandidateGenerationAttemptFailureCategoryV1::UnsupportedPlatform
        }
        IsolationError::OperationDeadlineExceeded
        | IsolationError::StartupTimeout
        | IsolationError::ControlledBuildTimeout
        | IsolationError::ControlledBuildSnapshotTimeout
        | IsolationError::ShutdownTimeout => {
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded
        }
        IsolationError::InvalidPolicy(_)
        | IsolationError::InvalidLaunch(_)
        | IsolationError::InvalidBootstrap(_) => {
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid
        }
        IsolationError::InvalidHelper
        | IsolationError::HostPolicyDenied
        | IsolationError::NamespaceSetup
        | IsolationError::LoopbackSetup
        | IsolationError::NetworkCanary
        | IsolationError::DescriptorLeak
        | IsolationError::PrivilegeDrop
        | IsolationError::SocketPolicyCompile
        | IsolationError::SocketPolicyInstall
        | IsolationError::SocketPolicyInactive
        | IsolationError::SocketPolicyBehavior
        | IsolationError::ManagedDeviceBoundaryUnavailable
        | IsolationError::ManagedDeviceBoundarySetup
        | IsolationError::ManagedDeviceBoundaryBehavior
        | IsolationError::ManagedContainmentPolicyCompile
        | IsolationError::ManagedContainmentPolicyInstall
        | IsolationError::ManagedContainmentPolicyInactive
        | IsolationError::ManagedContainmentPolicyBehavior
        | IsolationError::RuntimeInputObjectMismatch
        | IsolationError::RuntimeInputBoundaryUnavailable
        | IsolationError::RuntimeInputBoundarySetup
        | IsolationError::RuntimeInputBoundaryBehavior
        | IsolationError::FilesystemIsolationUnavailable
        | IsolationError::FilesystemIsolationSetup
        | IsolationError::FilesystemAliasSetup(_)
        | IsolationError::FilesystemIsolationBehavior
        | IsolationError::ControlledBuildObjectMismatch
        | IsolationError::ControlledBuildOutputNotEmpty
        | IsolationError::HelperProtocol
        | IsolationError::InvalidChannelEndpoint
        | IsolationError::ChannelAlreadyRequested
        | IsolationError::BootstrapRootPreparation
        | IsolationError::BootstrapRootVerification
        | IsolationError::ProcessExited
        | IsolationError::EvidenceChanged
        | IsolationError::NativeOperation { .. } => {
            if during_launch {
                CandidateGenerationAttemptFailureCategoryV1::LaunchFailed
            } else {
                CandidateGenerationAttemptFailureCategoryV1::ObservationMismatch
            }
        }
    }
}

pub(super) fn generation_has_embedded_cleanup(error: &LocalOllamaManagedGenerationError) -> bool {
    match error {
        LocalOllamaManagedGenerationError::Launch(
            ManagedOllamaLaunchError::PostAcquisitionFailure { finalization, .. },
        ) => finalization.has_failures(),
        LocalOllamaManagedGenerationError::Managed(
            LocalOllamaManagedPreflightError::Cleanup(_)
            | LocalOllamaManagedPreflightError::CleanupAfterFailure { .. },
        )
        | LocalOllamaManagedGenerationError::Cleanup(_)
        | LocalOllamaManagedGenerationError::CleanupAfterFailure { .. }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterCleanup(_)
        | LocalOllamaManagedGenerationError::RuntimePackageAfterOperationFailure { .. }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterCleanupFailure { .. }
        | LocalOllamaManagedGenerationError::RuntimePackageAfterOperationAndCleanupFailure {
            ..
        } => true,
        _ => false,
    }
}

fn preflight_failure_phase(
    observed: CandidateGenerationAttemptFailurePhaseV1,
    error: &LocalOllamaManagedPreflightError,
) -> CandidateGenerationAttemptFailurePhaseV1 {
    match error {
        LocalOllamaManagedPreflightError::Cleanup(_) => {
            CandidateGenerationAttemptFailurePhaseV1::Cleanup
        }
        LocalOllamaManagedPreflightError::CleanupAfterFailure { operation, .. } => {
            preflight_failure_phase(observed, operation)
        }
        LocalOllamaManagedPreflightError::InvalidInput
        | LocalOllamaManagedPreflightError::InvalidPackageBinding
        | LocalOllamaManagedPreflightError::InvalidHelperBinding
        | LocalOllamaManagedPreflightError::InvalidEvidenceBinding
        | LocalOllamaManagedPreflightError::TruncatedStartupOutput
        | LocalOllamaManagedPreflightError::CloudDisable(_)
        | LocalOllamaManagedPreflightError::CloudDisableFeatureUnavailable
        | LocalOllamaManagedPreflightError::Package(_)
        | LocalOllamaManagedPreflightError::Isolation(_)
        | LocalOllamaManagedPreflightError::Witness(_)
        | LocalOllamaManagedPreflightError::NativeLoad(_)
        | LocalOllamaManagedPreflightError::BoundObservation(_)
        | LocalOllamaManagedPreflightError::Preflight(_)
        | LocalOllamaManagedPreflightError::ReportEncoding => observed,
    }
}
