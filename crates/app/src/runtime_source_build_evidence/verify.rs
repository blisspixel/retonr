use std::{fs::File, io::Read as _, path::PathBuf};

use super::contract::{
    RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
    RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH, RUNTIME_SOURCE_BUILD_REPORT_PATH,
    RuntimeSourceBuildEvidenceBundleError, RuntimeSourceBuildEvidenceBundleLimits,
    RuntimeSourceBuildEvidenceBundleSource, ensure_active, map_storage,
};
use crate::artifact_storage::{
    ManagedTreeLimits, PinnedDirectory, StableMetadataFingerprint, fingerprint_std_file,
    is_indirect,
};
use rewrite_model::{
    ArtifactSetManifest, ArtifactSetRelativePath, MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES,
};
use rewrite_ollama_package::{
    MemberOpenError, RuntimeSourceBuildAttempt, RuntimeSourceBuildInputOpenError,
    RuntimeSourceBuildPlan, RuntimeSourceBuildReportOpenError, VerifiedRuntimeSourceBuildInputs,
    VerifiedRuntimeSourceBuildReport, verify_runtime_source_build_inputs,
    verify_runtime_source_build_report,
};
use rewrite_types::{CancellationToken, Digest};

mod open;
mod tree;
use tree::{expected_manifest, validate_tree};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const INPUT_PREFIX: &str = "inputs/";

/// Retained, independently verified durable controlled-build closure.
pub struct RuntimeSourceBuildEvidenceBundleLease {
    pinned: PinnedEvidenceRoot,
    manifest: ArtifactSetManifest,
    source_inputs: VerifiedRuntimeSourceBuildInputs,
    plan: RuntimeSourceBuildPlan,
    report: VerifiedRuntimeSourceBuildReport,
    limits: RuntimeSourceBuildEvidenceBundleLimits,
}

impl RuntimeSourceBuildEvidenceBundleLease {
    pub(crate) fn overlaps_evidence_path(&self, path: &std::path::Path) -> bool {
        if super::contract::paths_overlap(&self.pinned.path, path) {
            return true;
        }
        let Some(parent) = path.parent() else {
            return true;
        };
        let Some(name) = path.file_name() else {
            return true;
        };
        match (
            std::fs::canonicalize(&self.pinned.path),
            std::fs::canonicalize(parent),
        ) {
            (Ok(source), Ok(parent)) => super::contract::paths_overlap(&source, &parent.join(name)),
            _ => true,
        }
    }
    /// Returns the content manifest for every closure member except the manifest itself.
    #[must_use]
    pub const fn manifest(&self) -> &ArtifactSetManifest {
        &self.manifest
    }

    /// Returns the frozen input manifest whose complete component set was rehashed.
    #[must_use]
    pub const fn source_inputs(&self) -> &VerifiedRuntimeSourceBuildInputs {
        &self.source_inputs
    }

    /// Returns the build plan independently derived from the durable input manifest.
    #[must_use]
    pub const fn plan(&self) -> &RuntimeSourceBuildPlan {
        &self.plan
    }

    /// Returns the two-attempt report verified from only durable closure bytes.
    #[must_use]
    pub const fn report(&self) -> &VerifiedRuntimeSourceBuildReport {
        &self.report
    }

    /// Rehashes and re-verifies the entire held and named closure.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildEvidenceBundleError`] for cancellation or any
    /// boundary, tree, byte, manifest, report, or runtime-package drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
        self.pinned.revalidate(cancellation)?;
        let verified = verify_root(&self.pinned.root, self.limits, cancellation)?;
        self.pinned.revalidate(cancellation)?;
        if verified.manifest != self.manifest
            || verified.source_inputs != self.source_inputs
            || verified.plan != self.plan
            || verified.report != self.report
        {
            return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
        }
        Ok(())
    }
}

/// Read-only verifier for one previously published durable evidence root.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeSourceBuildEvidenceBundleVerifier;

impl RuntimeSourceBuildEvidenceBundleVerifier {
    /// Pins and independently verifies one complete durable evidence closure.
    ///
    /// The verifier performs no mutation, discovery, execution, or network access.
    /// It requires an exact tree, rehashes every frozen input and both complete
    /// runtime outputs, and re-derives the report comparison and package identities.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildEvidenceBundleError`] for invalid limits,
    /// unsafe or changing storage, malformed manifests, unavailable evidence,
    /// digest drift, cancellation, or invalid reconstructed runtime state.
    pub fn acquire(
        source: &RuntimeSourceBuildEvidenceBundleSource,
        limits: RuntimeSourceBuildEvidenceBundleLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildEvidenceBundleLease, RuntimeSourceBuildEvidenceBundleError> {
        let limits = limits.validate()?;
        ensure_active(cancellation)?;
        let metadata = std::fs::symlink_metadata(&source.path)
            .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
        if is_indirect(&metadata) || !metadata.is_dir() {
            return Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary);
        }
        let root = PinnedDirectory::open_existing(&source.path).map_err(map_storage)?;
        acquire_published(source.path.clone(), root, limits, cancellation)
    }
}

pub(super) fn acquire_published(
    path: PathBuf,
    root: PinnedDirectory,
    limits: RuntimeSourceBuildEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<RuntimeSourceBuildEvidenceBundleLease, RuntimeSourceBuildEvidenceBundleError> {
    let pinned = PinnedEvidenceRoot::new(path, root, cancellation)?;
    let verified = verify_root(&pinned.root, limits, cancellation)?;
    pinned.revalidate(cancellation)?;
    Ok(RuntimeSourceBuildEvidenceBundleLease {
        pinned,
        manifest: verified.manifest,
        source_inputs: verified.source_inputs,
        plan: verified.plan,
        report: verified.report,
        limits,
    })
}

struct PinnedEvidenceRoot {
    path: PathBuf,
    root: PinnedDirectory,
    baseline: StableMetadataFingerprint,
}

impl PinnedEvidenceRoot {
    fn new(
        path: PathBuf,
        root: PinnedDirectory,
        cancellation: &CancellationToken,
    ) -> Result<Self, RuntimeSourceBuildEvidenceBundleError> {
        ensure_active(cancellation)?;
        let baseline = root.fingerprint().map_err(map_storage)?.stable();
        let pinned = Self {
            path,
            root,
            baseline,
        };
        pinned.revalidate(cancellation)?;
        Ok(pinned)
    }

    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
        ensure_active(cancellation)?;
        let held = self.root.fingerprint().map_err(map_storage)?.stable();
        let named = PinnedDirectory::fingerprint_path(&self.path)
            .map_err(map_storage)?
            .stable();
        if held == self.baseline && named == self.baseline {
            Ok(())
        } else {
            Err(RuntimeSourceBuildEvidenceBundleError::Changed)
        }
    }
}

pub(super) struct VerifiedEvidenceRoot {
    manifest: ArtifactSetManifest,
    source_inputs: VerifiedRuntimeSourceBuildInputs,
    plan: RuntimeSourceBuildPlan,
    report: VerifiedRuntimeSourceBuildReport,
}

pub(super) fn verify_root(
    root: &PinnedDirectory,
    limits: RuntimeSourceBuildEvidenceBundleLimits,
    cancellation: &CancellationToken,
) -> Result<VerifiedEvidenceRoot, RuntimeSourceBuildEvidenceBundleError> {
    ensure_active(cancellation)?;
    let tree_limits = ManagedTreeLimits::new(limits.maximum_tree_entries).map_err(map_storage)?;
    let before = root
        .enumerate_tree(tree_limits, cancellation)
        .map_err(map_storage)?;
    let manifest_path = fixed_path(RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH);
    let manifest_bytes = read_bounded_file(
        root,
        &manifest_path,
        u64::try_from(MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES)
            .expect("artifact-set manifest ceiling fits u64"),
        cancellation,
    )?;
    let manifest = ArtifactSetManifest::from_json_bytes(&manifest_bytes)
        .map_err(RuntimeSourceBuildEvidenceBundleError::Manifest)?;
    if manifest.canonical_json().as_bytes() != manifest_bytes {
        return Err(RuntimeSourceBuildEvidenceBundleError::TreeMismatch);
    }
    if manifest.total_byte_size() > limits.maximum_total_bytes {
        return Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded);
    }
    validate_tree(&before, &manifest, manifest_bytes.len())?;

    let input_path = fixed_path(RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH);
    let input_bytes = read_declared_bytes(
        root,
        &manifest,
        &input_path,
        u64::try_from(limits.source_build_inputs.manifest_bytes)
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?,
        cancellation,
    )?;
    let source_inputs = verify_runtime_source_build_inputs(
        &input_bytes,
        limits.source_build_inputs,
        |path| {
            open_prefixed(root, INPUT_PREFIX, path).map_err(|_| RuntimeSourceBuildInputOpenError)
        },
        || cancellation.is_cancelled(),
    )?;
    let (plan, plan_binding_bytes) =
        read_plan_binding(root, &manifest, &source_inputs, cancellation)?;

    let report_path = fixed_path(RUNTIME_SOURCE_BUILD_REPORT_PATH);
    let report_bytes = read_declared_bytes(
        root,
        &manifest,
        &report_path,
        u64::try_from(limits.report_compilation.report.report_bytes)
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?,
        cancellation,
    )?;
    let report = verify_runtime_source_build_report(
        &report_bytes,
        source_inputs.manifest(),
        &plan,
        &limits.report_compilation.report,
        |_attempt, path| open_exact(root, path).map_err(|_| RuntimeSourceBuildReportOpenError),
        |attempt, path| {
            open_prefixed(root, attempt_prefix(attempt), path).map_err(|_| MemberOpenError)
        },
        || cancellation.is_cancelled(),
    )?;
    let expected = expected_manifest(
        &input_bytes,
        plan_binding_bytes.as_deref(),
        &report_bytes,
        &source_inputs,
        &report,
    )?;
    if expected != manifest {
        return Err(RuntimeSourceBuildEvidenceBundleError::TreeMismatch);
    }
    let after = root
        .enumerate_tree(tree_limits, cancellation)
        .map_err(map_storage)?;
    if before != after {
        return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
    }
    Ok(VerifiedEvidenceRoot {
        manifest,
        source_inputs,
        plan,
        report,
    })
}

fn read_plan_binding(
    root: &PinnedDirectory,
    manifest: &ArtifactSetManifest,
    inputs: &VerifiedRuntimeSourceBuildInputs,
    cancellation: &CancellationToken,
) -> Result<(RuntimeSourceBuildPlan, Option<Vec<u8>>), RuntimeSourceBuildEvidenceBundleError> {
    let path = fixed_path(RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH);
    let bytes = manifest
        .members()
        .iter()
        .any(|member| member.relative_path() == &path)
        .then(|| {
            read_declared_bytes(
                root,
                manifest,
                &path,
                u64::try_from(super::plan_binding::MAXIMUM_PLAN_BINDING_BYTES)
                    .expect("plan-binding ceiling fits u64"),
                cancellation,
            )
        })
        .transpose()?;
    let plan = match &bytes {
        Some(bytes) => super::plan_binding::parse(bytes, inputs)?,
        None if inputs.retained_program_lineage().is_none() => {
            RuntimeSourceBuildPlan::for_legacy_read_only_verification(inputs.manifest())
        }
        None => return Err(RuntimeSourceBuildEvidenceBundleError::InvalidPlanBinding),
    };
    Ok((plan, bytes))
}

fn read_declared_bytes(
    root: &PinnedDirectory,
    manifest: &ArtifactSetManifest,
    path: &ArtifactSetRelativePath,
    maximum: u64,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, RuntimeSourceBuildEvidenceBundleError> {
    let declared = manifest
        .members()
        .iter()
        .find(|member| member.relative_path() == path)
        .ok_or(RuntimeSourceBuildEvidenceBundleError::TreeMismatch)?;
    if declared.byte_size() > maximum {
        return Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded);
    }
    let bytes = read_bounded_file(root, path, maximum, cancellation)?;
    if u64::try_from(bytes.len())
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?
        != declared.byte_size()
        || Digest::sha256(&bytes) != *declared.artifact_id().digest()
    {
        return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
    }
    Ok(bytes)
}

fn read_bounded_file(
    root: &PinnedDirectory,
    path: &ArtifactSetRelativePath,
    maximum: u64,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, RuntimeSourceBuildEvidenceBundleError> {
    ensure_active(cancellation)?;
    let mut opened = root.open_relative_regular_file(path).map_err(map_storage)?;
    if !opened.fingerprint.has_single_link() || opened.byte_size > maximum {
        return Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded);
    }
    let capacity = usize::try_from(opened.byte_size)
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut remaining = opened.byte_size;
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    while remaining != 0 {
        ensure_active(cancellation)?;
        let maximum = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = opened
            .file
            .read(&mut buffer[..maximum])
            .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?;
        if read == 0 {
            return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
        }
        bytes.extend_from_slice(&buffer[..read]);
        remaining -= u64::try_from(read)
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::LimitExceeded)?;
    }
    let mut trailing = [0_u8; 1];
    if opened
        .file
        .read(&mut trailing)
        .map_err(RuntimeSourceBuildEvidenceBundleError::StorageIo)?
        != 0
    {
        return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
    }
    let current = fingerprint_std_file(&opened.file).map_err(map_storage)?;
    if current != opened.fingerprint {
        return Err(RuntimeSourceBuildEvidenceBundleError::Changed);
    }
    root.recheck_relative_regular_file(path, &opened.fingerprint)
        .map_err(map_storage)?;
    Ok(bytes)
}

fn open_exact(
    root: &PinnedDirectory,
    path: &ArtifactSetRelativePath,
) -> Result<File, RuntimeSourceBuildEvidenceBundleError> {
    root.open_relative_regular_file(path)
        .map(|opened| opened.file)
        .map_err(map_storage)
}

fn open_prefixed(
    root: &PinnedDirectory,
    prefix: &str,
    path: &ArtifactSetRelativePath,
) -> Result<File, RuntimeSourceBuildEvidenceBundleError> {
    let path = prefixed_path(prefix, path)?;
    open_exact(root, &path)
}

fn prefixed_path(
    prefix: &str,
    path: &ArtifactSetRelativePath,
) -> Result<ArtifactSetRelativePath, RuntimeSourceBuildEvidenceBundleError> {
    ArtifactSetRelativePath::new(format!("{prefix}{}", path.as_str()))
        .map_err(|_| RuntimeSourceBuildEvidenceBundleError::TreeMismatch)
}

fn fixed_path(path: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(path.to_owned()).expect("fixed evidence path is portable")
}

const fn attempt_prefix(attempt: RuntimeSourceBuildAttempt) -> &'static str {
    match attempt {
        RuntimeSourceBuildAttempt::Primary => "attempts/primary/",
        RuntimeSourceBuildAttempt::Rebuild => "attempts/rebuild/",
    }
}
