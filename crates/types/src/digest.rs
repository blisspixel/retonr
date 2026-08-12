use core::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// Lowercase hexadecimal SHA-256 digest.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Digest(String);

impl Digest {
    /// Computes a digest from raw bytes.
    #[must_use]
    pub fn sha256(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        Self(format!("{digest:x}"))
    }

    /// Parses a canonical lowercase hexadecimal SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns [`DigestError`] unless the input contains exactly 64 lowercase
    /// hexadecimal ASCII characters.
    pub fn from_sha256_hex(value: impl Into<String>) -> Result<Self, DigestError> {
        let value = value.into();
        if value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(Self(value))
        } else {
            Err(DigestError)
        }
    }

    /// Returns the lowercase hexadecimal representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_sha256_hex(value).map_err(D::Error::custom)
    }
}

/// Error returned for a noncanonical SHA-256 digest.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid lowercase SHA-256 digest")]
pub struct DigestError;

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Digest, DigestError};

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            Digest::sha256(b"abc").as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn rejects_noncanonical_digest_input_and_deserialization() {
        assert_eq!(Digest::from_sha256_hex("abc"), Err(DigestError));
        assert_eq!(Digest::from_sha256_hex("A".repeat(64)), Err(DigestError));
        assert!(serde_json::from_str::<Digest>("\"abc\"").is_err());
        let digest = Digest::sha256(b"round trip");
        let encoded = serde_json::to_string(&digest).expect("digest serializes");
        assert_eq!(
            serde_json::from_str::<Digest>(&encoded).expect("canonical digest deserializes"),
            digest
        );
    }
}
