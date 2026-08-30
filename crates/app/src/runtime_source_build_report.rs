use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Cursor, Read},
    path::Path,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::{
    CompiledRuntimeSourceBuildReport, MemberOpenError, ReconstructedRuntimePackage,
    RuntimeSourceBuildAttempt, RuntimeSourceBuildReportError, RuntimeSourceBuildReportLimits,
    RuntimeSourceBuildReportOpenError, compile_runtime_source_build_report,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    ExecutableRuntimeSourceBuildBundleLease, RuntimeSourceBuildBundleError,
    RuntimeSourceBuildExecution, RuntimeSourceBuildExecutionError,
    artifact_storage::{ManagedTreeEntryKind, ManagedTreeLimits, ManagedTreeSnapshot},
    runtime_source_build_execution::MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES,
};

mod receipt;

/// Caller-owned ceilings for compiling one two-attempt report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildReportCompilationLimits {
    /// Pure report, evidence, and runtime-layout ceilings.
    pub report: RuntimeSourceBuildReportLimits,
    /// Maximum files plus directories in either output tree.
    pub maximum_output_tree_entries: usize,
}

impl Default for RuntimeSourceBuildReportCompilationLimits {
    fn default() -> Self {
        Self {
            report: RuntimeSourceBuildReportLimits::default(),
            maximum_output_tree_entries: MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES,
        }
    }
}

impl RuntimeSourceBuildReportCompilationLimits {
    fn tree_limits(self) -> Result<ManagedTreeLimits, RuntimeSourceBuildReportCompilationError> {
        if self.maximum_output_tree_entries == 0
            || self.maximum_output_tree_entries > MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES
        {
            Err(RuntimeSourceBuildReportCompilationError::LimitExceeded)
        } else {
            ManagedTreeLimits::new(self.maximum_output_tree_entries)
                .map_err(|_| RuntimeSourceBuildReportCompilationError::LimitExceeded)
        }
    }
}

/// App-derived evidence that is not written by the untrusted build program.
#[derive(Clone, Eq, PartialEq)]
pub struct RuntimeSourceBuildManagedEvidence {
    execution_receipt: Vec<u8>,
    output_tree: Vec<u8>,
    standard_output: Vec<u8>,
    standard_error: Vec<u8>,
}

impl std::fmt::Debug for RuntimeSourceBuildManagedEvidence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeSourceBuildManagedEvidence")
            .field("execution_receipt_bytes", &self.execution_receipt.len())
            .field("output_tree_bytes", &self.output_tree.len())
            .field("standard_output_bytes", &self.standard_output.len())
            .field("standard_error_bytes", &self.standard_error.len())
            .finish()
    }
}

impl RuntimeSourceBuildManagedEvidence {
    /// Returns the canonical typed per-attempt execution receipt JSON.
    #[must_use]
    pub fn execution_receipt_bytes(&self) -> &[u8] {
        &self.execution_receipt
    }

    /// Returns the canonical path, mode, and byte identity of the original output.
    #[must_use]
    pub fn output_tree_bytes(&self) -> &[u8] {
        &self.output_tree
    }

    /// Returns the exact bounded standard-output bytes.
    #[must_use]
    pub fn standard_output(&self) -> &[u8] {
        &self.standard_output
    }

    /// Returns the exact bounded standard-error bytes.
    #[must_use]
    pub fn standard_error(&self) -> &[u8] {
        &self.standard_error
    }
}

/// Canonical report plus the app-owned evidence bytes needed to verify it later.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildReportCompilation {
    compiled: CompiledRuntimeSourceBuildReport,
    primary_managed_evidence: RuntimeSourceBuildManagedEvidence,
    rebuild_managed_evidence: RuntimeSourceBuildManagedEvidence,
}

impl RuntimeSourceBuildReportCompilation {
    /// Returns the canonical content-free report bytes.
    #[must_use]
    pub fn canonical_report_bytes(&self) -> &[u8] {
        self.compiled.canonical_bytes()
    }

    /// Returns the packages and comparison derived while compiling the report.
    #[must_use]
    pub const fn compiled(&self) -> &CompiledRuntimeSourceBuildReport {
        &self.compiled
    }

    /// Returns the app-owned evidence for the selected attempt.
    #[must_use]
    pub const fn managed_evidence(
        &self,
        attempt: RuntimeSourceBuildAttempt,
    ) -> &RuntimeSourceBuildManagedEvidence {
        match attempt {
            RuntimeSourceBuildAttempt::Primary => &self.primary_managed_evidence,
            RuntimeSourceBuildAttempt::Rebuild => &self.rebuild_managed_evidence,
        }
    }
}

/// Deterministic compiler for two retained controlled-build output trees.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeSourceBuildReportCompiler;

impl RuntimeSourceBuildReportCompiler {
    /// Hashes both exact outputs and derives a canonical report and comparison.
    ///
    /// Build-owned layout, SBOM, provenance, and transformation files are read
    /// beneath the retained output roots. Isolation evidence and bounded streams
    /// come only from the managed execution results. Both output trees and the
    /// frozen input bundle are revalidated before and after compilation.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReportCompilationError`] for invalid limits,
    /// overlapping or changing outputs, unexpected tree entries, bundle drift,
    /// cancellation, or any pure report compilation failure.
    pub fn compile(
        bundle: &ExecutableRuntimeSourceBuildBundleLease,
        primary: &RuntimeSourceBuildExecution,
        rebuild: &RuntimeSourceBuildExecution,
        limits: RuntimeSourceBuildReportCompilationLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildReportCompilation, RuntimeSourceBuildReportCompilationError> {
        let tree_limits = limits.tree_limits()?;
        ensure_active(cancellation)?;
        primary.revalidate_plan(bundle.plan())?;
        rebuild.revalidate_plan(bundle.plan())?;
        if paths_overlap(primary.output.path(), rebuild.output.path()) {
            return Err(RuntimeSourceBuildReportCompilationError::OutputOverlap);
        }
        bundle.revalidate_for_execution(cancellation)?;
        primary
            .output
            .validate_sealed(tree_limits, cancellation)
            .map_err(map_snapshot)?;
        rebuild
            .output
            .validate_sealed(tree_limits, cancellation)
            .map_err(map_snapshot)?;
        let primary_managed_evidence = managed_evidence(
            bundle,
            primary,
            RuntimeSourceBuildAttempt::Primary,
            cancellation,
        )?;
        let rebuild_managed_evidence = managed_evidence(
            bundle,
            rebuild,
            RuntimeSourceBuildAttempt::Rebuild,
            cancellation,
        )?;
        let compiled = compile_with_inputs(
            bundle,
            primary,
            rebuild,
            &primary_managed_evidence,
            &rebuild_managed_evidence,
            &limits.report,
            cancellation,
        )?;
        primary
            .output
            .validate_sealed(tree_limits, cancellation)
            .map_err(map_snapshot)?;
        rebuild
            .output
            .validate_sealed(tree_limits, cancellation)
            .map_err(map_snapshot)?;
        validate_output_shape(primary.output.sealed_snapshot()?, compiled.primary())?;
        validate_output_shape(rebuild.output.sealed_snapshot()?, compiled.rebuild())?;
        bundle.revalidate_for_execution(cancellation)?;
        Ok(RuntimeSourceBuildReportCompilation {
            compiled,
            primary_managed_evidence,
            rebuild_managed_evidence,
        })
    }
}

struct CompilationInputs<'a> {
    primary: &'a RuntimeSourceBuildExecution,
    rebuild: &'a RuntimeSourceBuildExecution,
    primary_managed: &'a RuntimeSourceBuildManagedEvidence,
    rebuild_managed: &'a RuntimeSourceBuildManagedEvidence,
    cancellation: &'a CancellationToken,
}

fn compile_with_inputs(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    primary: &RuntimeSourceBuildExecution,
    rebuild: &RuntimeSourceBuildExecution,
    primary_managed: &RuntimeSourceBuildManagedEvidence,
    rebuild_managed: &RuntimeSourceBuildManagedEvidence,
    limits: &RuntimeSourceBuildReportLimits,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeSourceBuildReport, RuntimeSourceBuildReportCompilationError> {
    let inputs = CompilationInputs {
        primary,
        rebuild,
        primary_managed,
        rebuild_managed,
        cancellation,
    };
    compile_runtime_source_build_report(
        bundle.bundle().inputs().manifest(),
        bundle.plan(),
        limits,
        |attempt, path| open_evidence(&inputs, attempt, path),
        |attempt, path| open_member(&inputs, attempt, path),
        || cancellation.is_cancelled(),
    )
    .map_err(RuntimeSourceBuildReportCompilationError::Report)
}

enum ReportInput {
    File(File),
    Memory(Cursor<Vec<u8>>),
}

impl Read for ReportInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.read(buffer),
            Self::Memory(cursor) => cursor.read(buffer),
        }
    }
}

fn open_evidence(
    inputs: &CompilationInputs<'_>,
    attempt: RuntimeSourceBuildAttempt,
    path: &ArtifactSetRelativePath,
) -> Result<ReportInput, RuntimeSourceBuildReportOpenError> {
    if inputs.cancellation.is_cancelled() {
        return Err(RuntimeSourceBuildReportOpenError);
    }
    let (execution, managed, prefix) = match attempt {
        RuntimeSourceBuildAttempt::Primary => {
            (inputs.primary, inputs.primary_managed, "attempts/primary/")
        }
        RuntimeSourceBuildAttempt::Rebuild => {
            (inputs.rebuild, inputs.rebuild_managed, "attempts/rebuild/")
        }
    };
    let name = path
        .as_str()
        .strip_prefix(prefix)
        .ok_or(RuntimeSourceBuildReportOpenError)?;
    match name {
        "runtime-layout.json" | "sbom.json" | "provenance.json" | "transformation.json" => {
            let relative = ArtifactSetRelativePath::new(name.to_owned())
                .map_err(|_| RuntimeSourceBuildReportOpenError)?;
            execution
                .output
                .open_regular_file(&relative, inputs.cancellation)
                .map(ReportInput::File)
                .map_err(|_| RuntimeSourceBuildReportOpenError)
        }
        "execution-receipt.json" => Ok(ReportInput::Memory(Cursor::new(
            managed.execution_receipt.clone(),
        ))),
        "output-tree.json" => Ok(ReportInput::Memory(Cursor::new(
            managed.output_tree.clone(),
        ))),
        "stdout.bin" => Ok(ReportInput::Memory(Cursor::new(
            managed.standard_output.clone(),
        ))),
        "stderr.bin" => Ok(ReportInput::Memory(Cursor::new(
            managed.standard_error.clone(),
        ))),
        _ => Err(RuntimeSourceBuildReportOpenError),
    }
}

fn open_member(
    inputs: &CompilationInputs<'_>,
    attempt: RuntimeSourceBuildAttempt,
    path: &ArtifactSetRelativePath,
) -> Result<File, MemberOpenError> {
    let execution = match attempt {
        RuntimeSourceBuildAttempt::Primary => inputs.primary,
        RuntimeSourceBuildAttempt::Rebuild => inputs.rebuild,
    };
    execution
        .output
        .open_regular_file(path, inputs.cancellation)
        .map_err(|_| MemberOpenError)
}

fn managed_evidence(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    execution: &RuntimeSourceBuildExecution,
    attempt: RuntimeSourceBuildAttempt,
    cancellation: &CancellationToken,
) -> Result<RuntimeSourceBuildManagedEvidence, RuntimeSourceBuildReportCompilationError> {
    let output_tree = execution
        .output
        .compile_portable_output_tree(cancellation)?;
    let execution_receipt = receipt::compile(bundle, execution, attempt)?;
    let streams = execution.managed().output().streams();
    if streams.standard_output_truncated() || streams.standard_error_truncated() {
        return Err(RuntimeSourceBuildReportCompilationError::OutputLimitExceeded);
    }
    Ok(RuntimeSourceBuildManagedEvidence {
        execution_receipt,
        output_tree: output_tree.canonical_bytes().to_vec(),
        standard_output: streams.standard_output().to_vec(),
        standard_error: streams.standard_error().to_vec(),
    })
}

fn validate_output_shape(
    snapshot: &ManagedTreeSnapshot,
    runtime: &ReconstructedRuntimePackage,
) -> Result<(), RuntimeSourceBuildReportCompilationError> {
    let mut expected = BTreeMap::new();
    for path in [
        "runtime-layout.json",
        "sbom.json",
        "provenance.json",
        "transformation.json",
    ] {
        insert_expected_file(&mut expected, path)?;
    }
    for member in runtime.layout().members() {
        insert_expected_file(&mut expected, member.relative_path().as_str())?;
    }
    if snapshot.entries().len() != expected.len() {
        return Err(RuntimeSourceBuildReportCompilationError::OutputTreeMismatch);
    }
    for entry in snapshot.entries() {
        if expected.get(entry.relative_path().as_str()) != Some(&entry.kind())
            || (entry.kind() == ManagedTreeEntryKind::RegularFile && !entry.has_single_link())
        {
            return Err(RuntimeSourceBuildReportCompilationError::OutputTreeMismatch);
        }
    }
    Ok(())
}

fn insert_expected_file(
    expected: &mut BTreeMap<String, ManagedTreeEntryKind>,
    path: &str,
) -> Result<(), RuntimeSourceBuildReportCompilationError> {
    let parts = path.split('/').collect::<Vec<_>>();
    let mut prefix = String::new();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        match expected.insert(prefix.clone(), ManagedTreeEntryKind::Directory) {
            Some(ManagedTreeEntryKind::RegularFile) => {
                return Err(RuntimeSourceBuildReportCompilationError::OutputTreeMismatch);
            }
            Some(ManagedTreeEntryKind::Directory) | None => {}
        }
    }
    match expected.insert(path.to_owned(), ManagedTreeEntryKind::RegularFile) {
        Some(_) => Err(RuntimeSourceBuildReportCompilationError::OutputTreeMismatch),
        None => Ok(()),
    }
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), RuntimeSourceBuildReportCompilationError> {
    if cancellation.is_cancelled() {
        Err(RuntimeSourceBuildReportCompilationError::Cancelled)
    } else {
        Ok(())
    }
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    path_is_within(left, right) || path_is_within(right, left)
}

fn path_is_within(path: &Path, ancestor: &Path) -> bool {
    let path = path.components().collect::<Vec<_>>();
    let ancestor = ancestor.components().collect::<Vec<_>>();
    path.len() >= ancestor.len()
        && path
            .iter()
            .zip(&ancestor)
            .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
}

fn map_snapshot(
    error: RuntimeSourceBuildExecutionError,
) -> RuntimeSourceBuildReportCompilationError {
    if matches!(
        &error,
        RuntimeSourceBuildExecutionError::OutputNotEmpty
            | RuntimeSourceBuildExecutionError::OutputTreeLimitExceeded
            | RuntimeSourceBuildExecutionError::OutputByteLimitExceeded
    ) {
        RuntimeSourceBuildReportCompilationError::LimitExceeded
    } else {
        RuntimeSourceBuildReportCompilationError::Output(error)
    }
}

/// Failure while deriving one two-attempt report from retained output objects.
#[derive(Debug, Error)]
pub enum RuntimeSourceBuildReportCompilationError {
    /// Caller-owned output tree ceilings were invalid or exceeded.
    #[error("runtime source-build report compilation limit was exceeded")]
    LimitExceeded,
    /// The selected attempt output roots overlap.
    #[error("runtime source-build attempt output boundaries overlap")]
    OutputOverlap,
    /// One output tree contained an absent, extra, indirect, or aliased entry.
    #[error("runtime source-build output tree does not match the derived package")]
    OutputTreeMismatch,
    /// One output tree changed while its report was compiled.
    #[error("runtime source-build output tree changed during report compilation")]
    OutputChanged,
    /// Managed evidence JSON could not be encoded canonically.
    #[error("runtime source-build managed evidence could not be encoded")]
    EvidenceEncoding,
    /// A managed stream exceeded the retained byte ceiling.
    #[error("runtime source-build output exceeded its stream limit")]
    OutputLimitExceeded,
    /// Cooperative cancellation was observed.
    #[error("runtime source-build report compilation was cancelled")]
    Cancelled,
    /// The frozen source-build bundle changed.
    #[error(transparent)]
    Bundle(#[from] RuntimeSourceBuildBundleError),
    /// A retained output boundary could not be revalidated or read.
    #[error(transparent)]
    Output(#[from] RuntimeSourceBuildExecutionError),
    /// Pure report compilation or runtime reconstruction failed.
    #[error(transparent)]
    Report(#[from] RuntimeSourceBuildReportError),
}

#[cfg(all(test, target_os = "linux"))]
#[path = "runtime_source_build_report/linux_tests.rs"]
mod linux_tests;
