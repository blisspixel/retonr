use rewrite_model::{ArtifactSetId, ArtifactSetRelativePath, RuntimeTarget};
use rewrite_types::Digest;

use super::{RuntimeSourceBuildInputError, VerifiedRuntimeSourceBuildInputs};
use super::{
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputRole, RuntimeSourceBuildPolicy,
    sequence_digest,
};

const MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const LINUX_CPU_ONLY_DEVICE_VISIBILITY_POLICY_CODE: u8 = 1;

/// Content-free deterministic handoff from a verified input manifest to a build runner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildPlan {
    source_inputs_id: ArtifactSetId,
    source_manifest_digest: Digest,
    target: RuntimeTarget,
    build_program_path: ArtifactSetRelativePath,
    build_program_digest: Digest,
    build_program_bytes: u64,
    isolation_helper_path: ArtifactSetRelativePath,
    isolation_helper_digest: Digest,
    isolation_helper_bytes: u64,
    controlled_build_capability_abi: u32,
    execution_policy: RuntimeSourceBuildExecutionPolicy,
    expected_launch_digest: Digest,
    policy: RuntimeSourceBuildPolicy,
    plan_digest: Digest,
    retained_program_closure_id: Option<Digest>,
}

/// Exact resource and time bounds for every attempt in one source-build plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildExecutionPolicy {
    startup_timeout_seconds: u64,
    shutdown_timeout_seconds: u64,
    execution_timeout_seconds: u64,
    maximum_arguments: u64,
    maximum_environment_variables: u64,
    maximum_value_bytes: u64,
    maximum_open_files: u64,
    maximum_processes: u64,
}

impl RuntimeSourceBuildExecutionPolicy {
    const fn controlled() -> Self {
        Self {
            startup_timeout_seconds: 30,
            shutdown_timeout_seconds: 30,
            execution_timeout_seconds: 3 * 60 * 60,
            maximum_arguments: 256,
            maximum_environment_variables: 256,
            maximum_value_bytes: 64 * 1024,
            maximum_open_files: 4_096,
            maximum_processes: 4_096,
        }
    }

    /// Returns the preparation and launch timeout in whole seconds.
    #[must_use]
    pub const fn startup_timeout_seconds(self) -> u64 {
        self.startup_timeout_seconds
    }

    /// Returns the graceful shutdown timeout in whole seconds.
    #[must_use]
    pub const fn shutdown_timeout_seconds(self) -> u64 {
        self.shutdown_timeout_seconds
    }

    /// Returns the complete target process-tree timeout in whole seconds.
    #[must_use]
    pub const fn execution_timeout_seconds(self) -> u64 {
        self.execution_timeout_seconds
    }

    /// Returns the argument-count ceiling.
    #[must_use]
    pub const fn maximum_arguments(self) -> u64 {
        self.maximum_arguments
    }

    /// Returns the environment-entry ceiling.
    #[must_use]
    pub const fn maximum_environment_variables(self) -> u64 {
        self.maximum_environment_variables
    }

    /// Returns the per-argument and per-environment-value byte ceiling.
    #[must_use]
    pub const fn maximum_value_bytes(self) -> u64 {
        self.maximum_value_bytes
    }

    /// Returns the open-file resource ceiling.
    #[must_use]
    pub const fn maximum_open_files(self) -> u64 {
        self.maximum_open_files
    }

    /// Returns the process-count resource ceiling.
    #[must_use]
    pub const fn maximum_processes(self) -> u64 {
        self.maximum_processes
    }

    /// Returns the runtime-isolation policy digest for these exact bounds.
    #[must_use]
    pub fn isolation_policy_digest(self) -> Digest {
        let mut digest = RedactedDigest::new(b"runtime-isolation/policy/v2");
        digest.push_u64(self.startup_timeout_seconds);
        digest.push_u32(0);
        digest.push_u64(self.shutdown_timeout_seconds);
        digest.push_u32(0);
        digest.push_u64(self.maximum_arguments);
        digest.push_u64(self.maximum_environment_variables);
        digest.push_u64(self.maximum_value_bytes);
        digest.push_u64(self.maximum_open_files);
        digest.push_u64(self.maximum_processes);
        digest.push_u8(LINUX_CPU_ONLY_DEVICE_VISIBILITY_POLICY_CODE);
        digest.finish()
    }
}

impl RuntimeSourceBuildPlan {
    /// Compiles an inert plan bound to one complete retained-program closure.
    ///
    /// The result contains only portable component identities and reviewed policy.
    /// It never contains a caller-local path or component content.
    ///
    /// # Errors
    ///
    /// This content-free value is not an execution capability. Applications must
    /// retain the non-serializable live closure that supplied `closure_id`.
    ///
    /// Returns [`RuntimeSourceBuildInputError::InvalidProgramLineage`] when the
    /// inputs are legacy or lack the exact production lineage wrapper.
    pub fn for_verified_retained_program_closure(
        inputs: &VerifiedRuntimeSourceBuildInputs,
        closure_id: Digest,
    ) -> Result<Self, RuntimeSourceBuildInputError> {
        inputs
            .retained_program_lineage()
            .ok_or(RuntimeSourceBuildInputError::InvalidProgramLineage)?;
        Ok(Self::compile(inputs.manifest(), Some(closure_id)))
    }

    /// Derives a non-executable plan solely for historical report verification.
    ///
    /// This compatibility path intentionally accepts legacy manifests. Its
    /// result is not eligible for a fresh controlled build.
    #[must_use]
    pub fn for_legacy_read_only_verification(manifest: &RuntimeSourceBuildInputManifest) -> Self {
        Self::compile(manifest, None)
    }

    fn compile(manifest: &RuntimeSourceBuildInputManifest, closure_id: Option<Digest>) -> Self {
        let build_program = manifest
            .components()
            .iter()
            .find(|component| {
                component
                    .roles()
                    .contains(&RuntimeSourceBuildInputRole::BuildScript)
            })
            .expect("validated manifests contain one build script");
        let isolation_helper = manifest
            .components()
            .iter()
            .find(|component| {
                component
                    .roles()
                    .contains(&RuntimeSourceBuildInputRole::IsolationHelper)
            })
            .expect("validated manifests contain one isolation helper");
        let source_inputs_id = manifest.artifact_set().artifact_set_id();
        let source_manifest_digest = manifest.manifest_digest().clone();
        let target = manifest.policy().target();
        let policy = manifest.policy().clone();
        let target_bytes = serde_json::to_vec(&target)
            .expect("a validated runtime target always has a canonical JSON encoding");
        let epoch = policy.source_date_epoch().to_be_bytes();
        let controlled_build_capability_abi = 2_u32;
        let execution_policy = RuntimeSourceBuildExecutionPolicy::controlled();
        let expected_launch_digest = expected_launch_digest(
            manifest,
            build_program.relative_path(),
            build_program.digest(),
            build_program.byte_size(),
            &policy,
            execution_policy,
            MAXIMUM_CONTROLLED_BUILD_WORKSPACE_BYTES,
        );
        let capability_abi = controlled_build_capability_abi.to_be_bytes();
        let build_program_bytes = build_program.byte_size().to_be_bytes();
        let isolation_helper_bytes = isolation_helper.byte_size().to_be_bytes();
        let isolation_policy_digest = execution_policy.isolation_policy_digest();
        let execution_timeout = execution_policy.execution_timeout_seconds.to_be_bytes();
        let fields = [
            source_inputs_id.digest().as_str().as_bytes(),
            source_manifest_digest.as_str().as_bytes(),
            target_bytes.as_slice(),
            b"denied",
            b"cpu_only",
            policy.cpu_feature_policy().as_bytes(),
            policy.locale().as_bytes(),
            policy.timezone().as_bytes(),
            epoch.as_slice(),
            policy.environment_digest().as_str().as_bytes(),
            policy.build_arguments_digest().as_str().as_bytes(),
            build_program.relative_path().as_str().as_bytes(),
            build_program.digest().as_str().as_bytes(),
            build_program_bytes.as_slice(),
            isolation_helper.relative_path().as_str().as_bytes(),
            isolation_helper.digest().as_str().as_bytes(),
            isolation_helper_bytes.as_slice(),
            capability_abi.as_slice(),
            isolation_policy_digest.as_str().as_bytes(),
            execution_timeout.as_slice(),
            expected_launch_digest.as_str().as_bytes(),
        ];
        let plan_digest = closure_id.as_ref().map_or_else(
            || sequence_digest(b"runtime-source-build/plan/v3", fields.iter().copied()),
            |closure_id| {
                sequence_digest(
                    b"runtime-source-build/plan/v5",
                    fields
                        .iter()
                        .copied()
                        .chain([closure_id.as_str().as_bytes()]),
                )
            },
        );
        Self {
            source_inputs_id,
            source_manifest_digest,
            target,
            build_program_path: build_program.relative_path().clone(),
            build_program_digest: build_program.digest().clone(),
            build_program_bytes: build_program.byte_size(),
            isolation_helper_path: isolation_helper.relative_path().clone(),
            isolation_helper_digest: isolation_helper.digest().clone(),
            isolation_helper_bytes: isolation_helper.byte_size(),
            controlled_build_capability_abi,
            execution_policy,
            expected_launch_digest,
            policy,
            plan_digest,
            retained_program_closure_id: closure_id,
        }
    }

    /// Returns the exact frozen input-set identity.
    #[must_use]
    pub const fn source_inputs_id(&self) -> &ArtifactSetId {
        &self.source_inputs_id
    }

    /// Returns the complete canonical source-build manifest identity.
    #[must_use]
    pub const fn source_manifest_digest(&self) -> &Digest {
        &self.source_manifest_digest
    }

    /// Returns the selected native target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the portable controlled-build program path.
    #[must_use]
    pub const fn build_program_path(&self) -> &ArtifactSetRelativePath {
        &self.build_program_path
    }

    /// Returns the exact controlled-build program digest.
    #[must_use]
    pub const fn build_program_digest(&self) -> &Digest {
        &self.build_program_digest
    }

    /// Returns the exact controlled-build program byte length.
    #[must_use]
    pub const fn build_program_bytes(&self) -> u64 {
        self.build_program_bytes
    }

    /// Returns the portable retained isolation-helper path.
    #[must_use]
    pub const fn isolation_helper_path(&self) -> &ArtifactSetRelativePath {
        &self.isolation_helper_path
    }

    /// Returns the exact retained isolation-helper digest.
    #[must_use]
    pub const fn isolation_helper_digest(&self) -> &Digest {
        &self.isolation_helper_digest
    }

    /// Returns the exact retained isolation-helper byte length.
    #[must_use]
    pub const fn isolation_helper_bytes(&self) -> u64 {
        self.isolation_helper_bytes
    }

    /// Returns the fixed inherited input and output directory capability ABI.
    #[must_use]
    pub const fn controlled_build_capability_abi(&self) -> u32 {
        self.controlled_build_capability_abi
    }

    /// Returns the immutable isolation and execution bounds for every attempt.
    #[must_use]
    pub const fn execution_policy(&self) -> RuntimeSourceBuildExecutionPolicy {
        self.execution_policy
    }

    /// Returns the independently derived complete controlled-launch identity.
    #[must_use]
    pub const fn expected_launch_digest(&self) -> &Digest {
        &self.expected_launch_digest
    }

    /// Returns the complete explicit build policy.
    #[must_use]
    pub const fn policy(&self) -> &RuntimeSourceBuildPolicy {
        &self.policy
    }

    /// Returns the domain-separated digest of this complete content-free plan.
    #[must_use]
    pub const fn plan_digest(&self) -> &Digest {
        &self.plan_digest
    }

    /// Returns the complete retained-program closure identity bound into this plan.
    ///
    /// Presence does not grant execution authority. Only an application-owned
    /// opaque lease retaining the live closure can authorize execution.
    #[must_use]
    pub const fn retained_program_closure_id(&self) -> Option<&Digest> {
        self.retained_program_closure_id.as_ref()
    }
}

fn expected_launch_digest(
    manifest: &RuntimeSourceBuildInputManifest,
    program_path: &ArtifactSetRelativePath,
    program_digest: &Digest,
    program_bytes: u64,
    policy: &RuntimeSourceBuildPolicy,
    execution: RuntimeSourceBuildExecutionPolicy,
    maximum_workspace_bytes: u64,
) -> Digest {
    let mut launch = RedactedDigest::new(b"runtime-isolation/controlled-build/v2");
    launch.push_bytes(program_path.as_str().as_bytes());
    launch.push_bytes(program_digest.as_str().as_bytes());
    launch.push_u64(program_bytes);
    launch.push_u64(execution.execution_timeout_seconds);
    launch.push_u32(0);
    launch.push_u64(maximum_workspace_bytes);
    launch.push_u64(262_144);
    launch.push_u64(8 * 1024 * 1024 * 1024);
    launch.push_u64(4 * 1024 * 1024 * 1024);
    launch.push_u64(10 * 60);
    launch.push_u32(0);
    launch.push_u64(4_096);
    launch.push_u64(1_024);
    launch.push_u64(u64::try_from(policy.build_arguments().len()).unwrap_or(u64::MAX));
    for argument in policy.build_arguments() {
        launch.push_bytes(argument.as_bytes());
    }
    launch.push_u64(u64::try_from(policy.environment().len()).unwrap_or(u64::MAX));
    for variable in policy.environment() {
        launch.push_bytes(variable.name().as_bytes());
        launch.push_bytes(variable.value().as_bytes());
    }
    let launch = launch.finish();
    let mut inputs = manifest.components().iter().collect::<Vec<_>>();
    inputs.sort_unstable_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let mut digest = RedactedDigest::new(b"runtime-isolation/controlled-build-input/v1");
    digest.push_bytes(launch.as_str().as_bytes());
    digest.push_u64(u64::try_from(inputs.len()).unwrap_or(u64::MAX));
    for input in inputs {
        digest.push_bytes(input.relative_path().as_str().as_bytes());
        digest.push_u64(input.byte_size());
        digest.push_bytes(input.digest().as_str().as_bytes());
    }
    digest.finish()
}

#[cfg(test)]
pub(super) fn expected_launch_digest_for_test(
    manifest: &RuntimeSourceBuildInputManifest,
    maximum_workspace_bytes: u64,
) -> Digest {
    let program = manifest
        .components()
        .iter()
        .find(|component| {
            component
                .roles()
                .contains(&RuntimeSourceBuildInputRole::BuildScript)
        })
        .expect("validated manifests contain one build script");
    expected_launch_digest(
        manifest,
        program.relative_path(),
        program.digest(),
        program.byte_size(),
        manifest.policy(),
        RuntimeSourceBuildExecutionPolicy::controlled(),
        maximum_workspace_bytes,
    )
}

struct RedactedDigest {
    bytes: Vec<u8>,
}

impl RedactedDigest {
    fn new(domain: &[u8]) -> Self {
        let mut digest = Self { bytes: Vec::new() };
        digest.push_bytes(b"retonr/redacted-digest/v1");
        digest.push_bytes(domain);
        digest
    }

    fn push_bytes(&mut self, value: &[u8]) {
        self.push_u64(u64::try_from(value.len()).unwrap_or(u64::MAX));
        self.bytes.extend_from_slice(value);
    }

    fn push_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn push_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn push_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn finish(self) -> Digest {
        Digest::sha256(&self.bytes)
    }
}
