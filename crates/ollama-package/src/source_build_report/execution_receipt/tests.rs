use serde_json::{Value, json};

use super::{
    ReceiptWire, controlled_output_tree, isolation_evidence_digest,
    verify_runtime_source_build_execution_receipt,
};
use crate::{
    RuntimeSourceBuildAttempt, RuntimeSourceBuildInputManifest, RuntimeSourceBuildOutputTree,
    RuntimeSourceBuildPlan,
};
use rewrite_types::Digest;

pub(crate) fn fixture_receipt(
    attempt: RuntimeSourceBuildAttempt,
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    output_tree: &RuntimeSourceBuildOutputTree,
) -> Vec<u8> {
    let execution = plan.execution_policy();
    let output = controlled_output_tree(output_tree).expect("fixture output commitment");
    let mut value = json!({
        "attempt": match attempt {
            RuntimeSourceBuildAttempt::Primary => "primary",
            RuntimeSourceBuildAttempt::Rebuild => "rebuild"
        },
        "build_plan_digest": plan.plan_digest(),
        "build_program": {
            "byte_size": plan.build_program_bytes(),
            "device": 31,
            "digest": plan.build_program_digest(),
            "inode": 37,
            "relative_path": plan.build_program_path()
        },
        "canary": {
            "loopback_interface_index": 1,
            "passed": true,
            "protocol_version": 1
        },
        "controlled_build_capability_abi": plan.controlled_build_capability_abi(),
        "execution_timeout_seconds": execution.execution_timeout_seconds(),
        "filesystem_policy": "landlock",
        "guardian_helper_status": {"kind": "exit_code", "value": 0},
        "guardian_pid": 41,
        "isolation_evidence_digest": Digest::sha256(b"placeholder"),
        "isolation_helper": {
            "byte_size": plan.isolation_helper_bytes(),
            "digest": plan.isolation_helper_digest(),
            "relative_path": plan.isolation_helper_path()
        },
        "isolation_policy": {
            "digest": execution.isolation_policy_digest(),
            "maximum_arguments": execution.maximum_arguments(),
            "maximum_environment_variables": execution.maximum_environment_variables(),
            "maximum_open_files": execution.maximum_open_files(),
            "maximum_processes": execution.maximum_processes(),
            "maximum_value_bytes": execution.maximum_value_bytes(),
            "shutdown_timeout_seconds": execution.shutdown_timeout_seconds(),
            "startup_timeout_seconds": execution.startup_timeout_seconds()
        },
        "landlock_abi": 3,
        "launch_digest": plan.expected_launch_digest(),
        "namespace_init_pid": 43,
        "namespaces": {
            "mount": {"device": 47, "inode": 53},
            "network": {"device": 59, "inode": 61},
            "process": {"device": 67, "inode": 71},
            "user": {"device": 73, "inode": 79}
        },
        "network_access": "denied",
        "output_tree": {
            "digest": output.digest,
            "entry_count": output.entry_count,
            "regular_file_count": output.regular_file_count,
            "total_file_bytes": output.total_file_bytes
        },
        "roots": {
            "input": {"device": 83, "inode": 89},
            "output": {"device": 83, "inode": 97}
        },
        "schema_version": 1,
        "source_build_inputs_id": plan.source_inputs_id(),
        "source_manifest_digest": plan.source_manifest_digest(),
        "target": plan.target(),
        "target_status": {"kind": "exit_code", "value": 0}
    });
    let wire: ReceiptWire = serde_json::from_value(value.clone()).expect("fixture receipt wire");
    value["isolation_evidence_digest"] =
        serde_json::to_value(isolation_evidence_digest(&wire)).expect("digest value");
    let bytes = serde_json::to_vec(&value).expect("canonical fixture receipt");
    verify_runtime_source_build_execution_receipt(
        &bytes,
        attempt,
        source_inputs,
        plan,
        output_tree,
    )
    .expect("fixture receipt verifies");
    bytes
}

pub(crate) fn mutate(bytes: &[u8], path: &[&str], value: Value) -> Vec<u8> {
    let mut receipt: Value = serde_json::from_slice(bytes).expect("receipt JSON");
    let mut selected = &mut receipt;
    for component in path {
        selected = selected.get_mut(*component).expect("receipt component");
    }
    *selected = value;
    serde_json::to_vec(&receipt).expect("canonical mutated receipt")
}
