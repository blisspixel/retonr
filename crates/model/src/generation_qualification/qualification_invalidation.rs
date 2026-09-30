//! Inert record that one compact qualification identity was invalidated.
//!
//! The record grants no qualification, activation, or live-use authority.
//! It does not change the qualification record it names.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::codec::{append_digest, append_u32};
use super::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationId,
    GenerationQualificationRecordV1,
};

/// Identity domain for one generation-qualification invalidation.
pub const GENERATION_QUALIFICATION_INVALIDATION_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-invalidation:v1\0";
/// Maximum JSON bytes accepted for one invalidation record.
pub const MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES: usize = 16_384;
/// Maximum canonical identity bytes for one invalidation record.
pub const MAX_GENERATION_QUALIFICATION_INVALIDATION_CANONICAL_BYTES: usize = 4_096;

/// Content-derived identity of one generation-qualification invalidation.
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GenerationQualificationInvalidationId(Digest);

impl GenerationQualificationInvalidationId {
    /// Returns the digest that defines this inert portable identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }

    fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Digest::sha256(bytes))
    }
}

/// Already validated qualification record named by one invalidation.
#[derive(Clone, Copy)]
pub struct GenerationQualificationInvalidationV1Relations<'a> {
    /// Compact qualification record whose identity is invalidated.
    pub qualification_record: &'a GenerationQualificationRecordV1,
}

/// Inert statement that one qualification identity is no longer selectable.
///
/// The record contains the qualification identity only. It grants no activation,
/// launch, generation, qualification-verification, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationInvalidationV1 {
    schema_version: u32,
    generation_qualification_id: GenerationQualificationId,
    #[serde(skip)]
    id: GenerationQualificationInvalidationId,
}

impl GenerationQualificationInvalidationV1 {
    /// Derives one invalidation from an already validated qualification record.
    ///
    /// This constructor produces inert data only. It does not re-run qualification
    /// and it does not change the named record.
    ///
    /// # Errors
    ///
    /// Returns a content-free error when the canonical identity exceeds its ceiling.
    pub fn new(
        relations: &GenerationQualificationInvalidationV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationInvalidationV1Error> {
        Self::build(*relations, None)
    }

    /// Decodes bounded canonical JSON against one qualification record.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, or substituted input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: &GenerationQualificationInvalidationV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationInvalidationV1Error> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_INVALIDATION_JSON_BYTES {
            return Err(GenerationQualificationInvalidationV1Error::EncodedRecordTooLarge);
        }
        let wire: InvalidationWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationInvalidationV1Error::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationInvalidationV1Error::UnsupportedSchema);
        }
        let value = Self::build(*relations, Some(&wire))?;
        let canonical = serde_json::to_vec(&value)
            .map_err(|_| GenerationQualificationInvalidationV1Error::InvalidEncoding)?;
        if canonical != bytes {
            return Err(GenerationQualificationInvalidationV1Error::NonCanonicalEncoding);
        }
        Ok(value)
    }

    /// Revalidates this record against the same qualification identity.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless the qualification identity reconstructs
    /// this exact record.
    pub fn validate_against(
        &self,
        relations: &GenerationQualificationInvalidationV1Relations<'_>,
    ) -> Result<(), GenerationQualificationInvalidationV1Error> {
        if &Self::new(relations)? == self {
            Ok(())
        } else {
            Err(GenerationQualificationInvalidationV1Error::RelationshipMismatch)
        }
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the invalidated qualification identity.
    #[must_use]
    pub const fn generation_qualification_id(&self) -> &GenerationQualificationId {
        &self.generation_qualification_id
    }

    /// Returns the content-derived invalidation identity.
    #[must_use]
    pub const fn generation_qualification_invalidation_id(
        &self,
    ) -> &GenerationQualificationInvalidationId {
        &self.id
    }

    fn build(
        relations: GenerationQualificationInvalidationV1Relations<'_>,
        wire: Option<&InvalidationWire>,
    ) -> Result<Self, GenerationQualificationInvalidationV1Error> {
        let mut value = Self {
            schema_version: GENERATION_QUALIFICATION_SCHEMA_VERSION,
            generation_qualification_id: relations
                .qualification_record
                .generation_qualification_id()
                .clone(),
            id: GenerationQualificationInvalidationId::from_canonical_bytes(
                b"uninitialized invalidation",
            ),
        };
        if wire.is_some_and(|expected| !expected.matches(&value)) {
            return Err(GenerationQualificationInvalidationV1Error::RelationshipMismatch);
        }
        let canonical = value.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_INVALIDATION_CANONICAL_BYTES {
            return Err(GenerationQualificationInvalidationV1Error::CanonicalEncodingTooLarge);
        }
        value.id = GenerationQualificationInvalidationId::from_canonical_bytes(&canonical);
        Ok(value)
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_INVALIDATION_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.generation_qualification_id.digest());
        output
    }
}

impl fmt::Debug for GenerationQualificationInvalidationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationInvalidationV1")
            .field("generation_qualification_invalidation_id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Content-free failure while deriving one qualification invalidation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationInvalidationV1Error {
    /// Encoded JSON exceeds its fixed predecode ceiling.
    #[error("generation qualification invalidation exceeds its limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, duplicated, trailing, or contains an unknown field.
    #[error("generation qualification invalidation encoding is invalid")]
    InvalidEncoding,
    /// JSON differs from the one canonical encoding.
    #[error("generation qualification invalidation encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The invalidation schema is unsupported.
    #[error("generation qualification invalidation schema is unsupported")]
    UnsupportedSchema,
    /// Canonical identity bytes exceed their fixed ceiling.
    #[error("generation qualification invalidation identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// A serialized qualification identity was substituted.
    #[error("generation qualification invalidation relationship does not match")]
    RelationshipMismatch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvalidationWire {
    schema_version: u32,
    generation_qualification_id: GenerationQualificationId,
}

impl InvalidationWire {
    fn matches(&self, value: &GenerationQualificationInvalidationV1) -> bool {
        self.schema_version == value.schema_version
            && self.generation_qualification_id == value.generation_qualification_id
    }
}
