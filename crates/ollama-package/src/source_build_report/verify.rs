use std::io::Read;

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use sha2::{Digest as _, Sha256};

use crate::{
    MemberOpenError, ReconstructedRuntimePackage, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildPlan, reconstruct_runtime_package_with_limits,
};

use super::{
    BuildAttempt, RuntimeSourceBuildAttempt, RuntimeSourceBuildComparison,
    RuntimeSourceBuildEvidenceKind, RuntimeSourceBuildOutputTree, RuntimeSourceBuildReport,
    RuntimeSourceBuildReportError, RuntimeSourceBuildReportLimits,
    RuntimeSourceBuildReportOpenError, VerifiedRuntimeSourceBuildReport, parse::parse_report,
    validates_transformed_runtime_binding, verify_runtime_source_build_execution_receipt,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Verifies both controlled-build attempts and derives their exact comparison.
///
/// Evidence and member openers receive only validated portable paths plus the
/// fixed attempt identity. This crate performs no filesystem or network discovery.
///
/// # Errors
///
/// Returns [`RuntimeSourceBuildReportError`] for malformed, inconsistent,
/// unavailable, changed, unreadable, excessive, or cancelled report input.
pub fn verify_runtime_source_build_report<ER, EF, MR, MF, C>(
    report_bytes: &[u8],
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    limits: &RuntimeSourceBuildReportLimits,
    mut open_evidence: EF,
    mut open_member: MF,
    mut cancelled: C,
) -> Result<VerifiedRuntimeSourceBuildReport, RuntimeSourceBuildReportError>
where
    ER: Read,
    EF: FnMut(
        RuntimeSourceBuildAttempt,
        &ArtifactSetRelativePath,
    ) -> Result<ER, RuntimeSourceBuildReportOpenError>,
    MR: Read,
    MF: FnMut(RuntimeSourceBuildAttempt, &ArtifactSetRelativePath) -> Result<MR, MemberOpenError>,
    C: FnMut() -> bool,
{
    let limits = limits.validate()?;
    let report = parse_report(report_bytes, limits)?;
    if report.source_build_inputs_id != source_inputs.artifact_set().artifact_set_id()
        || report.source_build_inputs_id != *plan.source_inputs_id()
        || source_inputs.manifest_digest() != plan.source_manifest_digest()
        || report.build_plan_digest != *plan.plan_digest()
    {
        return Err(RuntimeSourceBuildReportError::InvalidPlanBinding);
    }
    let primary = verify_attempt(
        &report.attempts[0],
        source_inputs,
        plan,
        limits,
        &mut open_evidence,
        &mut open_member,
        &mut cancelled,
    )?;
    let rebuild = verify_attempt(
        &report.attempts[1],
        source_inputs,
        plan,
        limits,
        &mut open_evidence,
        &mut open_member,
        &mut cancelled,
    )?;
    verify_comparison(&report, &primary, &rebuild)?;
    Ok(VerifiedRuntimeSourceBuildReport {
        report,
        primary: primary.runtime,
        rebuild: rebuild.runtime,
        primary_output_tree: primary.output_tree,
        rebuild_output_tree: rebuild.output_tree,
    })
}

struct VerifiedAttempt {
    runtime: ReconstructedRuntimePackage,
    output_tree: RuntimeSourceBuildOutputTree,
}

fn verify_attempt<ER, EF, MR, MF, C>(
    attempt: &BuildAttempt,
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    limits: RuntimeSourceBuildReportLimits,
    open_evidence: &mut EF,
    open_member: &mut MF,
    cancelled: &mut C,
) -> Result<VerifiedAttempt, RuntimeSourceBuildReportError>
where
    ER: Read,
    EF: FnMut(
        RuntimeSourceBuildAttempt,
        &ArtifactSetRelativePath,
    ) -> Result<ER, RuntimeSourceBuildReportOpenError>,
    MR: Read,
    MF: FnMut(RuntimeSourceBuildAttempt, &ArtifactSetRelativePath) -> Result<MR, MemberOpenError>,
    C: FnMut() -> bool,
{
    if &attempt.environment_digest != plan.policy().environment_digest()
        || &attempt.build_arguments_digest != plan.policy().build_arguments_digest()
    {
        return Err(RuntimeSourceBuildReportError::InvalidPlanBinding);
    }
    let mut layout_bytes = None;
    let mut output_tree_bytes = None;
    let mut execution_receipt_bytes = None;
    for evidence in &attempt.evidence {
        if cancelled() {
            return Err(RuntimeSourceBuildReportError::Cancelled);
        }
        let stream = open_evidence(attempt.attempt, &evidence.relative_path)
            .map_err(|_| RuntimeSourceBuildReportError::EvidenceUnavailable)?;
        let retain = matches!(
            evidence.kind,
            RuntimeSourceBuildEvidenceKind::RuntimeLayout
                | RuntimeSourceBuildEvidenceKind::OutputTree
                | RuntimeSourceBuildEvidenceKind::ExecutionReceipt
        );
        let bytes = verify_evidence(
            stream,
            evidence.byte_size,
            &evidence.digest,
            retain,
            cancelled,
        )?;
        if evidence.kind == RuntimeSourceBuildEvidenceKind::RuntimeLayout {
            layout_bytes.clone_from(&bytes);
        }
        if evidence.kind == RuntimeSourceBuildEvidenceKind::OutputTree {
            output_tree_bytes.clone_from(&bytes);
        }
        if evidence.kind == RuntimeSourceBuildEvidenceKind::ExecutionReceipt {
            execution_receipt_bytes = bytes;
        }
    }
    let layout_bytes = layout_bytes.ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?;
    let reconstructed = reconstruct_runtime_package_with_limits(
        &layout_bytes,
        &limits.runtime_layout,
        |path| open_member(attempt.attempt, path),
        cancelled,
    )
    .map_err(RuntimeSourceBuildReportError::RuntimeReconstruction)?;
    let output_tree = RuntimeSourceBuildOutputTree::parse(
        &output_tree_bytes.ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?,
    )?;
    output_tree.validate_binding(&attempt.evidence, &reconstructed)?;
    if output_tree.digest() != &attempt.output_tree_digest {
        return Err(RuntimeSourceBuildReportError::InvalidOutputTree);
    }
    verify_runtime_source_build_execution_receipt(
        &execution_receipt_bytes.ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?,
        attempt.attempt,
        source_inputs,
        plan,
        &output_tree,
    )?;
    let standard_output_digest = attempt
        .evidence
        .iter()
        .find(|item| item.kind == RuntimeSourceBuildEvidenceKind::StandardOutput)
        .map(|item| &item.digest);
    if reconstructed.layout().target() != plan.target()
        || !validates_transformed_runtime_binding(
            source_inputs,
            &reconstructed,
            standard_output_digest,
        )
    {
        return Err(RuntimeSourceBuildReportError::InvalidRuntimeBinding);
    }
    Ok(VerifiedAttempt {
        runtime: reconstructed,
        output_tree,
    })
}

fn verify_evidence<R, C>(
    mut stream: R,
    expected_size: u64,
    expected_digest: &Digest,
    retain: bool,
    cancelled: &mut C,
) -> Result<Option<Vec<u8>>, RuntimeSourceBuildReportError>
where
    R: Read,
    C: FnMut() -> bool,
{
    let capacity = if retain {
        usize::try_from(expected_size).map_err(|_| RuntimeSourceBuildReportError::LimitExceeded)?
    } else {
        0
    };
    let mut retained = retain.then(|| Vec::with_capacity(capacity));
    let mut hasher = Sha256::new();
    let mut remaining = expected_size;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(RuntimeSourceBuildReportError::Cancelled);
        }
        let read = stream
            .read(&mut buffer)
            .map_err(|_| RuntimeSourceBuildReportError::EvidenceRead)?;
        if read == 0 {
            break;
        }
        let read_bytes =
            u64::try_from(read).map_err(|_| RuntimeSourceBuildReportError::EvidenceRead)?;
        if read_bytes > remaining {
            return Err(RuntimeSourceBuildReportError::EvidenceSizeMismatch);
        }
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
        remaining -= read_bytes;
    }
    if remaining != 0 {
        return Err(RuntimeSourceBuildReportError::EvidenceSizeMismatch);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeSourceBuildReportError::EvidenceRead)?;
    if &digest != expected_digest {
        return Err(RuntimeSourceBuildReportError::EvidenceDigestMismatch);
    }
    Ok(retained)
}

fn verify_comparison(
    report: &RuntimeSourceBuildReport,
    primary: &VerifiedAttempt,
    rebuild: &VerifiedAttempt,
) -> Result<(), RuntimeSourceBuildReportError> {
    let primary_set = primary.runtime.artifact_set().artifact_set_id();
    let rebuild_set = rebuild.runtime.artifact_set().artifact_set_id();
    let primary_package = primary
        .runtime
        .runtime_package()
        .runtime_package_manifest_id();
    let rebuild_package = rebuild
        .runtime
        .runtime_package()
        .runtime_package_manifest_id();
    let primary_output_tree = primary.output_tree.digest();
    let rebuild_output_tree = rebuild.output_tree.digest();
    let matches = match &report.comparison {
        RuntimeSourceBuildComparison::ByteIdentical {
            artifact_set_id,
            runtime_package_manifest_id,
            output_tree_digest,
        } => {
            primary_set == rebuild_set
                && primary_package == rebuild_package
                && primary_output_tree == rebuild_output_tree
                && artifact_set_id == &primary_set
                && runtime_package_manifest_id == &primary_package
                && output_tree_digest == primary_output_tree
        }
        RuntimeSourceBuildComparison::Different {
            primary_artifact_set_id,
            rebuild_artifact_set_id,
            primary_runtime_package_manifest_id,
            rebuild_runtime_package_manifest_id,
            primary_output_tree_digest,
            rebuild_output_tree_digest,
        } => {
            (primary_set != rebuild_set
                || primary_package != rebuild_package
                || primary_output_tree != rebuild_output_tree)
                && primary_artifact_set_id == &primary_set
                && rebuild_artifact_set_id == &rebuild_set
                && primary_runtime_package_manifest_id == &primary_package
                && rebuild_runtime_package_manifest_id == &rebuild_package
                && primary_output_tree_digest == primary_output_tree
                && rebuild_output_tree_digest == rebuild_output_tree
        }
    };
    if matches {
        Ok(())
    } else {
        Err(RuntimeSourceBuildReportError::InvalidComparison)
    }
}
