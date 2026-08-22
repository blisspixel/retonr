use rewrite_model::{
    ArtifactSetId, ArtifactSetRelativePath, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageManifestId, RuntimeTarget,
};
use rewrite_types::Digest;
use serde::Deserialize;
use thiserror::Error;

use crate::json::validate_unique_json;

/// Current exact runtime-package review contract version.
pub const RUNTIME_PACKAGE_REVIEW_SCHEMA_VERSION: u32 = 1;

const MAX_REVIEW_BYTES: usize = 256 * 1024;
const MAX_EVIDENCE_RECORDS: usize = 32;
const MAX_LOCATOR_BYTES: usize = 2_048;
const MAX_IDENTITY_BYTES: usize = 128;

/// One independently decided control in an exact runtime-package review.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePackageReviewCheck {
    /// Published bytes are bound to an immutable source and build lineage.
    SourceLineage,
    /// Every change from the source artifact is deterministic and retained.
    Transformation,
    /// Code and bundled dependencies have an exact license disposition.
    License,
    /// Packaged and external native dependencies form a declared closure.
    NativeClosure,
    /// The exact package starts and remains retained under managed isolation.
    ManagedStartup,
    /// The exact package emits the required cloud-disabled startup evidence.
    CloudDisable,
}

impl RuntimePackageReviewCheck {
    const ALL: [Self; 6] = [
        Self::SourceLineage,
        Self::Transformation,
        Self::License,
        Self::NativeClosure,
        Self::ManagedStartup,
        Self::CloudDisable,
    ];
}

/// Review result for one exact package control.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePackageReviewCheckStatus {
    /// Retained evidence satisfies this control.
    Passed,
    /// Retained evidence exposes an unresolved failure.
    Blocked,
    /// This control has not yet been executed for the exact package.
    NotRun,
}

/// Admission result carried by an exact runtime-package review.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimePackageReviewDisposition {
    /// The candidate has no execution-policy authority.
    NotAdmitted {
        /// Exact controls preventing admission, in canonical control order.
        blockers: Vec<RuntimePackageReviewCheck>,
    },
    /// Every control passed for one reconstructed layout and package identity.
    Admitted {
        /// Digest of the exact layout bytes used for reconstruction.
        layout_digest: Digest,
        /// Content-derived identity of the reconstructed runtime package.
        runtime_package_manifest_id: RuntimePackageManifestId,
    },
}

/// Bounded, machine-checked disposition for one exact runtime-package candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageReview {
    runtime_family: String,
    reported_version: String,
    build_revision: String,
    target: RuntimeTarget,
    source_artifact_set_id: ArtifactSetId,
    source_locator: String,
    source_byte_size: u64,
    source_digest: Digest,
    evidence: Vec<ReviewEvidence>,
    checks: Vec<ReviewCheckResult>,
    disposition: RuntimePackageReviewDisposition,
}

impl RuntimePackageReview {
    /// Parses and validates one exact package review without granting authority.
    ///
    /// An admitted disposition is accepted only when every required control is
    /// present exactly once and passed. A non-admitted disposition must name every
    /// blocked or unrun control and cannot carry a package identity.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimePackageReviewError`] for oversized, ambiguous, malformed,
    /// unsupported, incomplete, or internally inconsistent review evidence.
    pub fn parse(bytes: &[u8]) -> Result<Self, RuntimePackageReviewError> {
        if bytes.len() > MAX_REVIEW_BYTES {
            return Err(RuntimePackageReviewError::TooLarge);
        }
        validate_unique_json(bytes).map_err(|()| RuntimePackageReviewError::InvalidEncoding)?;
        let wire: ReviewWire = serde_json::from_slice(bytes)
            .map_err(|_| RuntimePackageReviewError::InvalidEncoding)?;
        from_wire(wire)
    }

    /// Returns the reviewed runtime family.
    #[must_use]
    pub fn runtime_family(&self) -> &str {
        &self.runtime_family
    }

    /// Returns the exact reported runtime version.
    #[must_use]
    pub fn reported_version(&self) -> &str {
        &self.reported_version
    }

    /// Returns the exact upstream source revision.
    #[must_use]
    pub fn build_revision(&self) -> &str {
        &self.build_revision
    }

    /// Returns the reviewed native target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the exact source artifact-set identity.
    #[must_use]
    pub const fn source_artifact_set_id(&self) -> &ArtifactSetId {
        &self.source_artifact_set_id
    }

    /// Returns the immutable source artifact locator.
    #[must_use]
    pub fn source_locator(&self) -> &str {
        &self.source_locator
    }

    /// Returns the exact source artifact byte length.
    #[must_use]
    pub const fn source_byte_size(&self) -> u64 {
        self.source_byte_size
    }

    /// Returns the exact source artifact digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        &self.source_digest
    }

    /// Returns the exact admission disposition.
    #[must_use]
    pub const fn disposition(&self) -> &RuntimePackageReviewDisposition {
        &self.disposition
    }

    /// Returns the result for one required review control.
    ///
    /// # Panics
    ///
    /// Panics only if an already validated review is corrupted in memory.
    #[must_use]
    pub fn check_status(
        &self,
        check: RuntimePackageReviewCheck,
    ) -> RuntimePackageReviewCheckStatus {
        self.checks
            .iter()
            .find(|result| result.check == check)
            .expect("validated review contains every control")
            .status
    }

    /// Returns the number of retained evidence records bound into the review.
    #[must_use]
    pub fn evidence_count(&self) -> usize {
        self.evidence.len()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewWire {
    schema_version: u32,
    runtime_family: String,
    reported_version: String,
    build_revision: String,
    target: TargetWire,
    source: SourceWire,
    evidence: Vec<EvidenceWire>,
    checks: Vec<CheckWire>,
    disposition: RuntimePackageReviewDisposition,
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
struct SourceWire {
    artifact_set_id: ArtifactSetId,
    locator: String,
    byte_size: u64,
    digest: Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceWire {
    relative_path: String,
    digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewEvidence {
    relative_path: ArtifactSetRelativePath,
    digest: Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckWire {
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewCheckResult {
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<ArtifactSetRelativePath>,
}

fn from_wire(wire: ReviewWire) -> Result<RuntimePackageReview, RuntimePackageReviewError> {
    if wire.schema_version != RUNTIME_PACKAGE_REVIEW_SCHEMA_VERSION {
        return Err(RuntimePackageReviewError::UnsupportedSchema);
    }
    if wire.runtime_family != "ollama"
        || !valid_identity(&wire.reported_version)
        || !valid_identity(&wire.build_revision)
    {
        return Err(RuntimePackageReviewError::InvalidIdentity);
    }
    let target = RuntimeTarget::new(
        wire.target.operating_system,
        wire.target.architecture,
        wire.target.abi,
    )
    .map_err(|_| RuntimePackageReviewError::UnsupportedTarget)?;
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
        return Err(RuntimePackageReviewError::UnsupportedTarget);
    }
    if wire.source.byte_size == 0
        || wire.source.locator.len() > MAX_LOCATOR_BYTES
        || !wire
            .source
            .locator
            .starts_with("https://github.com/ollama/ollama/releases/download/")
        || !wire.source.locator.is_ascii()
    {
        return Err(RuntimePackageReviewError::InvalidSource);
    }
    let evidence = parse_evidence(wire.evidence)?;
    let checks = parse_checks(wire.checks, &evidence)?;
    validate_disposition(&checks, &wire.disposition)?;
    Ok(RuntimePackageReview {
        runtime_family: wire.runtime_family,
        reported_version: wire.reported_version,
        build_revision: wire.build_revision,
        target,
        source_artifact_set_id: wire.source.artifact_set_id,
        source_locator: wire.source.locator,
        source_byte_size: wire.source.byte_size,
        source_digest: wire.source.digest,
        evidence,
        checks,
        disposition: wire.disposition,
    })
}

fn parse_evidence(
    wire: Vec<EvidenceWire>,
) -> Result<Vec<ReviewEvidence>, RuntimePackageReviewError> {
    if wire.is_empty() || wire.len() > MAX_EVIDENCE_RECORDS {
        return Err(RuntimePackageReviewError::InvalidEvidence);
    }
    let evidence = wire
        .into_iter()
        .map(|item| {
            Ok(ReviewEvidence {
                relative_path: ArtifactSetRelativePath::new(item.relative_path)
                    .map_err(|_| RuntimePackageReviewError::InvalidEvidence)?,
                digest: item.digest,
            })
        })
        .collect::<Result<Vec<_>, RuntimePackageReviewError>>()?;
    if evidence.windows(2).any(|pair| {
        pair[0].relative_path.as_str().as_bytes() >= pair[1].relative_path.as_str().as_bytes()
    }) {
        return Err(RuntimePackageReviewError::InvalidEvidence);
    }
    Ok(evidence)
}

fn parse_checks(
    wire: Vec<CheckWire>,
    evidence: &[ReviewEvidence],
) -> Result<Vec<ReviewCheckResult>, RuntimePackageReviewError> {
    if wire.len() != RuntimePackageReviewCheck::ALL.len() {
        return Err(RuntimePackageReviewError::InvalidChecks);
    }
    let checks = wire
        .into_iter()
        .map(|result| {
            let paths = result
                .evidence
                .into_iter()
                .map(|path| {
                    ArtifactSetRelativePath::new(path)
                        .map_err(|_| RuntimePackageReviewError::InvalidChecks)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if paths.is_empty()
                || paths.windows(2).any(|pair| pair[0] >= pair[1])
                || paths
                    .iter()
                    .any(|path| !evidence.iter().any(|item| item.relative_path == *path))
            {
                return Err(RuntimePackageReviewError::InvalidChecks);
            }
            Ok(ReviewCheckResult {
                check: result.check,
                status: result.status,
                evidence: paths,
            })
        })
        .collect::<Result<Vec<_>, RuntimePackageReviewError>>()?;
    if checks
        .iter()
        .map(|result| result.check)
        .ne(RuntimePackageReviewCheck::ALL)
    {
        return Err(RuntimePackageReviewError::InvalidChecks);
    }
    Ok(checks)
}

fn validate_disposition(
    checks: &[ReviewCheckResult],
    disposition: &RuntimePackageReviewDisposition,
) -> Result<(), RuntimePackageReviewError> {
    let blockers = checks
        .iter()
        .filter(|result| result.status != RuntimePackageReviewCheckStatus::Passed)
        .map(|result| result.check)
        .collect::<Vec<_>>();
    match disposition {
        RuntimePackageReviewDisposition::NotAdmitted { blockers: declared }
            if !blockers.is_empty() && *declared == blockers =>
        {
            Ok(())
        }
        RuntimePackageReviewDisposition::Admitted { .. } if blockers.is_empty() => Ok(()),
        _ => Err(RuntimePackageReviewError::InvalidDisposition),
    }
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTITY_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

/// Exact runtime-package review parsing or relationship failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePackageReviewError {
    /// The review exceeds its fixed encoded byte ceiling.
    #[error("runtime-package review exceeds its byte limit")]
    TooLarge,
    /// JSON is malformed, ambiguous, duplicated, or contains unknown fields.
    #[error("runtime-package review encoding is invalid")]
    InvalidEncoding,
    /// The review schema version is unsupported.
    #[error("runtime-package review schema is unsupported")]
    UnsupportedSchema,
    /// Runtime family, version, or source revision is invalid.
    #[error("runtime-package review identity is invalid")]
    InvalidIdentity,
    /// The target is outside the first managed review subset.
    #[error("runtime-package review target is unsupported")]
    UnsupportedTarget,
    /// The source artifact declaration is invalid.
    #[error("runtime-package review source is invalid")]
    InvalidSource,
    /// Evidence paths are missing, excessive, invalid, or noncanonical.
    #[error("runtime-package review evidence is invalid")]
    InvalidEvidence,
    /// Required checks are missing, reordered, duplicated, or reference absent evidence.
    #[error("runtime-package review checks are invalid")]
    InvalidChecks,
    /// Admission status conflicts with one or more check results.
    #[error("runtime-package review disposition is invalid")]
    InvalidDisposition,
}
