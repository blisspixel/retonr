//! Deterministic editorial lint and anti-slop inspection with explicit input policy.

use std::{
    fmt::Write as _,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use rewrite_app::{
    EditorialComparison, EditorialFinding, EditorialLintService, MAX_CANDIDATE_CHECK_BYTES,
};
use serde::Serialize;

mod directory;

use crate::{
    contract::{
        CommandName, EXIT_POLICY, ReportFormat, STANDARD_STREAM_PATH, SuccessEnvelope,
        read_input_bounded,
    },
    failure::RunFailure,
};

/// Owned arguments for editorial lint inspection.
pub(crate) struct LintRequest {
    pub(crate) source: PathBuf,
    pub(crate) recursive: bool,
    pub(crate) candidate: Option<PathBuf>,
    pub(crate) fail_on_findings: bool,
}

/// Report for single-document editorial lint inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DocumentLintReport {
    /// Overall status: "clean" or "`findings_detected`".
    pub status: &'static str,
    /// Total count of detected findings.
    pub findings_count: usize,
    /// Detailed list of detected findings.
    pub findings: Vec<EditorialFinding>,
}

/// Report for comparative source-versus-candidate editorial lint inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparativeLintReport {
    /// Overall status: "clean", "`strict_improvement`", or "`findings_detected`".
    pub status: &'static str,
    /// Total findings detected in the source.
    pub source_findings_count: usize,
    /// Total findings detected in the candidate.
    pub candidate_findings_count: usize,
    /// Findings present in the source that were eliminated in the candidate.
    pub resolved_count: usize,
    /// Findings newly introduced by the candidate.
    pub introduced_count: usize,
    /// Findings retained unchanged from the source.
    pub retained_count: usize,
    /// Whether the candidate strictly resolves defects without introducing new ones.
    pub is_strict_improvement: bool,
    /// Full comparative findings breakdown.
    pub comparison: EditorialComparison,
}

/// Inspects one or two documents against deterministic editorial rules.
pub(crate) fn run(request: &LintRequest, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    if std::fs::symlink_metadata(&request.source)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return directory::run(request, format);
    }
    if request.recursive {
        return Err(RunFailure::usage_for(CommandName::Lint));
    }
    if let Some(candidate_path) = &request.candidate {
        run_comparative(
            &request.source,
            candidate_path,
            request.fail_on_findings,
            format,
        )
    } else {
        run_single(&request.source, request.fail_on_findings, format)
    }
}

fn run_single(
    source_path: &Path,
    fail_on_findings: bool,
    format: ReportFormat,
) -> Result<ExitCode, RunFailure> {
    let source_bytes = read_input_bounded(source_path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::lint_read(&error))?;
    let source_text =
        String::from_utf8(source_bytes).map_err(|_| RunFailure::lint_invalid_utf8())?;

    let findings = EditorialLintService::lint(&source_text);
    let findings_count = findings.len();
    let status = if findings_count == 0 {
        "clean"
    } else {
        "findings_detected"
    };

    let report = DocumentLintReport {
        status,
        findings_count,
        findings,
    };

    write_single_report(&report, format).map_err(|_| RunFailure::operational(CommandName::Lint))?;

    if fail_on_findings && findings_count > 0 {
        Ok(ExitCode::from(EXIT_POLICY))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn run_comparative(
    source_path: &Path,
    candidate_path: &Path,
    fail_on_findings: bool,
    format: ReportFormat,
) -> Result<ExitCode, RunFailure> {
    if is_standard_stream(source_path) && is_standard_stream(candidate_path) {
        return Err(RunFailure::usage_for(CommandName::Lint));
    }

    let source_bytes = read_input_bounded(source_path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::lint_read(&error))?;
    let source_text =
        String::from_utf8(source_bytes).map_err(|_| RunFailure::lint_invalid_utf8())?;

    let candidate_bytes = read_input_bounded(candidate_path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::lint_read(&error))?;
    let candidate_text =
        String::from_utf8(candidate_bytes).map_err(|_| RunFailure::lint_invalid_utf8())?;

    let comparison = EditorialLintService::compare(&source_text, &candidate_text);
    let is_strict_improvement = comparison.is_strict_improvement();
    let status = if comparison.candidate_findings.is_empty() {
        "clean"
    } else if is_strict_improvement {
        "strict_improvement"
    } else {
        "findings_detected"
    };

    let has_introduced = comparison.has_introduced_defects();
    let report = ComparativeLintReport {
        status,
        source_findings_count: comparison.source_findings.len(),
        candidate_findings_count: comparison.candidate_findings.len(),
        resolved_count: comparison.resolved_findings.len(),
        introduced_count: comparison.introduced_findings.len(),
        retained_count: comparison.retained_findings.len(),
        is_strict_improvement,
        comparison,
    };

    write_comparative_report(&report, format)
        .map_err(|_| RunFailure::operational(CommandName::Lint))?;

    if fail_on_findings && has_introduced {
        Ok(ExitCode::from(EXIT_POLICY))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn is_standard_stream(path: &Path) -> bool {
    path.as_os_str() == STANDARD_STREAM_PATH
}

fn write_single_report(report: &DocumentLintReport, format: ReportFormat) -> io::Result<()> {
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes = crate::render::to_safe_pretty_json(&SuccessEnvelope::new(
                CommandName::Lint,
                report,
            ))
            .map_err(io::Error::other)?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => {
            let mut text = String::new();
            let _ = writeln!(text, "status: {}", report.status);
            let _ = writeln!(text, "findings: {}", report.findings_count);
            for finding in &report.findings {
                let _ = writeln!(
                    text,
                    "  - [{}] '{}' (occurrence {}): {}",
                    finding.rule_id,
                    crate::render::escape_inline_for_display(&finding.evidence),
                    finding.occurrence,
                    finding.message
                );
            }
            text.into_bytes()
        }
    };
    let mut stdout = io::stdout().lock();
    stdout.write_all(&bytes)?;
    stdout.flush()
}

fn write_comparative_report(
    report: &ComparativeLintReport,
    format: ReportFormat,
) -> io::Result<()> {
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes = crate::render::to_safe_pretty_json(&SuccessEnvelope::new(
                CommandName::Lint,
                report,
            ))
            .map_err(io::Error::other)?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => {
            let mut text = String::new();
            let _ = writeln!(text, "status: {}", report.status);
            let _ = writeln!(
                text,
                "comparison: {} resolved, {} introduced, {} retained (strict improvement: {})",
                report.resolved_count,
                report.introduced_count,
                report.retained_count,
                report.is_strict_improvement
            );
            if !report.comparison.introduced_findings.is_empty() {
                let _ = writeln!(text, "introduced_findings:");
                for finding in &report.comparison.introduced_findings {
                    let _ = writeln!(
                        text,
                        "  - [{}] '{}' (occurrence {}): {}",
                        finding.rule_id,
                        crate::render::escape_inline_for_display(&finding.evidence),
                        finding.occurrence,
                        finding.message
                    );
                }
            }
            text.into_bytes()
        }
    };
    let mut stdout = io::stdout().lock();
    stdout.write_all(&bytes)?;
    stdout.flush()
}
