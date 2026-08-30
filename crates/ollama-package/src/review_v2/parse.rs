use rewrite_model::{
    ArtifactSetId, ArtifactSetRelativePath, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageManifestId, RuntimeTarget,
};
use rewrite_types::Digest;
use serde::Deserialize;

use crate::json::validate_unique_json;
use crate::{RuntimePackageReviewCheck, RuntimePackageReviewCheckStatus};

use super::{
    RUNTIME_PACKAGE_REVIEW_V2_SCHEMA_VERSION, ReviewCheckResult, ReviewEvidence,
    RuntimePackageReviewDispositionV2, RuntimePackageReviewEvidenceClass, RuntimePackageReviewV2,
    RuntimePackageReviewV2Error, RuntimePackageReviewV2Limits,
};

const MAX_IDENTITY_BYTES: usize = 128;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewWire {
    schema_version: u32,
    runtime_family: String,
    reported_version: String,
    build_revision: String,
    target: TargetWire,
    source_build_inputs: SourceBuildInputsWire,
    evidence: Vec<EvidenceWire>,
    checks: Vec<CheckWire>,
    disposition: DispositionWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWire {
    operating_system: RuntimeOperatingSystem,
    architecture: RuntimeArchitecture,
    abi: RuntimeAbi,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBuildInputsWire {
    evidence_path: String,
    artifact_set_id: ArtifactSetId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceWire {
    class: RuntimePackageReviewEvidenceClass,
    relative_path: String,
    byte_size: u64,
    digest: Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckWire {
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum DispositionWire {
    NotAdmitted {
        blockers: Vec<RuntimePackageReviewCheck>,
    },
    Admitted {
        runtime_layout: String,
        layout_digest: Digest,
        runtime_package_manifest_id: RuntimePackageManifestId,
    },
}

pub(super) fn parse_review(
    bytes: &[u8],
    limits: RuntimePackageReviewV2Limits,
) -> Result<RuntimePackageReviewV2, RuntimePackageReviewV2Error> {
    if bytes.len() > limits.review_bytes {
        return Err(RuntimePackageReviewV2Error::ReviewTooLarge);
    }
    validate_unique_json(bytes).map_err(|()| RuntimePackageReviewV2Error::InvalidEncoding)?;
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RuntimePackageReviewV2Error::InvalidEncoding)?;
    if serde_json::to_vec(&value).map_err(|_| RuntimePackageReviewV2Error::InvalidEncoding)?
        != bytes
    {
        return Err(RuntimePackageReviewV2Error::NoncanonicalEncoding);
    }
    let wire: ReviewWire =
        serde_json::from_value(value).map_err(|_| RuntimePackageReviewV2Error::InvalidEncoding)?;
    if wire.schema_version != RUNTIME_PACKAGE_REVIEW_V2_SCHEMA_VERSION {
        return Err(RuntimePackageReviewV2Error::UnsupportedSchema);
    }
    if wire.runtime_family != "ollama"
        || !valid_identity(&wire.reported_version)
        || !valid_identity(&wire.build_revision)
    {
        return Err(RuntimePackageReviewV2Error::InvalidIdentity);
    }
    let target = RuntimeTarget::new(
        wire.target.operating_system,
        wire.target.architecture,
        wire.target.abi,
    )
    .map_err(|_| RuntimePackageReviewV2Error::UnsupportedTarget)?;
    if !matches!(
        (
            target.operating_system(),
            target.architecture(),
            target.abi()
        ),
        (
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc
        )
    ) {
        return Err(RuntimePackageReviewV2Error::UnsupportedTarget);
    }
    let evidence = parse_evidence(wire.evidence, limits)?;
    let source_build_inputs_path =
        ArtifactSetRelativePath::new(wire.source_build_inputs.evidence_path)
            .map_err(|_| RuntimePackageReviewV2Error::InvalidSourceBuildInputs)?;
    if !has_evidence_class(
        &evidence,
        &source_build_inputs_path,
        RuntimePackageReviewEvidenceClass::FetchedInput,
    ) {
        return Err(RuntimePackageReviewV2Error::InvalidSourceBuildInputs);
    }
    let checks = parse_checks(wire.checks, &evidence)?;
    let disposition = parse_disposition(wire.disposition, &checks, &evidence)?;
    require_complete_evidence_references(
        &evidence,
        &source_build_inputs_path,
        &checks,
        &disposition,
    )?;
    Ok(RuntimePackageReviewV2 {
        runtime_family: wire.runtime_family,
        reported_version: wire.reported_version,
        build_revision: wire.build_revision,
        target,
        source_build_inputs_path,
        source_build_inputs_id: wire.source_build_inputs.artifact_set_id,
        evidence,
        checks,
        disposition,
    })
}

fn parse_evidence(
    wire: Vec<EvidenceWire>,
    limits: RuntimePackageReviewV2Limits,
) -> Result<Vec<ReviewEvidence>, RuntimePackageReviewV2Error> {
    if wire.is_empty() {
        return Err(RuntimePackageReviewV2Error::InvalidEvidence);
    }
    if wire.len() > limits.maximum_evidence_records {
        return Err(RuntimePackageReviewV2Error::LimitExceeded);
    }
    let mut total_bytes = 0_u64;
    let evidence = wire
        .into_iter()
        .map(|item| {
            if item.byte_size == 0 || item.byte_size > limits.maximum_evidence_bytes {
                return Err(RuntimePackageReviewV2Error::LimitExceeded);
            }
            total_bytes = total_bytes
                .checked_add(item.byte_size)
                .ok_or(RuntimePackageReviewV2Error::LimitExceeded)?;
            if total_bytes > limits.maximum_total_evidence_bytes {
                return Err(RuntimePackageReviewV2Error::LimitExceeded);
            }
            Ok(ReviewEvidence {
                class: item.class,
                relative_path: ArtifactSetRelativePath::new(item.relative_path)
                    .map_err(|_| RuntimePackageReviewV2Error::InvalidEvidence)?,
                byte_size: item.byte_size,
                digest: item.digest,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if evidence.windows(2).any(|pair| {
        pair[0].relative_path.as_str().as_bytes() >= pair[1].relative_path.as_str().as_bytes()
    }) {
        return Err(RuntimePackageReviewV2Error::InvalidEvidence);
    }
    Ok(evidence)
}

fn parse_checks(
    wire: Vec<CheckWire>,
    evidence: &[ReviewEvidence],
) -> Result<Vec<ReviewCheckResult>, RuntimePackageReviewV2Error> {
    const REQUIRED: [RuntimePackageReviewCheck; 6] = [
        RuntimePackageReviewCheck::SourceLineage,
        RuntimePackageReviewCheck::Transformation,
        RuntimePackageReviewCheck::License,
        RuntimePackageReviewCheck::NativeClosure,
        RuntimePackageReviewCheck::ManagedStartup,
        RuntimePackageReviewCheck::CloudDisable,
    ];
    if wire.len() != REQUIRED.len() {
        return Err(RuntimePackageReviewV2Error::InvalidChecks);
    }
    let checks = wire
        .into_iter()
        .map(|result| {
            let paths = result
                .evidence
                .into_iter()
                .map(|path| {
                    ArtifactSetRelativePath::new(path)
                        .map_err(|_| RuntimePackageReviewV2Error::InvalidChecks)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if paths.is_empty()
                || paths.windows(2).any(|pair| pair[0] >= pair[1])
                || paths
                    .iter()
                    .any(|path| !evidence.iter().any(|item| item.relative_path == *path))
            {
                return Err(RuntimePackageReviewV2Error::InvalidChecks);
            }
            let parsed = ReviewCheckResult {
                check: result.check,
                status: result.status,
                evidence: paths,
            };
            if parsed.status == RuntimePackageReviewCheckStatus::Passed
                && !passed_check_has_required_classes(&parsed, evidence)
            {
                return Err(RuntimePackageReviewV2Error::InvalidChecks);
            }
            Ok(parsed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if checks.iter().map(|result| result.check).ne(REQUIRED) {
        return Err(RuntimePackageReviewV2Error::InvalidChecks);
    }
    Ok(checks)
}

fn passed_check_has_required_classes(
    check: &ReviewCheckResult,
    evidence: &[ReviewEvidence],
) -> bool {
    let required: &[RuntimePackageReviewEvidenceClass] = match check.check {
        RuntimePackageReviewCheck::SourceLineage => &[
            RuntimePackageReviewEvidenceClass::FetchedInput,
            RuntimePackageReviewEvidenceClass::BuildTool,
        ],
        RuntimePackageReviewCheck::Transformation => &[
            RuntimePackageReviewEvidenceClass::BuildTool,
            RuntimePackageReviewEvidenceClass::BuildOutput,
        ],
        RuntimePackageReviewCheck::License => &[RuntimePackageReviewEvidenceClass::FetchedInput],
        RuntimePackageReviewCheck::NativeClosure => &[
            RuntimePackageReviewEvidenceClass::BuildOutput,
            RuntimePackageReviewEvidenceClass::Execution,
        ],
        RuntimePackageReviewCheck::ManagedStartup | RuntimePackageReviewCheck::CloudDisable => {
            &[RuntimePackageReviewEvidenceClass::Execution]
        }
    };
    required.iter().all(|class| {
        check
            .evidence
            .iter()
            .any(|path| has_evidence_class(evidence, path, *class))
    })
}

fn has_evidence_class(
    evidence: &[ReviewEvidence],
    path: &ArtifactSetRelativePath,
    class: RuntimePackageReviewEvidenceClass,
) -> bool {
    evidence
        .iter()
        .any(|item| item.relative_path == *path && item.class == class)
}

fn parse_disposition(
    wire: DispositionWire,
    checks: &[ReviewCheckResult],
    evidence: &[ReviewEvidence],
) -> Result<RuntimePackageReviewDispositionV2, RuntimePackageReviewV2Error> {
    let blockers = checks
        .iter()
        .filter(|result| result.status != RuntimePackageReviewCheckStatus::Passed)
        .map(|result| result.check)
        .collect::<Vec<_>>();
    match wire {
        DispositionWire::NotAdmitted { blockers: declared }
            if !blockers.is_empty() && declared == blockers =>
        {
            Ok(RuntimePackageReviewDispositionV2::NotAdmitted { blockers: declared })
        }
        DispositionWire::Admitted {
            runtime_layout,
            layout_digest,
            runtime_package_manifest_id,
        } if blockers.is_empty() => {
            let runtime_layout = ArtifactSetRelativePath::new(runtime_layout)
                .map_err(|_| RuntimePackageReviewV2Error::InvalidDisposition)?;
            let Some(layout_evidence) = evidence
                .iter()
                .find(|item| item.relative_path == runtime_layout)
            else {
                return Err(RuntimePackageReviewV2Error::InvalidDisposition);
            };
            if layout_evidence.class != RuntimePackageReviewEvidenceClass::BuildOutput
                || layout_evidence.digest != layout_digest
            {
                return Err(RuntimePackageReviewV2Error::InvalidDisposition);
            }
            Ok(RuntimePackageReviewDispositionV2::Admitted {
                runtime_layout,
                layout_digest,
                runtime_package_manifest_id,
            })
        }
        _ => Err(RuntimePackageReviewV2Error::InvalidDisposition),
    }
}

fn require_complete_evidence_references(
    evidence: &[ReviewEvidence],
    source_build_inputs: &ArtifactSetRelativePath,
    checks: &[ReviewCheckResult],
    disposition: &RuntimePackageReviewDispositionV2,
) -> Result<(), RuntimePackageReviewV2Error> {
    let runtime_layout = match disposition {
        RuntimePackageReviewDispositionV2::NotAdmitted { .. } => None,
        RuntimePackageReviewDispositionV2::Admitted { runtime_layout, .. } => Some(runtime_layout),
    };
    if evidence.iter().all(|item| {
        item.relative_path == *source_build_inputs
            || runtime_layout.is_some_and(|path| *path == item.relative_path)
            || checks
                .iter()
                .any(|check| check.evidence.contains(&item.relative_path))
    }) {
        Ok(())
    } else {
        Err(RuntimePackageReviewV2Error::InvalidEvidence)
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}
