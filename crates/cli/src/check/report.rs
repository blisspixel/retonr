use std::io::{self, Write};

use rewrite_types::{ReasonCode, RewriteRecord, RewriteStatus};

use super::ReportTarget;
use crate::contract::{CommandName, ReportFormat, SuccessEnvelope};

/// Writes one versioned report to the stream that does not carry document bytes.
pub(crate) fn write(
    command: CommandName,
    record: &RewriteRecord,
    format: ReportFormat,
    target: ReportTarget,
) -> io::Result<()> {
    let bytes = render(command, record, format)?;
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
) -> io::Result<Vec<u8>> {
    match format {
        ReportFormat::Json => {
            let mut bytes = serde_json::to_vec_pretty(&SuccessEnvelope::new(command, record))
                .map_err(io::Error::other)?;
            bytes.push(b'\n');
            Ok(bytes)
        }
        ReportFormat::Text => Ok(render_text(record).into_bytes()),
    }
}

fn render_text(record: &RewriteRecord) -> String {
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
    let _ = writeln!(text, "source_digest: {}", record.source_digest.as_str());
    let _ = writeln!(text, "output_digest: {}", record.output_digest.as_str());
    let _ = writeln!(text, "candidates: {}", record.assessments.len());
    let _ = writeln!(text, "eligible_candidates: {eligible}");
    text
}

pub(crate) const fn status_name(status: RewriteStatus) -> &'static str {
    match status {
        RewriteStatus::Rewritten => "rewritten",
        RewriteStatus::UnchangedNoEligibleContent => "unchanged_no_eligible_content",
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
        ReasonCode::SemanticMismatch => "semantic_mismatch",
        ReasonCode::SemanticUncertain => "semantic_uncertain",
        ReasonCode::ReassemblyVerification => "reassembly_verification",
        ReasonCode::Cancelled => "cancelled",
        ReasonCode::UnsupportedAtomicity => "unsupported_atomicity",
    }
}
