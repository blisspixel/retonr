//! Immutable artifact facts, qualification evidence, and activation contracts.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod activation;
mod artifact;
mod qualification;

pub use activation::{
    ActivationAction, ActivationDecision, ActivationError, ActivationId, ActiveArtifactBinding,
    InstalledArtifact, QualificationInvalidation, activate,
};
pub use artifact::{
    ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole, ArtifactSource,
    DeclaredCapabilities, LicenseRecord, ManifestError, TokenizerIdentity,
};
pub use qualification::{
    HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationId,
    QualificationRecord, QualificationStatus, RuntimeIdentity,
};
