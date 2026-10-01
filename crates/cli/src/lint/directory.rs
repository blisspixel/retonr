//! Read-only folder lint using the same bounded discovery contract as inspect.

use std::{
    fmt::Write as _,
    io::{self, Write},
    process::ExitCode,
};

use rewrite_app::{EditorialLintService, MAX_CANDIDATE_CHECK_BYTES, TextEncoding};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;

use super::{DocumentLintReport, LintRequest};
use crate::{
    contract::{CommandName, EXIT_POLICY, ReportFormat, SuccessEnvelope},
    failure::RunFailure,
    inspect_source::directory::{SkippedEntry, discover_cancellable},
    render::{escape_inline_for_display, to_safe_pretty_json},
};

const MAXIMUM_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_FINDINGS: usize = 16_384;

pub(super) fn run(request: &LintRequest, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    if request.candidate.is_some() {
        return Err(RunFailure::usage_for(CommandName::Lint));
    }
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())
        .map_err(|_| RunFailure::operational(CommandName::Lint))?;
    let report = build_report(request, &cancellation)?;
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes = to_safe_pretty_json(&SuccessEnvelope::new(CommandName::Lint, &report))
                .map_err(|_| RunFailure::operational(CommandName::Lint))?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => report.text().into_bytes(),
    };
    if cancellation.is_cancelled() {
        return Err(RunFailure::cancelled(CommandName::Lint));
    }
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&bytes)
        .and_then(|()| stdout.flush())
        .map_err(|_| RunFailure::operational(CommandName::Lint))?;
    Ok(if request.fail_on_findings && report.findings_count > 0 {
        ExitCode::from(EXIT_POLICY)
    } else {
        ExitCode::SUCCESS
    })
}

fn build_report(
    request: &LintRequest,
    cancellation: &CancellationToken,
) -> Result<DirectoryLintReport, RunFailure> {
    let discovery = discover_cancellable(
        &request.source,
        request.recursive,
        CommandName::Lint,
        cancellation,
    )?;
    let mut skipped = discovery.skipped;
    let mut documents = Vec::new();
    let mut findings_count = 0;
    let mut total_bytes = 0;
    for document in discovery.documents {
        if cancellation.is_cancelled() {
            return Err(RunFailure::cancelled(CommandName::Lint));
        }
        if document.encoding != TextEncoding::Utf8 {
            skipped.push(SkippedEntry {
                relative_path: Some(document.relative_path),
                reason: "unsupported_encoding",
            });
            continue;
        }
        let path = request.source.join(&document.relative_path);
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|error| RunFailure::lint_read(&error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(RunFailure::concurrent_modification(CommandName::Lint));
        }
        let remaining = MAXIMUM_TOTAL_BYTES
            .saturating_sub(total_bytes)
            .min(MAX_CANDIDATE_CHECK_BYTES);
        let bytes = crate::file_input::read_directory_bounded(
            &request.source,
            std::path::Path::new(&document.relative_path),
            remaining,
        )
        .map_err(|error| RunFailure::lint_read(&error))?;
        total_bytes = check_budget(total_bytes, bytes.len(), MAXIMUM_TOTAL_BYTES)?;
        if Digest::sha256(&bytes).as_str() != document.digest {
            return Err(RunFailure::concurrent_modification(CommandName::Lint));
        }
        let text = String::from_utf8(bytes).map_err(|_| RunFailure::lint_invalid_utf8())?;
        let findings = EditorialLintService::lint(&text);
        findings_count = check_budget(findings_count, findings.len(), MAXIMUM_FINDINGS)?;
        documents.push(DirectoryDocument {
            relative_path: document.relative_path,
            report: DocumentLintReport {
                status: status(findings.len()),
                findings_count: findings.len(),
                findings,
            },
        });
    }
    skipped.sort();
    Ok(DirectoryLintReport {
        scope: "directory",
        status: status(findings_count),
        recursion: if request.recursive { "bounded" } else { "none" },
        links: "not_followed",
        maximum_total_bytes: MAXIMUM_TOTAL_BYTES,
        maximum_findings: MAXIMUM_FINDINGS,
        document_count: documents.len(),
        findings_count,
        skipped_count: skipped.len(),
        documents,
        skipped,
    })
}

fn check_budget(current: usize, additional: usize, maximum: usize) -> Result<usize, RunFailure> {
    match current.checked_add(additional) {
        Some(total) if total <= maximum => Ok(total),
        _ => Err(RunFailure {
            command: CommandName::Lint,
            body: crate::contract::ErrorBody::new(
                crate::contract::ErrorCategory::Compatibility,
                crate::contract::ErrorCode::ResourceLimitExceeded,
                false,
            ),
            exit_code: ExitCode::from(crate::contract::EXIT_COMPATIBILITY),
            message: "directory lint exceeds the supported aggregate limit",
        }),
    }
}

const fn status(findings: usize) -> &'static str {
    if findings == 0 {
        "clean"
    } else {
        "findings_detected"
    }
}

#[derive(Serialize)]
struct DirectoryDocument {
    relative_path: String,
    #[serde(flatten)]
    report: DocumentLintReport,
}

#[derive(Serialize)]
struct DirectoryLintReport {
    scope: &'static str,
    status: &'static str,
    recursion: &'static str,
    links: &'static str,
    maximum_total_bytes: usize,
    maximum_findings: usize,
    document_count: usize,
    findings_count: usize,
    skipped_count: usize,
    documents: Vec<DirectoryDocument>,
    skipped: Vec<SkippedEntry>,
}

impl DirectoryLintReport {
    fn text(&self) -> String {
        let mut text = format!(
            "scope: {}\nstatus: {}\nrecursion: {}\nlinks: {}\ndocuments: {}\nfindings: {}\nskipped: {}\n",
            self.scope,
            self.status,
            self.recursion,
            self.links,
            self.document_count,
            self.findings_count,
            self.skipped_count
        );
        for document in &self.documents {
            let _ = writeln!(
                text,
                "document {} status={} findings={}",
                escape_inline_for_display(&document.relative_path),
                document.report.status,
                document.report.findings_count
            );
            for finding in &document.report.findings {
                let _ = writeln!(
                    text,
                    "  - [{}] '{}' (occurrence {}): {}",
                    finding.rule_id,
                    escape_inline_for_display(&finding.evidence),
                    finding.occurrence,
                    finding.message
                );
            }
        }
        for skipped in &self.skipped {
            let _ = writeln!(
                text,
                "skipped {} reason={}",
                skipped
                    .relative_path
                    .as_deref()
                    .map(escape_inline_for_display)
                    .unwrap_or_default(),
                skipped.reason
            );
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_limits_accept_the_boundary_and_refuse_overflow() {
        assert_eq!(check_budget(5, 5, 10).expect("exact boundary"), 10);
        assert!(check_budget(5, 6, 10).is_err());
        assert!(check_budget(usize::MAX, 1, usize::MAX).is_err());
    }

    #[test]
    fn cancelled_discovery_returns_no_partial_report() {
        let root = tempfile::tempdir().expect("temporary directory");
        std::fs::write(root.path().join("draft.txt"), "Certainly! Draft.").expect("write draft");
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let request = LintRequest {
            source: root.path().to_path_buf(),
            recursive: true,
            candidate: None,
            fail_on_findings: false,
        };
        let Err(failure) = build_report(&request, &cancellation) else {
            panic!("cancelled lint must refuse")
        };
        assert_eq!(
            failure.exit_code,
            ExitCode::from(crate::contract::EXIT_CANCELLED)
        );
    }
}
