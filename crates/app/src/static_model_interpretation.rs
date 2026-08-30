//! Stable static-model interpretation derived from verified managed launch facts.

use std::fmt;

use rewrite_model::{ArtifactId, ArtifactSetId, ModelPackageManifestId};
use rewrite_types::Digest;
use serde::Serialize;
use thiserror::Error;

use crate::{
    MANAGED_OLLAMA_INPUT_SCHEMA_VERSION, ModelPackageFoundationId, VerifiedManagedOllamaLaunchPlan,
};

/// Current stable static-model interpretation schema.
pub const STATIC_MODEL_INTERPRETATION_SCHEMA_VERSION: u32 = 1;
/// Hard ceiling for one canonical static-model interpretation record.
pub const MAX_STATIC_MODEL_INTERPRETATION_JSON_BYTES: usize = 4 * 1_024;

const ID_DOMAIN: &[u8] = b"retonr:static-model-interpretation:v1\0";

/// Content identity of one stable static-model interpretation.
///
/// ```compile_fail
/// use rewrite_app::StaticModelInterpretationId;
/// use rewrite_types::Digest;
///
/// let _forged = StaticModelInterpretationId(Digest::sha256(b"caller digest"));
/// ```
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct StaticModelInterpretationId(Digest);

impl StaticModelInterpretationId {
    /// Returns the digest defining this interpretation identity.
    #[must_use]
    pub fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Portable inert interpretation of one verified managed model input.
///
/// Installation generation, object identity, input-layout identity, process
/// state, and all other live attempt facts are deliberately excluded. Public
/// construction requires a verified launch plan, which already owns the exact
/// specialized foundation, retained input plan, and approved launch authority.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StaticModelInterpretationV1 {
    schema_version: u32,
    foundation_id: ModelPackageFoundationId,
    managed_input_schema_version: u32,
    artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    reference_digest: Digest,
    runtime_reference_digest: Digest,
    mapping_digest: Digest,
    model_artifact_id: ArtifactId,
    model_target_digest: Digest,
    member_count: u32,
    total_bytes: u64,
    model_bytes: u64,
    #[serde(skip)]
    id: StaticModelInterpretationId,
}

impl StaticModelInterpretationV1 {
    /// Derives one stable interpretation from an exact verified launch plan.
    ///
    /// # Errors
    ///
    /// Returns [`StaticModelInterpretationError`] if the verified facts are
    /// internally inconsistent or exceed the fixed canonical encoding bound.
    pub fn derive(
        launch: &VerifiedManagedOllamaLaunchPlan<'_>,
    ) -> Result<Self, StaticModelInterpretationError> {
        Self::derive_from_facts(facts_from_launch(launch))
    }

    fn derive_from_facts(
        facts: InterpretationFacts,
    ) -> Result<Self, StaticModelInterpretationError> {
        if facts.managed_input_schema_version != MANAGED_OLLAMA_INPUT_SCHEMA_VERSION
            || facts.target_artifact_id != facts.model_artifact_id
            || facts.target_digest != facts.model_target_digest
            || facts.member_count == 0
            || facts.total_bytes == 0
            || facts.model_bytes == 0
            || facts.model_bytes > facts.total_bytes
        {
            return Err(StaticModelInterpretationError::InvalidBinding);
        }
        let mut value = Self {
            schema_version: STATIC_MODEL_INTERPRETATION_SCHEMA_VERSION,
            foundation_id: facts.foundation_id,
            managed_input_schema_version: facts.managed_input_schema_version,
            artifact_set_id: facts.artifact_set_id,
            model_package_manifest_id: facts.model_package_manifest_id,
            reference_digest: facts.reference_digest,
            runtime_reference_digest: facts.runtime_reference_digest,
            mapping_digest: facts.mapping_digest,
            model_artifact_id: facts.model_artifact_id,
            model_target_digest: facts.model_target_digest,
            member_count: facts.member_count,
            total_bytes: facts.total_bytes,
            model_bytes: facts.model_bytes,
            id: StaticModelInterpretationId(Digest::sha256(b"pending interpretation")),
        };
        let canonical = value.canonical_json_bytes()?;
        let mut material = Vec::with_capacity(ID_DOMAIN.len() + canonical.len());
        material.extend_from_slice(ID_DOMAIN);
        material.extend_from_slice(&canonical);
        value.id = StaticModelInterpretationId(Digest::sha256(&material));
        Ok(value)
    }

    /// Parses canonical bounded JSON and rechecks it against a verified launch.
    ///
    /// # Errors
    ///
    /// Returns [`StaticModelInterpretationError`] for empty, oversized,
    /// malformed, noncanonical, unsupported, or substituted input.
    pub fn from_json_bytes(
        bytes: &[u8],
        launch: &VerifiedManagedOllamaLaunchPlan<'_>,
    ) -> Result<Self, StaticModelInterpretationError> {
        if bytes.is_empty() {
            return Err(StaticModelInterpretationError::InvalidEncoding);
        }
        if bytes.len() > MAX_STATIC_MODEL_INTERPRETATION_JSON_BYTES {
            return Err(StaticModelInterpretationError::LimitExceeded);
        }
        let wire: InterpretationWire = serde_json::from_slice(bytes)
            .map_err(|_error| StaticModelInterpretationError::InvalidEncoding)?;
        if wire.schema_version != STATIC_MODEL_INTERPRETATION_SCHEMA_VERSION {
            return Err(StaticModelInterpretationError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        let expected = Self::derive(launch)?;
        if !wire.matches(&expected) {
            return Err(StaticModelInterpretationError::InvalidBinding);
        }
        if expected.canonical_json_bytes()? != bytes {
            return Err(StaticModelInterpretationError::NonCanonicalEncoding);
        }
        Ok(expected)
    }

    /// Revalidates this record against an exact verified launch plan.
    ///
    /// # Errors
    ///
    /// Returns [`StaticModelInterpretationError`] if any stable launch fact differs.
    pub fn validate_against(
        &self,
        launch: &VerifiedManagedOllamaLaunchPlan<'_>,
    ) -> Result<(), StaticModelInterpretationError> {
        if &Self::derive(launch)? == self {
            Ok(())
        } else {
            Err(StaticModelInterpretationError::InvalidBinding)
        }
    }

    /// Returns canonical portable JSON without the derived identity field.
    ///
    /// # Errors
    ///
    /// Returns [`StaticModelInterpretationError`] if serialization fails or the
    /// fixed encoding bound would be exceeded.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, StaticModelInterpretationError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_error| StaticModelInterpretationError::InvalidEncoding)?;
        if bytes.len() > MAX_STATIC_MODEL_INTERPRETATION_JSON_BYTES {
            Err(StaticModelInterpretationError::LimitExceeded)
        } else {
            Ok(bytes)
        }
    }

    /// Returns the stable content identity.
    #[must_use]
    pub fn interpretation_id(&self) -> &StaticModelInterpretationId {
        &self.id
    }

    /// Returns the digest used by `GenerationSystemRecordV1`.
    #[must_use]
    pub fn binding_digest(&self) -> &Digest {
        self.id.digest()
    }

    /// Returns the exact stable specialized model foundation.
    #[must_use]
    pub fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }

    /// Returns the complete immutable artifact-set identity.
    #[must_use]
    pub fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the semantic model-package identity.
    #[must_use]
    pub fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the selected model-weight artifact identity.
    #[must_use]
    pub fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }
}

impl fmt::Debug for StaticModelInterpretationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticModelInterpretationV1")
            .field("interpretation_id", &self.id)
            .field("foundation_id", &self.foundation_id)
            .field("model_artifact_id", &self.model_artifact_id)
            .finish_non_exhaustive()
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InterpretationWire {
    schema_version: u32,
    foundation_id: Digest,
    managed_input_schema_version: u32,
    artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    reference_digest: Digest,
    runtime_reference_digest: Digest,
    mapping_digest: Digest,
    model_artifact_id: ArtifactId,
    model_target_digest: Digest,
    member_count: u32,
    total_bytes: u64,
    model_bytes: u64,
}

impl InterpretationWire {
    fn matches(&self, expected: &StaticModelInterpretationV1) -> bool {
        self.schema_version == expected.schema_version
            && self.foundation_id == *expected.foundation_id.digest()
            && self.managed_input_schema_version == expected.managed_input_schema_version
            && self.artifact_set_id == expected.artifact_set_id
            && self.model_package_manifest_id == expected.model_package_manifest_id
            && self.reference_digest == expected.reference_digest
            && self.runtime_reference_digest == expected.runtime_reference_digest
            && self.mapping_digest == expected.mapping_digest
            && self.model_artifact_id == expected.model_artifact_id
            && self.model_target_digest == expected.model_target_digest
            && self.member_count == expected.member_count
            && self.total_bytes == expected.total_bytes
            && self.model_bytes == expected.model_bytes
    }
}

#[derive(Clone)]
struct InterpretationFacts {
    foundation_id: ModelPackageFoundationId,
    managed_input_schema_version: u32,
    artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    reference_digest: Digest,
    runtime_reference_digest: Digest,
    mapping_digest: Digest,
    model_artifact_id: ArtifactId,
    model_target_digest: Digest,
    target_artifact_id: ArtifactId,
    target_digest: Digest,
    member_count: u32,
    total_bytes: u64,
    model_bytes: u64,
}

fn facts_from_launch(launch: &VerifiedManagedOllamaLaunchPlan<'_>) -> InterpretationFacts {
    let evidence = launch.input_evidence();
    InterpretationFacts {
        foundation_id: launch.foundation_id().clone(),
        managed_input_schema_version: evidence.schema_version(),
        artifact_set_id: evidence.artifact_set_id().clone(),
        model_package_manifest_id: evidence.model_package_manifest_id().clone(),
        reference_digest: evidence.reference_digest().clone(),
        runtime_reference_digest: evidence.runtime_reference_digest().clone(),
        mapping_digest: evidence.mapping_digest().clone(),
        model_artifact_id: evidence.model_artifact_id().clone(),
        model_target_digest: evidence.model_target_digest().clone(),
        target_artifact_id: launch.model_target().artifact_id().clone(),
        target_digest: launch.model_target().target_digest().clone(),
        member_count: evidence.member_count(),
        total_bytes: evidence.total_bytes(),
        model_bytes: launch.model_byte_size(),
    }
}

/// Static-model interpretation construction or decoding failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum StaticModelInterpretationError {
    /// Input was empty, malformed, noncanonical, or contained an unknown field.
    #[error("static-model interpretation encoding is invalid")]
    InvalidEncoding,
    /// Structurally valid JSON did not use the one canonical byte encoding.
    #[error("static-model interpretation encoding is not canonical")]
    NonCanonicalEncoding,
    /// Input or canonical output exceeded its fixed byte ceiling.
    #[error("static-model interpretation limit was exceeded")]
    LimitExceeded,
    /// The encoded record uses an unsupported schema version.
    #[error("unsupported static-model interpretation schema {0}")]
    UnsupportedSchema(u32),
    /// Stable foundation, input, target, count, or size facts did not agree.
    #[error("static-model interpretation binding is invalid")]
    InvalidBinding,
}

#[cfg(test)]
#[path = "static_model_interpretation/tests.rs"]
mod tests;
