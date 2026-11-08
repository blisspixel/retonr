//! Top-level query edits.
//!
//! Pairs split on raw `&` and the first `=`. Percent-encoding stays as it
//! appears. A value, path, or fragment is never searched.

use rewrite_types::TextPoint;

use super::rules::{self, LinkRule};
use super::{LinkCleanGroup, LinkCleanScope, LinkParameter};

pub(super) struct UrlEdit {
    pub replacement: String,
    pub removals: Vec<LinkParameter>,
    pub skipped: bool,
}

pub(super) fn edit_url(
    document: &str,
    url_start: usize,
    url: &str,
    groups: &[LinkCleanGroup],
    scope: LinkCleanScope<'_>,
) -> UrlEdit {
    let Some(parts) = split_url(url) else {
        return keep(url);
    };
    if !in_scope(&parts, scope) {
        return keep(url);
    }
    let Some(query) = parts.query else {
        return keep(url);
    };
    let pairs = segments(query);
    let keys = pair_keys(query, &pairs);
    let removals = removable_pairs(document, url_start, parts.query_at, query, &pairs, groups);
    if removals.is_empty() {
        return keep(url);
    }
    if rules::query_is_signed(&keys) {
        return UrlEdit {
            replacement: url.to_owned(),
            removals: Vec::new(),
            skipped: true,
        };
    }
    let kept = kept_segments(&pairs, query, groups);
    let query_text = join_kept(query, &kept);
    UrlEdit {
        replacement: rebuild(&parts, query_text),
        removals,
        skipped: false,
    }
}

pub(super) fn listed_parameters(document: &str, url_start: usize, url: &str) -> Vec<LinkParameter> {
    let Some(parts) = split_url(url) else {
        return Vec::new();
    };
    let Some(query) = parts.query else {
        return Vec::new();
    };
    let mut listed = Vec::new();
    for (start, end) in segments(query) {
        let Some((key, value)) = query[start..end].split_once('=') else {
            continue;
        };
        let Some(rule) = rules::inspection_rule(key, value) else {
            continue;
        };
        listed.push(parameter(
            document,
            url_start + parts.query_at + start,
            rule,
        ));
    }
    listed
}

fn keep(url: &str) -> UrlEdit {
    UrlEdit {
        replacement: url.to_owned(),
        removals: Vec::new(),
        skipped: false,
    }
}

fn removable_pairs(
    document: &str,
    url_start: usize,
    query_at: usize,
    query: &str,
    pairs: &[(usize, usize)],
    groups: &[LinkCleanGroup],
) -> Vec<LinkParameter> {
    let mut removals = Vec::new();
    for &(start, end) in pairs {
        let Some((key, value)) = query[start..end].split_once('=') else {
            continue;
        };
        let Some(rule) = rules::removal_rule(key, value, groups) else {
            continue;
        };
        removals.push(parameter(document, url_start + query_at + start, rule));
    }
    removals
}

fn kept_segments(
    pairs: &[(usize, usize)],
    query: &str,
    groups: &[LinkCleanGroup],
) -> Vec<(usize, usize)> {
    pairs
        .iter()
        .copied()
        .filter(|&(start, end)| match query[start..end].split_once('=') {
            Some((key, value)) => rules::removal_rule(key, value, groups).is_none(),
            None => true,
        })
        .collect()
}

fn parameter(document: &str, index: usize, rule: &LinkRule) -> LinkParameter {
    let point = TextPoint::from_byte_index(document, index).expect("query key is a char boundary");
    LinkParameter {
        line: point.line,
        column: point.column,
        rule_id: rule.id,
        name: rule.key,
    }
}

fn segments(query: &str) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    let mut start = 0;
    for (offset, _) in query.match_indices('&') {
        pairs.push((start, offset));
        start = offset + 1;
    }
    pairs.push((start, query.len()));
    pairs
}

fn pair_keys<'a>(query: &'a str, pairs: &[(usize, usize)]) -> Vec<&'a str> {
    pairs
        .iter()
        .filter_map(|(start, end)| query[*start..*end].split_once('=').map(|(key, _)| key))
        .collect()
}

fn join_kept(query: &str, kept: &[(usize, usize)]) -> Option<String> {
    if kept.is_empty() || kept.iter().all(|(start, end)| start == end) {
        return None;
    }
    let mut joined = String::new();
    for (index, (start, end)) in kept.iter().enumerate() {
        if index > 0 {
            joined.push('&');
        }
        joined.push_str(&query[*start..*end]);
    }
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

struct Parts<'a> {
    host: &'a str,
    path: &'a str,
    head: &'a str,
    query: Option<&'a str>,
    query_at: usize,
    tail: &'a str,
}

fn split_url(url: &str) -> Option<Parts<'_>> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let scheme_len = url.len() - rest.len();
    let authority_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = host_in_authority(&rest[..authority_len]);
    let tail_at = scheme_len + authority_len;
    let tail = &url[tail_at..];
    let path_len = tail.find(['?', '#']).unwrap_or(tail.len());
    let path = &tail[..path_len];
    let after_path = &tail[path_len..];
    if let Some(query_and_tail) = after_path.strip_prefix('?') {
        let fragment_at = query_and_tail.find('#').unwrap_or(query_and_tail.len());
        let query_at = tail_at + path_len + 1;
        return Some(Parts {
            host,
            path,
            head: &url[..query_at - 1],
            query: Some(&query_and_tail[..fragment_at]),
            query_at,
            tail: &query_and_tail[fragment_at..],
        });
    }
    Some(Parts {
        host,
        path,
        head: url,
        query: None,
        query_at: url.len(),
        tail: after_path,
    })
}

fn host_in_authority(authority: &str) -> &str {
    let hostport = match authority.rfind('@') {
        Some(index) => &authority[index + 1..],
        None => authority,
    };
    if let Some(rest) = hostport.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        return &hostport[..end + 2];
    }
    match hostport.find(':') {
        Some(index) => &hostport[..index],
        None => hostport,
    }
}

fn in_scope(parts: &Parts<'_>, scope: LinkCleanScope<'_>) -> bool {
    if scope.host.is_some_and(|host| parts.host != host) {
        return false;
    }
    scope
        .path_prefix
        .is_none_or(|prefix| path_in_scope(parts.path, prefix))
}

fn path_in_scope(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

fn rebuild(parts: &Parts<'_>, query: Option<String>) -> String {
    match query {
        Some(query) => format!("{}?{query}{}", parts.head, parts.tail),
        None => format!("{}{}", parts.head, parts.tail),
    }
}
