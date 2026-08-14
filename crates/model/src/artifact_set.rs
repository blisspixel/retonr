use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use crate::ArtifactId;

/// Current canonical artifact-set manifest contract version.
pub const ARTIFACT_SET_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Maximum number of regular-file members in one artifact set.
pub const MAX_ARTIFACT_SET_MEMBERS: usize = 4_096;
/// Maximum UTF-8 byte length of one logical relative path.
pub const MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES: usize = 512;
/// Maximum byte length of one logical path component.
pub const MAX_ARTIFACT_SET_PATH_COMPONENT_BYTES: usize = 255;
/// Maximum aggregate member-path bytes in one artifact set.
pub const MAX_ARTIFACT_SET_TOTAL_PATH_BYTES: usize = 262_144;
/// Maximum encoded JSON bytes admitted by the artifact-set decoder.
pub const MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES: usize = 1_048_576;

/// Content-derived identifier for one canonical artifact-set manifest.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactSetId(Digest);

impl ArtifactSetId {
    /// Creates an artifact-set identifier from a canonical manifest digest.
    #[must_use]
    pub fn from_digest(digest: Digest) -> Self {
        Self(digest)
    }

    /// Returns the digest that defines this artifact-set identifier.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Portable logical path for one artifact-set member.
///
/// This value is a manifest namespace, not an operating-system path. Version 1
/// admits only portable ASCII components separated by `/`.
#[derive(Clone, Debug, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArtifactSetRelativePath(String);

impl ArtifactSetRelativePath {
    /// Parses and validates a portable logical member path.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetPathError`] for an absolute, ambiguous, reserved,
    /// nonportable, or overlong path.
    pub fn new(value: impl Into<String>) -> Result<Self, ArtifactSetPathError> {
        let value = value.into();
        validate_path(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical logical path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ArtifactSetRelativePath {
    type Error = ArtifactSetPathError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// One immutable regular-file member of an artifact set.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSetMember {
    artifact_id: ArtifactId,
    byte_size: u64,
    relative_path: ArtifactSetRelativePath,
}

impl ArtifactSetMember {
    /// Creates one member from its complete byte identity and logical path.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        byte_size: u64,
        relative_path: ArtifactSetRelativePath,
    ) -> Self {
        Self {
            artifact_id,
            byte_size,
            relative_path,
        }
    }

    /// Returns the member byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the member byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the member's portable logical path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }
}

/// Canonical content manifest for a complete set of immutable regular files.
///
/// Structural validity does not prove that a producer listed every file that can
/// affect runtime output. Qualification must establish that completeness claim.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSetManifest {
    members: Vec<ArtifactSetMember>,
    schema_version: u32,
}

impl ArtifactSetManifest {
    /// Creates and validates a version 1 artifact-set manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetManifestError`] unless members are bounded, strictly
    /// path-sorted, collision-free, and contain at least one byte in total.
    pub fn new(members: Vec<ArtifactSetMember>) -> Result<Self, ArtifactSetManifestError> {
        Self::from_wire(ARTIFACT_SET_MANIFEST_SCHEMA_VERSION, members)
    }

    /// Parses a byte-bounded JSON manifest and revalidates all canonical invariants.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetManifestError`] before decoding when the input exceeds
    /// the fixed byte ceiling, or when JSON or domain validation fails.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, ArtifactSetManifestError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct MemberWire {
            artifact_id: ArtifactId,
            byte_size: u64,
            relative_path: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            members: Vec<MemberWire>,
            schema_version: u32,
        }

        if bytes.len() > MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES {
            return Err(ArtifactSetManifestError::EncodedManifestTooLarge);
        }

        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|_| ArtifactSetManifestError::InvalidEncoding)?;
        if wire.members.len() > MAX_ARTIFACT_SET_MEMBERS {
            return Err(ArtifactSetManifestError::TooManyMembers);
        }
        let members = wire
            .members
            .into_iter()
            .map(|member| {
                Ok(ArtifactSetMember::new(
                    member.artifact_id,
                    member.byte_size,
                    ArtifactSetRelativePath::new(member.relative_path)
                        .map_err(|_| ArtifactSetManifestError::InvalidMemberPath)?,
                ))
            })
            .collect::<Result<Vec<_>, ArtifactSetManifestError>>()?;
        Self::from_wire(wire.schema_version, members)
    }

    fn from_wire(
        schema_version: u32,
        members: Vec<ArtifactSetMember>,
    ) -> Result<Self, ArtifactSetManifestError> {
        let manifest = Self {
            members,
            schema_version,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates the complete canonical manifest contract.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactSetManifestError`] for unsupported, unbounded,
    /// noncanonical, colliding, empty, or overflowing state.
    pub fn validate(&self) -> Result<(), ArtifactSetManifestError> {
        if self.schema_version != ARTIFACT_SET_MANIFEST_SCHEMA_VERSION {
            return Err(ArtifactSetManifestError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        if self.members.is_empty() {
            return Err(ArtifactSetManifestError::EmptySet);
        }
        if self.members.len() > MAX_ARTIFACT_SET_MEMBERS {
            return Err(ArtifactSetManifestError::TooManyMembers);
        }

        let mut path_bytes = 0usize;
        let mut total_bytes = 0u64;
        let mut prior_path: Option<&str> = None;
        let mut root = PathNode::default();
        for member in &self.members {
            let path = member.relative_path.as_str();
            if prior_path.is_some_and(|prior| prior.as_bytes() >= path.as_bytes()) {
                return Err(ArtifactSetManifestError::NoncanonicalOrder);
            }
            prior_path = Some(path);
            path_bytes = path_bytes
                .checked_add(path.len())
                .ok_or(ArtifactSetManifestError::PathBudgetExceeded)?;
            if path_bytes > MAX_ARTIFACT_SET_TOTAL_PATH_BYTES {
                return Err(ArtifactSetManifestError::PathBudgetExceeded);
            }
            total_bytes = total_bytes
                .checked_add(member.byte_size)
                .ok_or(ArtifactSetManifestError::TotalSizeOverflow)?;
            root.insert(path)?;
        }
        if total_bytes == 0 {
            return Err(ArtifactSetManifestError::EmptyContent);
        }
        Ok(())
    }

    /// Returns the manifest contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns members in canonical path order.
    #[must_use]
    pub fn members(&self) -> &[ArtifactSetMember] {
        &self.members
    }

    /// Returns the checked sum of member byte lengths.
    #[must_use]
    pub fn total_byte_size(&self) -> u64 {
        self.members.iter().map(|member| member.byte_size).sum()
    }

    /// Returns the exact canonical UTF-8 JSON manifest material.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        let mut output = String::from("{\"members\":[");
        for (index, member) in self.members.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str("{\"artifact_id\":\"");
            output.push_str(member.artifact_id.digest().as_str());
            output.push_str("\",\"byte_size\":");
            output.push_str(&member.byte_size.to_string());
            output.push_str(",\"relative_path\":\"");
            output.push_str(member.relative_path.as_str());
            output.push_str("\"}");
        }
        output.push_str("],\"schema_version\":");
        output.push_str(&self.schema_version.to_string());
        output.push('}');
        output
    }

    /// Computes the content-derived identity of the complete canonical manifest.
    #[must_use]
    pub fn artifact_set_id(&self) -> ArtifactSetId {
        let canonical = self.canonical_json();
        let mut material = Vec::with_capacity(33 + canonical.len());
        material.extend_from_slice(b"retonr:artifact-set-manifest:v1\0");
        material.extend_from_slice(canonical.as_bytes());
        ArtifactSetId::from_digest(Digest::sha256(&material))
    }
}

#[derive(Default)]
struct PathNode {
    children: BTreeMap<String, PathNode>,
    spelling: String,
    terminal: bool,
}

impl PathNode {
    fn insert(&mut self, path: &str) -> Result<(), ArtifactSetManifestError> {
        let mut node = self;
        let mut components = path.split('/').peekable();
        while let Some(component) = components.next() {
            let is_last = components.peek().is_none();
            let key = component.to_ascii_lowercase();
            let child = node.children.entry(key).or_insert_with(|| Self {
                children: BTreeMap::new(),
                spelling: component.to_owned(),
                terminal: false,
            });
            if child.spelling != component
                || child.terminal
                || (is_last && !child.children.is_empty())
            {
                return Err(ArtifactSetManifestError::PathCollision);
            }
            if is_last {
                child.terminal = true;
            }
            node = child;
        }
        Ok(())
    }
}

fn validate_path(value: &str) -> Result<(), ArtifactSetPathError> {
    if value.is_empty()
        || value.len() > MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES
        || !value.is_ascii()
        || value.starts_with('/')
        || value.ends_with('/')
    {
        return Err(ArtifactSetPathError::InvalidPath);
    }
    for component in value.split('/') {
        if component.is_empty()
            || component.len() > MAX_ARTIFACT_SET_PATH_COMPONENT_BYTES
            || matches!(component, "." | "..")
            || component.starts_with(' ')
            || component.ends_with([' ', '.'])
            || component.bytes().any(|byte| {
                !(0x20..=0x7e).contains(&byte)
                    || matches!(byte, b'\\' | b'<' | b'>' | b':' | b'"' | b'|' | b'?' | b'*')
            })
        {
            return Err(ArtifactSetPathError::InvalidPath);
        }
        let basename = component.split('.').next().unwrap_or(component);
        if reserved_windows_basename(basename) {
            return Err(ArtifactSetPathError::ReservedComponent);
        }
    }
    Ok(())
}

fn reserved_windows_basename(value: &str) -> bool {
    value.eq_ignore_ascii_case("con")
        || value.eq_ignore_ascii_case("prn")
        || value.eq_ignore_ascii_case("aux")
        || value.eq_ignore_ascii_case("nul")
        || value.eq_ignore_ascii_case("conin$")
        || value.eq_ignore_ascii_case("conout$")
        || value.eq_ignore_ascii_case("clock$")
        || (value.len() == 4
            && (value[..3].eq_ignore_ascii_case("com") || value[..3].eq_ignore_ascii_case("lpt"))
            && matches!(value.as_bytes()[3], b'1'..=b'9'))
}

/// Portable artifact-set path validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ArtifactSetPathError {
    /// The path is absolute, empty, ambiguous, non-ASCII, or not portable.
    #[error("artifact-set path is not a portable relative path")]
    InvalidPath,
    /// A component is a reserved cross-platform device name.
    #[error("artifact-set path contains a reserved component")]
    ReservedComponent,
}

/// Canonical artifact-set manifest validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ArtifactSetManifestError {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded artifact-set manifest exceeds its limit")]
    EncodedManifestTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("artifact-set manifest encoding is invalid")]
    InvalidEncoding,
    /// A decoded member path violates the portable path contract.
    #[error("artifact-set member path is invalid")]
    InvalidMemberPath,
    /// The manifest schema is unsupported.
    #[error("unsupported artifact-set manifest schema {0}")]
    UnsupportedSchema(u32),
    /// A manifest must contain at least one member.
    #[error("artifact-set manifest has no members")]
    EmptySet,
    /// The member count exceeds the fixed contract bound.
    #[error("artifact-set member limit exceeded")]
    TooManyMembers,
    /// Members are not strictly sorted by canonical path bytes.
    #[error("artifact-set members are not in canonical order")]
    NoncanonicalOrder,
    /// Paths collide by spelling, case, or file and directory prefix.
    #[error("artifact-set member paths collide")]
    PathCollision,
    /// Aggregate logical path bytes exceed the fixed contract bound.
    #[error("artifact-set path budget exceeded")]
    PathBudgetExceeded,
    /// All manifest members are empty.
    #[error("artifact-set content must contain at least one byte")]
    EmptyContent,
    /// Member byte lengths overflow the aggregate size.
    #[error("artifact-set total size overflowed")]
    TotalSizeOverflow,
}

#[cfg(test)]
#[path = "artifact_set/tests.rs"]
mod tests;
