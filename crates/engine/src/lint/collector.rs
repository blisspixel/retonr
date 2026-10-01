use rewrite_types::EditorialFinding;

use super::FindingLimitExceeded;

pub(super) struct FindingCollector {
    findings: Vec<EditorialFinding>,
    maximum: usize,
    exceeded: bool,
}

impl FindingCollector {
    pub(super) const fn new(maximum: usize) -> Self {
        Self {
            findings: Vec::new(),
            maximum,
            exceeded: false,
        }
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
}
