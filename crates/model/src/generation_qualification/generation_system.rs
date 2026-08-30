use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

use rewrite_types::Digest;

use super::codec::{append_digest, append_u32, validate_canonical_json};
use super::{
    FrozenExternalComponentSetId, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GENERATION_SYSTEM_ID_DOMAIN, GenerationQualificationContractError, GenerationSystemId,
    ManagedGenerationPathId, RuntimeAdmissionJoinId,
};
use crate::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, EffectivePackageEvidenceV2,
    EffectivePackageEvidenceV2Id, EffectiveRuntimeState, EffectiveRuntimeStateId,
    ModelPackageManifest, ModelPackageManifestId, RuntimeBuildId, RuntimeBuildIdentity,
    RuntimePackageManifest, RuntimePackageManifestId,
};

mod validation;
mod wire;

use validation::validate_relation_inputs;
use wire::{ExpectedRelationshipIds, GenerationSystemWire};

/// Maximum JSON bytes accepted for one generation-system record.
pub const MAX_GENERATION_SYSTEM_JSON_BYTES: usize = 16_384;
const MAX_GENERATION_SYSTEM_CANONICAL_BYTES: usize = 4_096;

/// Exact typed records against which one generation system is constructed or decoded.
#[derive(Clone, Copy)]
pub struct GenerationSystemRecordV1Relations<'a> {
    /// Runtime-package manifest selected for the generation system.
    pub runtime_package_manifest: &'a RuntimePackageManifest,
    /// Runtime build derived from the selected runtime package.
    pub runtime_build: &'a RuntimeBuildIdentity,
    /// Stable effective runtime state admitted for qualification.
    pub effective_runtime_state: &'a EffectiveRuntimeState,
    /// Complete immutable model artifact set.
    pub model_artifact_set: &'a ArtifactSetManifest,
    /// Semantic model package for the exact artifact set.
    pub model_package_manifest: &'a ModelPackageManifest,
    /// Typed effective-package evidence for the model, build, and state tuple.
    pub effective_package_evidence_v2: &'a EffectivePackageEvidenceV2,
}

/// Caller-supplied inert policy and owner-bound facts for one generation system.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationSystemRecordV1Input {
    /// App-derived runtime-admission join identity.
    pub runtime_admission_join_id: RuntimeAdmissionJoinId,
    /// App-derived managed generation-path identity.
    pub managed_generation_path_id: ManagedGenerationPathId,
    /// Attestor-derived frozen external-component set identity.
    pub frozen_external_component_set_id: FrozenExternalComponentSetId,
    /// Exact model artifact selected from the package generation closure.
    pub model_artifact_id: ArtifactId,
    /// Digest binding the exact static model interpretation.
    pub static_model_binding_digest: Digest,
    /// Digest of the candidate-generation strategy.
    pub strategy_digest: Digest,
    /// Digest of the planning implementation and policy.
    pub planner_digest: Digest,
    /// Digest of the output validator.
    pub validator_digest: Digest,
    /// Digest of the provider adapter.
    pub adapter_digest: Digest,
    /// Digest of the exact prompt construction contract.
    pub prompt_digest: Digest,
    /// Digest of the exact structured output schema.
    pub output_schema_digest: Digest,
    /// Digest of the provider-neutral request policy.
    pub request_policy_digest: Digest,
    /// Digest of the selected language contract.
    pub language_digest: Digest,
    /// Digest of the selected generation mode.
    pub mode_digest: Digest,
    /// Digest of the selected output format.
    pub format_digest: Digest,
    /// Digest of the selected operating-system class.
    pub operating_system_digest: Digest,
    /// Digest of the selected architecture class.
    pub architecture_digest: Digest,
    /// Digest of the selected execution class.
    pub execution_class_digest: Digest,
    /// Digest of the admitted hardware envelope.
    pub hardware_envelope_digest: Digest,
}

/// Inert portable identity of one stable generation system.
///
/// This record binds typed static relationships and policy equality facts. It does
/// not prove process execution, model loading or use, semantic quality,
/// qualification, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationSystemRecordV1 {
    schema_version: u32,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    static_model_binding_digest: Digest,
    strategy_digest: Digest,
    planner_digest: Digest,
    validator_digest: Digest,
    adapter_digest: Digest,
    prompt_digest: Digest,
    output_schema_digest: Digest,
    request_policy_digest: Digest,
    language_digest: Digest,
    mode_digest: Digest,
    format_digest: Digest,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
    #[serde(skip)]
    id: GenerationSystemId,
}

impl GenerationSystemRecordV1 {
    /// Creates a stable record after validating every supplied typed relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if the runtime package,
    /// runtime build, state, model package, selected artifact, or effective-package
    /// evidence does not form one exact relationship closure.
    pub fn new(
        relations: GenerationSystemRecordV1Relations<'_>,
        input: GenerationSystemRecordV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::from_wire(GENERATION_QUALIFICATION_SCHEMA_VERSION, relations, input)
    }

    fn from_wire(
        schema_version: u32,
        relations: GenerationSystemRecordV1Relations<'_>,
        input: GenerationSystemRecordV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        validate_relation_inputs(relations, &input.model_artifact_id)?;
        let mut record = Self {
            schema_version,
            runtime_admission_join_id: input.runtime_admission_join_id,
            managed_generation_path_id: input.managed_generation_path_id,
            frozen_external_component_set_id: input.frozen_external_component_set_id,
            runtime_package_manifest_id: relations
                .runtime_package_manifest
                .runtime_package_manifest_id(),
            runtime_build_id: relations.runtime_build.runtime_build_id(),
            effective_runtime_state_id: relations
                .effective_runtime_state
                .effective_runtime_state_id(),
            model_artifact_set_id: relations.model_artifact_set.artifact_set_id(),
            model_package_manifest_id: relations.model_package_manifest.model_package_manifest_id(),
            model_artifact_id: input.model_artifact_id,
            effective_package_evidence_v2_id: relations
                .effective_package_evidence_v2
                .effective_package_evidence_v2_id(),
            static_model_binding_digest: input.static_model_binding_digest,
            strategy_digest: input.strategy_digest,
            planner_digest: input.planner_digest,
            validator_digest: input.validator_digest,
            adapter_digest: input.adapter_digest,
            prompt_digest: input.prompt_digest,
            output_schema_digest: input.output_schema_digest,
            request_policy_digest: input.request_policy_digest,
            language_digest: input.language_digest,
            mode_digest: input.mode_digest,
            format_digest: input.format_digest,
            operating_system_digest: input.operating_system_digest,
            architecture_digest: input.architecture_digest,
            execution_class_digest: input.execution_class_digest,
            hardware_envelope_digest: input.hardware_envelope_digest,
            id: GenerationSystemId(Digest::sha256(b"uninitialized generation system")),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_GENERATION_SYSTEM_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        record.id = GenerationSystemId(Digest::sha256(&canonical));
        Ok(record)
    }

    /// Parses canonical bounded JSON and rechecks every typed relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// noncanonical, unsupported, or relationship-inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationSystemRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_GENERATION_SYSTEM_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: GenerationSystemWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version() != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version(),
            ));
        }
        let expected = ExpectedRelationshipIds::from(relations);
        if !wire.relationship_ids_equal(&expected) {
            return Err(wire.relationship_error(&expected));
        }
        let schema_version = wire.schema_version();
        let record = Self::from_wire(schema_version, relations, wire.into_input())?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this decoded record against exact typed runtime and model records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if any record identity or
    /// relationship differs from the values used to derive this record.
    pub fn validate_against(
        &self,
        relations: GenerationSystemRecordV1Relations<'_>,
    ) -> Result<(), GenerationQualificationContractError> {
        validate_relation_inputs(relations, &self.model_artifact_id)?;
        let expected = ExpectedRelationshipIds::from(relations);
        if self.runtime_package_manifest_id != expected.runtime_package {
            return Err(GenerationQualificationContractError::RuntimePackageMismatch);
        }
        if self.runtime_build_id != expected.runtime_build
            || self.effective_runtime_state_id != expected.effective_runtime_state
        {
            return Err(GenerationQualificationContractError::RuntimeBuildMismatch);
        }
        if self.model_artifact_set_id != expected.model_artifact_set
            || self.model_package_manifest_id != expected.model_package_manifest
        {
            return Err(GenerationQualificationContractError::ModelPackageMismatch);
        }
        if self.effective_package_evidence_v2_id != expected.effective_package_evidence_v2 {
            return Err(GenerationQualificationContractError::EffectivePackageMismatch);
        }
        let expected_id = GenerationSystemId(Digest::sha256(&self.canonical_bytes()));
        if self.id != expected_id {
            return Err(GenerationQualificationContractError::InvalidEncoding);
        }
        Ok(())
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the app-derived runtime admission join identity.
    #[must_use]
    pub const fn runtime_admission_join_id(&self) -> &RuntimeAdmissionJoinId {
        &self.runtime_admission_join_id
    }
    /// Returns the app-derived managed generation-path identity.
    #[must_use]
    pub const fn managed_generation_path_id(&self) -> &ManagedGenerationPathId {
        &self.managed_generation_path_id
    }
    /// Returns the attestor-derived frozen external-component set identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalComponentSetId {
        &self.frozen_external_component_set_id
    }
    /// Returns the exact runtime-package manifest identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }
    /// Returns the exact runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }
    /// Returns the stable effective-runtime-state identity.
    #[must_use]
    pub const fn effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.effective_runtime_state_id
    }
    /// Returns the complete model artifact-set identity.
    #[must_use]
    pub const fn model_artifact_set_id(&self) -> &ArtifactSetId {
        &self.model_artifact_set_id
    }
    /// Returns the exact semantic model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }
    /// Returns the selected model artifact identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }
    /// Returns the exact effective-package V2 evidence identity.
    #[must_use]
    pub const fn effective_package_evidence_v2_id(&self) -> &EffectivePackageEvidenceV2Id {
        &self.effective_package_evidence_v2_id
    }
    /// Returns the static model binding digest.
    #[must_use]
    pub const fn static_model_binding_digest(&self) -> &Digest {
        &self.static_model_binding_digest
    }
    /// Returns the strategy digest.
    #[must_use]
    pub const fn strategy_digest(&self) -> &Digest {
        &self.strategy_digest
    }
    /// Returns the planner digest.
    #[must_use]
    pub const fn planner_digest(&self) -> &Digest {
        &self.planner_digest
    }
    /// Returns the validator digest.
    #[must_use]
    pub const fn validator_digest(&self) -> &Digest {
        &self.validator_digest
    }
    /// Returns the provider-adapter digest.
    #[must_use]
    pub const fn adapter_digest(&self) -> &Digest {
        &self.adapter_digest
    }
    /// Returns the prompt-construction digest.
    #[must_use]
    pub const fn prompt_digest(&self) -> &Digest {
        &self.prompt_digest
    }
    /// Returns the output-schema digest.
    #[must_use]
    pub const fn output_schema_digest(&self) -> &Digest {
        &self.output_schema_digest
    }
    /// Returns the request-policy digest.
    #[must_use]
    pub const fn request_policy_digest(&self) -> &Digest {
        &self.request_policy_digest
    }
    /// Returns the language digest.
    #[must_use]
    pub const fn language_digest(&self) -> &Digest {
        &self.language_digest
    }
    /// Returns the generation-mode digest.
    #[must_use]
    pub const fn mode_digest(&self) -> &Digest {
        &self.mode_digest
    }
    /// Returns the output-format digest.
    #[must_use]
    pub const fn format_digest(&self) -> &Digest {
        &self.format_digest
    }
    /// Returns the operating-system digest.
    #[must_use]
    pub const fn operating_system_digest(&self) -> &Digest {
        &self.operating_system_digest
    }
    /// Returns the architecture digest.
    #[must_use]
    pub const fn architecture_digest(&self) -> &Digest {
        &self.architecture_digest
    }
    /// Returns the execution-class digest.
    #[must_use]
    pub const fn execution_class_digest(&self) -> &Digest {
        &self.execution_class_digest
    }
    /// Returns the hardware-envelope digest.
    #[must_use]
    pub const fn hardware_envelope_digest(&self) -> &Digest {
        &self.hardware_envelope_digest
    }
    /// Returns the content-derived generation-system identity.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_SYSTEM_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in self.identity_digests() {
            append_digest(&mut output, digest);
        }
        output
    }

    fn identity_digests(&self) -> [&Digest; 25] {
        [
            self.runtime_admission_join_id.digest(),
            self.managed_generation_path_id.digest(),
            self.frozen_external_component_set_id.digest(),
            self.runtime_package_manifest_id.digest(),
            self.runtime_build_id.digest(),
            self.effective_runtime_state_id.digest(),
            self.model_artifact_set_id.digest(),
            self.model_package_manifest_id.digest(),
            self.model_artifact_id.digest(),
            self.effective_package_evidence_v2_id.digest(),
            &self.static_model_binding_digest,
            &self.strategy_digest,
            &self.planner_digest,
            &self.validator_digest,
            &self.adapter_digest,
            &self.prompt_digest,
            &self.output_schema_digest,
            &self.request_policy_digest,
            &self.language_digest,
            &self.mode_digest,
            &self.format_digest,
            &self.operating_system_digest,
            &self.architecture_digest,
            &self.execution_class_digest,
            &self.hardware_envelope_digest,
        ]
    }
}

impl fmt::Debug for GenerationSystemRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationSystemRecordV1")
            .field("schema_version", &self.schema_version)
            .field("generation_system_id", &self.id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "generation_system/tests/support.rs"]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
