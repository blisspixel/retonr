//! Immutable artifact facts, qualification evidence, and activation contracts.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod activation;
mod artifact;
mod qualification;

pub use activation::{
    ActivationAction, ActivationDecision, ActivationDecisionError, ActivationError, ActivationId,
    ActiveArtifactBinding, InstallationError, InstalledArtifact, QualificationInvalidation,
    QualificationInvalidationError, activate,
};
pub use artifact::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord, ManifestError, TokenizerIdentity,
};
pub use qualification::{
    HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationId,
    QualificationRecord, QualificationRecordError, QualificationStatus, RuntimeIdentity,
};
