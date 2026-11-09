use rewrite_types::TextPoint;

use super::{
    LinkCleanGroup, LinkCleanScope, LinkParameter, SIGNED_QUERY_REASON, clean_links,
    inventory_links,
};
use crate::links::rules::{LINK_RULES, LinkRuleGroup, SIGNATURE_INDICATORS};
use crate::protection::{
    ProtectedKind, ProtectionError, ProtectionPlan, protected_http_url_ranges,
};

fn entire(text: &str, groups: &[LinkCleanGroup]) -> super::LinkCleanReport {
    clean_links(text, groups, LinkCleanScope::entire_document()).expect("cleanup")
}

fn text_of(text: &str, groups: &[LinkCleanGroup]) -> String {
    entire(text, groups).text
}

#[test]
fn default_removes_only_the_exact_chatgpt_pair() {
    let cases = [
        (
            "https://example.com/guide?utm_source=chatgpt.com&id=7",
            "https://example.com/guide?id=7",
        ),
        (
            "https://example.com/guide?id=7&utm_source=chatgpt.com&x=1",
            "https://example.com/guide?id=7&x=1",
        ),
        (
            "https://example.com/guide?id=7&utm_source=chatgpt.com",
            "https://example.com/guide?id=7",
        ),
        (
            "https://example.com/guide?utm_source=chatgpt.com",
            "https://example.com/guide",
        ),
        (
            "https://example.com/guide?id=7&utm_source=chatgpt.com#section",
            "https://example.com/guide?id=7#section",
        ),
        (
            "https://example.com/guide?utm_source=chatgpt.com&id=7&utm_source=chatgpt.com",
            "https://example.com/guide?id=7",
        ),
        (
            "https://example.com/guide?id=7&utm_source=chatgpt.com&q=a%26utm_source%3Dchatgpt.com",
            "https://example.com/guide?id=7&q=a%26utm_source%3Dchatgpt.com",
        ),
        (
            "See [guide](https://example.com/guide?id=7&utm_source=chatgpt.com) now.",
            "See [guide](https://example.com/guide?id=7) now.",
        ),
        (
            "See https://example.com/guide?utm_source=chatgpt.com.",
            "See https://example.com/guide.",
        ),
        (
            "https://user:pass@example.com/a?utm_source=chatgpt.com",
            "https://user:pass@example.com/a",
        ),
        (
            "https://example.com:8443/a?utm_source=chatgpt.com",
            "https://example.com:8443/a",
        ),
        ("http://[::1]/a?utm_source=chatgpt.com", "http://[::1]/a"),
        (
            "https://example.com/guide?id=7&&utm_source=chatgpt.com",
            "https://example.com/guide?id=7&",
        ),
        (
            "https://example.com/guide?utm_source=chatgpt.com&&",
            "https://example.com/guide",
        ),
        (
            "https://example.com/guide?&utm_source=chatgpt.com&",
            "https://example.com/guide",
        ),
    ];
    for (input, expected) in cases {
        let report = entire(input, &[]);
        assert_eq!(report.text, expected, "{input}");
        assert_ne!(report.removals, []);
        assert_eq!(report.skips, []);
        assert_eq!(
            text_of(&report.text, &[]),
            report.text,
            "idempotent {input}"
        );
    }
}

#[test]
fn unselected_and_unlisted_pairs_stay_byte_for_byte() {
    let preserved = [
        "https://example.com/guide?ref=1&id=7&utm_x=1&sz=64",
        "https://example.com/guide?source=chatgpt.com&sz=32&id=7",
        "https://example.com/guide?utm_source=gemini&id=7",
        "https://example.com/guide?utm_source=newsletter",
        "https://example.com/?gad_custom=keep&source=keep&ref=keep&q=keep&sz=64",
        "https://example.com/go?next=https%3A%2F%2Fexample.com%2F%3Futm_source%3Dchatgpt.com&id=7",
        "https://example.com/guide#utm_source=chatgpt.com",
        "https://example.com/utm_source=chatgpt.com?id=7",
        "https://example.com/guide?utm%5Fsource=chatgpt.com",
        "https://example.com/guide?utm_source=chatgpt.com%20",
        "https://example.com/guide?UTM_source=chatgpt.com",
        "Visit chatgpt.com later.",
    ];
    let all = [
        LinkCleanGroup::Campaign,
        LinkCleanGroup::Tracking,
        LinkCleanGroup::SourceChatgpt,
        LinkCleanGroup::Sz32,
    ];
    for input in preserved {
        assert_eq!(text_of(input, &[]), input);
        if input.contains("gad_custom")
            || input.contains("utm%5Fsource")
            || input.contains("chatgpt.com later")
        {
            assert_eq!(text_of(input, &all), input);
        }
    }
}

#[test]
fn optional_groups_remove_only_their_pairs() {
    let source = "https://example.com/guide?source=chatgpt.com&sz=32&id=7";
    assert_eq!(
        text_of(source, &[LinkCleanGroup::Sz32]),
        "https://example.com/guide?source=chatgpt.com&id=7"
    );
    assert_eq!(
        text_of(source, &[LinkCleanGroup::SourceChatgpt]),
        "https://example.com/guide?sz=32&id=7"
    );
    assert_eq!(
        text_of(
            source,
            &[LinkCleanGroup::SourceChatgpt, LinkCleanGroup::Sz32]
        ),
        "https://example.com/guide?id=7"
    );
    assert_eq!(
        text_of("https://example.com/a?sz=64", &[LinkCleanGroup::Sz32]),
        "https://example.com/a?sz=64"
    );
    assert_eq!(
        text_of(
            "https://example.com/a?source=keep",
            &[LinkCleanGroup::SourceChatgpt]
        ),
        "https://example.com/a?source=keep"
    );
    assert_eq!(
        text_of(
            "https://example.com/guide?utm_source=gemini&id=7",
            &[LinkCleanGroup::Campaign]
        ),
        "https://example.com/guide?id=7"
    );
    assert_eq!(
        text_of(
            "https://example.com/guide?utm_source=newsletter",
            &[LinkCleanGroup::Campaign]
        ),
        "https://example.com/guide"
    );
}

#[test]
fn tracking_group_removes_each_exact_key_and_hides_values() {
    let keys = [
        "gclid",
        "gbraid",
        "wbraid",
        "gad_source",
        "gad_campaignid",
        "srsltid",
        "msclkid",
        "fbclid",
        "mc_cid",
        "mc_eid",
    ];
    for key in keys {
        let input = format!("https://example.com/guide?id=7&{key}=fixture#part");
        assert_eq!(text_of(&input, &[]), input, "{key} stays by default");
        let report = entire(&input, &[LinkCleanGroup::Tracking]);
        assert_eq!(report.text, "https://example.com/guide?id=7#part");
        assert_eq!(report.removals.len(), 1);
        assert_eq!(report.removals[0].name, key);
        assert_eq!(report.removals[0].rule_id, "ad_tracking");
        assert!(!format!("{:?}", report.removals).contains("fixture"));
    }
    let mail = "https://example.com/?mc_cid=campaign-fixture&mc_eid=recipient-fixture&id=7";
    let report = entire(mail, &[LinkCleanGroup::Tracking]);
    assert_eq!(report.text, "https://example.com/?id=7");
    assert!(!format!("{:?}", report.removals).contains("recipient-fixture"));
    assert!(!format!("{:?}", report.removals).contains("campaign-fixture"));
}

#[test]
fn signed_urls_are_skipped_without_copying_the_token() {
    let secret = "super-secret-token";
    let cases = [
        format!("https://example.com/guide?id=7&utm_source=chatgpt.com&X-Amz-Signature={secret}"),
        format!("https://example.com/guide?id=7&utm_source=chatgpt.com&X-Goog-Signature={secret}"),
        format!(
            "https://example.com/guide?GoogleAccessId=account&Signature={secret}&utm_source=chatgpt.com"
        ),
        format!("https://example.com/guide?sig={secret}&se=expiry&utm_source=chatgpt.com"),
    ];
    for input in cases {
        let report = entire(&input, &[]);
        assert_eq!(report.text, input);
        assert_eq!(report.removals, []);
        assert_eq!(report.skips.len(), 1);
        assert_eq!(report.skips[0].reason, SIGNED_QUERY_REASON);
        assert!(!format!("{:?}", report.skips).contains(secret));
    }
    let unsigned_half = "https://example.com/guide?GoogleAccessId=account&utm_source=chatgpt.com";
    assert_eq!(
        text_of(unsigned_half, &[]),
        "https://example.com/guide?GoogleAccessId=account"
    );
    let sig_only = "https://example.com/guide?sig=token&utm_source=chatgpt.com";
    assert_eq!(
        text_of(sig_only, &[]),
        "https://example.com/guide?sig=token"
    );
}

#[test]
fn scope_is_a_literal_host_and_path_boundary() {
    let input = "https://example.com/guide?utm_source=chatgpt.com https://other.test/guide?utm_source=chatgpt.com https://example.com/other?utm_source=chatgpt.com https://example.com/guidebook?utm_source=chatgpt.com";
    let scoped = clean_links(
        input,
        &[],
        LinkCleanScope {
            host: Some("example.com"),
            path_prefix: Some("/guide"),
        },
    )
    .expect("scoped");
    assert_eq!(
        scoped.text,
        "https://example.com/guide https://other.test/guide?utm_source=chatgpt.com https://example.com/other?utm_source=chatgpt.com https://example.com/guidebook?utm_source=chatgpt.com"
    );
    assert_eq!(scoped.removals.len(), 1);
    let dotted = clean_links(
        "https://example.com./a?utm_source=chatgpt.com",
        &[],
        LinkCleanScope {
            host: Some("example.com"),
            path_prefix: None,
        },
    )
    .expect("dotted host");
    assert_eq!(dotted.text, "https://example.com./a?utm_source=chatgpt.com");
}

#[test]
fn locations_name_the_source_key_and_inspect_lists_only_catalogued_names() {
    let text = "See https://example.com/guide?id=7&utm_source=chatgpt.com now.";
    let report = entire(text, &[]);
    let key_at = text.find("utm_source=").expect("key");
    let point = TextPoint::from_byte_index(text, key_at).expect("point");
    assert_eq!(
        report.removals,
        vec![LinkParameter {
            line: point.line,
            column: point.column,
            rule_id: "utm_source_chatgpt_com",
            name: "utm_source",
        }]
    );
    let inventory = inventory_links(text).expect("inventory");
    assert_eq!(inventory.http_url_count, 1);
    assert_eq!(inventory.parameters, report.removals);
    let mixed =
        "https://example.com/a?utm_source=gemini&source=chatgpt.com&sz=32&gclid=fixture&id=7";
    let listed = inventory_links(mixed).expect("mixed");
    assert_eq!(listed.http_url_count, 1);
    assert_eq!(listed.parameters.len(), 2);
    assert_eq!(listed.parameters[0].rule_id, "ga_campaign");
    assert_eq!(listed.parameters[0].name, "utm_source");
    assert_eq!(listed.parameters[1].rule_id, "ad_tracking");
    assert_eq!(listed.parameters[1].name, "gclid");
    assert!(!format!("{:?}", listed.parameters).contains("gemini"));
    assert!(!format!("{:?}", listed.parameters).contains("fixture"));
    assert_eq!(
        inventory_links("No links here.")
            .expect("none")
            .http_url_count,
        0
    );
    assert_eq!(
        inventory_links("No links here.").expect("none").parameters,
        []
    );
}

#[test]
fn url_boundaries_match_the_protection_plan() {
    let text = "See https://example.com/guide?id=7. And (https://example.com/a).";
    let plan = ProtectionPlan::build(text, &[]).expect("plan");
    let ranges = protected_http_url_ranges(text).expect("ranges");
    let surfaces: Vec<_> = plan
        .values()
        .iter()
        .filter(|value| value.kind == ProtectedKind::Url)
        .map(|value| value.surface.as_str())
        .collect();
    let extracted: Vec<_> = ranges
        .iter()
        .map(|(start, end)| &text[*start..*end])
        .collect();
    assert_eq!(extracted, surfaces);
    let source = "See https://example.com/guide?id=7&utm_source=chatgpt.com.";
    let plan = ProtectionPlan::build(source, &[]).expect("plan");
    for candidate in [
        "See https://example.com/guide?id=7.",
        "See https://example.com/guide?utm_source=chatgpt.com&id=7.",
        "See https://example.org/guide?id=7&utm_source=chatgpt.com.",
        "See https://example.com/guide?id=7&utm_source=chatgpt%2Ecom.",
    ] {
        assert_eq!(
            plan.mask_raw_candidate(candidate),
            Err(ProtectionError::ProtectedOccurrenceCount),
            "{candidate}"
        );
    }
}

#[test]
fn catalog_keeps_exact_keys_and_leaves_signatures_out_of_removal() {
    let attribution = LINK_RULES
        .iter()
        .filter(|rule| rule.group == LinkRuleGroup::Attribution)
        .collect::<Vec<_>>();
    assert_eq!(attribution.len(), 1);
    assert_eq!(attribution[0].key, "utm_source");
    assert_eq!(attribution[0].value, Some("chatgpt.com"));
    assert_eq!(attribution[0].reviewed, "2026-10-22");
    let campaign = LINK_RULES
        .iter()
        .filter(|rule| rule.group == LinkRuleGroup::Campaign)
        .map(|rule| rule.key)
        .collect::<Vec<_>>();
    assert_eq!(
        campaign,
        [
            "utm_source",
            "utm_medium",
            "utm_campaign",
            "utm_term",
            "utm_content",
            "utm_id",
            "utm_source_platform",
            "utm_creative_format",
            "utm_marketing_tactic",
        ]
    );
    assert!(LINK_RULES.iter().all(|rule| rule.reviewed == "2026-10-22"));
    assert!(
        LINK_RULES
            .iter()
            .filter(|rule| rule.group == LinkRuleGroup::ReviewedSize)
            .all(|rule| rule.value == Some("32") && rule.key == "sz")
    );
    let signature_keys: Vec<_> = SIGNATURE_INDICATORS
        .iter()
        .flat_map(|indicator| indicator.keys.iter().copied())
        .collect();
    assert!(
        LINK_RULES
            .iter()
            .all(|rule| !signature_keys.contains(&rule.key))
    );
}
