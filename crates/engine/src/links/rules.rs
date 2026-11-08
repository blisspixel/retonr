//! Reviewed query-parameter catalog.
//!
//! The table is the only removal and inspect list. A pair is removed only when
//! its key and, where set, its value match a row byte for byte. Nothing in
//! this catalog is decoded, and a prefix of a key is not a key.

use super::LinkCleanGroup;

/// Review date shared by this catalog.
pub(crate) const REVIEWED: &str = "2026-10-22";

/// Version of [`LINK_RULES`](super::LINK_RULE_CATALOG_VERSION).
pub(super) const CATALOG_VERSION: u32 = 1;

/// Family a rule belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LinkRuleGroup {
    /// Exact `utm_source=chatgpt.com`. Always eligible for cleanup.
    Attribution,
    /// Google Analytics campaign keys. Opt-in, any value.
    Campaign,
    /// Advertising and commerce click identifiers. Opt-in, any value.
    Tracking,
    /// Exact `source=chatgpt.com`. Opt-in, and not an inspect classification.
    ReviewedSource,
    /// Exact `sz=32`. Opt-in, and not an inspect classification.
    ReviewedSize,
}

/// One versioned removal or listing rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LinkRule {
    /// Stable identifier printed in reports.
    pub id: &'static str,
    /// Family that decides when the rule runs.
    pub group: LinkRuleGroup,
    /// Exact top-level query key.
    pub key: &'static str,
    /// Exact value. `None` means any value, still without decoding.
    pub value: Option<&'static str>,
    /// Citation recorded with the 2026-10-22 review.
    pub source: &'static str,
    /// What the review does and does not claim.
    pub evidence: &'static str,
    /// Review date, `YYYY-MM-DD`.
    pub reviewed: &'static str,
}

impl LinkRule {
    const fn row(
        id: &'static str,
        group: LinkRuleGroup,
        key: &'static str,
        value: Option<&'static str>,
        source: &'static str,
        evidence: &'static str,
    ) -> Self {
        Self {
            id,
            group,
            key,
            value,
            source,
            evidence,
            reviewed: REVIEWED,
        }
    }

    fn selected(&self, groups: &[LinkCleanGroup]) -> bool {
        match self.group {
            LinkRuleGroup::Attribution => true,
            LinkRuleGroup::Campaign => groups.contains(&LinkCleanGroup::Campaign),
            LinkRuleGroup::Tracking => groups.contains(&LinkCleanGroup::Tracking),
            LinkRuleGroup::ReviewedSource => groups.contains(&LinkCleanGroup::SourceChatgpt),
            LinkRuleGroup::ReviewedSize => groups.contains(&LinkCleanGroup::Sz32),
        }
    }

    fn listed_on_inspect(&self) -> bool {
        matches!(
            self.group,
            LinkRuleGroup::Attribution | LinkRuleGroup::Campaign | LinkRuleGroup::Tracking
        )
    }
}

const OPENAI_FAQ: &str =
    "https://help.openai.com/en/articles/12627856-publishers-and-developers-faq";
const GA_SOURCE: &str = "Google Analytics campaign parameters, reviewed 2026-10-22";

/// Rows in catalog order. An exact value wins over an any-value row for the same key.
pub(crate) const LINK_RULES: &[LinkRule] = &[
    LinkRule::row(
        "utm_source_chatgpt_com",
        LinkRuleGroup::Attribution,
        "utm_source",
        Some("chatgpt.com"),
        OPENAI_FAQ,
        "Indexed excerpts of the OpenAI publishers FAQ named this exact pair. Direct retrieval of that article on 2026-10-22 redirected elsewhere. This is not a claim about all current ChatGPT output.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_source",
        None,
        GA_SOURCE,
        "Campaign key, any value. A user observation of utm_source=gemini is not an official specification. The default rule does not remove other utm_source values.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_medium",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_campaign",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_term",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_content",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_id",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_source_platform",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_creative_format",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ga_campaign",
        LinkRuleGroup::Campaign,
        "utm_marketing_tactic",
        None,
        GA_SOURCE,
        "Campaign key, any value. Removed only when the campaign group is selected.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "gclid",
        None,
        "Google Ads help 1752125",
        "Exact key, any value. Not a prefix. Removed only when the tracking group is selected.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "gbraid",
        None,
        "Google Analytics help 11367152",
        "Exact key, any value. Removed only when the tracking group is selected.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "wbraid",
        None,
        "Google Analytics help 11367152",
        "Exact key, any value. Removed only when the tracking group is selected.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "gad_source",
        None,
        "Google Ads help 16193746",
        "Exact key, any value. Other gad_ keys are not this rule.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "gad_campaignid",
        None,
        "Google Ads help 16193746",
        "Exact key, any value. Other gad_ keys are not this rule.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "srsltid",
        None,
        "Google Merchant Center help 15191080",
        "Exact key, any value. Removed only when the tracking group is selected.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "msclkid",
        None,
        "Microsoft Advertising bulk API",
        "Exact key, any value. This is not a Copilot tag.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "fbclid",
        None,
        "https://arxiv.org/abs/2208.00710",
        "Exact key, any value. The citation is a research paper, not a newly verified Meta specification.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "mc_cid",
        None,
        "Mailchimp e-commerce documentation",
        "Exact key, any value. Reports name the key and never the value.",
    ),
    LinkRule::row(
        "ad_tracking",
        LinkRuleGroup::Tracking,
        "mc_eid",
        None,
        "Mailchimp e-commerce documentation",
        "Exact key, any value. Reports name the key and never a recipient value.",
    ),
    LinkRule::row(
        "source_chatgpt_com",
        LinkRuleGroup::ReviewedSource,
        "source",
        Some("chatgpt.com"),
        "2026-10-22 review of a generic source parameter",
        "Exact pair only. A generic source parameter can select content, so other values stay. Removed only when this rule is selected.",
    ),
    LinkRule::row(
        "sz_32",
        LinkRuleGroup::ReviewedSize,
        "sz",
        Some("32"),
        "2026-10-22 review; Google photo URLs use sz as an image dimension",
        "Exact pair only. sz is not classified as a tracker. Other sz values stay. Removed only when this rule is selected.",
    ),
];

/// Documented query keys that mean a whole URL is left unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SignatureIndicator {
    /// Stable skip reason shared by every supported indicator.
    pub reason: &'static str,
    /// Every listed key must be present as its own top-level pair.
    pub keys: &'static [&'static str],
    /// Citation for the indicator. Not a claim that every signature uses it.
    pub source: &'static str,
    /// Review date, `YYYY-MM-DD`.
    pub reviewed: &'static str,
}

/// Supported indicators. Other signing schemes are not inferred.
pub(crate) const SIGNATURE_INDICATORS: &[SignatureIndicator] = &[
    SignatureIndicator {
        reason: super::SIGNED_QUERY_REASON,
        keys: &["X-Amz-Signature"],
        source: "AWS Signature Version 4 query parameter",
        reviewed: REVIEWED,
    },
    SignatureIndicator {
        reason: super::SIGNED_QUERY_REASON,
        keys: &["X-Goog-Signature"],
        source: "Google Cloud Storage V4 signed URL parameter",
        reviewed: REVIEWED,
    },
    SignatureIndicator {
        reason: super::SIGNED_QUERY_REASON,
        keys: &["GoogleAccessId", "Signature"],
        source: "Google Cloud Storage V2 signed URL parameters, both required",
        reviewed: REVIEWED,
    },
    SignatureIndicator {
        reason: super::SIGNED_QUERY_REASON,
        keys: &["sig", "se"],
        source: "Azure Storage shared access signature parameters, both required",
        reviewed: REVIEWED,
    },
];

pub(super) fn removal_rule(
    key: &str,
    value: &str,
    groups: &[LinkCleanGroup],
) -> Option<&'static LinkRule> {
    best_match(key, value, |rule| rule.selected(groups))
}

pub(super) fn inspection_rule(key: &str, value: &str) -> Option<&'static LinkRule> {
    best_match(key, value, LinkRule::listed_on_inspect)
}

pub(super) fn query_is_signed(keys: &[&str]) -> bool {
    SIGNATURE_INDICATORS.iter().any(|indicator| {
        indicator
            .keys
            .iter()
            .all(|required| keys.contains(required))
    })
}

fn best_match(
    key: &str,
    value: &str,
    accept: impl Fn(&LinkRule) -> bool,
) -> Option<&'static LinkRule> {
    let mut found: Option<&LinkRule> = None;
    for rule in LINK_RULES {
        if rule.key != key || !value_matches(rule.value, value) || !accept(rule) {
            continue;
        }
        let replace = match found {
            None => true,
            Some(current) => current.value.is_none() && rule.value.is_some(),
        };
        if replace {
            found = Some(rule);
        }
    }
    found
}

fn value_matches(expected: Option<&str>, value: &str) -> bool {
    expected.is_none_or(|expected| expected == value)
}
