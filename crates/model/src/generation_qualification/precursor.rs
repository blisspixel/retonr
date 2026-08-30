use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{append_digest, append_u32, append_u64, validate_canonical_json};
use super::{
    CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_ID_DOMAIN, CandidateGenerationAttemptPrecursorId,
    FrozenExternalComponentSetId, GENERATION_QUALIFICATION_SCHEMA_VERSION,
    GenerationQualificationContractError, GenerationQualificationPlanId,
    GenerationQualificationPlanV1, ManagedGenerationPathId, PlannedCandidateAttemptId,
    PlannedCandidateAttemptV1, RuntimeAdmissionJoinId, StructuredCompletionRequestBindingId,
};
use crate::{
    ArtifactId, ArtifactSetId, EffectivePackageEvidenceV2Id, EffectiveRuntimeStateId,
    ModelPackageManifestId, RuntimeBuildId, RuntimePackageManifestId,
};

/// Maximum JSON bytes accepted for one candidate-attempt precursor.
pub const MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES: usize = 16_384;
const MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_CANONICAL_BYTES: usize = 4_096;

/// Live-installation facts fixed immediately before one managed launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateGenerationAttemptPrecursorV1Input {
    /// Exact nonzero runtime installation generation.
    pub runtime_installation_generation: u64,
    /// Exact nonzero model installation generation.
    pub model_installation_generation: u64,
    /// Exact provider wire-request binding.
    pub structured_request_binding_id: StructuredCompletionRequestBindingId,
}

/// Inert portable precursor joining one plan entry to installation-specific facts.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationAttemptPrecursorV1 {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    planned_attempt_id: PlannedCandidateAttemptId,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_build_id: RuntimeBuildId,
    expected_effective_runtime_state_id: EffectiveRuntimeStateId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    static_model_binding_digest: Digest,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    #[serde(skip)]
    id: CandidateGenerationAttemptPrecursorId,
}

impl CandidateGenerationAttemptPrecursorV1 {
    /// Creates one exact acyclic precursor from a plan, selected attempt, and system.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless the attempt occurs
    /// exactly once at its declared ordinal, names the supplied system, and both
    /// installation generations are nonzero.
    pub fn new(
        plan: &GenerationQualificationPlanV1,
        attempt: &PlannedCandidateAttemptV1,
        system: &super::GenerationSystemRecordV1,
        input: CandidateGenerationAttemptPrecursorV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            plan,
            attempt,
            system,
            input,
            None,
        )
    }

    fn build(
        schema_version: u32,
        plan: &GenerationQualificationPlanV1,
        attempt: &PlannedCandidateAttemptV1,
        system: &super::GenerationSystemRecordV1,
        input: CandidateGenerationAttemptPrecursorV1Input,
        wire: Option<&PrecursorWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        validate_selection(plan, attempt, system)?;
        if input.runtime_installation_generation == 0 || input.model_installation_generation == 0 {
            return Err(GenerationQualificationContractError::InvalidInstallationGeneration);
        }
        if wire.is_some_and(|wire| !wire.matches(plan, attempt, system, &input)) {
            return Err(GenerationQualificationContractError::PlannedAttemptMismatch);
        }
        let mut record = Self {
            schema_version,
            qualification_plan_id: plan.qualification_plan_id().clone(),
            planned_attempt_id: attempt.planned_attempt_id().clone(),
            runtime_installation_generation: input.runtime_installation_generation,
            model_installation_generation: input.model_installation_generation,
            runtime_admission_join_id: system.runtime_admission_join_id().clone(),
            managed_generation_path_id: system.managed_generation_path_id().clone(),
            frozen_external_component_set_id: system.frozen_external_component_set_id().clone(),
            runtime_package_manifest_id: system.runtime_package_manifest_id().clone(),
            runtime_build_id: system.runtime_build_id().clone(),
            expected_effective_runtime_state_id: system.effective_runtime_state_id().clone(),
            model_artifact_set_id: system.model_artifact_set_id().clone(),
            model_package_manifest_id: system.model_package_manifest_id().clone(),
            model_artifact_id: system.model_artifact_id().clone(),
            effective_package_evidence_v2_id: system.effective_package_evidence_v2_id().clone(),
            static_model_binding_digest: system.static_model_binding_digest().clone(),
            structured_request_binding_id: input.structured_request_binding_id,
            id: CandidateGenerationAttemptPrecursorId(Digest::sha256(b"uninitialized precursor")),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        record.id = CandidateGenerationAttemptPrecursorId(Digest::sha256(&canonical));
        Ok(record)
    }

    /// Parses canonical bounded JSON and reloads the exact plan selection and system.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unsupported, noncanonical, stale, or substituted input.
    pub fn from_json_bytes(
        bytes: &[u8],
        plan: &GenerationQualificationPlanV1,
        attempt: &PlannedCandidateAttemptV1,
        system: &super::GenerationSystemRecordV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: PrecursorWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        let record = Self::build(
            wire.schema_version,
            plan,
            attempt,
            system,
            wire.input(),
            Some(&wire),
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this precursor against its exact plan selection and system.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if any selected record,
    /// installation generation, request binding, or derived field differs.
    pub fn validate_against(
        &self,
        plan: &GenerationQualificationPlanV1,
        attempt: &PlannedCandidateAttemptV1,
        system: &super::GenerationSystemRecordV1,
    ) -> Result<(), GenerationQualificationContractError> {
        let expected = Self::new(
            plan,
            attempt,
            system,
            CandidateGenerationAttemptPrecursorV1Input {
                runtime_installation_generation: self.runtime_installation_generation,
                model_installation_generation: self.model_installation_generation,
                structured_request_binding_id: self.structured_request_binding_id.clone(),
            },
        )?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationContractError::PlannedAttemptMismatch)
        }
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact qualification plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }
    /// Returns the exact selected planned-attempt identity.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }
    /// Returns the runtime installation generation.
    #[must_use]
    pub const fn runtime_installation_generation(&self) -> u64 {
        self.runtime_installation_generation
    }
    /// Returns the model installation generation.
    #[must_use]
    pub const fn model_installation_generation(&self) -> u64 {
        self.model_installation_generation
    }
    /// Returns the runtime-admission join identity.
    #[must_use]
    pub const fn runtime_admission_join_id(&self) -> &RuntimeAdmissionJoinId {
        &self.runtime_admission_join_id
    }
    /// Returns the managed generation-path identity.
    #[must_use]
    pub const fn managed_generation_path_id(&self) -> &ManagedGenerationPathId {
        &self.managed_generation_path_id
    }
    /// Returns the frozen external-component set identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalComponentSetId {
        &self.frozen_external_component_set_id
    }
    /// Returns the runtime-package manifest identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }
    /// Returns the runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }
    /// Returns the expected effective-runtime-state identity.
    #[must_use]
    pub const fn expected_effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.expected_effective_runtime_state_id
    }
    /// Returns the model artifact-set identity.
    #[must_use]
    pub const fn model_artifact_set_id(&self) -> &ArtifactSetId {
        &self.model_artifact_set_id
    }
    /// Returns the model-package manifest identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }
    /// Returns the selected model artifact identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }
    /// Returns the effective-package V2 identity.
    #[must_use]
    pub const fn effective_package_evidence_v2_id(&self) -> &EffectivePackageEvidenceV2Id {
        &self.effective_package_evidence_v2_id
    }
    /// Returns the static model-binding digest.
    #[must_use]
    pub const fn static_model_binding_digest(&self) -> &Digest {
        &self.static_model_binding_digest
    }
    /// Returns the exact structured wire-request binding.
    #[must_use]
    pub const fn structured_request_binding_id(&self) -> &StructuredCompletionRequestBindingId {
        &self.structured_request_binding_id
    }
    /// Returns the content-derived precursor identity.
    #[must_use]
    pub const fn precursor_id(&self) -> &CandidateGenerationAttemptPrecursorId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = CANDIDATE_GENERATION_ATTEMPT_PRECURSOR_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.qualification_plan_id.digest());
        append_digest(&mut output, self.planned_attempt_id.digest());
        append_u64(&mut output, self.runtime_installation_generation);
        append_u64(&mut output, self.model_installation_generation);
        for digest in [
            self.runtime_admission_join_id.digest(),
            self.managed_generation_path_id.digest(),
            self.frozen_external_component_set_id.digest(),
            self.runtime_package_manifest_id.digest(),
            self.runtime_build_id.digest(),
            self.expected_effective_runtime_state_id.digest(),
            self.model_artifact_set_id.digest(),
            self.model_package_manifest_id.digest(),
            self.model_artifact_id.digest(),
            self.effective_package_evidence_v2_id.digest(),
            &self.static_model_binding_digest,
            self.structured_request_binding_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        output
    }
}

impl fmt::Debug for CandidateGenerationAttemptPrecursorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationAttemptPrecursorV1")
            .field("schema_version", &self.schema_version)
            .field("precursor_id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrecursorWire {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    planned_attempt_id: PlannedCandidateAttemptId,
    runtime_installation_generation: u64,
    model_installation_generation: u64,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    runtime_build_id: RuntimeBuildId,
    expected_effective_runtime_state_id: EffectiveRuntimeStateId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    effective_package_evidence_v2_id: EffectivePackageEvidenceV2Id,
    static_model_binding_digest: Digest,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
}

impl PrecursorWire {
    fn input(&self) -> CandidateGenerationAttemptPrecursorV1Input {
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: self.runtime_installation_generation,
            model_installation_generation: self.model_installation_generation,
            structured_request_binding_id: self.structured_request_binding_id.clone(),
        }
    }

    fn matches(
        &self,
        plan: &GenerationQualificationPlanV1,
        attempt: &PlannedCandidateAttemptV1,
        system: &super::GenerationSystemRecordV1,
        input: &CandidateGenerationAttemptPrecursorV1Input,
    ) -> bool {
        self.qualification_plan_id == *plan.qualification_plan_id()
            && self.planned_attempt_id == *attempt.planned_attempt_id()
            && self.runtime_installation_generation == input.runtime_installation_generation
            && self.model_installation_generation == input.model_installation_generation
            && self.runtime_admission_join_id == *system.runtime_admission_join_id()
            && self.managed_generation_path_id == *system.managed_generation_path_id()
            && self.frozen_external_component_set_id == *system.frozen_external_component_set_id()
            && self.runtime_package_manifest_id == *system.runtime_package_manifest_id()
            && self.runtime_build_id == *system.runtime_build_id()
            && self.expected_effective_runtime_state_id == *system.effective_runtime_state_id()
            && self.model_artifact_set_id == *system.model_artifact_set_id()
            && self.model_package_manifest_id == *system.model_package_manifest_id()
            && self.model_artifact_id == *system.model_artifact_id()
            && self.effective_package_evidence_v2_id == *system.effective_package_evidence_v2_id()
            && self.static_model_binding_digest == *system.static_model_binding_digest()
            && self.structured_request_binding_id == input.structured_request_binding_id
    }
}

fn validate_selection(
    plan: &GenerationQualificationPlanV1,
    attempt: &PlannedCandidateAttemptV1,
    system: &super::GenerationSystemRecordV1,
) -> Result<(), GenerationQualificationContractError> {
    let ordinal = usize::try_from(attempt.attempt_ordinal())
        .map_err(|_| GenerationQualificationContractError::AttemptOrdinalMismatch)?;
    if plan.suite_manifest_id() != attempt.suite_manifest_id()
        || plan.planned_attempt_ids().get(ordinal) != Some(attempt.planned_attempt_id())
        || plan
            .planned_attempt_ids()
            .iter()
            .filter(|id| *id == attempt.planned_attempt_id())
            .count()
            != 1
    {
        return Err(GenerationQualificationContractError::PlannedAttemptMismatch);
    }
    if attempt.generation_system_id() != system.generation_system_id()
        || !plan
            .generation_system_ids()
            .contains(system.generation_system_id())
    {
        return Err(GenerationQualificationContractError::GenerationSystemMismatch);
    }
    Ok(())
}
