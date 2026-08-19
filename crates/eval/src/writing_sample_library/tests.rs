use super::{
    WRITING_SAMPLE_LIBRARY_SCHEMA_VERSION, WritingSampleOrigin, WritingSampleRole,
    parse_writing_sample_library,
};

const LICENSED: &str = include_str!("../../fixtures/writing_samples/licensed_pre_ai_human_v1.json");
const IMPRESSIONS: &str =
    include_str!("../../fixtures/writing_samples/synthetic_model_impressions_v1.json");

#[test]
fn licensed_pre_ai_samples_are_human_controls_before_2018() {
    let library = parse_writing_sample_library(LICENSED).expect("licensed library");
    let summary = library.summary();
    assert_eq!(
        summary.schema_version,
        WRITING_SAMPLE_LIBRARY_SCHEMA_VERSION
    );
    assert_eq!(summary.total, 8);
    assert_eq!(summary.human_controls, 8);
    assert_eq!(summary.synthetic_impressions, 0);
    assert!(library.samples.iter().all(|sample| {
        sample.origin == WritingSampleOrigin::LicensedPublic
            && sample.role == WritingSampleRole::HumanControl
            && sample.year.is_some_and(|year| year < 2018)
            && sample.modeled_family.is_none()
    }));
    assert!(
        library
            .samples
            .iter()
            .filter(|sample| sample.year.is_some_and(|year| year < 2000))
            .count()
            >= 5
    );
    let ids = library
        .samples
        .iter()
        .map(|sample| sample.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ids,
        [
            "cdc-mmwr-program-evaluation-1999",
            "nist-sp-800-12-handbook-1995",
            "plos-plasmodium-transcriptome-2003",
            "rfc-1034-domain-concepts-1987",
            "rfc-1945-http-10-1996",
            "rfc-791-internet-protocol-1981",
            "rfc-793-transmission-control-1981",
            "usgs-this-dynamic-earth-1996"
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
    );
    let channels = library
        .samples
        .iter()
        .map(|sample| sample.channel.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(channels.contains("specification"));
    assert!(channels.contains("essay"));
    assert!(channels.contains("article"));
    assert!(channels.contains("handbook"));
    assert!(!LICENSED.contains("watermark_free"));
}

#[test]
fn synthetic_impressions_are_labeled_and_not_authorship_claims() {
    let library = parse_writing_sample_library(IMPRESSIONS).expect("impression library");
    let summary = library.summary();
    assert_eq!(summary.total, 7);
    assert_eq!(summary.synthetic_impressions, 7);
    let families = library
        .samples
        .iter()
        .filter_map(|sample| sample.modeled_family.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        families,
        ["chatgpt", "claude", "gemini", "grok"]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert!(IMPRESSIONS.contains("Not model output"));
    assert!(!IMPRESSIONS.contains("watermark_free"));
    assert!(
        library
            .samples
            .iter()
            .any(|sample| sample.channel == "article")
    );
    assert!(
        library
            .samples
            .iter()
            .any(|sample| sample.id.contains("article"))
    );
}
