use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current editorial-quality corpus schema version.
pub const EDITORIAL_CORPUS_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized editorial corpus size accepted by the validator.
pub const MAX_EDITORIAL_CORPUS_BYTES: usize = 16 * 1024 * 1024;
/// Maximum cases accepted in one editorial corpus.
pub const MAX_EDITORIAL_CASES: usize = 10_000;

const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_RULES_PER_CASE: usize = 32;
const MAX_FINDINGS_PER_TEXT: usize = 128;
const MAX_PROTECTED_TERMS: usize = 32;
const MAX_EVIDENCE_BYTES: usize = 512;

/// A versioned collection of editorial findings and clean controls.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorialCorpus {
    /// Corpus contract version.
    pub schema_version: u32,
    /// Stable corpus identity.
    pub corpus_id: String,
    /// Exact lint-rule catalog revision expected by the annotations.
    pub rule_catalog_version: u32,
    /// Provenance class admitted by this checked-in schema.
    pub content_origin: EditorialCorpusOrigin,
    /// SPDX license identifier for redistributed corpus content.
    pub license_spdx: String,
    /// Independently reportable cases.
    pub cases: Vec<EditorialCase>,
}

impl EditorialCorpus {
    /// Returns a content-free corpus summary suitable for validation output.
    #[must_use]
    pub fn summary(&self) -> EditorialCorpusSummary {
        let finding_cases = self
            .cases
            .iter()
            .filter(|case| case.kind == EditorialCaseKind::Finding)
            .count();
        let targeted_rules = self
            .cases
            .iter()
            .flat_map(|case| case.target_rules.iter())
            .collect::<BTreeSet<_>>()
            .len();
        EditorialCorpusSummary {
            schema_version: self.schema_version,
            corpus_id: self.corpus_id.clone(),
            total: self.cases.len(),
            finding_cases,
            clean_controls: self.cases.len().saturating_sub(finding_cases),
            targeted_rules,
        }
    }
}

/// Content origin allowed in the public synthetic development corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorialCorpusOrigin {
    /// Maintainer-constructed text containing no copied private or licensed source.
    Synthetic,
}

/// Whether a case expects a finding or protects a neighboring clean use.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorialCaseKind {
    /// One or more named findings must be reported in the source.
    Finding,
    /// Targeted rules must not report a finding in the source.
    CleanControl,
}

/// One synthetic editorial-quality fixture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorialCase {
    /// Stable fixture identifier.
    pub id: String,
    /// Fixture role in rule qualification.
    pub kind: EditorialCaseKind,
    /// BCP 47 language tag within the initial bounded validator.
    pub language: String,
    /// Stable communication-channel label.
    pub channel: String,
    /// Synthetic text inspected by editorial lint.
    pub source: String,
    /// One acceptable bounded revision, absent for review-only findings and controls.
    pub reference_revision: Option<String>,
    /// Rules whose behavior this case exercises.
    pub target_rules: Vec<String>,
    /// Findings expected in the source.
    pub expected_source_findings: Vec<EditorialFindingExpectation>,
    /// Findings expected in the optional reference revision.
    pub expected_reference_findings: Vec<EditorialFindingExpectation>,
    /// Exact terms a later rewrite evaluation must preserve.
    #[serde(default)]
    pub protected_terms: Vec<String>,
}

/// An expected rule and unambiguous textual occurrence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorialFindingExpectation {
    /// Stable lint-rule identifier.
    pub rule_id: String,
    /// Exact evidence substring used to locate the expected finding.
    pub evidence: String,
    /// Zero-based occurrence of the evidence substring in the selected text.
    pub occurrence: u16,
}

/// Content-free result returned after corpus validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EditorialCorpusSummary {
    /// Corpus contract version.
    pub schema_version: u32,
    /// Stable corpus identity.
    pub corpus_id: String,
    /// Total validated cases.
    pub total: usize,
    /// Cases requiring at least one finding.
    pub finding_cases: usize,
    /// Cases requiring no targeted finding.
    pub clean_controls: usize,
    /// Distinct lint rules exercised by the corpus.
    pub targeted_rules: usize,
}

/// Editorial corpus parsing or contract failure.
#[derive(Debug, Error)]
pub enum EditorialCorpusError {
    /// Serialized input exceeds the corpus byte bound.
    #[error("editorial corpus exceeds the supported byte limit")]
    TooLarge,
    /// Corpus JSON is invalid or contains an unknown field or enum value.
    #[error("invalid editorial corpus: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// Corpus contract version is unsupported.
    #[error("unsupported editorial corpus schema version {0}")]
    UnsupportedSchema(u32),
    /// Corpus-level identity, license, rule catalog, or case-count state is invalid.
    #[error("editorial corpus contract is invalid")]
    InvalidCorpus,
    /// Corpus contains more cases than the declared bound.
    #[error("editorial corpus exceeds the case-count limit")]
    TooManyCases,
    /// Case fields, annotations, evidence, or protected terms are inconsistent.
    #[error("editorial corpus case {index} is invalid")]
    InvalidCase {
        /// Zero-based case position.
        index: usize,
    },
    /// Case identifier is not unique within the corpus.
    #[error("editorial corpus case {index} has a duplicate identifier")]
    DuplicateCaseId {
        /// Zero-based case position.
        index: usize,
    },
}

/// Parses and validates one synthetic editorial-quality corpus.
///
/// # Errors
///
/// Returns [`EditorialCorpusError`] for a size, JSON, version, provenance, identity,
/// annotation, evidence, or case-contract violation.
pub fn parse_editorial_corpus(input: &str) -> Result<EditorialCorpus, EditorialCorpusError> {
    if input.len() > MAX_EDITORIAL_CORPUS_BYTES {
        return Err(EditorialCorpusError::TooLarge);
    }
    let corpus: EditorialCorpus = serde_json::from_str(input)?;
    validate_corpus(&corpus)?;
    Ok(corpus)
}

fn validate_corpus(corpus: &EditorialCorpus) -> Result<(), EditorialCorpusError> {
    if corpus.schema_version != EDITORIAL_CORPUS_SCHEMA_VERSION {
        return Err(EditorialCorpusError::UnsupportedSchema(
            corpus.schema_version,
        ));
    }
    if !valid_label(&corpus.corpus_id)
        || corpus.rule_catalog_version == 0
        || corpus.license_spdx != "Apache-2.0"
        || corpus.cases.is_empty()
    {
        return Err(EditorialCorpusError::InvalidCorpus);
    }
    if corpus.cases.len() > MAX_EDITORIAL_CASES {
        return Err(EditorialCorpusError::TooManyCases);
    }
    let mut identifiers = BTreeSet::new();
    for (index, case) in corpus.cases.iter().enumerate() {
        if !identifiers.insert(case.id.as_str()) {
            return Err(EditorialCorpusError::DuplicateCaseId { index });
        }
        if !valid_case(case) {
            return Err(EditorialCorpusError::InvalidCase { index });
        }
    }
    Ok(())
}

fn valid_case(case: &EditorialCase) -> bool {
    if !valid_label(&case.id)
        || !valid_language(&case.language)
        || !valid_label(&case.channel)
        || !valid_text(&case.source, MAX_TEXT_BYTES)
        || case.target_rules.is_empty()
        || case.target_rules.len() > MAX_RULES_PER_CASE
        || case.expected_source_findings.len() > MAX_FINDINGS_PER_TEXT
        || case.expected_reference_findings.len() > MAX_FINDINGS_PER_TEXT
        || case.protected_terms.len() > MAX_PROTECTED_TERMS
    {
        return false;
    }

    let targets = case.target_rules.iter().collect::<BTreeSet<_>>();
    if targets.len() != case.target_rules.len() || targets.iter().any(|rule| !valid_label(rule)) {
        return false;
    }

    let Some(source_findings) =
        validate_findings(&case.expected_source_findings, &case.source, &targets)
    else {
        return false;
    };
    let reference_findings = if let Some(reference) = &case.reference_revision {
        if !valid_text(reference, MAX_TEXT_BYTES) || reference == &case.source {
            return false;
        }
        let Some(findings) =
            validate_findings(&case.expected_reference_findings, reference, &targets)
        else {
            return false;
        };
        findings
    } else {
        if !case.expected_reference_findings.is_empty() {
            return false;
        }
        BTreeSet::new()
    };

    match case.kind {
        EditorialCaseKind::Finding => {
            if source_findings.is_empty() || source_findings != targets {
                return false;
            }
        }
        EditorialCaseKind::CleanControl => {
            if !source_findings.is_empty()
                || !reference_findings.is_empty()
                || case.reference_revision.is_some()
            {
                return false;
            }
        }
    }

    valid_protected_terms(case)
}

fn validate_findings<'a>(
    findings: &'a [EditorialFindingExpectation],
    text: &str,
    targets: &BTreeSet<&String>,
) -> Option<BTreeSet<&'a String>> {
    let mut unique = BTreeSet::new();
    let mut rules = BTreeSet::new();
    for finding in findings {
        if !valid_label(&finding.rule_id)
            || !targets.contains(&finding.rule_id)
            || !valid_text(&finding.evidence, MAX_EVIDENCE_BYTES)
            || text
                .match_indices(&finding.evidence)
                .nth(usize::from(finding.occurrence))
                .is_none()
            || !unique.insert((
                finding.rule_id.as_str(),
                finding.evidence.as_str(),
                finding.occurrence,
            ))
        {
            return None;
        }
        rules.insert(&finding.rule_id);
    }
    Some(rules)
}

fn valid_protected_terms(case: &EditorialCase) -> bool {
    let mut terms = BTreeSet::new();
    case.protected_terms.iter().all(|term| {
        valid_text(term, MAX_EVIDENCE_BYTES)
            && terms.insert(term.as_str())
            && case.source.contains(term)
            && case
                .reference_revision
                .as_ref()
                .is_none_or(|reference| reference.contains(term))
    })
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}

fn valid_language(value: &str) -> bool {
    let mut characters = value.chars();
    let first = characters.next();
    let last = value.chars().next_back();
    !value.is_empty()
        && value.len() <= 35
        && first.is_some_and(|character| character.is_ascii_alphabetic())
        && last.is_some_and(|character| character.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte))
}

#[cfg(test)]
mod tests;
