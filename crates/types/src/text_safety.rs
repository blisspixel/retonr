//! Shared plain-text safety policy for hard controls and retained invisible runs.
//!
//! Hard controls, bidi marks, and zero-width joiners still make a source
//! byte-identical only. Variation selectors, tag characters, and the other
//! inventoried invisible marks may stay only as runs the source already had.
//! An edit may drop those runs. It may not add or lengthen one.

/// One located text-safety rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextSafetyFault {
    /// The source contains a hard-unsafe character and the texts differ.
    HardAltered {
        /// Byte offset of the first difference.
        at: usize,
    },
    /// The candidate introduces a hard-unsafe character.
    HardIntroduced {
        /// Byte offset of that character in the candidate.
        at: usize,
    },
    /// The candidate introduces a retained invisible run the source lacks.
    InvisibleIntroduced {
        /// Byte offset of the first extra carrier in the candidate.
        at: usize,
    },
}

/// Reports whether a candidate keeps the source text-safety contract.
#[must_use]
pub fn replacement_preserves_text_safety(source: &str, replacement: &str) -> bool {
    first_text_safety_fault(source, replacement).is_none()
}

/// Returns the first text-safety fault, when the candidate is not safe.
#[must_use]
pub fn first_text_safety_fault(source: &str, replacement: &str) -> Option<TextSafetyFault> {
    if source.chars().any(is_hard_unsafe_text_character) {
        return first_difference(source, replacement).map(|at| TextSafetyFault::HardAltered { at });
    }
    if let Some(at) = first_hard_offset(replacement) {
        return Some(TextSafetyFault::HardIntroduced { at });
    }
    first_introduced_carrier(source, replacement)
        .map(|at| TextSafetyFault::InvisibleIntroduced { at })
}

/// A control, bidi mark, or zero-width joiner that freezes a source.
#[must_use]
pub fn is_hard_unsafe_text_character(character: char) -> bool {
    let codepoint = u32::from(character);
    (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || matches!(
            codepoint,
            0x061C | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F | 0xFEFF
        )
}

/// Explicit bidi controls counted separately from other format marks.
#[must_use]
pub fn is_bidi_mark(character: char) -> bool {
    matches!(
        u32::from(character),
        0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069
    )
}

/// Variation selectors, including the supplementary-plane selectors.
#[must_use]
pub fn is_variation_selector(character: char) -> bool {
    matches!(u32::from(character), 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

/// Unicode tag characters. These are not variation selectors and do not freeze a source.
#[must_use]
pub fn is_tag_character(character: char) -> bool {
    matches!(u32::from(character), 0xE0001 | 0xE0020..=0xE007F)
}

/// Zero-width marks, including the Mongolian vowel separator.
#[must_use]
pub fn is_zero_width_mark(character: char) -> bool {
    matches!(
        u32::from(character),
        0x200B..=0x200D | 0x2060 | 0xFEFF | 0x180E
    )
}

/// Other inventoried invisible marks that are not controls, selectors, or tag characters.
#[must_use]
pub fn is_other_invisible_mark(character: char) -> bool {
    matches!(
        u32::from(character),
        0x00AD
            | 0x034F
            | 0x115F
            | 0x1160
            | 0x17B4
            | 0x17B5
            | 0x180B..=0x180D
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x2061..=0x2064
            | 0x206A..=0x206F
            | 0x3164
            | 0xFFA0
            | 0xFFF9..=0xFFFB
    )
}

/// Invisible marks that may be kept only when the source already has the run.
#[must_use]
pub fn is_retained_invisible_character(character: char) -> bool {
    !is_hard_unsafe_text_character(character)
        && (is_variation_selector(character)
            || is_zero_width_mark(character)
            || is_tag_character(character)
            || is_other_invisible_mark(character))
}

struct CarrierChar {
    character: char,
    offset: usize,
}

struct LocatedRun {
    base: Option<char>,
    carriers: Vec<CarrierChar>,
}

impl LocatedRun {
    fn key(&self) -> (Option<char>, Vec<char>) {
        (
            self.base,
            self.carriers
                .iter()
                .map(|carrier| carrier.character)
                .collect(),
        )
    }
}

fn first_introduced_carrier(source: &str, candidate: &str) -> Option<usize> {
    let source_runs = carrier_runs(source);
    let candidate_runs = carrier_runs(candidate);
    let mut available = std::collections::BTreeMap::<(Option<char>, Vec<char>), usize>::new();
    for run in &source_runs {
        *available.entry(run.key()).or_default() += 1;
    }
    for run in &candidate_runs {
        if consume_run(&mut available, &run.key()) {
            continue;
        }
        return Some(introduced_offset(run, &source_runs));
    }
    None
}

fn consume_run(
    available: &mut std::collections::BTreeMap<(Option<char>, Vec<char>), usize>,
    key: &(Option<char>, Vec<char>),
) -> bool {
    let Some(count) = available.get_mut(key) else {
        return false;
    };
    if *count == 0 {
        return false;
    }
    *count -= 1;
    true
}

fn introduced_offset(run: &LocatedRun, source_runs: &[LocatedRun]) -> usize {
    let mut prefix = 0;
    for source in source_runs {
        if source.base != run.base {
            continue;
        }
        let shared = source
            .carriers
            .iter()
            .zip(run.carriers.iter())
            .take_while(|(left, right)| left.character == right.character)
            .count();
        if shared < run.carriers.len() {
            prefix = prefix.max(shared);
        }
    }
    run.carriers[prefix].offset
}

fn carrier_runs(text: &str) -> Vec<LocatedRun> {
    let mut runs = Vec::new();
    let mut index = 0;
    while let Some((character, width)) = char_at(text, index) {
        if is_retained_invisible_character(character) {
            let mut carriers = Vec::new();
            while let Some((current, current_width)) = char_at(text, index) {
                if !is_retained_invisible_character(current) {
                    break;
                }
                carriers.push(CarrierChar {
                    character: current,
                    offset: index,
                });
                index += current_width;
            }
            runs.push(LocatedRun {
                base: None,
                carriers,
            });
            continue;
        }
        index += width;
        let mut carriers = Vec::new();
        while let Some((current, current_width)) = char_at(text, index) {
            if !is_retained_invisible_character(current) {
                break;
            }
            carriers.push(CarrierChar {
                character: current,
                offset: index,
            });
            index += current_width;
        }
        if !carriers.is_empty() {
            runs.push(LocatedRun {
                base: Some(character),
                carriers,
            });
        }
    }
    runs
}

fn char_at(text: &str, index: usize) -> Option<(char, usize)> {
    let character = text.get(index..)?.chars().next()?;
    Some((character, character.len_utf8()))
}

fn first_hard_offset(text: &str) -> Option<usize> {
    let mut offset = 0;
    for character in text.chars() {
        if is_hard_unsafe_text_character(character) {
            return Some(offset);
        }
        offset += character.len_utf8();
    }
    None
}

fn first_difference(left: &str, right: &str) -> Option<usize> {
    let mut offset = 0;
    for (left_char, right_char) in left.chars().zip(right.chars()) {
        if left_char != right_char {
            return Some(offset);
        }
        offset += left_char.len_utf8();
    }
    if left.len() == right.len() {
        None
    } else {
        Some(offset.min(left.len()).min(right.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TextSafetyFault, is_hard_unsafe_text_character, is_retained_invisible_character,
        replacement_preserves_text_safety,
    };

    fn introduced(source: &str, candidate: &str) -> bool {
        matches!(
            super::first_text_safety_fault(source, candidate),
            Some(TextSafetyFault::InvisibleIntroduced { .. })
        )
    }

    #[test]
    fn hard_controls_still_freeze_the_source_and_reject_insertion() {
        assert!(!replacement_preserves_text_safety(
            "safe text",
            "safe\u{1b} text"
        ));
        assert!(!replacement_preserves_text_safety(
            "Existing\u{200b} marker retained.",
            "Existing\u{200b} marker retained!"
        ));
        assert!(replacement_preserves_text_safety(
            "Existing\u{200b} marker retained.",
            "Existing\u{200b} marker retained."
        ));
        assert!(replacement_preserves_text_safety(
            "safe\ttext",
            "safe\ttext."
        ));
    }

    #[test]
    fn added_invisible_runs_fail_and_kept_or_removed_runs_pass() {
        let source = "The meeting is at noon.\n";
        for character in [
            '\u{E0068}',
            '\u{E0001}',
            '\u{FE0F}',
            '\u{E0100}',
            '\u{180E}',
            '\u{034F}',
            '\u{00AD}',
            '\u{17B4}',
            '\u{180B}',
            '\u{FFF9}',
            '\u{202F}',
            '\u{3164}',
            '\u{115F}',
            '\u{FFA0}',
        ] {
            let mut candidate = source.to_owned();
            candidate.pop();
            candidate.push(character);
            candidate.push('\n');
            assert!(
                introduced(source, &candidate),
                "U+{code:04X} was accepted",
                code = u32::from(character)
            );
            assert!(
                !is_hard_unsafe_text_character(character),
                "U+{code:04X} must stay out of the freeze set",
                code = u32::from(character)
            );
        }

        let emoji = "I \u{2764}\u{FE0F} the plan.  It is good.";
        let tightened = "I \u{2764}\u{FE0F} the plan. It is good.";
        assert!(replacement_preserves_text_safety(emoji, tightened));
        assert!(replacement_preserves_text_safety(
            emoji,
            "I the plan.  It is good."
        ));

        let flag = "See \u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} today.";
        assert!(replacement_preserves_text_safety(
            flag,
            "See \u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F} today!"
        ));
        let lengthened = format!("{flag}\u{E0061}\u{E0062}\u{E0063}\u{E0064}");
        assert!(introduced(flag, &lengthened));
        assert!(is_retained_invisible_character('\u{FE0F}'));
        assert!(!is_retained_invisible_character('\u{200B}'));
        assert!(super::is_tag_character('\u{E0068}'));
        assert!(super::is_tag_character('\u{E0001}'));
        assert!(super::is_tag_character('\u{E007F}'));
        assert!(!super::is_tag_character('\u{FE0F}'));
        assert!(!super::is_tag_character('\u{00AD}'));
        assert!(!super::is_other_invisible_mark('\u{E0068}'));
        assert!(super::is_other_invisible_mark('\u{00AD}'));
    }
}
