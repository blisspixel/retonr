use rewrite_ollama_package::{
    RuntimeSourceBuildInputRole, VerifiedCargoSourceClosure,
    VerifiedRetainedProgramLicenseEvidenceClosure, VerifiedRetainedProgramUpstreamClosure,
};
use rewrite_runtime_isolation::{
    ControlledBuildOutputTree, ControlledBuildOutputTreeEntry, ControlledBuildProcessStatus,
    RetainedProgramBootstrapAttempt, RetainedProgramBootstrapExecution,
};
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use super::{
    RetainedProgramExecutableClosureError, RetainedProgramExecutableClosureId,
    RuntimeSourceBuildBundleLease,
};

const CLOSURE_DOMAIN: &[u8] = b"runtime-source-build/retained-program-executable-closure/v1";
const PROGRAM_OUTPUTS: [(RuntimeSourceBuildInputRole, &str); 4] = [
    (
        RuntimeSourceBuildInputRole::IsolationHelper,
        "rewrite-runtime-isolation-helper",
    ),
    (
        RuntimeSourceBuildInputRole::SourceArchivePreparationTool,
        "rewrite-runtime-source-archive",
    ),
    (
        RuntimeSourceBuildInputRole::BuildScript,
        "rewrite-runtime-source-builder",
    ),
    (
        RuntimeSourceBuildInputRole::SourceManifestPreparationTool,
        "rewrite-runtime-source-manifest",
    ),
];

pub(super) fn expected_program_tree(
    bundle: &RuntimeSourceBuildBundleLease,
) -> Result<ControlledBuildOutputTree, RetainedProgramExecutableClosureError> {
    let mut entries = Vec::with_capacity(PROGRAM_OUTPUTS.len());
    for (role, output_path) in PROGRAM_OUTPUTS {
        let mut matches = bundle
            .inputs()
            .manifest()
            .components()
            .iter()
            .filter(|component| component.roles().contains(&role));
        let component = matches
            .next()
            .ok_or(RetainedProgramExecutableClosureError::ClosureMismatch)?;
        if matches.next().is_some() {
            return Err(RetainedProgramExecutableClosureError::ClosureMismatch);
        }
        entries.push(ControlledBuildOutputTreeEntry::regular_file(
            output_path,
            component.byte_size(),
            component.digest().clone(),
            0o755,
        )?);
    }
    ControlledBuildOutputTree::compile(entries).map_err(RetainedProgramExecutableClosureError::from)
}

pub(super) fn validate_live_pair(
    primary: &RetainedProgramBootstrapExecution,
    rebuild: &RetainedProgramBootstrapExecution,
    expected_tree: &ControlledBuildOutputTree,
) -> Result<(), RetainedProgramExecutableClosureError> {
    let primary_root = primary.root_evidence();
    let rebuild_root = rebuild.root_evidence();
    let primary_build = primary.controlled_build();
    let rebuild_build = rebuild.controlled_build();
    if primary.attempt() != RetainedProgramBootstrapAttempt::Primary
        || rebuild.attempt() != RetainedProgramBootstrapAttempt::Rebuild
        || primary.launch_digest() == rebuild.launch_digest()
        || !primary_root.all_canaries_passed()
        || !rebuild_root.all_canaries_passed()
        || primary_root.alpine_installed_database_digest()
            != rebuild_root.alpine_installed_database_digest()
        || primary_root.alpine_loader_digest() != rebuild_root.alpine_loader_digest()
        || primary_root.alpine_libc_linker_name_digest()
            != rebuild_root.alpine_libc_linker_name_digest()
        || primary_root.alpine_libgcc_digest() != rebuild_root.alpine_libgcc_digest()
        || primary_root.alpine_libgcc_linker_name_digest()
            != rebuild_root.alpine_libgcc_linker_name_digest()
        || primary_root.alpine_busybox_digest() != rebuild_root.alpine_busybox_digest()
        || primary_root.normalized_alpine_link_plan_digest()
            != rebuild_root.normalized_alpine_link_plan_digest()
        || primary_root.rust_toolchain_layout_digest()
            != rebuild_root.rust_toolchain_layout_digest()
        || primary_build.output().status() != ControlledBuildProcessStatus::Success
        || rebuild_build.output().status() != ControlledBuildProcessStatus::Success
        || primary_build.guardian_helper_status() != ControlledBuildProcessStatus::Success
        || rebuild_build.guardian_helper_status() != ControlledBuildProcessStatus::Success
        || primary_build.output().streams().standard_output_truncated()
        || primary_build.output().streams().standard_error_truncated()
        || rebuild_build.output().streams().standard_output_truncated()
        || rebuild_build.output().streams().standard_error_truncated()
        || primary_build.isolation().landlock_abi() < 3
        || rebuild_build.isolation().landlock_abi() < 3
        || primary_build.output().tree() != Some(expected_tree)
        || rebuild_build.output().tree() != Some(expected_tree)
    {
        return Err(RetainedProgramExecutableClosureError::ClosureMismatch);
    }
    Ok(())
}

pub(super) fn compile_closure_id(
    bundle: &RuntimeSourceBuildBundleLease,
    cargo: &VerifiedCargoSourceClosure,
    license: &VerifiedRetainedProgramLicenseEvidenceClosure,
    upstream: &VerifiedRetainedProgramUpstreamClosure,
    primary: &RetainedProgramBootstrapExecution,
    rebuild: &RetainedProgramBootstrapExecution,
    expected_tree: &ControlledBuildOutputTree,
) -> Result<RetainedProgramExecutableClosureId, RetainedProgramExecutableClosureError> {
    let source_set = bundle.inputs().manifest().artifact_set().artifact_set_id();
    if cargo.source_input_set_id() != source_set.digest()
        || license.source_input_set_id() != source_set.digest()
        || upstream.source_input_set_id() != &source_set
        || license.cargo_source_closure_id() != cargo.closure_id()
        || license.upstream_closure_id() != upstream.closure_id().digest()
    {
        return Err(RetainedProgramExecutableClosureError::ClosureMismatch);
    }
    let lineage = bundle
        .inputs()
        .retained_program_lineage()
        .ok_or(RetainedProgramExecutableClosureError::ClosureMismatch)?;
    Ok(RetainedProgramExecutableClosureId(sequence_digest([
        source_set.digest(),
        lineage.recipe_id().digest(),
        lineage.lineage_id().digest(),
        cargo.closure_id(),
        license.closure_id().digest(),
        upstream.closure_id().digest(),
        expected_tree.digest(),
        &primary.redacted_digest(),
        &rebuild.redacted_digest(),
    ])))
}

fn sequence_digest<'a>(values: impl IntoIterator<Item = &'a Digest>) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(CLOSURE_DOMAIN.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(CLOSURE_DOMAIN);
    for value in values {
        let bytes = value.as_str().as_bytes();
        hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(bytes);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 output is a valid digest")
}
