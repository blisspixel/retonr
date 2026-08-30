use std::{fs::File, os::unix::fs::FileExt as _, time::Instant};

use rewrite_types::{CancellationToken, Digest as DomainDigest};
use sha2::{Digest as _, Sha256};

use crate::{
    ControlledBuildInputFile, IsolationError, IsolationPolicy, IsolationPreparationEvidence,
    IsolationResult, RetainedProgramBootstrapCapabilities, RetainedProgramBootstrapExecution,
    RetainedProgramBootstrapLaunchSpec, RetainedProgramBootstrapRootEvidence,
    contract::CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT,
};

use super::{
    linux_bootstrap_execution, linux_bootstrap_protocol,
    linux_build::{self, BuildRequest},
    linux_build_protocol::InputDeclaration,
    linux_helper_identity::validate_executable,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(super) fn run(
    helper: &File,
    preparation: &IsolationPreparationEvidence,
    specification: &RetainedProgramBootstrapLaunchSpec,
    capabilities: RetainedProgramBootstrapCapabilities,
    policy: IsolationPolicy,
    cancellation: &CancellationToken,
) -> IsolationResult<RetainedProgramBootstrapExecution> {
    ensure_active(cancellation)?;
    validate_executable(helper, true)?;
    specification.validate_input_files(capabilities.input_files())?;
    let (mut program, input_root, input_files, output_root) = capabilities.into_parts();
    let mut request = BuildRequest {
        helper,
        preparation,
        specification: specification.controlled_build(),
        program: &mut program,
        input_root: &input_root,
        input_files: &input_files,
        output_root: &output_root,
        policy,
        cancellation,
    };
    let metadata = linux_build::validate_retained_request(&mut request)?;
    require_empty_directory(&output_root)?;
    verify_bootstrap_measurements(specification, &input_files, cancellation)?;
    let encoded = linux_bootstrap_protocol::encode(specification, input_files.len())
        .ok_or(IsolationError::InvalidBootstrap("protocol request"))?;
    let decoded = linux_bootstrap_protocol::decode(&encoded)
        .ok_or(IsolationError::InvalidBootstrap("protocol request"))?;
    let declarations = declarations(&input_files);
    if !decoded.joins(&declarations) {
        return Err(IsolationError::BootstrapRootVerification);
    }
    let (root_observation, controlled_build) =
        linux_bootstrap_execution::execute(&mut request, &metadata, &encoded)?;
    let root = RetainedProgramBootstrapRootEvidence::from_observation(root_observation);
    RetainedProgramBootstrapExecution::new(specification, root, controlled_build)
}

fn declarations(input_files: &[ControlledBuildInputFile]) -> Vec<InputDeclaration> {
    input_files
        .iter()
        .map(|input| InputDeclaration {
            relative_path: input.relative_path().to_owned(),
            expected_digest: input.expected_digest().clone(),
            expected_bytes: input.expected_bytes(),
        })
        .collect()
}

fn require_empty_directory(directory: &File) -> IsolationResult<()> {
    let entries = std::fs::read_dir(format!(
        "/proc/self/fd/{}",
        std::os::fd::AsRawFd::as_raw_fd(directory)
    ))
    .map_err(|_| IsolationError::ControlledBuildObjectMismatch)?;
    if entries.count() == 0 {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildOutputNotEmpty)
    }
}

fn verify_bootstrap_measurements(
    specification: &RetainedProgramBootstrapLaunchSpec,
    input_files: &[ControlledBuildInputFile],
    cancellation: &CancellationToken,
) -> IsolationResult<()> {
    let started = Instant::now();
    verify_one(
        input_files,
        "lineage/build-recipe-v2.json",
        specification.recipe_digest(),
        specification.recipe_bytes(),
        cancellation,
        started,
    )?;
    for expected in specification.signed_inputs().measurements() {
        verify_one(
            input_files,
            expected.relative_path(),
            expected.digest(),
            expected.byte_size(),
            cancellation,
            started,
        )?;
    }
    verify_one(
        input_files,
        specification.busybox_executable().relative_path(),
        specification.busybox_executable().digest(),
        specification.busybox_executable().byte_size(),
        cancellation,
        started,
    )?;
    Ok(())
}

fn verify_one(
    input_files: &[ControlledBuildInputFile],
    relative_path: &str,
    expected_digest: &DomainDigest,
    expected_bytes: u64,
    cancellation: &CancellationToken,
    started: Instant,
) -> IsolationResult<()> {
    let input = input_files
        .iter()
        .find(|input| input.relative_path() == relative_path)
        .ok_or(IsolationError::BootstrapRootVerification)?;
    let metadata = input
        .file()
        .metadata()
        .map_err(|_| IsolationError::BootstrapRootVerification)?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(IsolationError::BootstrapRootVerification);
    }
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    while offset < expected_bytes {
        require_measurement_active(cancellation, started)?;
        let remaining = usize::try_from(expected_bytes - offset)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = input
            .file()
            .read_at(&mut buffer[..remaining], offset)
            .map_err(|_| IsolationError::BootstrapRootVerification)?;
        if read == 0 {
            return Err(IsolationError::BootstrapRootVerification);
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(
                u64::try_from(read).map_err(|_| IsolationError::BootstrapRootVerification)?,
            )
            .ok_or(IsolationError::BootstrapRootVerification)?;
    }
    let observed = DomainDigest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| IsolationError::BootstrapRootVerification)?;
    if &observed == expected_digest {
        Ok(())
    } else {
        Err(IsolationError::BootstrapRootVerification)
    }
}

fn require_measurement_active(
    cancellation: &CancellationToken,
    started: Instant,
) -> IsolationResult<()> {
    ensure_active(cancellation)?;
    if started.elapsed() < CONTROLLED_BUILD_INPUT_SNAPSHOT_TIMEOUT {
        Ok(())
    } else {
        Err(IsolationError::ControlledBuildSnapshotTimeout)
    }
}

fn ensure_active(cancellation: &CancellationToken) -> IsolationResult<()> {
    if cancellation.is_cancelled() {
        Err(IsolationError::Cancelled)
    } else {
        Ok(())
    }
}
