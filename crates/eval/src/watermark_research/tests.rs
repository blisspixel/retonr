use super::{
    WATERMARK_RESEARCH_SCHEMA_VERSION, WatermarkResearchOutcome, parse_watermark_research_corpus,
};

const CORPUS: &str =
    include_str!("../../fixtures/watermark_research/style_is_not_a_watermark_v1.json");

#[test]
fn style_confusion_corpus_refuses_mark_labels_and_pairs_controls() {
    let corpus = parse_watermark_research_corpus(CORPUS).expect("research corpus");
    let summary = corpus.summary();
    assert_eq!(summary.schema_version, WATERMARK_RESEARCH_SCHEMA_VERSION);
    assert_eq!(summary.total, 12);
    assert_eq!(summary.refused_style_as_mark, 4);
    assert_eq!(summary.unmarked_controls, 6);
    assert!(corpus.research_only);
    assert!(!CORPUS.contains("watermark_free"));
    assert!(!CORPUS.contains("\"watermarked\""));
    assert!(
        corpus.cases.iter().any(|case| {
            case.expected_outcome == WatermarkResearchOutcome::CarrierShapeUnparsed
        })
    );
    assert!(
        corpus
            .cases
            .iter()
            .any(|case| case.expected_outcome == WatermarkResearchOutcome::LiteralObservation)
    );
}
