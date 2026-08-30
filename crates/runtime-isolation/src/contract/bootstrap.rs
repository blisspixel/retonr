use std::fs::File;

use rewrite_types::Digest;

use super::{ControlledBuildInputFile, ControlledBuildLaunchSpec, RedactedDigestBuilder};
use crate::{
    IsolationError, IsolationResult, MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES,
    MAXIMUM_CONTROLLED_BUILD_INPUT_FILES,
};

const MAXIMUM_RECIPE_BYTES: u64 = 1024 * 1024;
const AUTHENTICATED_BUSYBOX_EXECUTABLE_PATH: &str = "toolchains/busybox";

mod evidence;

#[cfg(target_os = "linux")]
pub(crate) use evidence::RetainedProgramBootstrapRootObservation;
#[cfg(any(test, target_os = "linux"))]
pub(crate) use evidence::RetainedProgramBootstrapRootPostconditions;
pub use evidence::{RetainedProgramBootstrapExecution, RetainedProgramBootstrapRootEvidence};

/// One of the exact signed-distribution inputs required to prepare the retained
/// program bootstrap root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetainedProgramBootstrapInputKind {
    /// Signed Alpine minirootfs payload.
    AlpineMinirootfs,
    /// Detached signature for the Alpine minirootfs.
    AlpineMinirootfsSignature,
    /// Checksum record for the Alpine minirootfs.
    AlpineMinirootfsChecksum,
    /// Alpine release trust root.
    AlpineReleaseTrustRoot,
    /// Signed Alpine `libgcc` package.
    AlpineLibgccPackage,
    /// Signed Alpine `busybox-static` package used for bounded root preparation.
    AlpineBusyboxStaticPackage,
    /// Signed Rust channel manifest.
    RustChannelManifest,
    /// Detached signature for the Rust channel manifest.
    RustChannelManifestSignature,
    /// Checksum record for the Rust channel manifest.
    RustChannelManifestChecksum,
    /// Rust release trust root.
    RustReleaseTrustRoot,
    /// Exact Cargo distribution for the musl host.
    RustCargoDistribution,
    /// Exact Rust compiler distribution for the musl host.
    RustCompilerDistribution,
    /// Exact Rust standard-library distribution for the musl host and target.
    RustStandardLibraryDistribution,
}

impl RetainedProgramBootstrapInputKind {
    pub(crate) const ALL: [Self; 13] = [
        Self::AlpineMinirootfs,
        Self::AlpineMinirootfsSignature,
        Self::AlpineMinirootfsChecksum,
        Self::AlpineReleaseTrustRoot,
        Self::AlpineLibgccPackage,
        Self::AlpineBusyboxStaticPackage,
        Self::RustChannelManifest,
        Self::RustChannelManifestSignature,
        Self::RustChannelManifestChecksum,
        Self::RustReleaseTrustRoot,
        Self::RustCargoDistribution,
        Self::RustCompilerDistribution,
        Self::RustStandardLibraryDistribution,
    ];

    /// Returns the fixed portable path for this production bootstrap input.
    #[must_use]
    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::AlpineMinirootfs => "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz",
            Self::AlpineMinirootfsSignature => {
                "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc"
            }
            Self::AlpineMinirootfsChecksum => {
                "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256"
            }
            Self::AlpineReleaseTrustRoot => "lineage/host/alpine/ncopa.asc",
            Self::AlpineLibgccPackage => "lineage/host/alpine/libgcc-15.2.0-r2.apk",
            Self::AlpineBusyboxStaticPackage => "lineage/host/alpine/busybox-static-1.37.0-r30.apk",
            Self::RustChannelManifest => "lineage/rust/channel-rust-1.97.1.toml",
            Self::RustChannelManifestSignature => "lineage/rust/channel-rust-1.97.1.toml.asc",
            Self::RustChannelManifestChecksum => "lineage/rust/channel-rust-1.97.1.toml.sha256",
            Self::RustReleaseTrustRoot => "lineage/rust/rust-key.gpg.ascii",
            Self::RustCargoDistribution => {
                "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz"
            }
            Self::RustCompilerDistribution => {
                "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz"
            }
            Self::RustStandardLibraryDistribution => {
                "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz"
            }
        }
    }

    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::AlpineMinirootfs => 1,
            Self::AlpineMinirootfsSignature => 2,
            Self::AlpineMinirootfsChecksum => 3,
            Self::AlpineReleaseTrustRoot => 4,
            Self::AlpineLibgccPackage => 5,
            Self::AlpineBusyboxStaticPackage => 6,
            Self::RustChannelManifest => 7,
            Self::RustChannelManifestSignature => 8,
            Self::RustChannelManifestChecksum => 9,
            Self::RustReleaseTrustRoot => 10,
            Self::RustCargoDistribution => 11,
            Self::RustCompilerDistribution => 12,
            Self::RustStandardLibraryDistribution => 13,
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::AlpineMinirootfs),
            2 => Some(Self::AlpineMinirootfsSignature),
            3 => Some(Self::AlpineMinirootfsChecksum),
            4 => Some(Self::AlpineReleaseTrustRoot),
            5 => Some(Self::AlpineLibgccPackage),
            6 => Some(Self::AlpineBusyboxStaticPackage),
            7 => Some(Self::RustChannelManifest),
            8 => Some(Self::RustChannelManifestSignature),
            9 => Some(Self::RustChannelManifestChecksum),
            10 => Some(Self::RustReleaseTrustRoot),
            11 => Some(Self::RustCargoDistribution),
            12 => Some(Self::RustCompilerDistribution),
            13 => Some(Self::RustStandardLibraryDistribution),
            _ => None,
        }
    }
}

/// Exact byte measurement for one signed bootstrap input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedProgramBootstrapInputMeasurement {
    kind: RetainedProgramBootstrapInputKind,
    digest: Digest,
    byte_size: u64,
}

impl RetainedProgramBootstrapInputMeasurement {
    /// Creates one nonempty bounded input measurement.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::InvalidBootstrap`] for an empty or excessive input.
    pub fn new(
        kind: RetainedProgramBootstrapInputKind,
        digest: Digest,
        byte_size: u64,
    ) -> IsolationResult<Self> {
        if byte_size == 0 || byte_size > MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES {
            return Err(IsolationError::InvalidBootstrap("signed input size"));
        }
        Ok(Self {
            kind,
            digest,
            byte_size,
        })
    }

    /// Returns the exact input kind.
    #[must_use]
    pub const fn kind(&self) -> RetainedProgramBootstrapInputKind {
        self.kind
    }

    /// Returns the fixed portable input path.
    #[must_use]
    pub const fn relative_path(&self) -> &'static str {
        self.kind.relative_path()
    }

    /// Returns the exact SHA-256 digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Returns the exact byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// Complete exact signed-input set for root preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedProgramBootstrapSignedInputs {
    inputs: Vec<RetainedProgramBootstrapInputMeasurement>,
}

/// Exact extracted `BusyBox` executable authenticated by the upstream closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedBusyboxExecutable {
    digest: Digest,
    byte_size: u64,
}

impl AuthenticatedBusyboxExecutable {
    /// Creates the bounded executable measurement used during root preparation.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::InvalidBootstrap`] for an empty or excessive file.
    pub fn new(digest: Digest, byte_size: u64) -> IsolationResult<Self> {
        if byte_size == 0 || byte_size > MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES {
            return Err(IsolationError::InvalidBootstrap("busybox executable size"));
        }
        Ok(Self { digest, byte_size })
    }

    /// Returns the only portable path accepted for the authenticated executable.
    #[must_use]
    pub const fn relative_path(&self) -> &'static str {
        AUTHENTICATED_BUSYBOX_EXECUTABLE_PATH
    }

    /// Returns the authenticated executable digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Returns the authenticated executable byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

impl RetainedProgramBootstrapSignedInputs {
    /// Validates the exact, unique, canonically ordered signed-input set.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::InvalidBootstrap`] for an incomplete, duplicate,
    /// reordered, or excessive set.
    pub fn new(inputs: Vec<RetainedProgramBootstrapInputMeasurement>) -> IsolationResult<Self> {
        if inputs.len() != RetainedProgramBootstrapInputKind::ALL.len()
            || inputs
                .iter()
                .map(RetainedProgramBootstrapInputMeasurement::kind)
                .ne(RetainedProgramBootstrapInputKind::ALL)
            || inputs.len() > MAXIMUM_CONTROLLED_BUILD_INPUT_FILES
        {
            return Err(IsolationError::InvalidBootstrap("signed input set"));
        }
        inputs.iter().try_fold(0_u64, |total, input| {
            total
                .checked_add(input.byte_size)
                .filter(|total| *total <= MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES)
                .ok_or(IsolationError::InvalidBootstrap("signed input aggregate"))
        })?;
        Ok(Self { inputs })
    }

    /// Returns inputs in their fixed canonical order.
    #[must_use]
    pub fn measurements(&self) -> &[RetainedProgramBootstrapInputMeasurement] {
        &self.inputs
    }
}

/// Stable identity of one of the two required bootstrap attempts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedProgramBootstrapAttempt {
    /// First isolated build.
    Primary,
    /// Independently recreated isolated build.
    Rebuild,
}

impl RetainedProgramBootstrapAttempt {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Primary => 1,
            Self::Rebuild => 2,
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Primary),
            2 => Some(Self::Rebuild),
            _ => None,
        }
    }
}

/// Exact bootstrap launch layered over the ordinary controlled-build contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedProgramBootstrapLaunchSpec {
    attempt: RetainedProgramBootstrapAttempt,
    recipe_digest: Digest,
    recipe_bytes: u64,
    signed_inputs: RetainedProgramBootstrapSignedInputs,
    busybox_executable: AuthenticatedBusyboxExecutable,
    controlled_build: ControlledBuildLaunchSpec,
}

/// Retained filesystem capabilities for one bootstrap attempt.
#[derive(Debug)]
pub struct RetainedProgramBootstrapCapabilities {
    program: File,
    input_root: File,
    input_files: Vec<ControlledBuildInputFile>,
    output_root: File,
}

impl RetainedProgramBootstrapCapabilities {
    /// Groups the retained builder, exact input tree, and initially empty output.
    #[must_use]
    pub fn new(
        program: File,
        input_root: File,
        input_files: Vec<ControlledBuildInputFile>,
        output_root: File,
    ) -> Self {
        Self {
            program,
            input_root,
            input_files,
            output_root,
        }
    }

    pub(crate) fn input_files(&self) -> &[ControlledBuildInputFile] {
        &self.input_files
    }

    /// Consumes the capability group without converting retained files to paths.
    #[must_use]
    pub fn into_parts(self) -> (File, File, Vec<ControlledBuildInputFile>, File) {
        (
            self.program,
            self.input_root,
            self.input_files,
            self.output_root,
        )
    }
}

impl RetainedProgramBootstrapLaunchSpec {
    /// Creates one exact bootstrap attempt specification.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::InvalidBootstrap`] for an empty or excessive recipe.
    pub fn new(
        attempt: RetainedProgramBootstrapAttempt,
        recipe_digest: Digest,
        recipe_bytes: u64,
        signed_inputs: RetainedProgramBootstrapSignedInputs,
        busybox_executable: AuthenticatedBusyboxExecutable,
        controlled_build: ControlledBuildLaunchSpec,
    ) -> IsolationResult<Self> {
        if recipe_bytes == 0 || recipe_bytes > MAXIMUM_RECIPE_BYTES {
            return Err(IsolationError::InvalidBootstrap("recipe size"));
        }
        Ok(Self {
            attempt,
            recipe_digest,
            recipe_bytes,
            signed_inputs,
            busybox_executable,
            controlled_build,
        })
    }

    /// Returns the selected independent attempt.
    #[must_use]
    pub const fn attempt(&self) -> RetainedProgramBootstrapAttempt {
        self.attempt
    }

    /// Returns the exact recipe digest.
    #[must_use]
    pub const fn recipe_digest(&self) -> &Digest {
        &self.recipe_digest
    }

    /// Returns the exact recipe byte length.
    #[must_use]
    pub const fn recipe_bytes(&self) -> u64 {
        self.recipe_bytes
    }

    /// Returns the complete signed root-preparation inputs.
    #[must_use]
    pub const fn signed_inputs(&self) -> &RetainedProgramBootstrapSignedInputs {
        &self.signed_inputs
    }

    /// Returns the exact executable extracted from the authenticated `BusyBox` package.
    #[must_use]
    pub const fn busybox_executable(&self) -> &AuthenticatedBusyboxExecutable {
        &self.busybox_executable
    }

    /// Returns the underlying controlled-build launch.
    #[must_use]
    pub const fn controlled_build(&self) -> &ControlledBuildLaunchSpec {
        &self.controlled_build
    }

    /// Returns a domain-separated identity of the complete bootstrap launch.
    #[must_use]
    pub fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/bootstrap/v1");
        digest.push_u8(self.attempt.code());
        digest.push_bytes(self.recipe_digest.as_str().as_bytes());
        digest.push_u64(self.recipe_bytes);
        digest.push_bytes(self.controlled_build.redacted_digest().as_str().as_bytes());
        digest.push_usize(self.signed_inputs.inputs.len());
        for input in &self.signed_inputs.inputs {
            digest.push_u8(input.kind.code());
            digest.push_bytes(input.relative_path().as_bytes());
            digest.push_u64(input.byte_size);
            digest.push_bytes(input.digest.as_str().as_bytes());
        }
        digest.push_bytes(self.busybox_executable.relative_path().as_bytes());
        digest.push_u64(self.busybox_executable.byte_size);
        digest.push_bytes(self.busybox_executable.digest.as_str().as_bytes());
        digest.finish()
    }

    pub(crate) fn validate_input_files(
        &self,
        input_files: &[ControlledBuildInputFile],
    ) -> IsolationResult<()> {
        let recipe_matches = input_files.iter().any(|input| {
            input.relative_path() == "lineage/build-recipe-v2.json"
                && input.expected_bytes() == self.recipe_bytes
                && input.expected_digest() == &self.recipe_digest
        });
        let signed_match = self.signed_inputs.inputs.iter().all(|expected| {
            input_files.iter().any(|input| {
                input.relative_path() == expected.relative_path()
                    && input.expected_bytes() == expected.byte_size
                    && input.expected_digest() == &expected.digest
            })
        });
        let busybox_matches = input_files.iter().any(|input| {
            input.relative_path() == self.busybox_executable.relative_path()
                && input.expected_bytes() == self.busybox_executable.byte_size
                && input.expected_digest() == &self.busybox_executable.digest
        });
        if recipe_matches && signed_match && busybox_matches {
            Ok(())
        } else {
            Err(IsolationError::InvalidBootstrap("input measurement join"))
        }
    }
}

#[cfg(test)]
mod tests;
