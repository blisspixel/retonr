use rewrite_types::{EditorialFinding, FindingSpan, TextPoint};

use super::FindingLimitExceeded;

pub(super) struct FindingCollector {
    findings: Vec<EditorialFinding>,
    maximum: usize,
    exceeded: bool,
    lines: LineIndex,
}

impl FindingCollector {
    pub(super) fn new(text: &str, maximum: usize) -> Self {
        Self {
            findings: Vec::new(),
            maximum,
            exceeded: false,
            lines: LineIndex::new(text),
        }
    }

    /// Locates `start..end` in the same text passed to `new`.
    pub(super) fn locate(&self, text: &str, start: usize, end: usize) -> Option<FindingSpan> {
        if end < start
            || end > text.len()
            || !text.is_char_boundary(start)
            || !text.is_char_boundary(end)
        {
            return None;
        }
        let point = self.lines.point(start)?;
        Some(FindingSpan {
            start: u32::try_from(start).ok()?,
            end: u32::try_from(end).ok()?,
            line: point.line,
            column: point.column,
        })
    }

    pub(super) fn push(&mut self, finding: EditorialFinding) {
        if self.findings.len() >= self.maximum {
            self.exceeded = true;
        } else {
            self.findings.push(finding);
        }
    }

    pub(super) fn remaining(&self) -> usize {
        self.maximum.saturating_sub(self.findings.len())
    }

    pub(super) const fn exceeded(&self) -> bool {
        self.exceeded
    }

    pub(super) fn refuse(&mut self) {
        self.exceeded = true;
    }

    pub(super) fn finish(self) -> Result<Vec<EditorialFinding>, FindingLimitExceeded> {
        if self.exceeded {
            Err(FindingLimitExceeded)
        } else {
            Ok(self.findings)
        }
    }

    pub(super) fn into_findings(self) -> Vec<EditorialFinding> {
        self.findings
    }

    pub(super) fn overlaps_rule(&self, rule_id: &str, start: usize, end: usize) -> bool {
        self.findings.iter().any(|finding| {
            finding.rule_id == rule_id
                && finding.span.is_some_and(|span| {
                    let span_start = byte_offset(span.start);
                    let span_end = byte_offset(span.end);
                    start < span_end && end > span_start
                })
        })
    }

    pub(super) fn ranges_for(&self, rule_id: &str) -> Vec<(usize, usize)> {
        self.findings
            .iter()
            .filter(|finding| finding.rule_id == rule_id)
            .filter_map(|finding| {
                finding
                    .span
                    .map(|span| (byte_offset(span.start), byte_offset(span.end)))
            })
            .collect()
    }
}

fn byte_offset(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// Newline starts for one document, so each finding does not rescan its prefix.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = Vec::new();
        starts.push(0);
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(offset.saturating_add(1));
            }
        }
        Self { starts }
    }

    fn point(&self, byte_index: usize) -> Option<TextPoint> {
        let position = self.starts.partition_point(|&start| start <= byte_index);
        let line_index = position.checked_sub(1)?;
        let line_start = *self.starts.get(line_index)?;
        let column_offset = byte_index.checked_sub(line_start)?;
        let line = u32::try_from(line_index).ok()?.checked_add(1)?;
        let column = u32::try_from(column_offset).ok()?.checked_add(1)?;
        Some(TextPoint { line, column })
    }
}
