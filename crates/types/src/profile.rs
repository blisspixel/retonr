//! Versioned personal voice, brand constraint, and editorial brief profiles.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{EditLevel, LayoutConstraints};

/// Current personal voice profile schema version.
pub const PERSONAL_VOICE_PROFILE_SCHEMA_VERSION: u32 = 1;

/// Current brand constraint profile schema version.
pub const BRAND_CONSTRAINT_PROFILE_SCHEMA_VERSION: u32 = 1;

/// Current editorial brief schema version.
pub const EDITORIAL_BRIEF_SCHEMA_VERSION: u32 = 1;

/// Maximum allowed character length for an identifier string.
pub const MAX_PROFILE_ID_CHARS: usize = 64;

/// Maximum number of terms, preferences, or rules in one profile.
pub const MAX_PROFILE_ENTRIES: usize = 256;

/// Profile validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProfileValidationError {
    /// Schema version is unsupported.
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u32),
    /// Profile identifier is empty.
    #[error("profile identifier must not be empty")]
    EmptyId,
    /// Profile identifier exceeds the character limit.
    #[error("profile identifier length {0} exceeds maximum of {MAX_PROFILE_ID_CHARS}")]
    IdTooLong(usize),
    /// Total entries exceed the supported capacity.
    #[error("entry count {0} exceeds maximum of {MAX_PROFILE_ENTRIES}")]
    TooManyEntries(usize),
    /// An entry term is empty.
    #[error("term must not be empty")]
    EmptyTerm,
    /// Layout constraints are invalid.
    #[error("layout constraints are invalid: min exceeds max")]
    InvalidLayout,
}

/// Personal voice profile capturing the human speaker's authentic writing style.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PersonalVoiceProfile {
    /// Schema version.
    pub schema_version: u32,
    /// Unique profile identifier.
    pub id: String,
    /// Desired level of formality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formality: Option<FormalityLevel>,
    /// Desired level of directness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directness: Option<DirectnessLevel>,
    /// Target sentence length in words, if specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_sentence_words: Option<u32>,
    /// Whether contractions are permitted.
    #[serde(default)]
    pub allow_contractions: bool,
    /// Preferred phrasing and vocabulary selections.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vocabulary_preferences: Vec<VocabularyPreference>,
}

impl PersonalVoiceProfile {
    /// Validates the profile contract invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileValidationError`] if schema or bounds fail.
    pub fn validate(&self) -> Result<(), ProfileValidationError> {
        if self.schema_version != PERSONAL_VOICE_PROFILE_SCHEMA_VERSION {
            return Err(ProfileValidationError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        validate_profile_id(&self.id)?;
        if self.vocabulary_preferences.len() > MAX_PROFILE_ENTRIES {
            return Err(ProfileValidationError::TooManyEntries(
                self.vocabulary_preferences.len(),
            ));
        }
        for pref in &self.vocabulary_preferences {
            pref.validate()?;
        }
        Ok(())
    }
}

/// Formality level for personal voice.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FormalityLevel {
    /// Relaxed and conversational.
    Casual,
    /// Professional yet approachable.
    Balanced,
    /// Formal and authoritative.
    Formal,
}

/// Directness level for personal voice.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DirectnessLevel {
    /// Brief, to the point, minimal fluff.
    Concise,
    /// Standard balance of context and conclusion.
    Balanced,
    /// Rich explanations with contextual elaboration.
    Elaborate,
}

/// Preferred wording bias over alternatives.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VocabularyPreference {
    /// The preferred word or phrase.
    pub preferred: String,
    /// The dispreferred word or phrase to avoid.
    pub avoided: String,
}

impl VocabularyPreference {
    /// Validates that both preferred and avoided terms are non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileValidationError::EmptyTerm`] if either term is empty.
    pub fn validate(&self) -> Result<(), ProfileValidationError> {
        if self.preferred.trim().is_empty() || self.avoided.trim().is_empty() {
            return Err(ProfileValidationError::EmptyTerm);
        }
        Ok(())
    }
}

/// Artifact and brand constraint profile encapsulating extrinsic requirements.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrandConstraintProfile {
    /// Schema version.
    pub schema_version: u32,
    /// Unique profile identifier.
    pub id: String,
    /// Exact brand or product terms that must be protected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_terms: Vec<String>,
    /// Mandatory term substitutions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub term_substitutions: Vec<VocabularyPreference>,
    /// Prohibited words or phrases.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prohibited_terms: Vec<String>,
    /// Layout constraints for characters and lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConstraints>,
    /// Syntactic and presentation copy rules.
    #[serde(default, skip_serializing_if = "CopyRules::is_default")]
    pub copy_rules: CopyRules,
}

impl BrandConstraintProfile {
    /// Validates the brand constraint profile.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileValidationError`] if schema or bounds fail.
    pub fn validate(&self) -> Result<(), ProfileValidationError> {
        if self.schema_version != BRAND_CONSTRAINT_PROFILE_SCHEMA_VERSION {
            return Err(ProfileValidationError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        validate_profile_id(&self.id)?;
        let total_entries = self.protected_terms.len()
            + self.term_substitutions.len()
            + self.prohibited_terms.len();
        if total_entries > MAX_PROFILE_ENTRIES {
            return Err(ProfileValidationError::TooManyEntries(total_entries));
        }
        for term in &self.protected_terms {
            if term.trim().is_empty() {
                return Err(ProfileValidationError::EmptyTerm);
            }
        }
        for subst in &self.term_substitutions {
            subst.validate()?;
        }
        for term in &self.prohibited_terms {
            if term.trim().is_empty() {
                return Err(ProfileValidationError::EmptyTerm);
            }
        }
        if let Some(layout) = &self.layout {
            validate_layout(layout)?;
        }
        Ok(())
    }
}

/// Syntactic and presentation copy rules for structured documents.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CopyRules {
    /// Maximum word count for headlines, if constrained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headline_max_words: Option<u32>,
    /// Require call-to-action (CTA) elements to begin with an active verb.
    #[serde(default)]
    pub cta_begins_with_verb: bool,
    /// Disallow trailing periods in bulleted list items.
    #[serde(default)]
    pub avoid_periods_in_bullets: bool,
    /// Enforce sentence-case headings rather than title case.
    #[serde(default)]
    pub sentence_case_headings: bool,
}

impl CopyRules {
    /// Checks whether all rules are at their default inactive settings.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        self.headline_max_words.is_none()
            && !self.cta_begins_with_verb
            && !self.avoid_periods_in_bullets
            && !self.sentence_case_headings
    }
}

/// Document-specific editorial brief combining intent, audience, and constraints.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditorialBrief {
    /// Schema version.
    pub schema_version: u32,
    /// Unique brief identifier.
    pub id: String,
    /// Selected degree of editorial freedom.
    pub edit_level: EditLevel,
    /// Target audience description, if specified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    /// Non-negotiable protected claims or facts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_claims: Vec<String>,
    /// Non-negotiable exact terms to preserve.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protected_terms: Vec<String>,
    /// Situational layout bounds for this document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<LayoutConstraints>,
}

impl EditorialBrief {
    /// Validates the editorial brief invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileValidationError`] if schema or bounds fail.
    pub fn validate(&self) -> Result<(), ProfileValidationError> {
        if self.schema_version != EDITORIAL_BRIEF_SCHEMA_VERSION {
            return Err(ProfileValidationError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        validate_profile_id(&self.id)?;
        let total_entries = self.protected_claims.len() + self.protected_terms.len();
        if total_entries > MAX_PROFILE_ENTRIES {
            return Err(ProfileValidationError::TooManyEntries(total_entries));
        }
        for claim in &self.protected_claims {
            if claim.trim().is_empty() {
                return Err(ProfileValidationError::EmptyTerm);
            }
        }
        for term in &self.protected_terms {
            if term.trim().is_empty() {
                return Err(ProfileValidationError::EmptyTerm);
            }
        }
        if let Some(layout) = &self.layout {
            validate_layout(layout)?;
        }
        Ok(())
    }
}

fn validate_profile_id(id: &str) -> Result<(), ProfileValidationError> {
    if id.trim().is_empty() {
        return Err(ProfileValidationError::EmptyId);
    }
    if id.len() > MAX_PROFILE_ID_CHARS {
        return Err(ProfileValidationError::IdTooLong(id.len()));
    }
    Ok(())
}

fn validate_layout(layout: &LayoutConstraints) -> Result<(), ProfileValidationError> {
    if let Some(budget) = &layout.character_budget
        && let (Some(min), Some(max)) = (budget.min_characters, budget.max_characters)
        && min > max
    {
        return Err(ProfileValidationError::InvalidLayout);
    }
    if let Some(budget) = &layout.line_budget
        && let Some(max) = budget.max_lines
        && max == 0
    {
        return Err(ProfileValidationError::InvalidLayout);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn personal_voice_profile_round_trips() {
        let profile = PersonalVoiceProfile {
            schema_version: PERSONAL_VOICE_PROFILE_SCHEMA_VERSION,
            id: "author-default".to_owned(),
            formality: Some(FormalityLevel::Balanced),
            directness: Some(DirectnessLevel::Concise),
            target_sentence_words: Some(15),
            allow_contractions: true,
            vocabulary_preferences: vec![VocabularyPreference {
                preferred: "simple".to_owned(),
                avoided: "easy".to_owned(),
            }],
        };
        profile.validate().expect("profile must be valid");
        let encoded = serde_json::to_string(&profile).expect("serializes");
        let decoded: PersonalVoiceProfile = serde_json::from_str(&encoded).expect("deserializes");
        assert_eq!(profile, decoded);
    }

    #[test]
    fn brand_constraint_profile_round_trips() {
        let profile = BrandConstraintProfile {
            schema_version: BRAND_CONSTRAINT_PROFILE_SCHEMA_VERSION,
            id: "corp-brand-v1".to_owned(),
            protected_terms: vec!["Retonr".to_owned()],
            term_substitutions: vec![VocabularyPreference {
                preferred: "customers".to_owned(),
                avoided: "users".to_owned(),
            }],
            prohibited_terms: vec!["cheap".to_owned()],
            layout: None,
            copy_rules: CopyRules {
                headline_max_words: Some(8),
                cta_begins_with_verb: true,
                avoid_periods_in_bullets: true,
                sentence_case_headings: true,
            },
        };
        profile.validate().expect("profile must be valid");
        let encoded = serde_json::to_string(&profile).expect("serializes");
        let decoded: BrandConstraintProfile = serde_json::from_str(&encoded).expect("deserializes");
        assert_eq!(profile, decoded);
    }

    #[test]
    fn editorial_brief_round_trips() {
        let brief = EditorialBrief {
            schema_version: EDITORIAL_BRIEF_SCHEMA_VERSION,
            id: "q3-deck-brief".to_owned(),
            edit_level: EditLevel::VoicePass,
            audience: Some("Executive leadership".to_owned()),
            protected_claims: vec!["Revenue grew 37 percent year-over-year".to_owned()],
            protected_terms: vec!["Retonr".to_owned()],
            layout: None,
        };
        brief.validate().expect("brief must be valid");
        let encoded = serde_json::to_string(&brief).expect("serializes");
        let decoded: EditorialBrief = serde_json::from_str(&encoded).expect("deserializes");
        assert_eq!(brief, decoded);
    }

    #[test]
    fn rejects_invalid_schema_or_empty_id() {
        let bad_schema = PersonalVoiceProfile {
            schema_version: 999,
            id: "valid-id".to_owned(),
            formality: None,
            directness: None,
            target_sentence_words: None,
            allow_contractions: false,
            vocabulary_preferences: Vec::new(),
        };
        assert!(matches!(
            bad_schema.validate(),
            Err(ProfileValidationError::UnsupportedSchema(999))
        ));

        let empty_id = PersonalVoiceProfile {
            schema_version: PERSONAL_VOICE_PROFILE_SCHEMA_VERSION,
            id: "   ".to_owned(),
            formality: None,
            directness: None,
            target_sentence_words: None,
            allow_contractions: false,
            vocabulary_preferences: Vec::new(),
        };
        assert!(matches!(
            empty_id.validate(),
            Err(ProfileValidationError::EmptyId)
        ));
    }
}
