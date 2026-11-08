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
    failure::{InputRole, RunFailure},
};

/// Owned arguments for editorial lint inspection.
pub(crate) struct LintRequest {
    pub(crate) source: PathBuf,
    pub(crate) recursive: bool,
    pub(crate) candidate: Option<PathBuf>,
    pub(crate) fail_on_findings: bool,
    /// Color choice for the human report.
    pub(crate) color: crate::presentation::ColorChoice,
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
            request.color,
            format,
        )
    } else {
        run_single(
            &request.source,
            request.fail_on_findings,
            request.color,
            format,
        )
    }
}

fn run_single(
    source_path: &Path,
    fail_on_findings: bool,
    color: crate::presentation::ColorChoice,
    format: ReportFormat,
) -> Result<ExitCode, RunFailure> {
    let source_bytes = read_input_bounded(source_path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::lint_read_at(InputRole::Source, source_path, &error))?;
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

    write_single_report(&report, color, format)
        .map_err(|_| RunFailure::operational(CommandName::Lint))?;

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
    color: crate::presentation::ColorChoice,
    format: ReportFormat,
) -> Result<ExitCode, RunFailure> {
    if is_standard_stream(source_path) && is_standard_stream(candidate_path) {
        return Err(RunFailure::usage_for(CommandName::Lint));
    }

    let source_bytes = read_input_bounded(source_path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::lint_read_at(InputRole::Source, source_path, &error))?;
    let source_text =
        String::from_utf8(source_bytes).map_err(|_| RunFailure::lint_invalid_utf8())?;

    let candidate_bytes = read_input_bounded(candidate_path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::lint_read_at(InputRole::Candidate, candidate_path, &error))?;
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

    write_comparative_report(&report, color, format)
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

pub(super) fn format_finding_line(finding: &EditorialFinding) -> String {
    let location = finding.span.as_ref().map_or_else(
        || "line ? col ?".to_owned(),
        |span| format!("line {} col {}", span.line, span.column),
    );
    format!(
        "{location}  {}  {}\n  {}",
        finding.rule_id,
        finding.message,
        crate::render::escape_inline_for_display(&finding.evidence)
    )
}

pub(super) fn findings_in_document_order(findings: &[EditorialFinding]) -> Vec<&EditorialFinding> {
    let mut ordered: Vec<&EditorialFinding> = findings.iter().collect();
    ordered.sort_by(|left, right| {
        let left_at = left
            .span
            .map_or((u32::MAX, u32::MAX), |span| (span.line, span.column));
        let right_at = right
            .span
            .map_or((u32::MAX, u32::MAX), |span| (span.line, span.column));
        left_at
            .cmp(&right_at)
            .then(left.rule_id.cmp(&right.rule_id))
            .then(left.occurrence.cmp(&right.occurrence))
    });
    ordered
}

fn write_single_report(
    report: &DocumentLintReport,
    color: crate::presentation::ColorChoice,
    format: ReportFormat,
) -> io::Result<()> {
    let bytes = match format {
        ReportFormat::Json => json_report(report)?,
        ReportFormat::Text => {
            render_lint_text(report.findings_count, &report.findings, color).into_bytes()
        }
    };
    write_stdout(&bytes)
}

fn write_comparative_report(
    report: &ComparativeLintReport,
    color: crate::presentation::ColorChoice,
    format: ReportFormat,
) -> io::Result<()> {
    let bytes = match format {
        ReportFormat::Json => json_report(report)?,
        ReportFormat::Text => render_comparative_text(report, color).into_bytes(),
    };
    write_stdout(&bytes)
}

fn json_report(report: &impl Serialize) -> io::Result<Vec<u8>> {
    let mut bytes =
        crate::render::to_safe_pretty_json(&SuccessEnvelope::new(CommandName::Lint, report))
            .map_err(io::Error::other)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes)?;
    stdout.flush()
}

fn render_lint_text(
    findings_count: usize,
    findings: &[EditorialFinding],
    color: crate::presentation::ColorChoice,
) -> String {
    let mut text = String::new();
    if findings_count == 0 {
        let _ = writeln!(text, "clean       no findings");
    } else {
        let noun = if findings_count == 1 {
            "finding"
        } else {
            "findings"
        };
        let _ = writeln!(text, "{findings_count} {noun}");
        for finding in findings_in_document_order(findings) {
            let _ = writeln!(text, "{}", format_finding_line(finding));
        }
    }
    finish_lint_text(&text, color)
}

fn render_comparative_text(
    report: &ComparativeLintReport,
    color: crate::presentation::ColorChoice,
) -> String {
    let mut text = String::new();
    if report.candidate_findings_count == 0 {
        let _ = writeln!(text, "clean       no findings");
    } else {
        let noun = if report.candidate_findings_count == 1 {
            "finding"
        } else {
            "findings"
        };
        let _ = writeln!(text, "{} {noun}", report.candidate_findings_count);
    }
    let _ = writeln!(
        text,
        "comparison: {} resolved, {} introduced, {} retained (strict improvement: {})",
        report.resolved_count,
        report.introduced_count,
        report.retained_count,
        report.is_strict_improvement
    );
    if !report.comparison.introduced_findings.is_empty() {
        let _ = writeln!(text, "introduced:");
        for finding in findings_in_document_order(&report.comparison.introduced_findings) {
            let _ = writeln!(text, "{}", format_finding_line(finding));
        }
    }
    finish_lint_text(&text, color)
}

pub(super) fn finish_lint_text(text: &str, color: crate::presentation::ColorChoice) -> String {
    let terminal = crate::presentation::stdout_is_terminal();
    let wrapped = crate::presentation::wrap(text, crate::presentation::terminal_width(terminal));
    crate::presentation::paint_report(
        &wrapped,
        crate::presentation::color_enabled(color, terminal),
    )
}

#[cfg(test)]
mod tests {
    use rewrite_types::{EditorialFinding, FindingSpan};

    use super::format_finding_line;

    #[test]
    fn finding_line_names_the_phrase_location_and_escapes_evidence() {
        let mut finding = EditorialFinding::new(
            "conversational_residue",
            "Sure\nthing",
            0,
            "Conversational residue opening",
        );
        finding.span = Some(FindingSpan {
            start: 0,
            end: 9,
            line: 2,
            column: 4,
        });
        let line = format_finding_line(&finding);
        assert_eq!(
            line,
            "line 2 col 4  conversational_residue  Conversational residue opening\n  Sure\\nthing"
        );
        assert_eq!(line.matches('\n').count(), 1);
        assert!(!line.contains("Sure\nthing"));
    }

    #[test]
    fn findings_are_ordered_by_line_then_column() {
        let later = finding_at(3, 1, "later");
        let earlier = finding_at(1, 4, "earlier");
        let findings = [later, earlier];
        let ordered = super::findings_in_document_order(&findings);
        assert_eq!(ordered[0].evidence, "earlier");
        assert_eq!(ordered[1].evidence, "later");
    }

    fn finding_at(line: u32, column: u32, evidence: &str) -> EditorialFinding {
        let mut finding = EditorialFinding::new("rule", evidence, 0, "message");
        finding.span = Some(FindingSpan {
            start: 0,
            end: 1,
            line,
            column,
        });
        finding
    }
}
