//! In-memory style-feature distance.
//!
//! Samples are pooled into counts and then discarded by the caller. The
//! distance compares rates, not raw text. It is not an authorship verdict and
//! it has no acceptance or ranking authority.

use serde::Serialize;

use crate::lint::{RuleCatalog, lint_text};

/// Stable notice carried beside every style-feature distance.
pub const STYLE_DISTANCE_NOTICE: &str = "This is a style-feature distance, not an authorship verdict. It has no acceptance or ranking authority.";

const KIND: &str = "style-feature-distance-v1";

const FUNCTION_WORDS: &[&str] = &[
    "a", "about", "after", "an", "and", "are", "as", "at", "be", "because", "been", "before",
    "being", "between", "but", "by", "can", "could", "did", "do", "does", "for", "from", "had",
    "has", "have", "he", "i", "if", "in", "into", "is", "it", "its", "may", "me", "might", "my",
    "no", "not", "of", "on", "or", "our", "over", "she", "should", "so", "that", "the", "then",
    "these", "they", "this", "those", "through", "to", "was", "we", "were", "will", "with",
    "would", "yes", "you", "your",
];

const SCALAR_FEATURES: &[&str] = &[
    "function_word_per_thousand",
    "mean_sentence_tokens_millis",
    "sentences_1_to_8_per_thousand",
    "sentences_9_to_18_per_thousand",
    "sentences_19_to_30_per_thousand",
    "sentences_over_30_per_thousand",
    "comma_per_thousand",
    "period_per_thousand",
    "semicolon_per_thousand",
    "colon_per_thousand",
    "exclamation_per_thousand",
    "question_per_thousand",
    "contraction_per_thousand",
];

/// One rate in the documented feature set.
///
/// Values are counts per thousand, or milliseconds of mean sentence length.
/// None of the fields copy sample text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct StyleFeature {
    /// Stable feature name.
    pub name: &'static str,
    /// Pooled sample rate.
    pub sample: u32,
    /// Source rate.
    pub source: u32,
    /// Candidate rate. This is the reviewed candidate, including when a check abstains.
    pub output: u32,
    /// `output` minus `source`.
    pub output_minus_source: i64,
}

/// Report-only distance from pooled samples to the source and the candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StyleDistance {
    /// Measurement identifier.
    pub kind: &'static str,
    /// Statement that this is not an authorship verdict.
    pub notice: &'static str,
    /// L1 distance from the pooled samples to the source.
    pub source_distance: u64,
    /// L1 distance from the pooled samples to the candidate.
    pub output_distance: u64,
    /// Per-feature rates and the candidate-minus-source delta.
    pub features: Vec<StyleFeature>,
}

/// Pooled sample counts. The text used to build this value is not retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSampleProfile {
    counts: Counts,
}

impl StyleSampleProfile {
    /// Pools authorized sample texts into rates. The strings are not stored.
    #[must_use]
    pub fn from_samples<'a>(samples: impl IntoIterator<Item = &'a str>) -> Self {
        let mut counts = Counts::default();
        for sample in samples {
            counts.add(&measure(sample));
        }
        Self { counts }
    }

    /// Compares this profile with a source and the candidate under review.
    #[must_use]
    pub fn distance(&self, source: &str, output: &str) -> StyleDistance {
        let sample = rates(&self.counts);
        let source_rates = rates(&measure(source));
        let output_rates = rates(&measure(output));
        let features = feature_rows(&sample, &source_rates, &output_rates);
        StyleDistance {
            kind: KIND,
            notice: STYLE_DISTANCE_NOTICE,
            source_distance: l1(&sample, &source_rates),
            output_distance: l1(&sample, &output_rates),
            features,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Counts {
    tokens: u64,
    function_words: u64,
    sentences: u64,
    sentence_tokens: u64,
    short_sentences: u64,
    medium_sentences: u64,
    long_sentences: u64,
    longer_sentences: u64,
    commas: u64,
    periods: u64,
    semicolons: u64,
    colons: u64,
    exclamations: u64,
    questions: u64,
    contractions: u64,
    lint: [u64; RuleCatalog::ALL.len()],
}

impl Counts {
    fn add(&mut self, other: &Self) {
        self.tokens = self.tokens.saturating_add(other.tokens);
        self.function_words = self.function_words.saturating_add(other.function_words);
        self.sentences = self.sentences.saturating_add(other.sentences);
        self.sentence_tokens = self.sentence_tokens.saturating_add(other.sentence_tokens);
        self.short_sentences = self.short_sentences.saturating_add(other.short_sentences);
        self.medium_sentences = self.medium_sentences.saturating_add(other.medium_sentences);
        self.long_sentences = self.long_sentences.saturating_add(other.long_sentences);
        self.longer_sentences = self.longer_sentences.saturating_add(other.longer_sentences);
        self.commas = self.commas.saturating_add(other.commas);
        self.periods = self.periods.saturating_add(other.periods);
        self.semicolons = self.semicolons.saturating_add(other.semicolons);
        self.colons = self.colons.saturating_add(other.colons);
        self.exclamations = self.exclamations.saturating_add(other.exclamations);
        self.questions = self.questions.saturating_add(other.questions);
        self.contractions = self.contractions.saturating_add(other.contractions);
        for (slot, value) in self.lint.iter_mut().zip(other.lint) {
            *slot = slot.saturating_add(value);
        }
    }
}

struct Rates {
    values: Vec<u32>,
}

fn measure(text: &str) -> Counts {
    let mut counts = Counts::default();
    for word in text.split_whitespace() {
        counts.tokens = counts.tokens.saturating_add(1);
        if is_function_word(word) {
            counts.function_words = counts.function_words.saturating_add(1);
        }
        if is_contraction(word) {
            counts.contractions = counts.contractions.saturating_add(1);
        }
    }
    for sentence in text.split(['.', '!', '?', '\n']) {
        let tokens = u64::try_from(sentence.split_whitespace().count()).unwrap_or(u64::MAX);
        if tokens == 0 {
            continue;
        }
        counts.sentences = counts.sentences.saturating_add(1);
        counts.sentence_tokens = counts.sentence_tokens.saturating_add(tokens);
        match tokens {
            1..=8 => counts.short_sentences = counts.short_sentences.saturating_add(1),
            9..=18 => counts.medium_sentences = counts.medium_sentences.saturating_add(1),
            19..=30 => counts.long_sentences = counts.long_sentences.saturating_add(1),
            _ => counts.longer_sentences = counts.longer_sentences.saturating_add(1),
        }
    }
    for character in text.chars() {
        match character {
            ',' => counts.commas = counts.commas.saturating_add(1),
            '.' => counts.periods = counts.periods.saturating_add(1),
            ';' => counts.semicolons = counts.semicolons.saturating_add(1),
            ':' => counts.colons = counts.colons.saturating_add(1),
            '!' => counts.exclamations = counts.exclamations.saturating_add(1),
            '?' => counts.questions = counts.questions.saturating_add(1),
            _ => {}
        }
    }
    for finding in lint_text(text) {
        if let Some(index) = RuleCatalog::ALL
            .iter()
            .position(|rule| rule.id() == finding.rule_id)
        {
            counts.lint[index] = counts.lint[index].saturating_add(1);
        }
    }
    counts
}

fn rates(counts: &Counts) -> Rates {
    let mut values = vec![
        per_thousand(counts.function_words, counts.tokens),
        mean_millis(counts.sentence_tokens, counts.sentences),
        per_thousand(counts.short_sentences, counts.sentences),
        per_thousand(counts.medium_sentences, counts.sentences),
        per_thousand(counts.long_sentences, counts.sentences),
        per_thousand(counts.longer_sentences, counts.sentences),
        per_thousand(counts.commas, counts.tokens),
        per_thousand(counts.periods, counts.tokens),
        per_thousand(counts.semicolons, counts.tokens),
        per_thousand(counts.colons, counts.tokens),
        per_thousand(counts.exclamations, counts.tokens),
        per_thousand(counts.questions, counts.tokens),
        per_thousand(counts.contractions, counts.tokens),
    ];
    for lint_count in counts.lint {
        values.push(per_thousand(lint_count, counts.tokens));
    }
    Rates { values }
}

fn feature_rows(sample: &Rates, source: &Rates, output: &Rates) -> Vec<StyleFeature> {
    let mut names = SCALAR_FEATURES.to_vec();
    for rule in RuleCatalog::ALL {
        names.push(lint_feature_name(rule));
    }
    names
        .into_iter()
        .zip(sample.values.iter())
        .zip(source.values.iter())
        .zip(output.values.iter())
        .map(
            |(((name, sample_rate), source_rate), output_rate)| StyleFeature {
                name,
                sample: *sample_rate,
                source: *source_rate,
                output: *output_rate,
                output_minus_source: i64::from(*output_rate) - i64::from(*source_rate),
            },
        )
        .collect()
}

fn lint_feature_name(rule: RuleCatalog) -> &'static str {
    match rule {
        RuleCatalog::PrefabricatedSceneSetting => "lint_prefabricated_scene_setting_per_thousand",
        RuleCatalog::ContrastiveReframe => "lint_contrastive_reframe_per_thousand",
        RuleCatalog::StackedTricolon => "lint_stacked_tricolon_per_thousand",
        RuleCatalog::PromotionalPuffery => "lint_promotional_puffery_per_thousand",
        RuleCatalog::OrnamentalAbstraction => "lint_ornamental_abstraction_per_thousand",
        RuleCatalog::ReaderSegmentation => "lint_reader_segmentation_per_thousand",
        RuleCatalog::PerformativeQuestionAnswer => "lint_performative_question_answer_per_thousand",
        RuleCatalog::EmptySignificance => "lint_empty_significance_per_thousand",
        RuleCatalog::SummaryPileup => "lint_summary_pileup_per_thousand",
        RuleCatalog::GenericPositiveClose => "lint_generic_positive_close_per_thousand",
        RuleCatalog::TransitionPileup => "lint_transition_pileup_per_thousand",
        RuleCatalog::PerformativeCollaboration => "lint_performative_collaboration_per_thousand",
        RuleCatalog::ConversationalResidue => "lint_conversational_residue_per_thousand",
        RuleCatalog::ThroatClearing => "lint_throat_clearing_per_thousand",
        RuleCatalog::CannedTransition => "lint_canned_transition_per_thousand",
        RuleCatalog::RepeatedConclusion => "lint_repeated_conclusion_per_thousand",
        RuleCatalog::VagueAttribution => "lint_vague_attribution_per_thousand",
        RuleCatalog::RedundantQualifier => "lint_redundant_qualifier_per_thousand",
        RuleCatalog::ExcessiveExclamation => "lint_excessive_exclamation_per_thousand",
        RuleCatalog::ExcessiveDashDensity => "lint_excessive_dash_density_per_thousand",
        RuleCatalog::ExcessiveEmojiDensity => "lint_excessive_emoji_density_per_thousand",
    }
}

fn l1(left: &Rates, right: &Rates) -> u64 {
    left.values
        .iter()
        .zip(&right.values)
        .map(|(left, right)| u64::from(left.abs_diff(*right)))
        .fold(0, u64::saturating_add)
}

fn per_thousand(count: u64, total: u64) -> u32 {
    if total == 0 || count == 0 {
        return 0;
    }
    let rounded = count.saturating_mul(1000).saturating_add(total / 2) / total;
    u32::try_from(rounded).unwrap_or(u32::MAX)
}

fn mean_millis(tokens: u64, sentences: u64) -> u32 {
    if sentences == 0 || tokens == 0 {
        return 0;
    }
    let rounded = tokens.saturating_mul(1000).saturating_add(sentences / 2) / sentences;
    u32::try_from(rounded).unwrap_or(u32::MAX)
}

fn is_function_word(word: &str) -> bool {
    let mut normalized = String::new();
    let trimmed = word.trim_matches(|character: char| !character.is_alphanumeric());
    for character in trimmed.chars() {
        for lower in character.to_lowercase() {
            normalized.push(lower);
        }
    }
    FUNCTION_WORDS.binary_search(&normalized.as_str()).is_ok()
}

fn is_contraction(word: &str) -> bool {
    let mut saw_letter = false;
    let mut chars = word.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\'' || character == '\u{2019}' {
            if saw_letter && chars.peek().is_some_and(|next| next.is_alphanumeric()) {
                return true;
            }
        } else if character.is_alphanumeric() {
            saw_letter = true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{FUNCTION_WORDS, STYLE_DISTANCE_NOTICE, StyleSampleProfile};

    #[test]
    fn function_words_stay_sorted_and_distance_keeps_no_sample_text() {
        for pair in FUNCTION_WORDS.windows(2) {
            assert!(pair[0] < pair[1], "{} then {}", pair[0], pair[1]);
        }
        let profile = StyleSampleProfile::from_samples(["qxsampletokenqx truly groundbreaking"]);
        let distance = profile.distance("the cat sat", "The cat sat");
        assert_eq!(distance.kind, "style-feature-distance-v1");
        assert_eq!(distance.notice, STYLE_DISTANCE_NOTICE);
        assert!(distance.source_distance > 0);
        let encoded = format!("{distance:?}");
        assert!(!encoded.contains("qxsampletokenqx"));
        assert!(!encoded.contains("groundbreaking"));
        assert!(
            distance
                .features
                .iter()
                .any(|feature| feature.name == "function_word_per_thousand")
        );
        let function_words = distance
            .features
            .iter()
            .find(|feature| feature.name == "function_word_per_thousand")
            .expect("function-word feature");
        assert_eq!(
            function_words.output_minus_source,
            i64::from(function_words.output) - i64::from(function_words.source)
        );
    }

    #[test]
    fn punctuation_contractions_and_long_sentences_change_rates() {
        let profile = StyleSampleProfile::from_samples([
            "Don't stop; wait: really? Yes, go!\n",
            "one two three four five six seven eight nine ten.",
        ]);
        let distance = profile.distance("the cat sat", "the cat sat.");
        let contraction = distance
            .features
            .iter()
            .find(|feature| feature.name == "contraction_per_thousand")
            .expect("contraction feature");
        assert!(contraction.sample > 0);
        assert_eq!(contraction.source, 0);
        let medium = distance
            .features
            .iter()
            .find(|feature| feature.name == "sentences_9_to_18_per_thousand")
            .expect("medium sentence feature");
        assert!(medium.sample > 0);
        assert!(distance.output_distance > 0);
    }

    #[test]
    fn identical_texts_have_zero_distance_and_zero_deltas() {
        let profile = StyleSampleProfile::from_samples(["the cat sat"]);
        let distance = profile.distance("the cat sat", "the cat sat");
        assert_eq!(distance.source_distance, 0);
        assert_eq!(distance.output_distance, 0);
        assert!(
            distance
                .features
                .iter()
                .all(|feature| feature.output_minus_source == 0)
        );
    }
}
