use std::io::Read;

use rewrite_model::{
    ArtifactSetRelativePath, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};
use rewrite_types::Digest;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::source_build_report::validates_transformed_runtime_binding;
use crate::{
    MemberOpenError, ReconstructedRuntimePackage, RuntimePackageReviewCheck,
    RuntimePackageReviewCheckStatus, RuntimeSourceBuildInputOpenError,
    reconstruct_runtime_package_with_limits, verify_runtime_source_build_inputs,
};

use super::parse::parse_review;
use super::{
    RUNTIME_PACKAGE_REVIEW_V2_SCHEMA_VERSION, RuntimePackageReviewEvidenceClass,
    RuntimePackageReviewEvidenceOpenError, RuntimePackageReviewV2Error,
    RuntimePackageReviewV2Limits, VerifiedRuntimePackageReviewV2,
};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const REQUIRED_CHECKS: [RuntimePackageReviewCheck; 6] = [
    RuntimePackageReviewCheck::SourceLineage,
    RuntimePackageReviewCheck::Transformation,
    RuntimePackageReviewCheck::License,
    RuntimePackageReviewCheck::NativeClosure,
    RuntimePackageReviewCheck::ManagedStartup,
    RuntimePackageReviewCheck::CloudDisable,
];

/// One evidence path and semantic class selected for review compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageReviewV2EvidenceInput {
    class: RuntimePackageReviewEvidenceClass,
    relative_path: ArtifactSetRelativePath,
}

impl RuntimePackageReviewV2EvidenceInput {
    /// Creates one typed evidence declaration without accepting a copied digest.
    #[must_use]
    pub const fn new(
        class: RuntimePackageReviewEvidenceClass,
        relative_path: ArtifactSetRelativePath,
    ) -> Self {
        Self {
            class,
            relative_path,
        }
    }

    /// Returns the selected evidence class.
    #[must_use]
    pub const fn class(&self) -> RuntimePackageReviewEvidenceClass {
        self.class
    }

    /// Returns the selected portable evidence path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }
}

/// One required control result and the exact evidence paths supporting it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageReviewV2CheckInput {
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<ArtifactSetRelativePath>,
}

impl RuntimePackageReviewV2CheckInput {
    /// Creates one control result. Compilation validates order and evidence classes.
    #[must_use]
    pub fn new(
        check: RuntimePackageReviewCheck,
        status: RuntimePackageReviewCheckStatus,
        evidence: Vec<ArtifactSetRelativePath>,
    ) -> Self {
        Self {
            check,
            status,
            evidence,
        }
    }

    /// Returns the required control.
    #[must_use]
    pub const fn check(&self) -> RuntimePackageReviewCheck {
        self.check
    }

    /// Returns the selected control status.
    #[must_use]
    pub const fn status(&self) -> RuntimePackageReviewCheckStatus {
        self.status
    }

    /// Returns supporting paths in canonical order.
    #[must_use]
    pub fn evidence(&self) -> &[ArtifactSetRelativePath] {
        &self.evidence
    }
}

/// Structural inputs for compiling one schema-2 review from opened bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageReviewV2CompilationInput {
    source_build_inputs: ArtifactSetRelativePath,
    runtime_layout: ArtifactSetRelativePath,
    evidence: Vec<RuntimePackageReviewV2EvidenceInput>,
    checks: Vec<RuntimePackageReviewV2CheckInput>,
}

impl RuntimePackageReviewV2CompilationInput {
    /// Creates one compilation request without accepting identities or digests.
    #[must_use]
    pub fn new(
        source_build_inputs: ArtifactSetRelativePath,
        runtime_layout: ArtifactSetRelativePath,
        evidence: Vec<RuntimePackageReviewV2EvidenceInput>,
        checks: Vec<RuntimePackageReviewV2CheckInput>,
    ) -> Self {
        Self {
            source_build_inputs,
            runtime_layout,
            evidence,
            checks,
        }
    }
}

/// Canonical schema-2 bytes plus the identities derived while compiling them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRuntimePackageReviewV2 {
    canonical_bytes: Vec<u8>,
    verified: VerifiedRuntimePackageReviewV2,
}

impl CompiledRuntimePackageReviewV2 {
    /// Returns canonical content-free schema-2 review bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the byte-verified review state derived during compilation.
    #[must_use]
    pub const fn verified(&self) -> &VerifiedRuntimePackageReviewV2 {
        &self.verified
    }
}

/// Compiles one canonical review by hashing evidence and deriving runtime identity.
///
/// The request supplies only portable paths, evidence classes, and control results.
/// The compiler hashes every evidence stream, verifies the complete frozen input
/// closure, reconstructs the selected runtime layout for both blocked and admitted
/// dispositions, derives all runtime identity fields, and emits an admitted result
/// only when every required control passed.
///
/// # Errors
///
/// Returns [`RuntimePackageReviewV2Error`] for invalid limits or request shape,
/// unavailable or excessive evidence, invalid frozen inputs, runtime reconstruction
/// failure, inconsistent bindings, cancellation, or canonical encoding failure.
pub fn compile_runtime_package_review_v2<ER, EF, SR, SF, MR, MF, C>(
    input: &RuntimePackageReviewV2CompilationInput,
    limits: &RuntimePackageReviewV2Limits,
    mut open_evidence: EF,
    open_source_input: SF,
    open_member: MF,
    mut cancelled: C,
) -> Result<CompiledRuntimePackageReviewV2, RuntimePackageReviewV2Error>
where
    ER: Read,
    EF: FnMut(&ArtifactSetRelativePath) -> Result<ER, RuntimePackageReviewEvidenceOpenError>,
    SR: Read,
    SF: FnMut(&ArtifactSetRelativePath) -> Result<SR, RuntimeSourceBuildInputOpenError>,
    MR: Read,
    MF: FnMut(&ArtifactSetRelativePath) -> Result<MR, MemberOpenError>,
    C: FnMut() -> bool,
{
    let limits = limits.validate()?;
    validate_input(input, limits)?;
    if cancelled() {
        return Err(RuntimePackageReviewV2Error::Cancelled);
    }
    let collected = collect_evidence(input, limits, &mut open_evidence, &mut cancelled)?;
    let source_bytes = collected
        .source_bytes
        .ok_or(RuntimePackageReviewV2Error::InvalidSourceBuildInputs)?;
    let source_inputs = verify_runtime_source_build_inputs(
        &source_bytes,
        limits.source_build_inputs,
        open_source_input,
        &mut cancelled,
    )
    .map_err(RuntimePackageReviewV2Error::SourceBuildInputs)?;
    let source_manifest = source_inputs.manifest().clone();
    let layout_bytes = collected
        .layout_bytes
        .ok_or(RuntimePackageReviewV2Error::InvalidDisposition)?;
    let reconstructed = reconstruct_runtime_package_with_limits(
        &layout_bytes,
        &limits.runtime_layout,
        open_member,
        &mut cancelled,
    )
    .map_err(RuntimePackageReviewV2Error::RuntimeReconstruction)?;
    validate_runtime(&source_manifest, &reconstructed)?;

    let blockers = input
        .checks
        .iter()
        .filter(|check| check.status != RuntimePackageReviewCheckStatus::Passed)
        .map(|check| check.check)
        .collect::<Vec<_>>();
    let admitted = blockers.is_empty();
    let disposition = if admitted {
        json!({
            "layout_digest": Digest::sha256(&layout_bytes),
            "runtime_layout": input.runtime_layout,
            "runtime_package_manifest_id": reconstructed
                .runtime_package()
                .runtime_package_manifest_id(),
            "status": "admitted"
        })
    } else {
        json!({"blockers": blockers, "status": "not_admitted"})
    };
    let checks = input
        .checks
        .iter()
        .map(|check| {
            json!({
                "check": check.check,
                "evidence": check.evidence,
                "status": check.status
            })
        })
        .collect::<Vec<_>>();
    let layout = reconstructed.layout();
    let bytes = serde_json::to_vec(&json!({
        "build_revision": layout.build_revision(),
        "checks": checks,
        "disposition": disposition,
        "evidence": collected.values,
        "reported_version": layout.reported_version(),
        "runtime_family": layout.runtime_family(),
        "schema_version": RUNTIME_PACKAGE_REVIEW_V2_SCHEMA_VERSION,
        "source_build_inputs": {
            "artifact_set_id": source_manifest.artifact_set().artifact_set_id(),
            "evidence_path": input.source_build_inputs
        },
        "target": {
            "abi": layout.target().abi(),
            "architecture": layout.target().architecture(),
            "operating_system": layout.target().operating_system()
        }
    }))
    .map_err(|_| RuntimePackageReviewV2Error::InvalidEncoding)?;
    if bytes.len() > limits.review_bytes {
        return Err(RuntimePackageReviewV2Error::ReviewTooLarge);
    }
    let review = parse_review(&bytes, limits)?;
    let reconstructed_runtime = admitted.then_some(reconstructed);
    Ok(CompiledRuntimePackageReviewV2 {
        canonical_bytes: bytes,
        verified: VerifiedRuntimePackageReviewV2 {
            review,
            source_build_inputs: source_manifest,
            reconstructed_runtime,
        },
    })
}

struct MeasuredEvidence {
    byte_size: u64,
    digest: Digest,
    retained: Option<Vec<u8>>,
}

struct CollectedEvidence {
    values: Vec<Value>,
    source_bytes: Option<Vec<u8>>,
    layout_bytes: Option<Vec<u8>>,
}

fn collect_evidence<ER, EF, C>(
    input: &RuntimePackageReviewV2CompilationInput,
    limits: RuntimePackageReviewV2Limits,
    open_evidence: &mut EF,
    cancelled: &mut C,
) -> Result<CollectedEvidence, RuntimePackageReviewV2Error>
where
    ER: Read,
    EF: FnMut(&ArtifactSetRelativePath) -> Result<ER, RuntimePackageReviewEvidenceOpenError>,
    C: FnMut() -> bool,
{
    let mut source_bytes = None;
    let mut layout_bytes = None;
    let mut values = Vec::with_capacity(input.evidence.len());
    let mut total_bytes = 0_u64;
    for evidence in &input.evidence {
        if cancelled() {
            return Err(RuntimePackageReviewV2Error::Cancelled);
        }
        let stream = open_evidence(&evidence.relative_path)
            .map_err(|_| RuntimePackageReviewV2Error::EvidenceUnavailable)?;
        let retain = evidence.relative_path == input.source_build_inputs
            || evidence.relative_path == input.runtime_layout;
        let measured = measure_evidence(stream, limits, &mut total_bytes, retain, cancelled)?;
        if measured.byte_size == 0 {
            return Err(RuntimePackageReviewV2Error::LimitExceeded);
        }
        let MeasuredEvidence {
            byte_size,
            digest,
            retained,
        } = measured;
        if evidence.relative_path == input.source_build_inputs {
            source_bytes = retained;
        } else if evidence.relative_path == input.runtime_layout {
            layout_bytes = retained;
        }
        values.push(json!({
            "byte_size": byte_size,
            "class": evidence.class,
            "digest": digest,
            "relative_path": evidence.relative_path
        }));
    }
    Ok(CollectedEvidence {
        values,
        source_bytes,
        layout_bytes,
    })
}

fn measure_evidence<R, C>(
    mut stream: R,
    limits: RuntimePackageReviewV2Limits,
    total_bytes: &mut u64,
    retain: bool,
    cancelled: &mut C,
) -> Result<MeasuredEvidence, RuntimePackageReviewV2Error>
where
    R: Read,
    C: FnMut() -> bool,
{
    let aggregate_remaining = limits
        .maximum_total_evidence_bytes
        .checked_sub(*total_bytes)
        .ok_or(RuntimePackageReviewV2Error::LimitExceeded)?;
    let maximum = limits.maximum_evidence_bytes.min(aggregate_remaining);
    let mut retained = retain.then(Vec::new);
    let mut hasher = Sha256::new();
    let mut byte_size = 0_u64;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        if cancelled() {
            return Err(RuntimePackageReviewV2Error::Cancelled);
        }
        let remaining = maximum.saturating_sub(byte_size);
        let read_limit = usize::try_from(remaining.saturating_add(1))
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = stream
            .read(&mut buffer[..read_limit])
            .map_err(|_| RuntimePackageReviewV2Error::EvidenceRead)?;
        if read == 0 {
            break;
        }
        byte_size = byte_size
            .checked_add(
                u64::try_from(read).map_err(|_| RuntimePackageReviewV2Error::LimitExceeded)?,
            )
            .ok_or(RuntimePackageReviewV2Error::LimitExceeded)?;
        if byte_size > maximum {
            return Err(RuntimePackageReviewV2Error::LimitExceeded);
        }
        hasher.update(&buffer[..read]);
        if let Some(bytes) = &mut retained {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    *total_bytes = total_bytes
        .checked_add(byte_size)
        .ok_or(RuntimePackageReviewV2Error::LimitExceeded)?;
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| RuntimePackageReviewV2Error::EvidenceRead)?;
    Ok(MeasuredEvidence {
        byte_size,
        digest,
        retained,
    })
}

fn validate_input(
    input: &RuntimePackageReviewV2CompilationInput,
    limits: RuntimePackageReviewV2Limits,
) -> Result<(), RuntimePackageReviewV2Error> {
    if input.evidence.is_empty() || input.evidence.len() > limits.maximum_evidence_records {
        return Err(RuntimePackageReviewV2Error::InvalidEvidence);
    }
    if input.evidence.windows(2).any(|pair| {
        pair[0].relative_path.as_str().as_bytes() >= pair[1].relative_path.as_str().as_bytes()
    }) {
        return Err(RuntimePackageReviewV2Error::InvalidEvidence);
    }
    let has_source = input.evidence.iter().any(|item| {
        item.relative_path == input.source_build_inputs
            && item.class == RuntimePackageReviewEvidenceClass::FetchedInput
    });
    let has_layout = input.evidence.iter().any(|item| {
        item.relative_path == input.runtime_layout
            && item.class == RuntimePackageReviewEvidenceClass::BuildOutput
    });
    if !has_source || !has_layout || input.checks.len() != REQUIRED_CHECKS.len() {
        return Err(RuntimePackageReviewV2Error::InvalidChecks);
    }
    for (result, required) in input.checks.iter().zip(REQUIRED_CHECKS) {
        if result.check != required
            || result.evidence.is_empty()
            || result.evidence.windows(2).any(|pair| pair[0] >= pair[1])
            || result.evidence.iter().any(|path| {
                !input
                    .evidence
                    .iter()
                    .any(|item| item.relative_path == *path)
            })
            || (result.status == RuntimePackageReviewCheckStatus::Passed
                && !has_required_classes(result, &input.evidence))
        {
            return Err(RuntimePackageReviewV2Error::InvalidChecks);
        }
    }
    if input.evidence.iter().any(|item| {
        item.relative_path != input.source_build_inputs
            && item.relative_path != input.runtime_layout
            && !input
                .checks
                .iter()
                .any(|check| check.evidence.contains(&item.relative_path))
    }) {
        return Err(RuntimePackageReviewV2Error::InvalidEvidence);
    }
    Ok(())
}

fn has_required_classes(
    check: &RuntimePackageReviewV2CheckInput,
    evidence: &[RuntimePackageReviewV2EvidenceInput],
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
        check.evidence.iter().any(|path| {
            evidence
                .iter()
                .any(|item| item.relative_path == *path && item.class == *class)
        })
    })
}

fn validate_runtime(
    source_inputs: &crate::RuntimeSourceBuildInputManifest,
    reconstructed: &ReconstructedRuntimePackage,
) -> Result<(), RuntimePackageReviewV2Error> {
    let layout = reconstructed.layout();
    let target = layout.target();
    if layout.runtime_family() != "ollama"
        || !valid_identity(layout.reported_version())
        || !valid_identity(layout.build_revision())
        || !matches!(
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
        )
        || !validates_transformed_runtime_binding(source_inputs, reconstructed, None)
    {
        return Err(RuntimePackageReviewV2Error::InvalidRuntimeBinding);
    }
    Ok(())
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}
