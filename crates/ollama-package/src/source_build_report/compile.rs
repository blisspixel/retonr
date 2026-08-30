use std::io::Read;

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::{
    MemberOpenError, ReconstructedRuntimePackage, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildPlan, reconstruct_runtime_package_with_limits,
};

use super::{
    CompiledRuntimeSourceBuildReport, RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS,
    RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION, RuntimeSourceBuildAttempt,
    RuntimeSourceBuildEvidenceKind, RuntimeSourceBuildEvidenceRecord, RuntimeSourceBuildOutputTree,
    RuntimeSourceBuildReportError, RuntimeSourceBuildReportLimits,
    RuntimeSourceBuildReportOpenError, attempt_name, canonical_evidence_path,
    validates_transformed_runtime_binding, verify_runtime_source_build_execution_receipt,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Derives one canonical two-attempt report from exact evidence and runtime bytes.
///
/// The openers receive only fixed validated report paths and attempt identities.
/// This function hashes every evidence stream, reconstructs both runtime packages,
/// derives their comparison, and serializes no caller-local path or component bytes.
///
/// # Errors
///
/// Returns [`RuntimeSourceBuildReportError`] for invalid limits or plan binding,
/// unavailable or unreadable evidence, excessive input, cancellation, invalid
/// runtime output, or canonical serialization failure.
pub fn compile_runtime_source_build_report<ER, EF, MR, MF, C>(
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    limits: &RuntimeSourceBuildReportLimits,
    mut open_evidence: EF,
    mut open_member: MF,
    mut cancelled: C,
) -> Result<CompiledRuntimeSourceBuildReport, RuntimeSourceBuildReportError>
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
    if source_inputs.artifact_set().artifact_set_id() != *plan.source_inputs_id()
        || source_inputs.manifest_digest() != plan.source_manifest_digest()
    {
        return Err(RuntimeSourceBuildReportError::InvalidPlanBinding);
    }
    let mut compiler = AttemptCompiler {
        source_inputs,
        plan,
        limits,
        total_evidence_bytes: 0,
        open_evidence: &mut open_evidence,
        open_member: &mut open_member,
        cancelled: &mut cancelled,
    };
    let primary = compile_attempt(RuntimeSourceBuildAttempt::Primary, &mut compiler)?;
    let rebuild = compile_attempt(RuntimeSourceBuildAttempt::Rebuild, &mut compiler)?;
    let comparison = comparison_value(
        &primary.runtime,
        &rebuild.runtime,
        &primary.output_tree,
        &rebuild.output_tree,
    );
    let attempts = [primary.value, rebuild.value];
    let bytes = serde_json::to_vec(&json!({
        "attempts": attempts,
        "build_plan_digest": plan.plan_digest(),
        "comparison": comparison,
        "schema_version": RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION,
        "source_build_inputs_id": plan.source_inputs_id()
    }))
    .map_err(|_| RuntimeSourceBuildReportError::InvalidEncoding)?;
    if bytes.len() > limits.report_bytes {
        Err(RuntimeSourceBuildReportError::LimitExceeded)
    } else {
        Ok(CompiledRuntimeSourceBuildReport {
            canonical_bytes: bytes,
            primary: primary.runtime,
            rebuild: rebuild.runtime,
            primary_output_tree: primary.output_tree,
            rebuild_output_tree: rebuild.output_tree,
        })
    }
}

struct CompiledAttempt {
    value: Value,
    runtime: ReconstructedRuntimePackage,
    output_tree: RuntimeSourceBuildOutputTree,
}

struct AttemptCompiler<'a, EF, MF, C> {
    source_inputs: &'a RuntimeSourceBuildInputManifest,
    plan: &'a RuntimeSourceBuildPlan,
    limits: RuntimeSourceBuildReportLimits,
    total_evidence_bytes: u64,
    open_evidence: &'a mut EF,
    open_member: &'a mut MF,
    cancelled: &'a mut C,
}

fn compile_attempt<ER, EF, MR, MF, C>(
    attempt: RuntimeSourceBuildAttempt,
    compiler: &mut AttemptCompiler<'_, EF, MF, C>,
) -> Result<CompiledAttempt, RuntimeSourceBuildReportError>
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
    let name = attempt_name(attempt);
    let mut retained = RetainedAttemptEvidence::default();
    let mut evidence_values = Vec::with_capacity(RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS.len());
    let mut evidence_records = Vec::with_capacity(RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS.len());
    for kind in RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS {
        if (compiler.cancelled)() {
            return Err(RuntimeSourceBuildReportError::Cancelled);
        }
        let path = canonical_evidence_path(attempt, kind)?;
        let stream = (compiler.open_evidence)(attempt, &path)
            .map_err(|_| RuntimeSourceBuildReportError::EvidenceUnavailable)?;
        let retain = matches!(
            kind,
            RuntimeSourceBuildEvidenceKind::RuntimeLayout
                | RuntimeSourceBuildEvidenceKind::OutputTree
                | RuntimeSourceBuildEvidenceKind::ExecutionReceipt
        );
        let measured = measure_evidence(
            stream,
            compiler.limits.maximum_evidence_bytes,
            compiler.limits.maximum_total_evidence_bytes,
            &mut compiler.total_evidence_bytes,
            retain,
            compiler.cancelled,
        )?;
        let permits_empty = matches!(
            kind,
            RuntimeSourceBuildEvidenceKind::StandardOutput
                | RuntimeSourceBuildEvidenceKind::StandardError
        );
        if !permits_empty && measured.byte_size == 0 {
            return Err(RuntimeSourceBuildReportError::LimitExceeded);
        }
        retained.record(kind, &measured);
        evidence_values.push(json!({
            "byte_size": measured.byte_size,
            "digest": measured.digest.clone(),
            "kind": evidence_kind_name(kind),
            "relative_path": path.as_str()
        }));
        evidence_records.push(RuntimeSourceBuildEvidenceRecord {
            kind,
            relative_path: path,
            byte_size: measured.byte_size,
            digest: measured.digest,
        });
    }
    let layout_bytes = retained
        .layout_bytes
        .ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?;
    let runtime = reconstruct_runtime_package_with_limits(
        &layout_bytes,
        &compiler.limits.runtime_layout,
        |path| (compiler.open_member)(attempt, path),
        &mut *compiler.cancelled,
    )
    .map_err(RuntimeSourceBuildReportError::RuntimeReconstruction)?;
    let output_tree = RuntimeSourceBuildOutputTree::parse(
        &retained
            .output_tree_bytes
            .ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?,
    )?;
    verify_receipt(
        retained.execution_receipt_bytes,
        attempt,
        compiler.source_inputs,
        compiler.plan,
        &output_tree,
    )?;
    output_tree.validate_binding(&evidence_records, &runtime)?;
    if runtime.layout().target() != compiler.plan.target()
        || !validates_transformed_runtime_binding(
            compiler.source_inputs,
            &runtime,
            retained.standard_output_digest.as_ref(),
        )
    {
        return Err(RuntimeSourceBuildReportError::InvalidRuntimeBinding);
    }
    Ok(CompiledAttempt {
        value: json!({
            "accelerator": "cpu_only",
            "attempt": name,
            "build_arguments_digest": compiler.plan.policy().build_arguments_digest(),
            "environment_digest": compiler.plan.policy().environment_digest(),
            "evidence": evidence_values,
            "execution_receipt_digest": retained.execution_receipt_digest
                .ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?,
            "network_access": "denied",
            "output_tree_digest": output_tree.digest()
        }),
        runtime,
        output_tree,
    })
}

#[derive(Default)]
struct RetainedAttemptEvidence {
    layout_bytes: Option<Vec<u8>>,
    output_tree_bytes: Option<Vec<u8>>,
    execution_receipt_bytes: Option<Vec<u8>>,
    execution_receipt_digest: Option<Digest>,
    standard_output_digest: Option<Digest>,
}

impl RetainedAttemptEvidence {
    fn record(&mut self, kind: RuntimeSourceBuildEvidenceKind, measured: &MeasuredEvidence) {
        match kind {
            RuntimeSourceBuildEvidenceKind::RuntimeLayout => {
                self.layout_bytes.clone_from(&measured.retained);
            }
            RuntimeSourceBuildEvidenceKind::OutputTree => {
                self.output_tree_bytes.clone_from(&measured.retained);
            }
            RuntimeSourceBuildEvidenceKind::ExecutionReceipt => {
                self.execution_receipt_bytes.clone_from(&measured.retained);
                self.execution_receipt_digest = Some(measured.digest.clone());
            }
            RuntimeSourceBuildEvidenceKind::StandardOutput => {
                self.standard_output_digest = Some(measured.digest.clone());
            }
            RuntimeSourceBuildEvidenceKind::Sbom
            | RuntimeSourceBuildEvidenceKind::Provenance
            | RuntimeSourceBuildEvidenceKind::Transformation
            | RuntimeSourceBuildEvidenceKind::StandardError => {}
        }
    }
}

fn verify_receipt(
    bytes: Option<Vec<u8>>,
    attempt: RuntimeSourceBuildAttempt,
    source_inputs: &RuntimeSourceBuildInputManifest,
    plan: &RuntimeSourceBuildPlan,
    output_tree: &RuntimeSourceBuildOutputTree,
) -> Result<(), RuntimeSourceBuildReportError> {
    verify_runtime_source_build_execution_receipt(
        &bytes.ok_or(RuntimeSourceBuildReportError::InvalidAttempt)?,
        attempt,
        source_inputs,
        plan,
        output_tree,
    )?;
    Ok(())
}

struct MeasuredEvidence {
    byte_size: u64,
    digest: Digest,
    retained: Option<Vec<u8>>,
}

fn measure_evidence<R, C>(
    mut stream: R,
    maximum_evidence_bytes: u64,
    maximum_total_evidence_bytes: u64,
    total_evidence_bytes: &mut u64,
    retain: bool,
    cancelled: &mut C,
) -> Result<MeasuredEvidence, RuntimeSourceBuildReportError>
where
    R: Read,
    C: FnMut() -> bool,
{
    let aggregate_remaining = maximum_total_evidence_bytes
        .checked_sub(*total_evidence_bytes)
        .ok_or(RuntimeSourceBuildReportError::LimitExceeded)?;
    let maximum = maximum_evidence_bytes.min(aggregate_remaining);
    let mut retained = retain.then(Vec::new);
    let mut hasher = Sha256::new();
    let mut byte_size = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(RuntimeSourceBuildReportError::Cancelled);
        }
        let remaining = maximum.saturating_sub(byte_size);
        let read_limit = usize::try_from(remaining.saturating_add(1))
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = stream
            .read(&mut buffer[..read_limit])
            .map_err(|_| RuntimeSourceBuildReportError::EvidenceRead)?;
        if read == 0 {
            break;
        }
        byte_size = byte_size
            .checked_add(
                u64::try_from(read).map_err(|_| RuntimeSourceBuildReportError::LimitExceeded)?,
            )
            .ok_or(RuntimeSourceBuildReportError::LimitExceeded)?;
        if byte_size > maximum {
            return Err(RuntimeSourceBuildReportError::LimitExceeded);
        }
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    *total_evidence_bytes = total_evidence_bytes
        .checked_add(byte_size)
        .ok_or(RuntimeSourceBuildReportError::LimitExceeded)?;
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimeSourceBuildReportError::EvidenceRead)?;
    Ok(MeasuredEvidence {
        byte_size,
        digest,
        retained,
    })
}

fn comparison_value(
    primary: &ReconstructedRuntimePackage,
    rebuild: &ReconstructedRuntimePackage,
    primary_output_tree: &RuntimeSourceBuildOutputTree,
    rebuild_output_tree: &RuntimeSourceBuildOutputTree,
) -> Value {
    let primary_set = primary.artifact_set().artifact_set_id();
    let rebuild_set = rebuild.artifact_set().artifact_set_id();
    let primary_package = primary.runtime_package().runtime_package_manifest_id();
    let rebuild_package = rebuild.runtime_package().runtime_package_manifest_id();
    if primary_set == rebuild_set
        && primary_package == rebuild_package
        && primary_output_tree == rebuild_output_tree
    {
        json!({
            "artifact_set_id": primary_set,
            "output_tree_digest": primary_output_tree.digest(),
            "runtime_package_manifest_id": primary_package,
            "status": "byte_identical"
        })
    } else {
        json!({
            "primary_artifact_set_id": primary_set,
            "primary_output_tree_digest": primary_output_tree.digest(),
            "primary_runtime_package_manifest_id": primary_package,
            "rebuild_artifact_set_id": rebuild_set,
            "rebuild_output_tree_digest": rebuild_output_tree.digest(),
            "rebuild_runtime_package_manifest_id": rebuild_package,
            "status": "different"
        })
    }
}

const fn evidence_kind_name(kind: RuntimeSourceBuildEvidenceKind) -> &'static str {
    match kind {
        RuntimeSourceBuildEvidenceKind::RuntimeLayout => "runtime_layout",
        RuntimeSourceBuildEvidenceKind::Sbom => "sbom",
        RuntimeSourceBuildEvidenceKind::Provenance => "provenance",
        RuntimeSourceBuildEvidenceKind::Transformation => "transformation",
        RuntimeSourceBuildEvidenceKind::OutputTree => "output_tree",
        RuntimeSourceBuildEvidenceKind::ExecutionReceipt => "execution_receipt",
        RuntimeSourceBuildEvidenceKind::StandardOutput => "standard_output",
        RuntimeSourceBuildEvidenceKind::StandardError => "standard_error",
    }
}
