use rewrite_model::{ArtifactSetId, ArtifactSetRelativePath, RuntimeTarget};
use rewrite_types::Digest;
use serde::Deserialize;

use crate::{
    RuntimeSourceBuildExecutionPolicy, RuntimeSourceBuildInputManifest, RuntimeSourceBuildPlan,
    json::validate_unique_json,
};

use super::{
    RuntimeSourceBuildAttempt, RuntimeSourceBuildOutputTree, RuntimeSourceBuildOutputTreeEntryKind,
    RuntimeSourceBuildReportError,
};

/// Typed execution receipt accepted by controlled source-build report schema 3.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeSourceBuildExecutionReceipt {
    attempt: RuntimeSourceBuildAttempt,
    canonical_bytes: Vec<u8>,
    evidence_digest: Digest,
}

impl VerifiedRuntimeSourceBuildExecutionReceipt {
    /// Returns the controlled-build attempt bound by this receipt.
    #[must_use]
    pub const fn attempt(&self) -> RuntimeSourceBuildAttempt {
        self.attempt
    }

    /// Returns the exact canonical receipt bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the independently recomputed managed-isolation evidence digest.
    #[must_use]
    pub const fn evidence_digest(&self) -> &Digest {
        &self.evidence_digest
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptWire {
    schema_version: u32,
    attempt: RuntimeSourceBuildAttempt,
    source_build_inputs_id: ArtifactSetId,
    source_manifest_digest: Digest,
    build_plan_digest: Digest,
    target: RuntimeTarget,
    controlled_build_capability_abi: u32,
    build_program: ExecutableWire,
    isolation_helper: ExecutableWire,
    isolation_policy: IsolationPolicyWire,
    execution_timeout_seconds: u64,
    launch_digest: Digest,
    isolation_evidence_digest: Digest,
    guardian_pid: u32,
    namespace_init_pid: u32,
    namespaces: NamespacesWire,
    roots: RootsWire,
    canary: CanaryWire,
    network_access: NetworkPolicyWire,
    filesystem_policy: FilesystemPolicyWire,
    landlock_abi: u32,
    target_status: ProcessStatusWire,
    guardian_helper_status: ProcessStatusWire,
    output_tree: OutputTreeWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutableWire {
    relative_path: String,
    byte_size: u64,
    digest: Digest,
    #[serde(default)]
    device: Option<u64>,
    #[serde(default)]
    inode: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IsolationPolicyWire {
    startup_timeout_seconds: u64,
    shutdown_timeout_seconds: u64,
    maximum_arguments: u64,
    maximum_environment_variables: u64,
    maximum_value_bytes: u64,
    maximum_open_files: u64,
    maximum_processes: u64,
    digest: Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NamespacesWire {
    network: ObjectIdentityWire,
    user: ObjectIdentityWire,
    process: ObjectIdentityWire,
    mount: ObjectIdentityWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RootsWire {
    input: ObjectIdentityWire,
    output: ObjectIdentityWire,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectIdentityWire {
    device: u64,
    inode: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CanaryWire {
    loopback_interface_index: u32,
    protocol_version: u8,
    passed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum NetworkPolicyWire {
    Denied,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum FilesystemPolicyWire {
    Landlock,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ProcessStatusWire {
    ExitCode { value: i32 },
    Signal { value: i32 },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputTreeWire {
    digest: Digest,
    entry_count: u32,
    regular_file_count: u32,
    total_file_bytes: u64,
}

/// Parses a canonical receipt and independently verifies every plan-bound fact.
///
/// # Errors
///
/// Returns [`RuntimeSourceBuildReportError`] for malformed, noncanonical, false,
/// incomplete, failed, or plan-divergent receipt claims.
pub fn verify_runtime_source_build_execution_receipt(
    bytes: &[u8],
    attempt: RuntimeSourceBuildAttempt,
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    output_tree: &RuntimeSourceBuildOutputTree,
) -> Result<VerifiedRuntimeSourceBuildExecutionReceipt, RuntimeSourceBuildReportError> {
    validate_unique_json(bytes)
        .map_err(|()| RuntimeSourceBuildReportError::InvalidExecutionReceipt)?;
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeSourceBuildReportError::InvalidExecutionReceipt)?;
    if serde_json::to_vec(&value)
        .map_err(|_| RuntimeSourceBuildReportError::InvalidExecutionReceipt)?
        != bytes
    {
        return Err(RuntimeSourceBuildReportError::NoncanonicalExecutionReceipt);
    }
    let receipt: ReceiptWire = serde_json::from_value(value)
        .map_err(|_| RuntimeSourceBuildReportError::InvalidExecutionReceipt)?;
    verify_binding(&receipt, attempt, source_inputs, plan, output_tree)?;
    Ok(VerifiedRuntimeSourceBuildExecutionReceipt {
        attempt,
        canonical_bytes: bytes.to_vec(),
        evidence_digest: receipt.isolation_evidence_digest,
    })
}

fn verify_binding(
    receipt: &ReceiptWire,
    attempt: RuntimeSourceBuildAttempt,
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    output_tree: &RuntimeSourceBuildOutputTree,
) -> Result<(), RuntimeSourceBuildReportError> {
    let execution = plan.execution_policy();
    let program_path = parse_path(&receipt.build_program.relative_path)?;
    let helper_path = parse_path(&receipt.isolation_helper.relative_path)?;
    if receipt.schema_version != 1
        || receipt.attempt != attempt
        || receipt.source_build_inputs_id != source_inputs.artifact_set().artifact_set_id()
        || receipt.source_build_inputs_id != *plan.source_inputs_id()
        || receipt.source_manifest_digest != *source_inputs.manifest_digest()
        || receipt.source_manifest_digest != *plan.source_manifest_digest()
        || receipt.build_plan_digest != *plan.plan_digest()
        || receipt.target != plan.target()
        || receipt.controlled_build_capability_abi != plan.controlled_build_capability_abi()
        || program_path != *plan.build_program_path()
        || receipt.build_program.digest != *plan.build_program_digest()
        || receipt.build_program.byte_size != plan.build_program_bytes()
        || helper_path != *plan.isolation_helper_path()
        || receipt.isolation_helper.digest != *plan.isolation_helper_digest()
        || receipt.isolation_helper.byte_size != plan.isolation_helper_bytes()
        || receipt.execution_timeout_seconds != execution.execution_timeout_seconds()
        || receipt.launch_digest != *plan.expected_launch_digest()
        || !matches!(receipt.network_access, NetworkPolicyWire::Denied)
        || !matches!(receipt.filesystem_policy, FilesystemPolicyWire::Landlock)
        || receipt.landlock_abi < 3
        || !receipt.canary.passed
        || receipt.canary.loopback_interface_index == 0
        || receipt.canary.protocol_version != 1
        || status_succeeded(&receipt.target_status) != Some(true)
        || status_succeeded(&receipt.guardian_helper_status) != Some(true)
        || !valid_identities(receipt)
        || !policy_matches(&receipt.isolation_policy, execution)
    {
        return Err(RuntimeSourceBuildReportError::InvalidExecutionReceipt);
    }
    let expected_output = controlled_output_tree(output_tree)?;
    if receipt.output_tree.digest != expected_output.digest
        || receipt.output_tree.entry_count != expected_output.entry_count
        || receipt.output_tree.regular_file_count != expected_output.regular_file_count
        || receipt.output_tree.total_file_bytes != expected_output.total_file_bytes
        || receipt.isolation_evidence_digest != isolation_evidence_digest(receipt)
    {
        return Err(RuntimeSourceBuildReportError::InvalidExecutionReceipt);
    }
    Ok(())
}

fn policy_matches(wire: &IsolationPolicyWire, expected: RuntimeSourceBuildExecutionPolicy) -> bool {
    wire.startup_timeout_seconds == expected.startup_timeout_seconds()
        && wire.shutdown_timeout_seconds == expected.shutdown_timeout_seconds()
        && wire.maximum_arguments == expected.maximum_arguments()
        && wire.maximum_environment_variables == expected.maximum_environment_variables()
        && wire.maximum_value_bytes == expected.maximum_value_bytes()
        && wire.maximum_open_files == expected.maximum_open_files()
        && wire.maximum_processes == expected.maximum_processes()
        && wire.digest == expected.isolation_policy_digest()
}

fn status_succeeded(status: &ProcessStatusWire) -> Option<bool> {
    match status {
        ProcessStatusWire::ExitCode { value } if *value >= 0 => Some(*value == 0),
        ProcessStatusWire::Signal { value } if (1..=64).contains(value) => Some(false),
        ProcessStatusWire::ExitCode { .. } | ProcessStatusWire::Signal { .. } => None,
    }
}

fn valid_identities(receipt: &ReceiptWire) -> bool {
    let identities = [
        receipt.namespaces.network,
        receipt.namespaces.user,
        receipt.namespaces.process,
        receipt.namespaces.mount,
        receipt.roots.input,
        receipt.roots.output,
    ];
    receipt.guardian_pid > 0
        && receipt.namespace_init_pid > 0
        && receipt.guardian_pid != receipt.namespace_init_pid
        && receipt.build_program.device.is_some_and(|value| value > 0)
        && receipt.build_program.inode.is_some_and(|value| value > 0)
        && receipt.isolation_helper.device.is_none()
        && receipt.isolation_helper.inode.is_none()
        && identities
            .iter()
            .all(|identity| identity.device > 0 && identity.inode > 0)
        && (receipt.roots.input.device != receipt.roots.output.device
            || receipt.roots.input.inode != receipt.roots.output.inode)
}

fn parse_path(value: &str) -> Result<ArtifactSetRelativePath, RuntimeSourceBuildReportError> {
    ArtifactSetRelativePath::new(value.to_owned())
        .map_err(|_| RuntimeSourceBuildReportError::InvalidExecutionReceipt)
}

struct OutputCommitment {
    digest: Digest,
    entry_count: u32,
    regular_file_count: u32,
    total_file_bytes: u64,
}

fn controlled_output_tree(
    tree: &RuntimeSourceBuildOutputTree,
) -> Result<OutputCommitment, RuntimeSourceBuildReportError> {
    let mut digest = RedactedDigest::new(b"controlled-build-output-tree/v1");
    digest.push_u64(u64::try_from(tree.entries().len()).unwrap_or(u64::MAX));
    let mut regular_file_count = 0_u32;
    let mut total_file_bytes = 0_u64;
    for entry in tree.entries() {
        digest.push_bytes(entry.relative_path().as_str().as_bytes());
        digest.push_u32(entry.unix_mode());
        match entry.kind() {
            RuntimeSourceBuildOutputTreeEntryKind::Directory => digest.push_u8(0),
            RuntimeSourceBuildOutputTreeEntryKind::RegularFile => {
                digest.push_u8(1);
                let bytes = entry
                    .byte_size()
                    .ok_or(RuntimeSourceBuildReportError::InvalidOutputTree)?;
                let file_digest = entry
                    .digest()
                    .ok_or(RuntimeSourceBuildReportError::InvalidOutputTree)?;
                digest.push_u64(bytes);
                digest.push_bytes(file_digest.as_str().as_bytes());
                regular_file_count = regular_file_count
                    .checked_add(1)
                    .ok_or(RuntimeSourceBuildReportError::InvalidOutputTree)?;
                total_file_bytes = total_file_bytes
                    .checked_add(bytes)
                    .ok_or(RuntimeSourceBuildReportError::InvalidOutputTree)?;
            }
        }
    }
    Ok(OutputCommitment {
        digest: digest.finish(),
        entry_count: u32::try_from(tree.entries().len())
            .map_err(|_| RuntimeSourceBuildReportError::InvalidOutputTree)?,
        regular_file_count,
        total_file_bytes,
    })
}

fn isolation_evidence_digest(receipt: &ReceiptWire) -> Digest {
    let mut digest = RedactedDigest::new(b"runtime-isolation/controlled-build-evidence/v3");
    digest.push_u32(receipt.guardian_pid);
    digest.push_u32(receipt.namespace_init_pid);
    for identity in [
        receipt.namespaces.network,
        receipt.namespaces.user,
        receipt.namespaces.process,
        receipt.namespaces.mount,
    ] {
        digest.push_u64(identity.device);
        digest.push_u64(identity.inode);
    }
    digest.push_bytes(receipt.isolation_helper.digest.as_str().as_bytes());
    digest.push_u64(receipt.isolation_helper.byte_size);
    digest.push_u32(receipt.canary.loopback_interface_index);
    digest.push_u8(receipt.canary.protocol_version);
    digest.push_bytes(receipt.isolation_policy.digest.as_str().as_bytes());
    digest.push_u32(receipt.landlock_abi);
    digest.push_bytes(receipt.build_program.digest.as_str().as_bytes());
    digest.push_u64(receipt.build_program.byte_size);
    for value in [
        receipt.build_program.device.unwrap_or_default(),
        receipt.build_program.inode.unwrap_or_default(),
        receipt.roots.input.device,
        receipt.roots.input.inode,
        receipt.roots.output.device,
        receipt.roots.output.inode,
    ] {
        digest.push_u64(value);
    }
    digest.push_bytes(receipt.launch_digest.as_str().as_bytes());
    digest.finish()
}

struct RedactedDigest {
    bytes: Vec<u8>,
}

#[cfg(test)]
pub(super) mod tests;

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

    fn push_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn push_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn push_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn finish(self) -> Digest {
        Digest::sha256(&self.bytes)
    }
}
