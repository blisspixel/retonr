use std::io::Read;

use rewrite_model::{ArtifactSetId, ArtifactSetRelativePath};
use rewrite_types::Digest;
use thiserror::Error;

use super::{
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError,
    VerifiedRuntimeSourceBuildInputs, sequence_digest,
};

mod alpine_apk;
mod cargo_source;
mod checksum;
mod license_evidence;
mod member;
mod rust_channel;
mod signature;

use member::{ALPINE_MEMBERS, RUST_MEMBERS, read_and_verify_members, validate_manifest_members};
use rust_channel::verify_rust_channel;
use signature::{SignatureExpectation, verify_detached_signature};

pub use cargo_source::{
    CARGO_SOURCE_CLOSURE_PROCEDURE_ID, CARGO_SOURCE_CLOSURE_PROCEDURE_VERSION,
    CargoSourceClosureError, CargoSourceClosureLimits, CargoSourceClosureReviewerFacts,
    VerifiedCargoSourceClosure, verify_cargo_source_closure,
};
pub use license_evidence::{
    RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_ID,
    RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_VERSION, RetainedProgramLicenseEvidenceClosureId,
    RetainedProgramLicenseEvidenceError, RetainedProgramLicenseEvidenceLimits,
    RetainedProgramLicenseEvidenceReviewerFacts, VerifiedRetainedProgramLicenseEvidenceClosure,
    verify_retained_program_license_evidence_closure,
};

/// Domain-separated identity of the exact authenticated Rust and Alpine
/// upstream closure used by the retained-program build.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetainedProgramUpstreamClosureId(Digest);

impl RetainedProgramUpstreamClosureId {
    /// Returns the digest defining this closure identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Reviewer-visible facts established for the official Rust release inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRustReleaseUpstream {
    closure_id: Digest,
}

impl VerifiedRustReleaseUpstream {
    /// Returns the Rust-only domain-separated closure identity.
    #[must_use]
    pub const fn closure_id(&self) -> &Digest {
        &self.closure_id
    }

    /// Returns the exact Rust release version.
    #[must_use]
    pub const fn release(&self) -> &'static str {
        "1.97.1"
    }

    /// Returns the exact signed channel date.
    #[must_use]
    pub const fn channel_date(&self) -> &'static str {
        "2026-07-16"
    }

    /// Returns the exact Rust compiler and standard-library commit.
    #[must_use]
    pub const fn rust_commit(&self) -> &'static str {
        "8bab26f4f68e0e26f0bb7960be334d5b520ea452"
    }

    /// Returns the exact Cargo package version recorded by the channel.
    #[must_use]
    pub const fn cargo_version(&self) -> &'static str {
        "0.98.0 (c980f4866 2026-06-30)"
    }

    /// Returns the exact Cargo commit recorded by the channel.
    #[must_use]
    pub const fn cargo_commit(&self) -> &'static str {
        "8bab26f4f68e0e26f0bb7960be334d5b520ea452"
    }

    /// Returns the exact host and target triple.
    #[must_use]
    pub const fn target(&self) -> &'static str {
        "x86_64-unknown-linux-musl"
    }

    /// Returns the authenticated release-key fingerprint.
    #[must_use]
    pub const fn release_key_fingerprint(&self) -> &'static str {
        signature::RUST_FINGERPRINT
    }

    /// Returns the authenticated release-key ID.
    #[must_use]
    pub const fn release_key_id(&self) -> &'static str {
        signature::RUST_KEY_ID
    }

    /// Returns the detached-signature creation time in Unix seconds.
    #[must_use]
    pub const fn signature_creation_time(&self) -> u64 {
        signature::RUST_SIGNATURE_CREATION
    }

    /// Reports the intentional physical alias between host and target `rust-std`.
    #[must_use]
    pub const fn host_and_target_std_are_one_member(&self) -> bool {
        true
    }
}

/// Reviewer-visible facts established for the official Alpine release inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAlpineReleaseUpstream {
    closure_id: Digest,
}

impl VerifiedAlpineReleaseUpstream {
    /// Returns the Alpine-only domain-separated closure identity.
    #[must_use]
    pub const fn closure_id(&self) -> &Digest {
        &self.closure_id
    }

    /// Returns the exact Alpine release version.
    #[must_use]
    pub const fn release(&self) -> &'static str {
        "3.23.3"
    }

    /// Returns the authenticated release-key fingerprint.
    #[must_use]
    pub const fn release_key_fingerprint(&self) -> &'static str {
        signature::ALPINE_FINGERPRINT
    }

    /// Returns the authenticated release-key ID.
    #[must_use]
    pub const fn release_key_id(&self) -> &'static str {
        signature::ALPINE_KEY_ID
    }

    /// Returns the detached-signature creation time in Unix seconds.
    #[must_use]
    pub const fn signature_creation_time(&self) -> u64 {
        signature::ALPINE_SIGNATURE_CREATION
    }

    /// Returns the exact joined Alpine package name.
    #[must_use]
    pub const fn libgcc_package_name(&self) -> &'static str {
        "libgcc"
    }

    /// Returns the exact joined Alpine package version.
    #[must_use]
    pub const fn libgcc_package_version(&self) -> &'static str {
        "15.2.0-r2"
    }

    /// Returns the exact authenticated static `BusyBox` package version.
    #[must_use]
    pub const fn busybox_package_version(&self) -> &'static str {
        "1.37.0-r30"
    }

    /// Returns the digest of `bin/busybox.static` extracted from the authenticated APK.
    #[must_use]
    pub const fn busybox_executable_digest(&self) -> &'static str {
        "82bbbabec12a985ae58810cfe975c3399264dc888aa592d8e460732bdd30a8dd"
    }

    /// Reports whether the APK's embedded RSA/SHA-1 signature, authenticated
    /// through the PGP-signed minirootfs key, was verified.
    #[must_use]
    pub const fn embedded_apk_signature_verified(&self) -> bool {
        true
    }
}

/// Inert, exact upstream authentication and semantic-join result.
///
/// This value grants no build, execution, review, policy, or runtime-admission
/// authority. It only records the facts established by this verifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRetainedProgramUpstreamClosure {
    closure_id: RetainedProgramUpstreamClosureId,
    source_input_set_id: ArtifactSetId,
    rust: VerifiedRustReleaseUpstream,
    alpine: VerifiedAlpineReleaseUpstream,
}

impl VerifiedRetainedProgramUpstreamClosure {
    /// Returns the complete Rust-and-Alpine closure identity.
    #[must_use]
    pub const fn closure_id(&self) -> &RetainedProgramUpstreamClosureId {
        &self.closure_id
    }

    /// Returns the exact source-input artifact set authenticated by this closure.
    #[must_use]
    pub const fn source_input_set_id(&self) -> &ArtifactSetId {
        &self.source_input_set_id
    }

    /// Returns authenticated Rust reviewer facts.
    #[must_use]
    pub const fn rust(&self) -> &VerifiedRustReleaseUpstream {
        &self.rust
    }

    /// Returns authenticated Alpine reviewer facts.
    #[must_use]
    pub const fn alpine(&self) -> &VerifiedAlpineReleaseUpstream {
        &self.alpine
    }
}

/// Failure to establish the exact retained-program upstream closure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RetainedProgramUpstreamClosureError {
    /// The input does not carry a verified retained-program lineage wrapper.
    #[error("verified retained-program lineage is required")]
    MissingProgramLineage,
    /// Required manifest membership or exact production metadata does not match.
    #[error("retained-program upstream manifest membership does not match")]
    ManifestMismatch,
    /// A required component could not be opened.
    #[error("retained-program upstream component is unavailable")]
    ComponentUnavailable,
    /// A required component could not be read.
    #[error("retained-program upstream component could not be read")]
    ComponentRead,
    /// A required component has the wrong byte length or digest.
    #[error("retained-program upstream component measurement does not match")]
    ComponentMeasurementMismatch,
    /// Cooperative cancellation was requested.
    #[error("retained-program upstream verification was cancelled")]
    Cancelled,
    /// A checksum file is malformed, noncanonical, or does not join its payload.
    #[error("retained-program upstream checksum is invalid")]
    InvalidChecksum,
    /// The Rust channel is malformed, ambiguous, or does not expose the exact release.
    #[error("retained-program Rust channel is invalid")]
    InvalidRustChannel,
    /// A trust root is malformed, has invalid bindings, or has the wrong identity.
    #[error("retained-program upstream trust root is invalid")]
    InvalidTrustRoot,
    /// A detached signature is malformed, composed, drifted, or invalid.
    #[error("retained-program upstream signature is invalid")]
    InvalidSignature,
    /// The Alpine v2 APK framing, metadata, data hash, or embedded signature is invalid.
    #[error("retained-program Alpine package authentication is invalid")]
    InvalidAlpinePackage,
}

/// Reopens, remeasures, authenticates, and semantically joins every official
/// Rust and Alpine member in the exact retained-program production profile.
///
/// The opener receives already validated portable member paths. This function
/// performs no path or network discovery. Its result is inert evidence and is
/// not an admission or execution capability.
///
/// # Errors
///
/// Returns [`RetainedProgramUpstreamClosureError`] for a missing strong lineage
/// wrapper, any manifest or byte drift, malformed metadata, failed certificate
/// binding, or failed cryptographic verification.
pub fn verify_retained_program_upstream_closure<R, F, C>(
    inputs: &VerifiedRuntimeSourceBuildInputs,
    open_component: F,
    cancelled: C,
) -> Result<VerifiedRetainedProgramUpstreamClosure, RetainedProgramUpstreamClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    if inputs.retained_program_lineage().is_none() {
        return Err(RetainedProgramUpstreamClosureError::MissingProgramLineage);
    }
    verify_closure(inputs.manifest(), open_component, cancelled)
}

fn verify_closure<R, F, C>(
    manifest: &RuntimeSourceBuildInputManifest,
    mut open_component: F,
    mut cancelled: C,
) -> Result<VerifiedRetainedProgramUpstreamClosure, RetainedProgramUpstreamClosureError>
where
    R: Read,
    F: FnMut(&ArtifactSetRelativePath) -> Result<R, RuntimeSourceBuildInputOpenError>,
    C: FnMut() -> bool,
{
    validate_manifest_members(manifest, &RUST_MEMBERS)?;
    validate_manifest_members(manifest, &ALPINE_MEMBERS)?;
    let rust_bytes =
        read_and_verify_members(manifest, &RUST_MEMBERS, &mut open_component, &mut cancelled)?;
    let alpine_bytes = read_and_verify_members(
        manifest,
        &ALPINE_MEMBERS,
        &mut open_component,
        &mut cancelled,
    )?;

    checksum::verify_rust_checksum(rust_bytes.required(member::RUST_CHANNEL_CHECKSUM)?)?;
    checksum::verify_alpine_checksum(alpine_bytes.required(member::ALPINE_CHECKSUM)?)?;
    verify_rust_channel(rust_bytes.required(member::RUST_CHANNEL)?)?;
    verify_detached_signature(
        rust_bytes.required(member::RUST_TRUST_ROOT)?,
        rust_bytes.required(member::RUST_CHANNEL_SIGNATURE)?,
        rust_bytes.required(member::RUST_CHANNEL)?,
        SignatureExpectation::rust(),
    )?;
    verify_detached_signature(
        alpine_bytes.required(member::ALPINE_TRUST_ROOT)?,
        alpine_bytes.required(member::ALPINE_SIGNATURE)?,
        alpine_bytes.required(member::ALPINE_MINIROOTFS)?,
        SignatureExpectation::alpine(),
    )?;
    alpine_apk::verify_alpine_libgcc_package(
        alpine_bytes.required(member::ALPINE_MINIROOTFS)?,
        alpine_bytes.required(member::ALPINE_LIBGCC)?,
    )?;
    let authenticated_busybox = alpine_apk::verify_alpine_busybox_package(
        alpine_bytes.required(member::ALPINE_MINIROOTFS)?,
        alpine_bytes.required(member::ALPINE_BUSYBOX_APK)?,
    )?;
    if authenticated_busybox != alpine_bytes.required(member::BUSYBOX_EXECUTABLE)? {
        return Err(RetainedProgramUpstreamClosureError::InvalidAlpinePackage);
    }

    let rust_id = member::closure_id(b"retained-program-upstream/rust/v1", &RUST_MEMBERS);
    let alpine_id = member::closure_id(b"retained-program-upstream/alpine/v1", &ALPINE_MEMBERS);
    let source_input_set_id = manifest.artifact_set().artifact_set_id();
    let closure_id = RetainedProgramUpstreamClosureId(sequence_digest(
        b"retained-program-upstream/complete/v1",
        [
            source_input_set_id.digest().as_str().as_bytes(),
            rust_id.as_str().as_bytes(),
            alpine_id.as_str().as_bytes(),
        ],
    ));
    Ok(VerifiedRetainedProgramUpstreamClosure {
        closure_id,
        source_input_set_id,
        rust: VerifiedRustReleaseUpstream {
            closure_id: rust_id,
        },
        alpine: VerifiedAlpineReleaseUpstream {
            closure_id: alpine_id,
        },
    })
}

#[cfg(test)]
mod tests;
