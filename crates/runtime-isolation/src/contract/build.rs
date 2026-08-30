use std::{collections::BTreeMap, ffi::OsString, fs::File, time::Duration};

use rewrite_types::Digest;

use super::{
    ControlledBuildOutputTree, IsolationPolicy, ManagedStartupOutput, RedactedDigestBuilder,
    build_evidence::ControlledBuildIsolationEvidence,
    validation::{validate_environment_key, validate_value},
};
use crate::{IsolationError, IsolationResult};
use crate::{
    MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES, MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES,
    MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES, MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES,
};

const MAXIMUM_EXECUTION_TIMEOUT: Duration = Duration::from_hours(4);
const MAXIMUM_PORTABLE_PATH_BYTES: usize = 4_096;

pub(crate) const CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT: Duration = Duration::from_mins(10);

/// Maximum aggregate logical bytes copied into one controlled-build input snapshot.
pub const MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Maximum retained regular-file capabilities in one controlled-build input tree.
pub const MAXIMUM_CONTROLLED_BUILD_INPUT_FILES: usize = 1_024;

/// One exact retained regular-file capability mapped into the private input tree.
#[derive(Debug)]
pub struct ControlledBuildInputFile {
    relative_path: String,
    expected_digest: Digest,
    expected_bytes: u64,
    #[cfg_attr(
        not(target_os = "linux"),
        expect(
            dead_code,
            reason = "the capability is consumed only by Linux isolation"
        )
    )]
    file: File,
}

impl ControlledBuildInputFile {
    /// Binds one retained regular file to one portable path in the private input tree.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::ControlledBuildObjectMismatch`] when the path is
    /// invalid or the retained object is not a regular file.
    pub fn new(
        relative_path: impl Into<String>,
        expected_digest: Digest,
        expected_bytes: u64,
        file: File,
    ) -> IsolationResult<Self> {
        let relative_path = relative_path.into();
        let metadata = file
            .metadata()
            .map_err(|_| IsolationError::ControlledBuildObjectMismatch)?;
        if !valid_portable_relative_path(&relative_path)
            || !metadata.is_file()
            || metadata.len() != expected_bytes
            || expected_bytes > MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES
        {
            return Err(IsolationError::ControlledBuildObjectMismatch);
        }
        Ok(Self {
            relative_path,
            expected_digest,
            expected_bytes,
            file,
        })
    }

    /// Returns the exact portable path created beneath the private input root.
    #[must_use]
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// Returns the exact SHA-256 digest required of the snapshotted bytes.
    #[must_use]
    pub const fn expected_digest(&self) -> &Digest {
        &self.expected_digest
    }

    /// Returns the exact byte length required of the snapshotted file.
    #[must_use]
    pub const fn expected_bytes(&self) -> u64 {
        self.expected_bytes
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn file(&self) -> &File {
        &self.file
    }
}

/// Bounded, content-bound description of one controlled build process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledBuildLaunchSpec {
    program_relative_path: String,
    program_digest: Digest,
    program_bytes: u64,
    arguments: Vec<OsString>,
    environment: BTreeMap<OsString, OsString>,
    execution_timeout: Duration,
}

impl ControlledBuildLaunchSpec {
    /// Creates a controlled build description for one program inside the input root.
    ///
    /// # Errors
    ///
    /// Returns [`IsolationError::InvalidLaunch`] for a nonportable path, an empty
    /// program, or an execution timeout outside the fixed hard ceiling.
    pub fn new(
        program_relative_path: impl Into<String>,
        program_digest: Digest,
        program_bytes: u64,
        execution_timeout: Duration,
    ) -> IsolationResult<Self> {
        let specification = Self {
            program_relative_path: program_relative_path.into(),
            program_digest,
            program_bytes,
            arguments: Vec::new(),
            environment: BTreeMap::new(),
            execution_timeout,
        };
        specification.validate_program()?;
        Ok(specification)
    }

    /// Appends one ordered build argument.
    pub fn push_argument(&mut self, argument: impl Into<OsString>) {
        self.arguments.push(argument.into());
    }

    /// Inserts one build environment variable into an otherwise cleared block.
    pub fn insert_environment(&mut self, key: impl Into<OsString>, value: impl Into<OsString>) {
        self.environment.insert(key.into(), value.into());
    }

    /// Returns the portable path resolved beneath the retained input root.
    #[must_use]
    pub fn program_relative_path(&self) -> &str {
        &self.program_relative_path
    }

    /// Returns the expected digest of the retained program object.
    #[must_use]
    pub const fn program_digest(&self) -> &Digest {
        &self.program_digest
    }

    /// Returns the expected byte length of the retained program object.
    #[must_use]
    pub const fn program_bytes(&self) -> u64 {
        self.program_bytes
    }

    /// Returns the maximum wall time for the complete build process tree.
    #[must_use]
    pub const fn execution_timeout(&self) -> Duration {
        self.execution_timeout
    }

    /// Returns a domain-separated digest of the complete content-free launch.
    #[must_use]
    pub fn redacted_digest(&self) -> Digest {
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/controlled-build/v2");
        digest.push_bytes(self.program_relative_path.as_bytes());
        digest.push_bytes(self.program_digest.as_str().as_bytes());
        digest.push_u64(self.program_bytes);
        digest.push_u64(self.execution_timeout.as_secs());
        digest.push_u32(self.execution_timeout.subsec_nanos());
        digest.push_u64(MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES);
        digest.push_u64(MAXIMUM_CONTROLLED_BUILD_WORKSPACE_INODES);
        digest.push_u64(MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES);
        digest.push_u64(MAXIMUM_CONTROLLED_BUILD_INPUT_BYTES);
        digest.push_u64(CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT.as_secs());
        digest.push_u32(CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT.subsec_nanos());
        digest.push_usize(MAXIMUM_CONTROLLED_BUILD_OUTPUT_TREE_ENTRIES);
        digest.push_usize(MAXIMUM_CONTROLLED_BUILD_INPUT_FILES);
        digest.push_usize(self.arguments.len());
        for argument in &self.arguments {
            digest.push_bytes(argument.as_encoded_bytes());
        }
        digest.push_usize(self.environment.len());
        for (key, value) in &self.environment {
            digest.push_bytes(key.as_encoded_bytes());
            digest.push_bytes(value.as_encoded_bytes());
        }
        digest.finish()
    }

    /// Returns the launch digest extended with the canonical retained-input map.
    #[must_use]
    pub fn redacted_digest_with_inputs(&self, input_files: &[ControlledBuildInputFile]) -> Digest {
        let mut declarations = input_files.iter().collect::<Vec<_>>();
        declarations.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let mut digest = RedactedDigestBuilder::new(b"runtime-isolation/controlled-build-input/v1");
        digest.push_bytes(self.redacted_digest().as_str().as_bytes());
        digest.push_usize(declarations.len());
        for declaration in declarations {
            digest.push_bytes(declaration.relative_path.as_bytes());
            digest.push_u64(declaration.expected_bytes);
            digest.push_bytes(declaration.expected_digest.as_str().as_bytes());
        }
        digest.finish()
    }

    pub(crate) fn validate(&self, policy: IsolationPolicy) -> IsolationResult<()> {
        self.validate_program()?;
        if self.arguments.len() > policy.maximum_arguments() {
            return Err(IsolationError::InvalidLaunch("argument count"));
        }
        if self.environment.len() > policy.maximum_environment_variables() {
            return Err(IsolationError::InvalidLaunch("environment count"));
        }
        for argument in &self.arguments {
            validate_value(argument, policy.maximum_value_bytes())?;
        }
        for (key, value) in &self.environment {
            validate_environment_key(key, policy.maximum_value_bytes())?;
            validate_value(value, policy.maximum_value_bytes())?;
        }
        Ok(())
    }

    fn validate_program(&self) -> IsolationResult<()> {
        if self.program_bytes == 0
            || self.execution_timeout.is_zero()
            || self.execution_timeout > MAXIMUM_EXECUTION_TIMEOUT
            || !valid_portable_relative_path(&self.program_relative_path)
        {
            Err(IsolationError::InvalidLaunch("controlled build program"))
        } else {
            Ok(())
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn environment(&self) -> &BTreeMap<OsString, OsString> {
        &self.environment
    }
}

fn valid_portable_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PORTABLE_PATH_BYTES
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        })
}

/// Portable completion state for one controlled build process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlledBuildProcessStatus {
    /// The build program exited with code zero.
    Success,
    /// The build program returned a nonzero portable exit code.
    ExitCode(i32),
    /// The build program was terminated by a signal inside the PID namespace.
    Signal(i32),
}

/// Bounded captured output and status for one controlled build process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledBuildOutput {
    status: ControlledBuildProcessStatus,
    streams: ManagedStartupOutput,
    tree: Option<ControlledBuildOutputTree>,
}

impl ControlledBuildOutput {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        status: ControlledBuildProcessStatus,
        streams: ManagedStartupOutput,
    ) -> Self {
        Self {
            status,
            streams,
            tree: None,
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn with_tree(mut self, tree: ControlledBuildOutputTree) -> Self {
        self.tree = Some(tree);
        self
    }

    /// Returns the exact portable process completion state.
    #[must_use]
    pub const fn status(&self) -> ControlledBuildProcessStatus {
        self.status
    }

    /// Returns the bounded standard-output and standard-error prefixes.
    #[must_use]
    pub const fn streams(&self) -> &ManagedStartupOutput {
        &self.streams
    }

    /// Returns the exact private-output tree commitment for a successful build.
    #[must_use]
    pub const fn tree(&self) -> Option<&ControlledBuildOutputTree> {
        self.tree.as_ref()
    }
}

/// Completed controlled build with its retained isolation observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlledBuildExecution {
    isolation: ControlledBuildIsolationEvidence,
    output: ControlledBuildOutput,
    guardian_helper_status: ControlledBuildProcessStatus,
}

impl ControlledBuildExecution {
    #[cfg(target_os = "linux")]
    pub(crate) const fn new(
        isolation: ControlledBuildIsolationEvidence,
        output: ControlledBuildOutput,
        guardian_helper_status: ControlledBuildProcessStatus,
    ) -> Self {
        Self {
            isolation,
            output,
            guardian_helper_status,
        }
    }

    /// Returns the independently observed managed namespace and process evidence.
    #[must_use]
    pub const fn isolation(&self) -> &ControlledBuildIsolationEvidence {
        &self.isolation
    }

    /// Returns the bounded build result.
    #[must_use]
    pub const fn output(&self) -> &ControlledBuildOutput {
        &self.output
    }

    /// Returns the reaped outer guardian/helper operating-system status.
    #[must_use]
    pub const fn guardian_helper_status(&self) -> ControlledBuildProcessStatus {
        self.guardian_helper_status
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use rewrite_types::Digest;
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn controlled_build_spec_is_portable_bounded_and_digest_sensitive() {
        for invalid in ["", "/program", "program/", "a//b", "a/../b", "a\\b"] {
            assert_eq!(
                ControlledBuildLaunchSpec::new(
                    invalid,
                    Digest::sha256(b"program"),
                    7,
                    Duration::from_secs(1),
                ),
                Err(IsolationError::InvalidLaunch("controlled build program"))
            );
        }
        assert_eq!(
            ControlledBuildLaunchSpec::new(
                "bin/build",
                Digest::sha256(b"program"),
                0,
                Duration::from_secs(1),
            ),
            Err(IsolationError::InvalidLaunch("controlled build program"))
        );
        let mut valid = ControlledBuildLaunchSpec::new(
            "bin/build",
            Digest::sha256(b"program"),
            7,
            Duration::from_secs(2),
        )
        .expect("valid build");
        valid.push_argument("build");
        valid.insert_environment("LC_ALL", "C");
        valid
            .validate(IsolationPolicy::default())
            .expect("valid bounded build");
        let digest = valid.redacted_digest();
        valid.push_argument("changed");
        assert_ne!(valid.redacted_digest(), digest);
    }

    #[test]
    fn controlled_build_input_file_requires_one_portable_regular_file() {
        let temporary = NamedTempFile::new().expect("temporary input");
        let retained = File::open(temporary.path()).expect("open retained input");
        assert_eq!(
            ControlledBuildInputFile::new("source/member", Digest::sha256(b""), 0, retained,)
                .expect("valid retained input")
                .relative_path(),
            "source/member"
        );
        for invalid in ["", "/member", "member/", "a//b", "a/../b", "a\\b"] {
            let retained = File::open(temporary.path()).expect("reopen retained input");
            assert!(matches!(
                ControlledBuildInputFile::new(invalid, Digest::sha256(b""), 0, retained),
                Err(IsolationError::ControlledBuildObjectMismatch)
            ));
        }
    }
}
