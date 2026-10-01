//! Read-only file review through the existing application services.

use std::path::Path;

use rewrite_app::{
    CandidateCheckRequest, CandidateCheckService, EditorialLintService, MAX_CANDIDATE_CHECK_BYTES,
    PlainTextInventory, TextEncoding, inspect_plain_text,
};
use rewrite_types::{
    CancellationToken, EditorialComparison, EditorialFinding, ReasonCode, RewriteRecord,
};

use super::{TuiArgs, state};
use crate::{
    contract::{
        CommandName, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode, STANDARD_STREAM_PATH,
    },
    failure::RunFailure,
};

const MAX_DOMAIN_FINDINGS: usize = 4096;
const PREVIEW_BYTES: usize = 65_536;

/// Owned domain result. Exact records never become a terminal rendering path.
pub(super) struct Snapshot {
    source_label: String,
    candidate_label: Option<String>,
    source: String,
    candidate: Option<String>,
    inspection: PlainTextInventory,
    candidate_inspection: Option<PlainTextInventory>,
    check: Option<RewriteRecord>,
    editorial: EditorialSnapshot,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Snapshot")
            .field("source_digest", &self.inspection.digest)
            .field("candidate_check_present", &self.check.is_some())
            .finish_non_exhaustive()
    }
}

enum EditorialSnapshot {
    Document(Vec<EditorialFinding>),
    Comparison(EditorialComparison),
}

impl Snapshot {
    /// Converts only bounded preview fields through the shared terminal sanitizer.
    pub(super) fn into_presentation(self) -> state::Snapshot {
        let (source_count, candidate_count, displayed) = match &self.editorial {
            EditorialSnapshot::Document(findings) => (findings.len(), None, findings.as_slice()),
            EditorialSnapshot::Comparison(comparison) => (
                comparison.source_findings.len(),
                Some(comparison.candidate_findings.len()),
                comparison.candidate_findings.as_slice(),
            ),
        };
        let mut findings = vec![format!(
            "Source: {} bytes; {source_count} editorial findings.",
            self.inspection.byte_size
        )];
        findings.push(format!(
            "Source inventory: UTF-8; BOM: {}; line endings: {:?}.",
            self.inspection.utf8_bom, self.inspection.line_endings
        ));
        findings.push(format!("Source digest: {}", self.inspection.digest));
        if let Some(inventory) = &self.candidate_inspection {
            findings.push(format!(
                "Proposed candidate: {} bytes; {} editorial findings.",
                inventory.byte_size,
                candidate_count.unwrap_or_default()
            ));
        }
        if let EditorialSnapshot::Comparison(comparison) = &self.editorial {
            findings.push(format!("Editorial comparison: {} resolved, {} introduced, {} retained. Strict improvement: {}.", comparison.resolved_findings.len(), comparison.introduced_findings.len(), comparison.retained_findings.len(), comparison.is_strict_improvement()));
        }
        findings.extend(
            displayed
                .iter()
                .take(state::FINDING_LIMIT + 1)
                .map(|finding| {
                    format!(
                        "{}: {} [evidence: {}]",
                        prefix(&finding.rule_id, 128),
                        prefix(&finding.message, 256),
                        prefix(&finding.evidence, 256)
                    )
                }),
        );
        let status = self.check.as_ref().map_or_else(
            || "Read-only inspection; local generation unavailable in this view.".to_owned(),
            |record| format!("Candidate check: {}; reason: {}. Read-only; local generation unavailable in this view.", crate::check::report::status_name(record.status), record.reason.map_or("none", crate::check::report::reason_name)),
        );
        state::Snapshot {
            source_label: self.source_label,
            candidate_label: self.candidate_label,
            source: self.source,
            candidate: self.candidate,
            findings,
            status,
        }
        .bounded()
    }
}

/// Loads regular UTF-8 files without stdin, writes, signals, or generation.
pub(super) fn load(
    request: &TuiArgs,
    cancellation: &CancellationToken,
) -> Result<Snapshot, RunFailure> {
    load_with_post_lint(request, cancellation, || {})
}

fn load_with_post_lint(
    request: &TuiArgs,
    cancellation: &CancellationToken,
    after_lint: impl FnOnce(),
) -> Result<Snapshot, RunFailure> {
    ensure_active(cancellation)?;
    if request.source.as_os_str() == STANDARD_STREAM_PATH
        || request
            .candidate
            .as_ref()
            .is_some_and(|path| path.as_os_str() == STANDARD_STREAM_PATH)
        || (request.candidate.is_none() && !request.protected_terms.is_empty())
    {
        return Err(RunFailure::usage_for(CommandName::Tui));
    }
    let (source, inspection) = read_document(&request.source, cancellation)?;
    let candidate = request
        .candidate
        .as_ref()
        .map(|path| read_document(path, cancellation))
        .transpose()?;
    let (check, editorial) = if let Some((text, _)) = &candidate {
        let result = CandidateCheckService::check_with_cancellation(
            CandidateCheckRequest::new(
                source.as_bytes().to_vec(),
                text.clone(),
                request.protected_terms.clone(),
            ),
            cancellation,
        )
        .map_err(|error| RunFailure::app(CommandName::Tui, &error))?;
        if result.record.reason == Some(ReasonCode::Cancelled) {
            return Err(RunFailure::cancelled(CommandName::Tui));
        }
        ensure_active(cancellation)?;
        let comparison = EditorialLintService::compare(&source, text);
        (
            Some(result.record),
            EditorialSnapshot::Comparison(comparison),
        )
    } else {
        (
            None,
            EditorialSnapshot::Document(EditorialLintService::lint(&source)),
        )
    };
    // The lint kernel is deliberately reused unchanged. A cancellation that
    // arrives during its noncooperative work discards the entire result here.
    after_lint();
    ensure_active(cancellation)?;
    let finding_count = match &editorial {
        EditorialSnapshot::Document(findings) => findings.len(),
        EditorialSnapshot::Comparison(comparison) => comparison
            .source_findings
            .len()
            .saturating_add(comparison.candidate_findings.len()),
    };
    if finding_count > MAX_DOMAIN_FINDINGS {
        return Err(resource_limit());
    }
    let snapshot = Snapshot {
        source_label: prefix(&request.source.to_string_lossy(), 512).to_owned(),
        candidate_label: request
            .candidate
            .as_ref()
            .map(|path| prefix(&path.to_string_lossy(), 512).to_owned()),
        source: preview(&source),
        candidate: candidate.as_ref().map(|(text, _)| preview(text)),
        inspection,
        candidate_inspection: candidate.map(|(_, inventory)| inventory),
        check,
        editorial,
    };
    ensure_active(cancellation)?;
    Ok(snapshot)
}

fn read_document(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<(String, PlainTextInventory), RunFailure> {
    ensure_active(cancellation)?;
    let bytes = crate::file_input::read_regular_bounded(path, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::input_read(CommandName::Tui, &error))?;
    ensure_active(cancellation)?;
    let inventory =
        inspect_plain_text(&bytes).map_err(|error| RunFailure::app(CommandName::Tui, &error))?;
    if inventory.encoding != TextEncoding::Utf8 {
        return Err(unsupported_text());
    }
    if crate::inspect_source::requires_derivative_decision(path, &bytes, CommandName::Tui)? {
        return Err(RunFailure {
            command: CommandName::Tui,
            body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
            exit_code: std::process::ExitCode::from(EXIT_COMPATIBILITY),
            message: "document metadata requires an explicit derivative decision",
        });
    }
    let text = String::from_utf8(bytes).map_err(|_| unsupported_text())?;
    ensure_active(cancellation)?;
    Ok((text, inventory))
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), RunFailure> {
    if cancellation.is_cancelled() {
        Err(RunFailure::cancelled(CommandName::Tui))
    } else {
        Ok(())
    }
}

fn resource_limit() -> RunFailure {
    RunFailure {
        command: CommandName::Tui,
        body: ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false,
        ),
        exit_code: std::process::ExitCode::from(EXIT_COMPATIBILITY),
        message: "review exceeds the 4096 finding limit; use lint for full results",
    }
}

fn unsupported_text() -> RunFailure {
    RunFailure {
        command: CommandName::Tui,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
        exit_code: std::process::ExitCode::from(crate::contract::EXIT_USAGE),
        message: "input text is not a supported UTF-8 document",
    }
}

fn prefix(value: &str, maximum: usize) -> &str {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn preview(value: &str) -> String {
    if value.len() <= PREVIEW_BYTES {
        return value.to_owned();
    }
    let marker = "\n[Document preview truncated; validation used the complete input.]";
    let mut preview = prefix(value, PREVIEW_BYTES.saturating_sub(marker.len())).to_owned();
    preview.push_str(marker);
    preview
}

#[cfg(test)]
mod tests;
