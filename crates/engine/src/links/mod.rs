//! Offline query tidying for the writer's own text.
//!
//! Cleanup never runs from check or rewrite. It does not fetch a link, decode
//! a query, unwrap a redirect, or follow a scheme-less address. Reports name
//! a line, a column, a rule, and a parameter. They do not copy a URL or a
//! query value. A known signature indicator keeps the whole URL and records
//! [`SIGNED_QUERY_REASON`]. Other signing schemes are not inferred.

mod query;
mod rules;

#[cfg(test)]
mod tests;

use thiserror::Error;

use crate::protection::protected_http_url_ranges;
use query::UrlEdit;

/// Catalog version stored with the reviewed rules.
pub const LINK_RULE_CATALOG_VERSION: u32 = rules::CATALOG_VERSION;

/// Redacted reason for a URL whose supported signature keys were left in place.
pub const SIGNED_QUERY_REASON: &str = "signed_query";

/// Opt-in cleanup family. The exact `utm_source=chatgpt.com` pair is always included.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LinkCleanGroup {
    /// Google Analytics campaign keys, whatever the value.
    Campaign,
    /// Reviewed advertising and commerce click-identifier keys, whatever the value.
    Tracking,
    /// The exact pair `source=chatgpt.com`.
    SourceChatgpt,
    /// The exact pair `sz=32`.
    Sz32,
}

/// Literal host and path limits. Neither value is normalized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkCleanScope<'a> {
    /// Host bytes after userinfo and before a port. No DNS and no case folding.
    pub host: Option<&'a str>,
    /// Path prefix, compared to the path bytes. `/guide` matches `/guide` and `/guide/page`.
    pub path_prefix: Option<&'a str>,
}

impl LinkCleanScope<'_> {
    /// Every host and path.
    #[must_use]
    pub const fn entire_document() -> Self {
        Self {
            host: None,
            path_prefix: None,
        }
    }
}

/// One located parameter. The name is the key, never the value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkParameter {
    /// One-based line of the key.
    pub line: u32,
    /// One-based byte column of the key.
    pub column: u32,
    /// Catalog rule that selected the pair.
    pub rule_id: &'static str,
    /// Exact parameter name.
    pub name: &'static str,
}

/// One URL left unchanged because a supported signature indicator was present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkSkip {
    /// One-based line of the URL.
    pub line: u32,
    /// One-based byte column of the URL.
    pub column: u32,
    /// Redacted reason. This is [`SIGNED_QUERY_REASON`].
    pub reason: &'static str,
}

/// HTTP and HTTPS URLs, plus the listed parameters inspect may name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkInventory {
    /// Protected `http` and `https` URLs. Scheme-less links are not counted.
    pub http_url_count: u32,
    /// Listed parameters in document order. Unlisted keys are absent.
    pub parameters: Vec<LinkParameter>,
}

/// Cleaned copy and the redacted account of what changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkCleanReport {
    /// Document after the selected removals. Unselected bytes stay identical.
    pub text: String,
    /// Protected `http` and `https` URLs in the source.
    pub http_url_count: u32,
    /// Pairs removed, located in the source.
    pub removals: Vec<LinkParameter>,
    /// Signed URLs that contained a selected pair and were not edited.
    pub skips: Vec<LinkSkip>,
}

/// Link inventory or cleanup could not finish inside the protection ceilings.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum LinkError {
    /// The document or its URL list exceeds a protection ceiling.
    #[error("document exceeds the supported link limit")]
    ResourceLimit,
}

/// Lists protected HTTP URLs and the catalogued parameters inspect is allowed to name.
///
/// `source=chatgpt.com` and `sz=32` are omitted here. They are cleanup rules,
/// not inspect classifications. No URL text or query value is copied into the
/// result.
///
/// # Errors
///
/// Returns [`LinkError::ResourceLimit`] when URL extraction exceeds its ceiling.
pub fn inventory_links(text: &str) -> Result<LinkInventory, LinkError> {
    let ranges = url_ranges(text)?;
    let mut parameters = Vec::new();
    for (start, end) in &ranges {
        parameters.extend(query::listed_parameters(text, *start, &text[*start..*end]));
    }
    Ok(LinkInventory {
        http_url_count: url_count(ranges.len())?,
        parameters,
    })
}

/// Removes selected top-level pairs and returns the new document.
///
/// The exact pair `utm_source=chatgpt.com` is always eligible. `groups` adds
/// families. `scope` limits which URLs are eligible; it does not decode the
/// host or the path. A second call on the returned text removes nothing.
///
/// # Errors
///
/// Returns [`LinkError::ResourceLimit`] when URL extraction exceeds its ceiling.
pub fn clean_links(
    text: &str,
    groups: &[LinkCleanGroup],
    scope: LinkCleanScope<'_>,
) -> Result<LinkCleanReport, LinkError> {
    let ranges = url_ranges(text)?;
    let mut output = String::with_capacity(text.len());
    let mut removals = Vec::new();
    let mut skips = Vec::new();
    let mut cursor = 0;
    for (start, end) in &ranges {
        output.push_str(&text[cursor..*start]);
        let edit = query::edit_url(text, *start, &text[*start..*end], groups, scope);
        push_edit(&mut output, &mut removals, &mut skips, text, *start, edit);
        cursor = *end;
    }
    output.push_str(&text[cursor..]);
    Ok(LinkCleanReport {
        text: output,
        http_url_count: url_count(ranges.len())?,
        removals,
        skips,
    })
}

fn push_edit(
    output: &mut String,
    removals: &mut Vec<LinkParameter>,
    skips: &mut Vec<LinkSkip>,
    document: &str,
    url_start: usize,
    edit: UrlEdit,
) {
    output.push_str(&edit.replacement);
    removals.extend(edit.removals);
    if edit.skipped {
        let point = rewrite_types::TextPoint::from_byte_index(document, url_start)
            .expect("url start is a char boundary");
        skips.push(LinkSkip {
            line: point.line,
            column: point.column,
            reason: SIGNED_QUERY_REASON,
        });
    }
}

fn url_ranges(text: &str) -> Result<Vec<(usize, usize)>, LinkError> {
    protected_http_url_ranges(text).map_err(|_| LinkError::ResourceLimit)
}

fn url_count(count: usize) -> Result<u32, LinkError> {
    u32::try_from(count).map_err(|_| LinkError::ResourceLimit)
}
