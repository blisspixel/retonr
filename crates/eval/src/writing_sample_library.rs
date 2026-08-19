//! Versioned writing-sample library for development tests.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current writing-sample library schema version.
pub const WRITING_SAMPLE_LIBRARY_SCHEMA_VERSION: u32 = 1;
/// Maximum serialized library size.
pub const MAX_WRITING_SAMPLE_LIBRARY_BYTES: usize = 4 * 1024 * 1024;
/// Maximum samples in one library file.
pub const MAX_WRITING_SAMPLES: usize = 256;
const MAX_EXCERPT_BYTES: usize = 8 * 1024;
const MAX_NOTE_BYTES: usize = 1_024;

/// A versioned collection of labeled writing samples.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingSampleLibrary {
    /// Library contract version.
    pub schema_version: u32,
    /// Stable library identity.
    pub library_id: String,
    /// Declared use of this file.
    pub purpose: WritingSamplePurpose,
    /// Independently reportable samples.
    pub samples: Vec<WritingSample>,
}

/// Allowed uses of a writing-sample library.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingSamplePurpose {
    /// Development and review fixtures with no live rewrite authority.
    DevelopmentTestLibrary,
}

/// One labeled excerpt or synthetic impression.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WritingSample {
    /// Stable sample identifier.
    pub id: String,
    /// How the sample may be used.
    pub role: WritingSampleRole,
    /// How the text was obtained.
    pub origin: WritingSampleOrigin,
    /// Publication year when known.
    pub year: Option<u16>,
    /// SPDX license identifier for the excerpt.
    pub license_spdx: String,
    /// Work title for licensed public excerpts.
    pub source_title: Option<String>,
    /// Canonical source URL.
    pub source_url: Option<String>,
    /// Required attribution or copyright notice.
    pub attribution: String,
    /// BCP 47 language tag.
    pub language: String,
    /// Communication channel.
    pub channel: String,
    /// Optional modeled family for synthetic impressions only.
    pub modeled_family: Option<String>,
    /// Exact excerpt used in tests.
    pub excerpt: String,
    /// Why the sample is in the library.
    pub notes: String,
}

/// Role of a writing sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingSampleRole {
    /// Licensed pre-model human technical prose used as a control.
    HumanControl,
    /// Maintainer-written impression of a public model style.
    SyntheticImpression,
}

/// Provenance class for a writing sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WritingSampleOrigin {
    /// Maintainer-constructed text with no copied licensed source.
    Synthetic,
    /// Redistributable public work with a recorded license decision.
    LicensedPublic,
}

/// Content-free summary after validation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WritingSampleLibrarySummary {
    /// Library contract version.
    pub schema_version: u32,
    /// Stable library identity.
    pub library_id: String,
    /// Total samples.
    pub total: usize,
    /// Licensed human-control excerpts.
    pub human_controls: usize,
    /// Synthetic model-style impressions.
    pub synthetic_impressions: usize,
}

/// Writing-sample library parse or contract failure.
#[derive(Debug, Error)]
pub enum WritingSampleLibraryError {
    /// Serialized input exceeds the byte bound.
    #[error("writing-sample library exceeds the supported byte limit")]
    TooLarge,
    /// JSON is invalid or contains an unknown field.
    #[error("invalid writing-sample library: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// Unsupported schema version.
    #[error("unsupported writing-sample library schema version {0}")]
    UnsupportedSchema(u32),
    /// Library identity or purpose is invalid.
    #[error("writing-sample library contract is invalid")]
    InvalidLibrary,
    /// Too many samples.
    #[error("writing-sample library exceeds the sample-count limit")]
    TooManySamples,
    /// One sample failed validation.
    #[error("writing-sample {index} is invalid")]
    InvalidSample {
        /// Zero-based sample position.
        index: usize,
    },
    /// Duplicate sample identifier.
    #[error("writing-sample {index} has a duplicate identifier")]
    DuplicateSampleId {
        /// Zero-based sample position.
        index: usize,
    },
}

impl WritingSampleLibrary {
    /// Returns a content-free summary.
    #[must_use]
    pub fn summary(&self) -> WritingSampleLibrarySummary {
        let human_controls = self
            .samples
            .iter()
            .filter(|sample| sample.role == WritingSampleRole::HumanControl)
            .count();
        WritingSampleLibrarySummary {
            schema_version: self.schema_version,
            library_id: self.library_id.clone(),
            total: self.samples.len(),
            human_controls,
            synthetic_impressions: self.samples.len().saturating_sub(human_controls),
        }
    }
}

/// Parses and validates one writing-sample library.
///
/// # Errors
///
/// Returns [`WritingSampleLibraryError`] for size, JSON, or contract failures.
pub fn parse_writing_sample_library(
    input: &str,
) -> Result<WritingSampleLibrary, WritingSampleLibraryError> {
    if input.len() > MAX_WRITING_SAMPLE_LIBRARY_BYTES {
        return Err(WritingSampleLibraryError::TooLarge);
    }
    let library: WritingSampleLibrary = serde_json::from_str(input)?;
    validate_library(&library)?;
    Ok(library)
}

fn validate_library(library: &WritingSampleLibrary) -> Result<(), WritingSampleLibraryError> {
    if library.schema_version != WRITING_SAMPLE_LIBRARY_SCHEMA_VERSION
        || !valid_label(&library.library_id)
        || library.samples.is_empty()
    {
        return Err(WritingSampleLibraryError::InvalidLibrary);
    }
    if library.samples.len() > MAX_WRITING_SAMPLES {
        return Err(WritingSampleLibraryError::TooManySamples);
    }
    let mut identifiers = BTreeSet::new();
    for (index, sample) in library.samples.iter().enumerate() {
        if !identifiers.insert(sample.id.as_str()) {
            return Err(WritingSampleLibraryError::DuplicateSampleId { index });
        }
        if !valid_sample(sample) {
            return Err(WritingSampleLibraryError::InvalidSample { index });
        }
    }
    Ok(())
}

fn valid_sample(sample: &WritingSample) -> bool {
    if !valid_label(&sample.id)
        || !valid_label(&sample.language)
        || !valid_label(&sample.channel)
        || !valid_text(&sample.excerpt, MAX_EXCERPT_BYTES)
        || !valid_text(&sample.attribution, MAX_NOTE_BYTES)
        || !valid_text(&sample.notes, MAX_NOTE_BYTES)
        || !valid_license(&sample.license_spdx)
        || sample.excerpt.contains("watermark_free")
        || sample.notes.contains("watermark_free")
    {
        return false;
    }
    match sample.origin {
        WritingSampleOrigin::Synthetic => {
            sample.role == WritingSampleRole::SyntheticImpression
                && sample.license_spdx == "Apache-2.0"
                && sample.year.is_none()
                && sample.source_title.is_none()
                && sample.source_url.is_none()
                && sample.modeled_family.as_deref().is_some_and(valid_label)
        }
        WritingSampleOrigin::LicensedPublic => {
            sample.role == WritingSampleRole::HumanControl
                && sample.year.is_some_and(|year| (1..2018).contains(&year))
                && sample.source_title.as_deref().is_some_and(valid_text_title)
                && sample.source_url.as_deref().is_some_and(valid_http_url)
                && sample.modeled_family.is_none()
        }
    }
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && !value.chars().any(|ch| ch.is_control() && ch != '\n')
}

fn valid_text_title(value: &str) -> bool {
    valid_text(value, 256)
}

fn valid_license(value: &str) -> bool {
    matches!(
        value,
        "Apache-2.0" | "IETF-TLP" | "CC0-1.0" | "CC-BY-4.0" | "CC-PDDC"
    )
}

fn valid_http_url(value: &str) -> bool {
    (value.starts_with("https://") || value.starts_with("http://"))
        && value.len() <= 256
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;
