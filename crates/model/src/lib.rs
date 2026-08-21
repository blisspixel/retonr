//! Immutable artifact facts, qualification evidence, and activation contracts.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod activation;
mod artifact;
mod artifact_set;
mod effective_package;
mod installed_artifact_set;
mod model_package;
mod native_load;
mod package_source;
mod qualification;
mod qualification_v2;
mod runtime_identity;
mod runtime_package;

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
pub use installed_artifact_set::{
    INSTALLED_ARTIFACT_SET_SCHEMA_VERSION, InstalledArtifactSet, InstalledArtifactSetError,
    MAX_INSTALLED_ARTIFACT_SET_JSON_BYTES,
};
pub use model_package::{
    EMBEDDED_MODEL_COMPONENT_LIMIT, EmbeddedModelComponent, EmbeddedModelComponentPurpose,
    MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES, MODEL_PACKAGE_MANIFEST_SCHEMA_VERSION,
    ModelPackageManifest, ModelPackageManifestError, ModelPackageManifestId, ModelPackageMember,
    ModelPackageMemberRole, ModelWeightLayout,
};
pub use native_load::{
    MAX_NATIVE_LOAD_COMPONENTS, MAX_NATIVE_LOAD_OBSERVATION_JSON_BYTES,
    NATIVE_LOAD_OBSERVATION_SCHEMA_VERSION, NativeLoadEvidenceClass, NativeLoadObservation,
    NativeLoadObservationError, NativeLoadObservationId, NativeLoadObservationInput,
    NativeLoadOrigin, NativeLoadVisibilityScope, NativeLoadedComponent, NativeMappingClass,
};
pub use package_source::{
    MAX_PACKAGE_SOURCE_JSON_BYTES, PACKAGE_SOURCE_SCHEMA_VERSION, PackageSource,
    PackageSourceError, PackageSourceId, PackageSourceKind, PackageTransformation,
};
pub use qualification::{
    HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationId,
    QualificationRecord, QualificationRecordError, QualificationStatus, RuntimeIdentity,
};
pub use qualification_v2::{
    MAX_QUALIFICATION_V2_CANONICAL_BYTES, MAX_QUALIFICATION_V2_JSON_BYTES,
    QUALIFICATION_V2_SCHEMA_VERSION, QualificationRecordV2, QualificationRecordV2Error,
    QualificationRecordV2Input, QualificationV2Id,
};
pub use runtime_identity::{
    ComputeBackend, EFFECTIVE_RUNTIME_STATE_SCHEMA_VERSION, EffectiveRuntimeState,
    EffectiveRuntimeStateError, EffectiveRuntimeStateFromLoadInput, EffectiveRuntimeStateId,
    EffectiveRuntimeStateInput, ExecutionPlacement, MAX_RUNTIME_IDENTITY_JSON_BYTES,
    RUNTIME_BUILD_IDENTITY_SCHEMA_VERSION, RuntimeAbi, RuntimeArchitecture, RuntimeBuildId,
    RuntimeBuildIdentity, RuntimeBuildIdentityError, RuntimeBuildIdentityInput, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimeTarget, RuntimeTargetError,
};
pub use runtime_package::{
    MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES, RUNTIME_PACKAGE_MANIFEST_SCHEMA_VERSION,
    RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageManifestError,
    RuntimePackageManifestId, RuntimePackageMember, RuntimePackageMemberRole,
};
