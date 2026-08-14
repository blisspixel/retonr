//! Immutable artifact facts, qualification evidence, and activation contracts.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod activation;
mod artifact;
mod artifact_set;
mod effective_package;
mod qualification;
mod runtime_identity;

pub use activation::{
    ActivationAction, ActivationDecision, ActivationDecisionError, ActivationError, ActivationId,
    ActiveArtifactBinding, InstallationError, InstalledArtifact, QualificationInvalidation,
    QualificationInvalidationError, activate,
};
pub use artifact::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord, ManifestError, TokenizerIdentity,
};
pub use artifact_set::{
    ARTIFACT_SET_MANIFEST_SCHEMA_VERSION, ArtifactSetId, ArtifactSetManifest,
    ArtifactSetManifestError, ArtifactSetMember, ArtifactSetPathError, ArtifactSetRelativePath,
    MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES, MAX_ARTIFACT_SET_MEMBERS,
    MAX_ARTIFACT_SET_PATH_COMPONENT_BYTES, MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES,
    MAX_ARTIFACT_SET_TOTAL_PATH_BYTES,
};
pub use effective_package::{
    EFFECTIVE_PACKAGE_EVIDENCE_SCHEMA_VERSION, EffectivePackageEvidence,
    EffectivePackageEvidenceError, EffectivePackageEvidenceId, EffectivePackageEvidenceInput,
    EffectivePackageEvidenceMode, EffectivePackageMemberEvidence, EffectivePackageMemberPurpose,
    MAX_EFFECTIVE_PACKAGE_CANONICAL_BYTES, MAX_EFFECTIVE_PACKAGE_EVIDENCE_JSON_BYTES,
    MAX_EFFECTIVE_PACKAGE_MEMBER_PURPOSES, MAX_EFFECTIVE_PACKAGE_PURPOSE_ASSIGNMENTS,
    PackageTransformationDisposition,
};
pub use qualification::{
    HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationId,
    QualificationRecord, QualificationRecordError, QualificationStatus, RuntimeIdentity,
};
pub use runtime_identity::{
    ComputeBackend, EFFECTIVE_RUNTIME_STATE_SCHEMA_VERSION, EffectiveRuntimeState,
    EffectiveRuntimeStateError, EffectiveRuntimeStateId, EffectiveRuntimeStateInput,
    ExecutionPlacement, MAX_RUNTIME_IDENTITY_JSON_BYTES, RUNTIME_BUILD_IDENTITY_SCHEMA_VERSION,
    RuntimeAbi, RuntimeArchitecture, RuntimeBuildId, RuntimeBuildIdentity,
    RuntimeBuildIdentityError, RuntimeBuildIdentityInput, RuntimeBuildMode, RuntimeOperatingSystem,
    RuntimeTarget, RuntimeTargetError,
};
