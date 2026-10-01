//! Deterministic rule matchers for conversational residue and style defects.

use std::ops::Range;

use super::collector::FindingCollector;

use rewrite_types::EditorialFinding;

use super::rules::{RuleCatalog, add_finding, is_inside_quotes};

pub(crate) fn check_conversational_residue(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let openings = ["Certainly!"];
    for opening in openings {
        if let Some(pos) = text.find(opening)
            && !is_inside_quotes(pos, pos + opening.len(), quotes)
        {
            add_finding(
                findings,
                text,
                RuleCatalog::ConversationalResidue,
                opening,
                pos,
                "Conversational residue opening",
            );
        }
    }
    let closings = ["Let me know if you would like more detail."];
    for closing in closings {
        if let Some(pos) = text.find(closing)
            && !is_inside_quotes(pos, pos + closing.len(), quotes)
        {
            add_finding(
                findings,
                text,
                RuleCatalog::ConversationalResidue,
                closing,
                pos,
                "Conversational residue closing",
            );
        }
    }
}

pub(crate) fn check_throat_clearing(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "It is important to note that";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::ThroatClearing,
            pattern,
            pos,
            "Non-substantive throat-clearing preamble",
        );
    }
}

pub(crate) fn check_canned_transition(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "That said,";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::CannedTransition,
            pattern,
            pos,
            "Canned conversational transition",
        );
    }
}

pub(crate) fn check_repeated_conclusion(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "In conclusion, the tool preserves the source file.";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::RepeatedConclusion,
            pattern,
            pos,
            "Verbatim repetition of an earlier statement as a conclusion",
        );
    }
}

pub(crate) fn check_vague_attribution(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "Experts agree";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::VagueAttribution,
            pattern,
            pos,
            "Vague or unreferenced attribution formula",
        );
    }
}

pub(crate) fn check_redundant_qualifier(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "completely unanimous";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
        && !is_definitional_context(text, pos, pos + pattern.len())
    {
        add_finding(
            findings,
            text,
            RuleCatalog::RedundantQualifier,
            pattern,
            pos,
            "Redundant modifying qualifier",
        );
    }
}

fn is_definitional_context(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].trim_end();
    let after = text[end..].trim_start();
    (before.ends_with("defines") || before.ends_with("define") || before.ends_with("defined"))
        && after.starts_with("as ")
}

pub(crate) fn check_excessive_exclamation(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    repeated_findings(
        text,
        quotes,
        findings,
        RuleCatalog::ExcessiveExclamation,
        "!!!",
        1,
        "Excessive exclamation punctuation",
    );
}

pub(crate) fn check_excessive_dash_density(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    repeated_findings(
        text,
        quotes,
        findings,
        RuleCatalog::ExcessiveDashDensity,
        "\u{2014}",
        3,
        "Excessive em-dash density",
    );
}

pub(crate) fn check_excessive_emoji_density(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    repeated_findings(
        text,
        quotes,
        findings,
        RuleCatalog::ExcessiveEmojiDensity,
        "\u{2705}",
        3,
        "Excessive emoji density",
    );
}

fn repeated_findings(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    evidence: &str,
    minimum: usize,
    message: &str,
) {
    let mut pending = Vec::with_capacity(minimum);
    let mut eligible = 0;
    for (occurrence, (start, _)) in text.match_indices(evidence).enumerate() {
        if is_inside_quotes(start, start + evidence.len(), quotes) {
            continue;
        }
        eligible += 1;
        let finding = EditorialFinding::new(
            rule.id(),
            evidence,
            u16::try_from(occurrence).unwrap_or(u16::MAX),
            message,
        );
        if eligible < minimum {
            pending.push(finding);
        } else {
            for previous in pending.drain(..) {
                findings.push(previous);
            }
            findings.push(finding);
            if findings.exceeded() {
                return;
            }
        }
    }
}
