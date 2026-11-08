//! Stable paired-directory review reports without document contents.

use crate::{
    contract::{CommandName, ReportFormat, SuccessEnvelope},
    failure::RunFailure,
    inspect_source::directory::SkippedEntry,
    render::{escape_inline_for_display, to_safe_pretty_json},
};
use rewrite_app::StyleDistance;
use rewrite_types::{EditLevel, RewriteRecord, RewriteStatus};
use serde::Serialize;
use std::{
    fmt::Write as _,
    io::{self, Write},
};

#[derive(Serialize)]
pub(super) struct PairReport {
    pub(super) relative_path: String,
    pub(super) record: RewriteRecord,
    pub(super) changed_token_pct: u8,
    pub(super) changed_char_pct: u8,
    /// Text receipt only. The directory JSON record stays free of this count.
    #[serde(skip)]
    pub(super) changed_word_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) style_distance: Option<StyleDistance>,
}

#[derive(Serialize)]
pub(super) struct UnmatchedPath {
    pub(super) relative_path: String,
    pub(super) side: &'static str,
    pub(super) reason: &'static str,
}

#[derive(Serialize)]
pub(super) struct DirectoryCheckReport {
    scope: &'static str,
    mode: &'static str,
    recursion: &'static str,
    links: &'static str,
    maximum_total_bytes: usize,
    pub(super) checked_count: usize,
    pub(super) abstained_count: usize,
    pub(super) skipped_count: usize,
    pub(super) pairs: Vec<PairReport>,
    pub(super) unmatched: Vec<UnmatchedPath>,
    pub(super) source_skipped: Vec<SkippedEntry>,
    pub(super) candidate_skipped: Vec<SkippedEntry>,
    semantic_evaluator: &'static str,
    edit_level_effective: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_requested: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit_level_note: Option<&'static str>,
}

impl DirectoryCheckReport {
    pub(super) fn new(
        recursive: bool,
        maximum_total_bytes: usize,
        source_skipped: Vec<SkippedEntry>,
        candidate_skipped: Vec<SkippedEntry>,
        edit_level: Option<EditLevel>,
    ) -> Self {
        let fidelity = crate::check::report::fidelity_for(edit_level);
        Self {
            scope: "directory",
            mode: "dry_run",
            recursion: if recursive { "bounded" } else { "none" },
            links: "not_followed",
            maximum_total_bytes,
            checked_count: 0,
            abstained_count: 0,
            skipped_count: 0,
            pairs: Vec::new(),
            unmatched: Vec::new(),
            source_skipped,
            candidate_skipped,
            semantic_evaluator: fidelity.semantic_evaluator,
            edit_level_effective: fidelity.edit_level_effective,
            edit_level_requested: fidelity.edit_level_requested,
            edit_level_note: fidelity.note,
        }
    }

    pub(super) fn write(
        &self,
        format: ReportFormat,
        human: &crate::check::HumanReport,
    ) -> Result<(), RunFailure> {
        let bytes = match format {
            ReportFormat::Json => {
                let mut bytes =
                    to_safe_pretty_json(&SuccessEnvelope::new(CommandName::Check, self))
                        .map_err(|_| RunFailure::operational(CommandName::Check))?;
                bytes.push(b'\n');
                bytes
            }
            ReportFormat::Text => {
                let terminal = crate::presentation::stdout_is_terminal();
                let width = crate::presentation::terminal_width(terminal);
                let text = self.text(width);
                let wrapped = crate::presentation::wrap(&text, width);
                crate::presentation::paint_report(
                    &wrapped,
                    crate::presentation::color_enabled(human.color, terminal),
                )
                .into_bytes()
            }
        };
        let mut stream = io::stdout().lock();
        stream
            .write_all(&bytes)
            .and_then(|()| stream.flush())
            .map_err(|_| RunFailure::operational(CommandName::Check))
    }

    fn text(&self, width: Option<usize>) -> String {
        let mut text = format!(
            "{}\nscope: {}\nmode: {}\nrecursion: {}\nlinks: {}\nchecked: {}\nabstained: {}\nunmatched: {}\nskipped: {}\n",
            directory_verdict(self),
            self.scope,
            self.mode,
            self.recursion,
            self.links,
            self.checked_count,
            self.abstained_count,
            self.unmatched.len(),
            self.skipped_count
        );
        let _ = writeln!(text, "semantic_evaluator: {}", self.semantic_evaluator);
        let _ = writeln!(text, "edit_level_effective: {}", self.edit_level_effective);
        if let Some(note) = self.edit_level_note {
            let _ = writeln!(text, "note: {note}");
        }
        for pair in &self.pairs {
            let _ = writeln!(
                text,
                "pair {} status={}",
                escape_inline_for_display(&pair.relative_path),
                crate::check::report::status_name(pair.record.status)
            );
            if let Some(reason) = pair.record.reason {
                let _ = writeln!(
                    text,
                    "  reason: {}",
                    crate::check::report::reason_name(reason)
                );
            }
            let _ = writeln!(text, "  changed_token_pct: {}", pair.changed_token_pct);
            let _ = writeln!(text, "  changed_char_pct: {}", pair.changed_char_pct);
            if let Some(style) = &pair.style_distance {
                let mut style_lines = String::new();
                crate::check::report::append_style_lines(&mut style_lines, style);
                for line in style_lines.lines() {
                    let _ = writeln!(text, "  {line}");
                }
            }
            let mut locations = String::new();
            crate::check::report::append_span_lines(&mut locations, &pair.record);
            for line in locations.lines() {
                let _ = writeln!(text, "  {line}");
            }
            let receipt_width = width.map(|columns| columns.saturating_sub(2));
            let receipt = crate::check::report::fidelity_receipt(
                &pair.record,
                Some(pair.changed_word_count),
                receipt_width,
            );
            let _ = writeln!(text, "  {receipt}");
        }
        for unmatched in &self.unmatched {
            let _ = writeln!(
                text,
                "unmatched {} side={} reason={}",
                escape_inline_for_display(&unmatched.relative_path),
                unmatched.side,
                unmatched.reason
            );
        }
        for (side, skipped) in [
            ("source", &self.source_skipped),
            ("candidate", &self.candidate_skipped),
        ] {
            for entry in skipped {
                let _ = writeln!(
                    text,
                    "skipped {} side={} reason={}",
                    entry
                        .relative_path
                        .as_deref()
                        .map(escape_inline_for_display)
                        .unwrap_or_default(),
                    side,
                    entry.reason
                );
            }
        }
        text
    }
}

fn directory_verdict(report: &DirectoryCheckReport) -> String {
    if report
        .pairs
        .iter()
        .any(|pair| pair.record.status == RewriteStatus::Failed)
    {
        return "failed      a directory check did not finish".to_owned();
    }
    if report.abstained_count > 0 {
        return "abstained   original kept where a pair abstained".to_owned();
    }
    if report.pairs.is_empty() {
        return "abstained   no pair was checked".to_owned();
    }
    let rewritten = report
        .pairs
        .iter()
        .filter(|pair| pair.record.status == RewriteStatus::Rewritten)
        .count();
    if rewritten == report.pairs.len() {
        let noun = if rewritten == 1 { "pair" } else { "pairs" };
        return format!("accepted    {rewritten} {noun} passed");
    }
    if rewritten == 0 {
        return "unchanged   directory pairs already match".to_owned();
    }
    let kept = report.pairs.len() - rewritten;
    format!("accepted    {rewritten} passed; {kept} kept")
}
