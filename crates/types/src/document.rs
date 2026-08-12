use core::{fmt, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{Digest, SCHEMA_VERSION};

/// Identifier for one parsed document instance.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DocumentId(String);

impl DocumentId {
    /// Creates an identifier from the source digest.
    #[must_use]
    pub fn from_digest(digest: &Digest) -> Self {
        Self(format!("document:{}", digest.as_str()))
    }

    /// Returns the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for DocumentId {
    type Err = IdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(digest) = value.strip_prefix("document:") else {
            return Err(IdentifierError::Document);
        };
        Digest::from_sha256_hex(digest).map_err(|_error| IdentifierError::Document)?;
        Ok(Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for DocumentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Identifier for one independently addressable rewrite unit.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RewriteUnitId(String);

impl RewriteUnitId {
    /// Creates an identifier scoped to a parsed document.
    #[must_use]
    pub fn new(document: &DocumentId, ordinal: usize) -> Self {
        Self(format!("{}:unit:{ordinal}", document.as_str()))
    }

    /// Returns the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RewriteUnitId {
    type Err = IdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((document, ordinal)) = value.rsplit_once(":unit:") else {
            return Err(IdentifierError::RewriteUnit);
        };
        document
            .parse::<DocumentId>()
            .map_err(|_error| IdentifierError::RewriteUnit)?;
        let parsed = ordinal
            .parse::<usize>()
            .map_err(|_error| IdentifierError::RewriteUnit)?;
        if parsed.to_string() != ordinal {
            return Err(IdentifierError::RewriteUnit);
        }
        Ok(Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for RewriteUnitId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Error returned for a noncanonical domain identifier.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum IdentifierError {
    /// Document identifier is not `document:` plus a canonical digest.
    #[error("invalid document identifier")]
    Document,
    /// Rewrite-unit identifier does not contain a valid document and ordinal.
    #[error("invalid rewrite-unit identifier")]
    RewriteUnit,
}

/// Inclusive start and exclusive end byte offsets in decoded UTF-8 source.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct SourceSpan {
    start: usize,
    end: usize,
}

impl<'de> Deserialize<'de> for SourceSpan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireSpan {
            start: usize,
            end: usize,
        }

        let wire = WireSpan::deserialize(deserializer)?;
        Self::new(wire.start, wire.end).map_err(D::Error::custom)
    }
}

impl SourceSpan {
    /// Creates a validated half-open span.
    ///
    /// # Errors
    ///
    /// Returns [`SpanError`] when `start` is greater than `end`.
    pub fn new(start: usize, end: usize) -> Result<Self, SpanError> {
        if start > end {
            return Err(SpanError { start, end });
        }
        Ok(Self { start, end })
    }

    /// Returns the inclusive start byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the exclusive end byte offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Returns the span length in bytes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    /// Returns whether the span is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Error returned for an invalid half-open source span.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("source span start {start} is greater than end {end}")]
pub struct SpanError {
    start: usize,
    end: usize,
}

/// Media types accepted by the current prototype.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    /// UTF-8 plain text with an optional UTF-8 byte order mark.
    PlainText,
}

impl fmt::Display for MediaType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlainText => formatter.write_str("text/plain"),
        }
    }
}

/// Adapter-owned structural identity represented without format-specific state.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct StructuralFingerprint {
    /// Versioned fingerprint algorithm identifier.
    pub kind: String,
    /// Digest of the structural representation.
    pub digest: Digest,
}

/// One text region offered to the rewrite engine.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct RewriteUnit {
    /// Stable identifier inside this parsed document.
    pub id: RewriteUnitId,
    /// Source byte range owned by this unit.
    pub source_span: SourceSpan,
    /// Decoded text for candidate generation and validation.
    pub text: String,
}

/// Format-neutral document representation consumed by the engine.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct DocumentIr {
    /// Contract schema version.
    pub schema_version: u32,
    /// Identifier for this ingest instance.
    pub document_id: DocumentId,
    /// Detected source media type.
    pub media_type: MediaType,
    /// Digest of the complete original byte sequence.
    pub source_digest: Digest,
    /// Independently addressable rewrite units.
    pub rewrite_units: Vec<RewriteUnit>,
    /// Adapter-owned structural identity.
    pub structure: StructuralFingerprint,
}

impl DocumentIr {
    /// Constructs and validates a document using the current contract schema.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentError`] when unit identifiers, ordering, or spans do not
    /// agree with the supplied source text.
    pub fn new(
        source_digest: Digest,
        media_type: MediaType,
        rewrite_units: Vec<RewriteUnit>,
        structure: StructuralFingerprint,
    ) -> Result<Self, DocumentError> {
        let document_id = DocumentId::from_digest(&source_digest);
        let document = Self {
            schema_version: SCHEMA_VERSION,
            document_id,
            media_type,
            source_digest,
            rewrite_units,
            structure,
        };
        document.validate()?;
        Ok(document)
    }

    /// Verifies schema, identifier, ordering, and span invariants.
    ///
    /// # Errors
    ///
    /// Returns [`DocumentError`] for the first invalid unit or schema property.
    pub fn validate(&self) -> Result<(), DocumentError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(DocumentError::UnsupportedSchema(self.schema_version));
        }
        if self.document_id != DocumentId::from_digest(&self.source_digest) {
            return Err(DocumentError::DocumentIdentifier);
        }

        let mut previous_end = 0;
        for (index, unit) in self.rewrite_units.iter().enumerate() {
            if unit.id != RewriteUnitId::new(&self.document_id, index) {
                return Err(DocumentError::UnitIdentifier { index });
            }
            if unit.source_span.start() < previous_end {
                return Err(DocumentError::OverlappingSpan { index });
            }
            if unit.source_span.len() != unit.text.len() {
                return Err(DocumentError::SpanLength { index });
            }
            previous_end = unit.source_span.end();
        }
        Ok(())
    }
}

/// Invalid format-neutral document contract.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DocumentError {
    /// Contract schema is not supported by this engine version.
    #[error("unsupported document schema version {0}")]
    UnsupportedSchema(u32),
    /// Document ID does not match the source digest.
    #[error("document identifier does not match source digest")]
    DocumentIdentifier,
    /// Rewrite-unit ID does not match its document position.
    #[error("rewrite unit {index} has an invalid identifier")]
    UnitIdentifier {
        /// Zero-based unit position.
        index: usize,
    },
    /// Rewrite-unit source spans overlap or are out of order.
    #[error("rewrite unit {index} has an overlapping source span")]
    OverlappingSpan {
        /// Zero-based unit position.
        index: usize,
    },
    /// Rewrite-unit span length does not equal its UTF-8 text length.
    #[error("rewrite unit {index} source span length does not match text")]
    SpanLength {
        /// Zero-based unit position.
        index: usize,
    },
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use super::{
        DocumentError, DocumentId, DocumentIr, IdentifierError, MediaType, RewriteUnit,
        RewriteUnitId, SourceSpan, StructuralFingerprint,
    };
    use crate::Digest;

    #[test]
    fn rejects_reversed_span() {
        assert!(SourceSpan::new(2, 1).is_err());
    }

    #[test]
    fn span_reports_length_and_empty_state() {
        let span = SourceSpan::new(3, 7).expect("valid fixture span");
        assert_eq!(span.start(), 3);
        assert_eq!(span.end(), 7);
        assert_eq!(span.len(), 4);
        assert!(!span.is_empty());
        assert!(
            SourceSpan::new(4, 4)
                .expect("empty span is valid")
                .is_empty()
        );
    }

    #[test]
    fn identifiers_are_namespaced_by_document() {
        let document = DocumentId::from_digest(&Digest::sha256(b"source"));
        let unit = RewriteUnitId::new(&document, 2);
        assert!(unit.as_str().starts_with(document.as_str()));
        assert!(unit.as_str().ends_with(":unit:2"));
    }

    #[test]
    fn serialized_identifiers_and_spans_enforce_invariants() {
        assert_eq!(
            DocumentId::from_str("document:nope"),
            Err(IdentifierError::Document)
        );
        assert_eq!(
            RewriteUnitId::from_str("document:nope:unit:0"),
            Err(IdentifierError::RewriteUnit)
        );
        assert!(serde_json::from_str::<DocumentId>("\"invalid\"").is_err());
        assert!(serde_json::from_str::<SourceSpan>(r#"{"start":2,"end":1}"#).is_err());

        let document = DocumentId::from_digest(&Digest::sha256(b"fixture"));
        let unit = RewriteUnitId::new(&document, 3);
        let encoded = serde_json::to_string(&unit).expect("unit ID serializes");
        assert_eq!(
            serde_json::from_str::<RewriteUnitId>(&encoded)
                .expect("canonical unit ID deserializes"),
            unit
        );
    }

    #[test]
    fn document_validation_rejects_mismatched_unit_contracts() {
        let digest = Digest::sha256(b"text");
        let document = DocumentId::from_digest(&digest);
        let error = DocumentIr::new(
            digest,
            MediaType::PlainText,
            vec![RewriteUnit {
                id: RewriteUnitId::new(&document, 0),
                source_span: SourceSpan::new(0, 3).expect("valid span shape"),
                text: "text".to_owned(),
            }],
            StructuralFingerprint {
                kind: "fixture".to_owned(),
                digest: Digest::sha256(b"structure"),
            },
        )
        .expect_err("span length must match text bytes");
        assert_eq!(error, DocumentError::SpanLength { index: 0 });
    }
}
