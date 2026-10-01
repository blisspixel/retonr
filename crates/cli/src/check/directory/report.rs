//! Stable paired-directory review reports without document contents.

use crate::{
    contract::{CommandName, ReportFormat, SuccessEnvelope},
    failure::RunFailure,
    inspect_source::directory::SkippedEntry,
    render::{escape_inline_for_display, to_safe_pretty_json},
};
use rewrite_types::RewriteRecord;
use serde::Serialize;
use std::{
    fmt::Write as _,
    io::{self, Write},
};

#[derive(Serialize)]
pub(super) struct PairReport {
    pub(super) relative_path: String,
    pub(super) record: RewriteRecord,
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
}

impl DirectoryCheckReport {
    pub(super) fn new(
        recursive: bool,
        maximum_total_bytes: usize,
        source_skipped: Vec<SkippedEntry>,
        candidate_skipped: Vec<SkippedEntry>,
    ) -> Self {
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
        }
    }

    pub(super) fn write(&self, format: ReportFormat) -> Result<(), RunFailure> {
        let bytes = match format {
            ReportFormat::Json => {
                let mut bytes =
                    to_safe_pretty_json(&SuccessEnvelope::new(CommandName::Check, self))
                        .map_err(|_| RunFailure::operational(CommandName::Check))?;
                bytes.push(b'\n');
                bytes
            }
            ReportFormat::Text => self.text().into_bytes(),
        };
        let mut stream = io::stdout().lock();
        stream
            .write_all(&bytes)
            .and_then(|()| stream.flush())
            .map_err(|_| RunFailure::operational(CommandName::Check))
    }

    fn text(&self) -> String {
        let mut text = format!(
            "scope: {}\nmode: {}\nrecursion: {}\nlinks: {}\nchecked: {}\nabstained: {}\nunmatched: {}\nskipped: {}\n",
            self.scope,
            self.mode,
            self.recursion,
            self.links,
            self.checked_count,
            self.abstained_count,
            self.unmatched.len(),
            self.skipped_count
        );
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
