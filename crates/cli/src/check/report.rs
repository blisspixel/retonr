use std::io::{self, Write};

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
    format: ReportFormat,
    target: ReportTarget,
    backup: Option<&str>,
    fidelity: Option<FidelityReport>,
) -> io::Result<()> {
    let bytes = render(command, record, format, backup, fidelity)?;
    match target {
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
    format: ReportFormat,
    backup: Option<&str>,
    fidelity: Option<FidelityReport>,
) -> io::Result<Vec<u8>> {
    match format {
        ReportFormat::Json => {
            let mut bytes = crate::render::to_safe_pretty_json(&SuccessEnvelope::new(
                command,
                DocumentReport {
                    record,
                    backup,
                    semantic_evaluator: fidelity.map(|item| item.semantic_evaluator),
                    edit_level_effective: fidelity.map(|item| item.edit_level_effective),
                    edit_level_requested: fidelity.and_then(|item| item.edit_level_requested),
                    edit_level_note: fidelity.and_then(|item| item.note),
                },
            ))
            .map_err(io::Error::other)?;
            bytes.push(b'\n');
            Ok(bytes)
        }
        ReportFormat::Text => Ok(render_text(record, backup, fidelity).into_bytes()),
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
}

fn render_text(
    record: &RewriteRecord,
    backup: Option<&str>,
    fidelity: Option<FidelityReport>,
) -> String {
    use std::fmt::Write as _;

    let eligible = record
        .assessments
        .iter()
        .filter(|assessment| assessment.eligible)
        .count();
    let mut text = String::new();
    let _ = writeln!(text, "status: {}", status_name(record.status));
    if let Some(reason) = record.reason {
        let _ = writeln!(text, "reason: {}", reason_name(reason));
    }
    append_span_lines(&mut text, record);
    if let Some(fidelity) = fidelity {
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
    let _ = writeln!(text, "source_digest: {}", record.source_digest.as_str());
    let _ = writeln!(text, "output_digest: {}", record.output_digest.as_str());
    let _ = writeln!(text, "candidates: {}", record.assessments.len());
    let _ = writeln!(text, "eligible_candidates: {eligible}");
    if let Some(backup) = backup {
        let _ = writeln!(
            text,
            "backup: {}",
            crate::render::escape_inline_for_display(backup)
        );
    }
    text
}

pub(crate) fn append_span_lines(text: &mut String, record: &RewriteRecord) {
    use std::fmt::Write as _;
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
                    let _ = writeln!(
                        text,
                        "line {} col {}: {} {}",
                        point.line,
                        point.column,
                        failure.kind,
                        span_change_name(failure.change)
                    );
                }
            }
        }
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
        ReasonCode::SemanticMismatch => "semantic_mismatch",
        ReasonCode::SemanticUncertain => "semantic_uncertain",
        ReasonCode::ReassemblyVerification => "reassembly_verification",
        ReasonCode::Cancelled => "cancelled",
        ReasonCode::UnsupportedAtomicity => "unsupported_atomicity",
    }
}
