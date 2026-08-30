use rewrite_ollama_package::RuntimeSourceBuildAttempt;
use rewrite_runtime_isolation::ControlledBuildProcessStatus;
use serde_json::json;

use crate::{ExecutableRuntimeSourceBuildBundleLease, RuntimeSourceBuildExecution};

use super::RuntimeSourceBuildReportCompilationError;

pub(super) fn compile(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    execution: &RuntimeSourceBuildExecution,
    attempt: RuntimeSourceBuildAttempt,
) -> Result<Vec<u8>, RuntimeSourceBuildReportCompilationError> {
    let evidence = execution.managed().isolation();
    let preparation = evidence.preparation();
    let network = evidence.network_namespace();
    let user = evidence.user_namespace();
    let process = evidence.process_namespace();
    let mount = evidence.mount_namespace();
    let plan = bundle.plan();
    let execution_policy = plan.execution_policy();
    let helper_tree = execution
        .managed()
        .output()
        .tree()
        .ok_or(RuntimeSourceBuildReportCompilationError::OutputChanged)?;
    let (program_device, program_inode) = evidence.program_object_identity();
    let (input_device, input_inode) = evidence.input_root_identity();
    let (output_device, output_inode) = evidence.output_root_identity();
    serde_json::to_vec(&json!({
        "attempt": match attempt {
            RuntimeSourceBuildAttempt::Primary => "primary",
            RuntimeSourceBuildAttempt::Rebuild => "rebuild"
        },
        "build_plan_digest": plan.plan_digest(),
        "build_program": {
            "byte_size": evidence.program_bytes(),
            "device": program_device,
            "digest": evidence.program_digest(),
            "inode": program_inode,
            "relative_path": plan.build_program_path()
        },
        "canary": {
            "loopback_interface_index": preparation.loopback_interface_index(),
            "passed": preparation.all_canaries_passed(),
            "protocol_version": preparation.canary_protocol_version()
        },
        "controlled_build_capability_abi": plan.controlled_build_capability_abi(),
        "execution_timeout_seconds": execution_policy.execution_timeout_seconds(),
        "filesystem_policy": "landlock",
        "guardian_helper_status": process_status(execution.managed().guardian_helper_status()),
        "guardian_pid": evidence.guardian_pid(),
        "isolation_evidence_digest": evidence.redacted_digest(),
        "isolation_helper": {
            "byte_size": preparation.helper_bytes(),
            "digest": preparation.helper_digest(),
            "relative_path": plan.isolation_helper_path()
        },
        "isolation_policy": {
            "digest": evidence.isolation_policy_digest(),
            "maximum_arguments": execution_policy.maximum_arguments(),
            "maximum_environment_variables": execution_policy.maximum_environment_variables(),
            "maximum_open_files": execution_policy.maximum_open_files(),
            "maximum_processes": execution_policy.maximum_processes(),
            "maximum_value_bytes": execution_policy.maximum_value_bytes(),
            "shutdown_timeout_seconds": execution_policy.shutdown_timeout_seconds(),
            "startup_timeout_seconds": execution_policy.startup_timeout_seconds()
        },
        "landlock_abi": evidence.landlock_abi(),
        "launch_digest": evidence.launch_digest(),
        "namespace_init_pid": evidence.namespace_init_pid(),
        "namespaces": {
            "mount": {"device": mount.device(), "inode": mount.inode()},
            "network": {"device": network.device(), "inode": network.inode()},
            "process": {"device": process.device(), "inode": process.inode()},
            "user": {"device": user.device(), "inode": user.inode()}
        },
        "network_access": "denied",
        "output_tree": {
            "digest": helper_tree.digest(),
            "entry_count": helper_tree.entry_count(),
            "regular_file_count": helper_tree.regular_file_count(),
            "total_file_bytes": helper_tree.total_file_bytes()
        },
        "roots": {
            "input": {"device": input_device, "inode": input_inode},
            "output": {"device": output_device, "inode": output_inode}
        },
        "schema_version": 1,
        "source_build_inputs_id": plan.source_inputs_id(),
        "source_manifest_digest": plan.source_manifest_digest(),
        "target": plan.target(),
        "target_status": process_status(execution.managed().output().status())
    }))
    .map_err(|_| RuntimeSourceBuildReportCompilationError::EvidenceEncoding)
}

fn process_status(status: ControlledBuildProcessStatus) -> serde_json::Value {
    match status {
        ControlledBuildProcessStatus::Success => json!({"kind": "exit_code", "value": 0}),
        ControlledBuildProcessStatus::ExitCode(value) => {
            json!({"kind": "exit_code", "value": value})
        }
        ControlledBuildProcessStatus::Signal(value) => json!({"kind": "signal", "value": value}),
    }
}
