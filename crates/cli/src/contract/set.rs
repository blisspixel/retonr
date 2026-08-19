use std::{io::Read, path::Path};

use rewrite_app::ArtifactSetInstallationKey;
use rewrite_model::{ArtifactSetManifest, ArtifactSetManifestError};

use super::{ArtifactIdArgument, InstallationGeneration, ManifestInputError, open_regular_file};

/// Persistence-neutral selector for one exact installed artifact-set generation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ArtifactSetSelectionDto {
    /// Canonical content-derived artifact-set identity.
    pub artifact_set_id: ArtifactIdArgument,
    /// Exact positive installation generation represented as a JSON string.
    pub installation_generation: InstallationGeneration,
}

impl From<&ArtifactSetInstallationKey> for ArtifactSetSelectionDto {
    fn from(value: &ArtifactSetInstallationKey) -> Self {
        Self {
            artifact_set_id: ArtifactIdArgument::from_digest(value.artifact_set_id().digest()),
            installation_generation: InstallationGeneration::new(value.installation_generation())
                .expect("application installation keys always contain a positive generation"),
        }
    }
}

/// Opens and parses one strict artifact-set manifest under a byte ceiling.
///
/// Errors disclose no input path, manifest content, or parser source chain.
///
/// # Errors
///
/// Returns [`ManifestInputError`] when the byte ceiling is zero, the file cannot be
/// read, the encoded input exceeds the ceiling, JSON is malformed, or the decoded
/// manifest violates its domain contract.
pub fn read_set_manifest_bounded(
    path: &Path,
    maximum_bytes: usize,
) -> Result<ArtifactSetManifest, ManifestInputError> {
    let file = open_regular_file(path).map_err(|error| ManifestInputError::Io(error.kind()))?;
    parse_set_manifest_bounded(file, maximum_bytes)
}

/// Parses one strict artifact-set manifest from a bounded byte stream.
///
/// Errors disclose no manifest content or parser source chain.
///
/// # Errors
///
/// Returns [`ManifestInputError`] when the byte ceiling is zero, the stream cannot
/// be read, the encoded input exceeds the ceiling, JSON is malformed, or the decoded
/// manifest violates its domain contract.
pub fn parse_set_manifest_bounded(
    reader: impl Read,
    maximum_bytes: usize,
) -> Result<ArtifactSetManifest, ManifestInputError> {
    let bytes = read_bounded_manifest_bytes(reader, maximum_bytes)?;
    ArtifactSetManifest::from_json_bytes(&bytes).map_err(map_set_manifest_error)
}

fn read_bounded_manifest_bytes(
    reader: impl Read,
    maximum_bytes: usize,
) -> Result<Vec<u8>, ManifestInputError> {
    if maximum_bytes == 0 {
        return Err(ManifestInputError::InvalidLimit);
    }
    let read_limit = u64::try_from(maximum_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(maximum_bytes.min(64 * 1024));
    reader
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| ManifestInputError::Io(error.kind()))?;
    if bytes.len() > maximum_bytes {
        return Err(ManifestInputError::TooLarge);
    }
    Ok(bytes)
}

const fn map_set_manifest_error(error: ArtifactSetManifestError) -> ManifestInputError {
    match error {
        ArtifactSetManifestError::EncodedManifestTooLarge => ManifestInputError::TooLarge,
        ArtifactSetManifestError::InvalidEncoding => ManifestInputError::InvalidJson,
        ArtifactSetManifestError::UnsupportedSchema(_) => ManifestInputError::UnsupportedSchema,
        ArtifactSetManifestError::InvalidMemberPath
        | ArtifactSetManifestError::EmptySet
        | ArtifactSetManifestError::TooManyMembers
        | ArtifactSetManifestError::NoncanonicalOrder
        | ArtifactSetManifestError::PathCollision
        | ArtifactSetManifestError::PathBudgetExceeded
        | ArtifactSetManifestError::EmptyContent
        | ArtifactSetManifestError::TotalSizeOverflow => ManifestInputError::InvalidManifest,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use rewrite_model::{ArtifactId, ArtifactSetMember, ArtifactSetRelativePath};
    use rewrite_types::Digest;
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::MAX_MANIFEST_BYTES;

    fn fixture_manifest() -> ArtifactSetManifest {
        ArtifactSetManifest::new(vec![ArtifactSetMember::new(
            ArtifactId::from_digest(Digest::sha256(b"weights")),
            7,
            ArtifactSetRelativePath::new("model/weights.bin").expect("portable path"),
        )])
        .expect("valid set manifest")
    }

    #[test]
    fn set_manifest_parser_is_bounded_strict_and_domain_validated() {
        let encoded = fixture_manifest().canonical_json().into_bytes();
        let parsed = parse_set_manifest_bounded(Cursor::new(&encoded), encoded.len())
            .expect("valid exact-boundary set manifest");
        assert_eq!(parsed.members().len(), 1);
        assert_eq!(
            parse_set_manifest_bounded(Cursor::new(&encoded), encoded.len() - 1),
            Err(ManifestInputError::TooLarge)
        );
        assert_eq!(
            parse_set_manifest_bounded(Cursor::new(&encoded), 0),
            Err(ManifestInputError::InvalidLimit)
        );

        let mut unknown: Value = serde_json::from_slice(&encoded).expect("fixture JSON");
        unknown["unknown"] = json!(true);
        assert_eq!(
            parse_set_manifest_bounded(
                Cursor::new(serde_json::to_vec(&unknown).expect("serialize unknown field")),
                MAX_MANIFEST_BYTES,
            ),
            Err(ManifestInputError::InvalidJson)
        );

        let mut invalid: Value = serde_json::from_slice(&encoded).expect("fixture JSON");
        invalid["schema_version"] = json!(0);
        assert_eq!(
            parse_set_manifest_bounded(
                Cursor::new(serde_json::to_vec(&invalid).expect("serialize invalid set manifest")),
                MAX_MANIFEST_BYTES,
            ),
            Err(ManifestInputError::UnsupportedSchema)
        );
    }

    #[test]
    fn every_set_manifest_error_has_a_stable_mapping() {
        for error in [
            ArtifactSetManifestError::EncodedManifestTooLarge,
            ArtifactSetManifestError::InvalidEncoding,
            ArtifactSetManifestError::InvalidMemberPath,
            ArtifactSetManifestError::UnsupportedSchema(2),
            ArtifactSetManifestError::EmptySet,
            ArtifactSetManifestError::TooManyMembers,
            ArtifactSetManifestError::NoncanonicalOrder,
            ArtifactSetManifestError::PathCollision,
            ArtifactSetManifestError::PathBudgetExceeded,
            ArtifactSetManifestError::EmptyContent,
            ArtifactSetManifestError::TotalSizeOverflow,
        ] {
            let mapped = map_set_manifest_error(error);
            assert_ne!(mapped, ManifestInputError::InvalidLimit);
        }
        assert_eq!(
            map_set_manifest_error(ArtifactSetManifestError::EncodedManifestTooLarge),
            ManifestInputError::TooLarge
        );
        assert_eq!(
            map_set_manifest_error(ArtifactSetManifestError::InvalidEncoding),
            ManifestInputError::InvalidJson
        );
        assert_eq!(
            map_set_manifest_error(ArtifactSetManifestError::UnsupportedSchema(9)),
            ManifestInputError::UnsupportedSchema
        );
    }
}
