use std::collections::BTreeSet;

use rewrite_model::{ArtifactSetId, ArtifactSetRelativePath, RuntimePackageManifestId};
use rewrite_types::Digest;
use serde::Deserialize;

use crate::{
    RuntimeSourceBuildAcceleratorPolicy, RuntimeSourceBuildNetworkPolicy,
    json::validate_unique_json,
};

use super::{
    BuildAttempt, RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS, RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION,
    RuntimeSourceBuildAttempt, RuntimeSourceBuildComparison, RuntimeSourceBuildEvidenceKind,
    RuntimeSourceBuildEvidenceRecord, RuntimeSourceBuildReport, RuntimeSourceBuildReportError,
    RuntimeSourceBuildReportLimits, canonical_evidence_path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportWire {
    schema_version: u32,
    source_build_inputs_id: ArtifactSetId,
    build_plan_digest: Digest,
    attempts: Vec<AttemptWire>,
    comparison: ComparisonWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttemptWire {
    attempt: RuntimeSourceBuildAttempt,
    network_access: RuntimeSourceBuildNetworkPolicy,
    accelerator: RuntimeSourceBuildAcceleratorPolicy,
    environment_digest: Digest,
    build_arguments_digest: Digest,
    execution_receipt_digest: Digest,
    output_tree_digest: Digest,
    evidence: Vec<EvidenceWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceWire {
    kind: RuntimeSourceBuildEvidenceKind,
    relative_path: String,
    byte_size: u64,
    digest: Digest,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum ComparisonWire {
    ByteIdentical {
        artifact_set_id: ArtifactSetId,
        runtime_package_manifest_id: RuntimePackageManifestId,
        output_tree_digest: Digest,
    },
    Different {
        primary_artifact_set_id: ArtifactSetId,
        rebuild_artifact_set_id: ArtifactSetId,
        primary_runtime_package_manifest_id: RuntimePackageManifestId,
        rebuild_runtime_package_manifest_id: RuntimePackageManifestId,
        primary_output_tree_digest: Digest,
        rebuild_output_tree_digest: Digest,
    },
}

pub(super) fn parse_report(
    bytes: &[u8],
    limits: RuntimeSourceBuildReportLimits,
) -> Result<RuntimeSourceBuildReport, RuntimeSourceBuildReportError> {
    if bytes.len() > limits.report_bytes {
        return Err(RuntimeSourceBuildReportError::LimitExceeded);
    }
    validate_unique_json(bytes).map_err(|()| RuntimeSourceBuildReportError::InvalidEncoding)?;
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeSourceBuildReportError::InvalidEncoding)?;
    if serde_json::to_vec(&value).map_err(|_| RuntimeSourceBuildReportError::InvalidEncoding)?
        != bytes
    {
        return Err(RuntimeSourceBuildReportError::NoncanonicalEncoding);
    }
    let wire: ReportWire = serde_json::from_value(value)
        .map_err(|_| RuntimeSourceBuildReportError::InvalidEncoding)?;
    if wire.schema_version != RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION {
        return Err(RuntimeSourceBuildReportError::UnsupportedSchema);
    }
    let mut total_bytes = 0_u64;
    let attempts = parse_attempts(wire.attempts, limits, &mut total_bytes)?;
    let comparison = match wire.comparison {
        ComparisonWire::ByteIdentical {
            artifact_set_id,
            runtime_package_manifest_id,
            output_tree_digest,
        } => RuntimeSourceBuildComparison::ByteIdentical {
            artifact_set_id,
            runtime_package_manifest_id,
            output_tree_digest,
        },
        ComparisonWire::Different {
            primary_artifact_set_id,
            rebuild_artifact_set_id,
            primary_runtime_package_manifest_id,
            rebuild_runtime_package_manifest_id,
            primary_output_tree_digest,
            rebuild_output_tree_digest,
        } => RuntimeSourceBuildComparison::Different {
            primary_artifact_set_id,
            rebuild_artifact_set_id,
            primary_runtime_package_manifest_id,
            rebuild_runtime_package_manifest_id,
            primary_output_tree_digest,
            rebuild_output_tree_digest,
        },
    };
    Ok(RuntimeSourceBuildReport {
        source_build_inputs_id: wire.source_build_inputs_id,
        build_plan_digest: wire.build_plan_digest,
        attempts,
        comparison,
    })
}

fn parse_attempts(
    wire: Vec<AttemptWire>,
    limits: RuntimeSourceBuildReportLimits,
    total_bytes: &mut u64,
) -> Result<Vec<BuildAttempt>, RuntimeSourceBuildReportError> {
    const ATTEMPTS: [RuntimeSourceBuildAttempt; 2] = [
        RuntimeSourceBuildAttempt::Primary,
        RuntimeSourceBuildAttempt::Rebuild,
    ];
    if wire.len() != ATTEMPTS.len() {
        return Err(RuntimeSourceBuildReportError::InvalidAttempt);
    }
    let mut paths = BTreeSet::new();
    let attempts = wire
        .into_iter()
        .map(|attempt| {
            if attempt.network_access != RuntimeSourceBuildNetworkPolicy::Denied
                || attempt.accelerator != RuntimeSourceBuildAcceleratorPolicy::CpuOnly
            {
                return Err(RuntimeSourceBuildReportError::InvalidAttempt);
            }
            let evidence = parse_evidence(
                attempt.attempt,
                attempt.evidence,
                limits,
                total_bytes,
                &mut paths,
            )?;
            if evidence
                .iter()
                .find(|item| item.kind == RuntimeSourceBuildEvidenceKind::ExecutionReceipt)
                .is_none_or(|item| item.digest != attempt.execution_receipt_digest)
            {
                return Err(RuntimeSourceBuildReportError::InvalidAttempt);
            }
            Ok(BuildAttempt {
                attempt: attempt.attempt,
                environment_digest: attempt.environment_digest,
                build_arguments_digest: attempt.build_arguments_digest,
                execution_receipt_digest: attempt.execution_receipt_digest,
                output_tree_digest: attempt.output_tree_digest,
                evidence,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if attempts.iter().map(|attempt| attempt.attempt).ne(ATTEMPTS) {
        return Err(RuntimeSourceBuildReportError::InvalidAttempt);
    }
    Ok(attempts)
}

fn parse_evidence(
    attempt: RuntimeSourceBuildAttempt,
    wire: Vec<EvidenceWire>,
    limits: RuntimeSourceBuildReportLimits,
    total_bytes: &mut u64,
    paths: &mut BTreeSet<ArtifactSetRelativePath>,
) -> Result<Vec<RuntimeSourceBuildEvidenceRecord>, RuntimeSourceBuildReportError> {
    if wire.len() != RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS.len() {
        return Err(RuntimeSourceBuildReportError::InvalidAttempt);
    }
    let evidence = wire
        .into_iter()
        .map(|evidence| {
            let permits_empty = matches!(
                evidence.kind,
                RuntimeSourceBuildEvidenceKind::StandardOutput
                    | RuntimeSourceBuildEvidenceKind::StandardError
            );
            if (!permits_empty && evidence.byte_size == 0)
                || evidence.byte_size > limits.maximum_evidence_bytes
            {
                return Err(RuntimeSourceBuildReportError::LimitExceeded);
            }
            *total_bytes = total_bytes
                .checked_add(evidence.byte_size)
                .ok_or(RuntimeSourceBuildReportError::LimitExceeded)?;
            if *total_bytes > limits.maximum_total_evidence_bytes {
                return Err(RuntimeSourceBuildReportError::LimitExceeded);
            }
            let relative_path = ArtifactSetRelativePath::new(evidence.relative_path)
                .map_err(|_| RuntimeSourceBuildReportError::InvalidAttempt)?;
            if relative_path != canonical_evidence_path(attempt, evidence.kind)? {
                return Err(RuntimeSourceBuildReportError::InvalidAttempt);
            }
            if !paths.insert(relative_path.clone()) {
                return Err(RuntimeSourceBuildReportError::InvalidAttempt);
            }
            Ok(RuntimeSourceBuildEvidenceRecord {
                kind: evidence.kind,
                relative_path,
                byte_size: evidence.byte_size,
                digest: evidence.digest,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if evidence
        .iter()
        .map(|evidence| evidence.kind)
        .ne(RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS)
    {
        return Err(RuntimeSourceBuildReportError::InvalidAttempt);
    }
    Ok(evidence)
}
