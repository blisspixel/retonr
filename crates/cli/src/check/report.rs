use std::io::{self, Write};

use rewrite_app::StyleDistance;
use rewrite_types::{
    EditLevel, GateEvidenceDetails, ReasonCode, RewriteRecord, RewriteStatus, SpanChange,
};

use super::ReportTarget;
use crate::contract::{CommandName, ReportFormat, SuccessEnvelope};

/// Writes one versioned report to the stream that does not carry document bytes.
/// Literal semantic tolerance reported beside a check result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FidelityReport {
    /// Evaluator that actually decided semantic acceptance.
    pub semantic_evaluator: &'static str,
    /// Level the evaluator enforces.
    pub edit_level_effective: &'static str,
    /// Level the caller requested, when the flag was present.
    pub edit_level_requested: Option<&'static str>,
    /// Present when the requested level is wider than the evaluator.
    pub note: Option<&'static str>,
}

/// Builds the check report's effective-level fields.
#[must_use]
pub(crate) fn fidelity_for(level: Option<EditLevel>) -> FidelityReport {
    let note = match level {
        Some(EditLevel::VoicePass) => Some(
            "edit level voice-pass requested; the current evaluator accepts touch-up changes only (punctuation and capitalization)",
        ),
        Some(EditLevel::Rewrite) => Some(
            "edit level rewrite requested; the current evaluator accepts touch-up changes only (punctuation and capitalization)",
        ),
        Some(EditLevel::Reconstruct) => Some(
            "edit level reconstruct requested; the current evaluator accepts touch-up changes only (punctuation and capitalization)",
        ),
        Some(EditLevel::TouchUp) | None => None,
    };
    FidelityReport {
        semantic_evaluator: "literal-token-sequence-v1",
        edit_level_effective: "touch-up",
        edit_level_requested: level.map(edit_level_name),
        note,
    }
}

/// Change share and optional style distance for one check report.
pub(crate) struct CheckMeasurement<'a> {
    pub(crate) changed_token_pct: u8,
    pub(crate) changed_char_pct: u8,
    pub(crate) changed_word_count: u64,
    pub(crate) style_distance: Option<&'a StyleDistance>,
}

/// Human-report choices that do not change the JSON envelope.
pub(crate) struct ReportContext<'a> {
    pub(crate) format: ReportFormat,
    pub(crate) target: ReportTarget,
    pub(crate) backup: Option<&'a str>,
    pub(crate) fidelity: Option<FidelityReport>,
    pub(crate) measurement: Option<&'a CheckMeasurement<'a>>,
    pub(crate) verbose: bool,
    pub(crate) color: bool,
    pub(crate) width: Option<usize>,
    pub(crate) emission: ReportEmission,
}

/// Whether the accepted document was written, withheld by dry-run, or not requested.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReportEmission {
    /// The accepted bytes were written.
    Written,
    /// Dry-run computed the report and wrote nothing.
    DryRun,
    /// The caller did not ask for an output path.
    NotRequested,
}

/// Chooses the human verdict from dry-run and whether an output was requested.
#[must_use]
pub(crate) const fn emission(dry_run: bool, writes_document: bool) -> ReportEmission {
    if dry_run {
        ReportEmission::DryRun
    } else if writes_document {
        ReportEmission::Written
    } else {
        ReportEmission::NotRequested
    }
}

const fn edit_level_name(level: EditLevel) -> &'static str {
    match level {
        EditLevel::TouchUp => "touch-up",
        EditLevel::VoicePass => "voice-pass",
        EditLevel::Rewrite => "rewrite",
        EditLevel::Reconstruct => "reconstruct",
    }
}

pub(crate) fn write(
    command: CommandName,
    record: &RewriteRecord,
    context: &ReportContext<'_>,
) -> io::Result<()> {
    let bytes = render(command, record, context)?;
    match context.target {
        ReportTarget::Data => {
            let mut stream = io::stdout().lock();
            stream.write_all(&bytes)?;
            stream.flush()
        }
        ReportTarget::Diagnostic => {
            let mut stream = io::stderr().lock();
            stream.write_all(&bytes)?;
            stream.flush()
        }
    }
}

fn render(
    command: CommandName,
    record: &RewriteRecord,
    context: &ReportContext<'_>,
) -> io::Result<Vec<u8>> {
    match context.format {
        ReportFormat::Json => {
            let mut bytes = crate::render::to_safe_pretty_json(&SuccessEnvelope::new(
                command,
                DocumentReport {
                    record,
                    backup: context.backup,
                    semantic_evaluator: context.fidelity.map(|item| item.semantic_evaluator),
                    edit_level_effective: context.fidelity.map(|item| item.edit_level_effective),
                    edit_level_requested: context
                        .fidelity
                        .and_then(|item| item.edit_level_requested),
                    edit_level_note: context.fidelity.and_then(|item| item.note),
                    changed_token_pct: context.measurement.map(|item| item.changed_token_pct),
                    changed_char_pct: context.measurement.map(|item| item.changed_char_pct),
                    style_distance: context.measurement.and_then(|item| item.style_distance),
                },
            ))
            .map_err(io::Error::other)?;
            bytes.push(b'\n');
            Ok(bytes)
        }
        ReportFormat::Text => Ok(render_text(record, context).into_bytes()),
    }
}

#[derive(serde::Serialize)]
struct DocumentReport<'a> {
    #[serde(flatten)]
    record: &'a RewriteRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    semantic_evaluator: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_effective: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_requested: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_note: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_token_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    changed_char_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    style_distance: Option<&'a StyleDistance>,
}

fn render_text(record: &RewriteRecord, context: &ReportContext<'_>) -> String {
    use std::fmt::Write as _;

    let eligible = record
        .assessments
        .iter()
        .filter(|assessment| assessment.eligible)
        .count();
    let mut text = String::new();
    let _ = writeln!(
        text,
        "{}",
        verdict_line(record.status, eligible, context.emission)
    );
    if let Some(reason) = record.reason {
        let _ = writeln!(text, "reason: {}", reason_name(reason));
    }
    append_span_lines(&mut text, record);
    if let Some(measurement) = context.measurement {
        let _ = writeln!(text, "changed_token_pct: {}", measurement.changed_token_pct);
        let _ = writeln!(text, "changed_char_pct: {}", measurement.changed_char_pct);
        if let Some(style) = measurement.style_distance {
            append_style_lines(&mut text, style);
        }
    }
    if let Some(fidelity) = context.fidelity {
        let _ = writeln!(text, "semantic_evaluator: {}", fidelity.semantic_evaluator);
        let _ = writeln!(
            text,
            "edit_level_effective: {}",
            fidelity.edit_level_effective
        );
        if let Some(requested) = fidelity.edit_level_requested {
            let _ = writeln!(text, "edit_level_requested: {requested}");
        }
        if let Some(note) = fidelity.note {
            let _ = writeln!(text, "note: {note}");
        }
    }
    let _ = writeln!(text, "candidates: {}", record.assessments.len());
    let _ = writeln!(text, "eligible_candidates: {eligible}");
    if let Some(backup) = context.backup {
        let _ = writeln!(
            text,
            "backup: {}",
            crate::render::escape_inline_for_display(backup)
        );
    }
    if context.verbose {
        append_digests(&mut text, record);
    }
    let _ = writeln!(
        text,
        "{}",
        fidelity_receipt(
            record,
            context.measurement.map(|item| item.changed_word_count),
            context.width,
        )
    );
    let wrapped = crate::presentation::wrap(&text, context.width);
    crate::presentation::paint_report(&wrapped, context.color)
}

fn verdict_line(status: RewriteStatus, eligible: usize, emission: ReportEmission) -> String {
    match status {
        RewriteStatus::Rewritten if matches!(emission, ReportEmission::Written) => {
            "accepted    candidate passed".to_owned()
        }
        RewriteStatus::Rewritten if matches!(emission, ReportEmission::DryRun) => {
            "accepted    candidate passed; nothing written".to_owned()
        }
        RewriteStatus::Rewritten => {
            let count = eligible.max(1);
            let noun = if count == 1 {
                "candidate"
            } else {
                "candidates"
            };
            format!("accepted    {count} {noun} passed; nothing written (use -o)")
        }
        RewriteStatus::Abstained => "abstained   original kept unchanged".to_owned(),
        RewriteStatus::UnchangedIdentical => {
            "unchanged   source and candidate already match".to_owned()
        }
        RewriteStatus::UnchangedNoEligibleContent => "unchanged   no eligible content".to_owned(),
        RewriteStatus::Failed => "failed      the check did not finish".to_owned(),
    }
}

fn append_digests(text: &mut String, record: &RewriteRecord) {
    use std::fmt::Write as _;
    let source = short_digest(record.source_digest.as_str());
    if record.output_digest.as_str() == record.source_digest.as_str() {
        let _ = writeln!(text, "output identical to source");
        return;
    }
    let output = short_digest(record.output_digest.as_str());
    let _ = writeln!(text, "source_digest: {source}");
    let _ = writeln!(text, "output_digest: {output}");
}

pub(crate) fn fidelity_receipt(
    record: &RewriteRecord,
    changed_word_count: Option<u64>,
    width: Option<usize>,
) -> String {
    let summary = invariant_summary(record);
    let protected = format!(
        "{} numbers, {} urls, {} emails, {} terms",
        summary.numbers, summary.urls, summary.emails, summary.declared_terms
    );
    match record.status {
        RewriteStatus::Rewritten => match changed_word_count {
            Some(count) => {
                let noun = if count == 1 { "word" } else { "words" };
                format!("passed    {count} {noun} changed; protected values intact: {protected}")
            }
            None => format!("passed    protected values intact: {protected}"),
        },
        RewriteStatus::Failed => "failed     no document was accepted".to_owned(),
        RewriteStatus::Abstained
        | RewriteStatus::UnchangedIdentical
        | RewriteStatus::UnchangedNoEligibleContent => {
            let digest = short_digest(record.source_digest.as_str());
            let full = format!("kept      original, byte-identical (sha256 {digest})");
            if width.is_some_and(|width| full.chars().count() > width) {
                "kept      original, byte-identical".to_owned()
            } else {
                full
            }
        }
    }
}

fn invariant_summary(record: &RewriteRecord) -> rewrite_types::InvariantEvidenceSummary {
    for assessment in &record.assessments {
        for gate in &assessment.gates {
            for evidence in &gate.evidence {
                if let Some(GateEvidenceDetails::InvariantSummary(summary)) = &evidence.details {
                    return *summary;
                }
            }
        }
    }
    rewrite_types::InvariantEvidenceSummary {
        declared_terms: 0,
        urls: 0,
        emails: 0,
        numbers: 0,
        total: 0,
    }
}

fn short_digest(digest: &str) -> String {
    digest.chars().take(12).collect()
}

pub(crate) fn append_style_lines(text: &mut String, style: &StyleDistance) {
    use std::fmt::Write as _;
    let _ = writeln!(text, "style_distance: {}", style.kind);
    let _ = writeln!(text, "style_notice: {}", style.notice);
    let _ = writeln!(text, "style_source_distance: {}", style.source_distance);
    let _ = writeln!(text, "style_output_distance: {}", style.output_distance);
    for feature in &style.features {
        let _ = writeln!(
            text,
            "style_feature {} sample={} source={} output={} delta={}",
            feature.name,
            feature.sample,
            feature.source,
            feature.output,
            feature.output_minus_source
        );
    }
}

/// First located span, using the source point when the record has one.
pub(crate) struct LocatedSpan {
    pub line: u32,
    pub column: u32,
    pub kind: String,
    pub change: &'static str,
}

/// Spans in the same order as a text check report.
#[must_use]
pub(crate) fn located_spans(record: &RewriteRecord) -> Vec<LocatedSpan> {
    let mut spans = Vec::new();
    for assessment in &record.assessments {
        for gate in &assessment.gates {
            for evidence in &gate.evidence {
                let Some(GateEvidenceDetails::SpanFailures { failures }) = &evidence.details else {
                    continue;
                };
                for failure in failures {
                    let Some(point) = failure.source.or(failure.candidate) else {
                        continue;
                    };
                    spans.push(LocatedSpan {
                        line: point.line,
                        column: point.column,
                        kind: failure.kind.clone(),
                        change: span_change_name(failure.change),
                    });
                }
            }
        }
    }
    spans
}

/// The span a review status names. Later spans stay in the check report.
#[must_use]
pub(crate) fn first_located_span(record: &RewriteRecord) -> Option<LocatedSpan> {
    located_spans(record).into_iter().next()
}

pub(crate) fn append_span_lines(text: &mut String, record: &RewriteRecord) {
    use std::fmt::Write as _;
    for span in located_spans(record) {
        let _ = writeln!(
            text,
            "line {} col {}: {} {}",
            span.line, span.column, span.kind, span.change
        );
    }
}

const fn span_change_name(change: SpanChange) -> &'static str {
    match change {
        SpanChange::Altered => "altered",
        SpanChange::Missing => "missing",
        SpanChange::Added => "added",
        SpanChange::Moved => "moved",
        SpanChange::Introduced => "introduced",
    }
}

pub(crate) const fn status_name(status: RewriteStatus) -> &'static str {
    match status {
        RewriteStatus::Rewritten => "rewritten",
        RewriteStatus::UnchangedNoEligibleContent => "unchanged_no_eligible_content",
        RewriteStatus::UnchangedIdentical => "unchanged_identical",
        RewriteStatus::Abstained => "abstained",
        RewriteStatus::Failed => "failed",
    }
}

pub(crate) const fn reason_name(reason: ReasonCode) -> &'static str {
    match reason {
        ReasonCode::NoCandidate => "no_candidate",
        ReasonCode::InvalidCandidate => "invalid_candidate",
        ReasonCode::SentinelIntegrity => "sentinel_integrity",
        ReasonCode::ProtectedValueChanged => "protected_value_changed",
        ReasonCode::StructureChanged => "structure_changed",
        ReasonCode::UnsafeText => "unsafe_text",
        ReasonCode::CharacterBudgetExceeded => "character_budget_exceeded",
        ReasonCode::LineBudgetExceeded => "line_budget_exceeded",
        ReasonCode::ChangeBudgetExceeded => "change_budget_exceeded",
        ReasonCode::SemanticMismatch => "semantic_mismatch",
        ReasonCode::SemanticUncertain => "semantic_uncertain",
        ReasonCode::ReassemblyVerification => "reassembly_verification",
        ReasonCode::Cancelled => "cancelled",
        ReasonCode::UnsupportedAtomicity => "unsupported_atomicity",
    }
}
