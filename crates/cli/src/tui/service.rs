//! Read-only file review through the existing application services.

use std::path::Path;

use rewrite_app::{
    DocumentReviewError, DocumentReviewRequest, DocumentReviewService, EditorialReview,
    MAX_CANDIDATE_CHECK_BYTES, PlainTextInventory, TextEncoding, inspect_plain_text,
};
use rewrite_types::{CancellationToken, RewriteRecord};

use super::{TuiArgs, state};
use crate::{
    contract::{
        CommandName, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode, STANDARD_STREAM_PATH,
    },
    failure::RunFailure,
};

#[cfg(test)]
const PREVIEW_BYTES: usize = 65_536;
#[cfg(test)]
const MAX_DOMAIN_FINDINGS: usize = 4096;
#[cfg(test)]
use rewrite_app::{CandidateCheckRequest, CandidateCheckService, EditorialLintService};

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

type EditorialSnapshot = EditorialReview;

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
            directory: None,
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
    let source = read_document(&request.source, cancellation)?;
    let candidate = request
        .candidate
        .as_ref()
        .map(|path| read_document(path, cancellation))
        .transpose()?;
    let snapshot = load_bytes(request, source, candidate, cancellation)?;
    after_lint();
    ensure_active(cancellation)?;
    Ok(snapshot)
}

pub(super) fn load_bytes(
    request: &TuiArgs,
    source: Vec<u8>,
    candidate: Option<Vec<u8>>,
    cancellation: &CancellationToken,
) -> Result<Snapshot, RunFailure> {
    let review = DocumentReviewService::review(
        DocumentReviewRequest {
            source,
            candidate,
            protected_terms: request.protected_terms.clone(),
        },
        cancellation,
    )
    .map_err(|error| match error {
        DocumentReviewError::CandidateRequired => RunFailure::usage_for(CommandName::Tui),
        DocumentReviewError::UnsupportedEncoding => unsupported_text(),
        DocumentReviewError::Cancelled => RunFailure::cancelled(CommandName::Tui),
        DocumentReviewError::FindingLimitExceeded => resource_limit(),
        DocumentReviewError::Application(error) => RunFailure::app(CommandName::Tui, &error),
    })?;
    Ok(Snapshot {
        source_label: prefix(&request.source.to_string_lossy(), 512).to_owned(),
        candidate_label: request
            .candidate
            .as_ref()
            .map(|path| prefix(&path.to_string_lossy(), 512).to_owned()),
        source: review.source_preview,
        candidate: review.candidate_preview,
        inspection: review.inspection,
        candidate_inspection: review.candidate_inspection,
        check: review.check,
        editorial: review.editorial,
    })
}

fn read_document(path: &Path, cancellation: &CancellationToken) -> Result<Vec<u8>, RunFailure> {
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
    ensure_active(cancellation)?;
    Ok(bytes)
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

#[cfg(test)]
mod tests;
