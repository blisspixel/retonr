use rewrite_app::effective_runtime_state_observation::ManagedOllamaEffectiveRuntimeState;
use rewrite_app::{
    ManagedOllamaInputEvidence, RuntimePackageLease, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath,
};
use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{
    ArtifactId, ComputeBackend, EffectiveRuntimeStateId, ExecutionPlacement,
    ManagedOllamaGenerationBracketObservationV1Id, ModelPackageManifestId, NativeLoadObservation,
    RuntimeBuildId,
};
use rewrite_ollama::{OllamaModelBinding, OllamaResidentSessionExecutionReceipt};
use rewrite_runtime_attestor::{AttachedProcessEvidence, RetainedTcpConnectionEvidence};
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::Digest;
use serde::Serialize;

use crate::{
    LocalOllamaModelBindingEvidence,
    local_ollama_model_binding::validate_local_ollama_model_binding_evidence,
};

use super::super::super::{
    LocalOllamaEffectiveStateMissingRelationship, LocalOllamaManagedBuildBinding,
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightReport,
};
use super::super::validation::FinalManagedGenerationWorkerEvidence;

/// Current managed Ollama generation-bracket observation contract version.
pub const MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION: u32 = 1;

const GENERATION_RESPONSE_COUNT: usize = 9;
const FIRST_RESIDENCY_RESPONSE_OFFSET: usize = 4;
/// Redacted observation for one completion bracketed by the retained managed runtime.
///
/// This record proves that one process, isolated namespace, package lease, native
/// load observer, and direct HTTP/1 connection remained stable through the exact
/// response sequence. The Ollama API additionally reported two equal residency
/// snapshots and an effective context length. It also binds the app-observed
/// effective runtime state constructed from this exact bracket. It does not prove
/// handler execution, model weight use, resident-page identity, page immutability,
/// formal placement, semantic correctness, or qualification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each evidence strength and limitation remains independently explicit"
)]
pub struct ManagedOllamaGenerationBracketObservationV1 {
    schema_version: u32,
    binding_digest: Digest,
    managed_preflight_binding_digest: Digest,
    managed_build_binding_digest: Digest,
    runtime_build_id: RuntimeBuildId,
    admitted_runtime_id: Digest,
    generation_path_id: Digest,
    frozen_external_component_set_id: Digest,
    runtime_package_installation_generation: u64,
    model_package_installation_generation: u64,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    effective_runtime_state_join_digest: Digest,
    static_model_binding_digest: Digest,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    request_binding_digest: Digest,
    response_binding_digest: Digest,
    residency_contract_digest: Digest,
    residency_observation_digest: Digest,
    post_generation_process_evidence_digest: Digest,
    post_generation_native_load_observation_digest: Digest,
    managed_input_mapping_digest: Digest,
    managed_input_layout_digest: Digest,
    input_bound_launch_spec_digest: Digest,
    generation_worker_evidence_digest: Digest,
    generation_worker_native_load_observation_digest: Digest,
    generation_worker_model_mapping_observation_digest: Digest,
    final_isolation_evidence_digest: Digest,
    connection_observation_digest: Digest,
    connection_observation_count: u64,
    first_generation_response_ordinal: u64,
    last_generation_response_ordinal: u64,
    effective_context_tokens: u32,
    runtime_reported_accelerator_bytes: u64,
    missing_effective_state_relationships: Vec<LocalOllamaEffectiveStateMissingRelationship>,
    static_model_package_relationship_verified: bool,
    process_retained_through_generation: bool,
    runtime_package_lease_retained_through_generation: bool,
    model_package_lease_retained_through_generation: bool,
    package_leases_revalidated_immediately_after_generation: bool,
    package_leases_revalidated_after_final_observation: bool,
    private_model_input_reobserved_after_generation: bool,
    exact_model_weight_mapping_observed: bool,
    all_responses_used_retained_transport: bool,
    kernel_attribution_checked_around_every_response: bool,
    runtime_reported_residency_proven: bool,
    effective_context_capacity_observed: bool,
    process_retained_after_return: bool,
    model_loaded_proven: bool,
    model_used_proven: bool,
    application_handler_proven: bool,
    effective_runtime_state_proven: bool,
    qualified: bool,
}

impl ManagedOllamaGenerationBracketObservationV1 {
    /// Returns the contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the digest binding every positive and negative claim.
    #[must_use]
    pub const fn binding_digest(&self) -> &Digest {
        &self.binding_digest
    }

    /// Returns the typed identity of this unchanged bracket V1 binding.
    ///
    /// The returned value is inert and grants no execution or qualification
    /// authority.
    #[must_use]
    pub fn bracket_observation_v1_id(&self) -> ManagedOllamaGenerationBracketObservationV1Id {
        ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(
            self.binding_digest.clone(),
        )
    }

    /// Returns the exact package-declared runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the exact opaque runtime-admission identity used for the launch.
    #[must_use]
    pub const fn admitted_runtime_id(&self) -> &Digest {
        &self.admitted_runtime_id
    }

    /// Returns the exact reviewed generation-path identity used for the worker.
    #[must_use]
    pub const fn generation_path_id(&self) -> &Digest {
        &self.generation_path_id
    }

    /// Returns the exact frozen external-component discovery identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &Digest {
        &self.frozen_external_component_set_id
    }

    /// Returns the retained runtime package installation generation.
    #[must_use]
    pub const fn runtime_package_installation_generation(&self) -> u64 {
        self.runtime_package_installation_generation
    }

    /// Returns the retained model package installation generation.
    #[must_use]
    pub const fn model_package_installation_generation(&self) -> u64 {
        self.model_package_installation_generation
    }

    /// Returns the exact app-observed effective runtime-state identity.
    #[must_use]
    pub const fn effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.effective_runtime_state_id
    }

    /// Returns the exact app-owned live relationship-join digest.
    #[must_use]
    pub const fn effective_runtime_state_join_digest(&self) -> &Digest {
        &self.effective_runtime_state_join_digest
    }

    /// Returns the exact statically bound model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the immutable model artifact selected for generation.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the content-free structured-response binding.
    #[must_use]
    pub const fn response_binding_digest(&self) -> &Digest {
        &self.response_binding_digest
    }

    /// Returns the runtime-reported effective context length.
    #[must_use]
    pub const fn effective_context_tokens(&self) -> u32 {
        self.effective_context_tokens
    }

    /// Returns every relationship still absent from an effective runtime state.
    #[must_use]
    pub fn missing_effective_state_relationships(
        &self,
    ) -> &[LocalOllamaEffectiveStateMissingRelationship] {
        &self.missing_effective_state_relationships
    }

    /// Returns whether the managed process spanned the complete generation bracket.
    #[must_use]
    pub const fn process_retained_through_generation(&self) -> bool {
        self.process_retained_through_generation
    }

    /// Returns whether the exact runtime-package lease spanned generation.
    ///
    /// This does not claim that a model-package lease was acquired or retained.
    #[must_use]
    pub const fn runtime_package_lease_retained_through_generation(&self) -> bool {
        self.runtime_package_lease_retained_through_generation
    }

    /// Returns whether the exact concrete model-package lease spanned traffic and
    /// passed canonical-tree revalidation after generation.
    #[must_use]
    pub const fn model_package_lease_retained_through_generation(&self) -> bool {
        self.model_package_lease_retained_through_generation
    }

    /// Returns whether both package leases passed the distinct immediate
    /// post-generation revalidation before worker and final-state observation.
    #[must_use]
    pub const fn package_leases_revalidated_immediately_after_generation(&self) -> bool {
        self.package_leases_revalidated_immediately_after_generation
    }

    /// Returns whether both package leases passed again after final observations.
    #[must_use]
    pub const fn package_leases_revalidated_after_final_observation(&self) -> bool {
        self.package_leases_revalidated_after_final_observation
    }

    /// Returns whether the exact private GGUF mapping was bounded and reobserved.
    ///
    /// This does not claim model use, page immutability, or handler execution.
    #[must_use]
    pub const fn exact_model_weight_mapping_observed(&self) -> bool {
        self.exact_model_weight_mapping_observed
    }

    /// Returns whether the exact retained GGUF was observed as a stable mapping in
    /// the separately retained generation worker.
    ///
    /// This bounded load claim does not prove model use, resident-page identity,
    /// page immutability, or application-handler execution.
    #[must_use]
    pub const fn model_loaded_proven(&self) -> bool {
        self.model_loaded_proven
    }

    /// Returns whether both post-generation API residency observations agreed.
    #[must_use]
    pub const fn runtime_reported_residency_proven(&self) -> bool {
        self.runtime_reported_residency_proven
    }

    /// Returns whether direct runtime-reported context-capacity evidence was retained.
    #[must_use]
    pub const fn effective_context_capacity_observed(&self) -> bool {
        self.effective_context_capacity_observed
    }

    /// Always false because cleanup completes before this result is returned.
    #[must_use]
    pub const fn process_retained_after_return(&self) -> bool {
        self.process_retained_after_return
    }

    /// Always false because residency APIs do not prove weight use.
    #[must_use]
    pub const fn model_used_proven(&self) -> bool {
        self.model_used_proven
    }

    /// Always false because connection attribution does not identify a handler.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        self.application_handler_proven
    }

    /// Returns whether all effective-state relationships were observed and joined.
    ///
    /// This remains inert and does not grant generation or qualification authority.
    #[must_use]
    pub const fn effective_runtime_state_proven(&self) -> bool {
        self.effective_runtime_state_proven
    }

    /// Always false because this evidence has no qualification authority.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        self.qualified
    }
}

pub(crate) struct GenerationBracketObservationInput<'a> {
    pub(crate) managed_report: &'a LocalOllamaManagedPreflightReport,
    pub(crate) build_binding: &'a LocalOllamaManagedBuildBinding,
    pub(crate) admitted_runtime: &'a VerifiedAdmittedRuntime,
    pub(crate) generation_path: &'a VerifiedManagedGenerationPath,
    pub(crate) runtime_package_lease: &'a RuntimePackageLease,
    pub(crate) static_model: &'a LocalOllamaModelBindingEvidence,
    pub(crate) model: &'a OllamaModelBinding,
    pub(crate) request: &'a StructuredCompletionRequest,
    pub(crate) receipt: &'a OllamaResidentSessionExecutionReceipt,
    pub(crate) post_generation_process: &'a AttachedProcessEvidence,
    pub(crate) post_generation_native_load: &'a NativeLoadObservation,
    pub(crate) worker: &'a FinalManagedGenerationWorkerEvidence,
    pub(crate) managed_input: &'a ManagedOllamaInputEvidence,
    pub(crate) input_bound_launch_spec_digest: &'a Digest,
    pub(crate) final_isolation: &'a IsolationEvidence,
    pub(crate) connection_observations: &'a [RetainedTcpConnectionEvidence],
    pub(crate) effective_runtime_state: &'a ManagedOllamaEffectiveRuntimeState,
}

pub(crate) fn build_generation_bracket_observation(
    input: &GenerationBracketObservationInput<'_>,
) -> Result<ManagedOllamaGenerationBracketObservationV1, LocalOllamaManagedPreflightError> {
    validate_relationships(input)?;
    let execution = input.receipt.execution();
    let observation_bytes = serde_json::to_vec(input.connection_observations)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut evidence = ManagedOllamaGenerationBracketObservationV1 {
        schema_version: MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION,
        binding_digest: Digest::sha256(b"pending"),
        managed_preflight_binding_digest: input.managed_report.binding_digest.clone(),
        managed_build_binding_digest: input.build_binding.binding_digest().clone(),
        runtime_build_id: input.build_binding.runtime_build().runtime_build_id(),
        admitted_runtime_id: input
            .admitted_runtime
            .admitted_runtime_id()
            .digest()
            .clone(),
        generation_path_id: input.generation_path.generation_path_id().clone(),
        frozen_external_component_set_id: input
            .generation_path
            .frozen_external_component_set_id()
            .digest()
            .clone(),
        runtime_package_installation_generation: input
            .runtime_package_lease
            .installation_key()
            .installation_generation(),
        model_package_installation_generation: input.managed_input.installation_generation(),
        effective_runtime_state_id: input.effective_runtime_state.effective_runtime_state_id(),
        effective_runtime_state_join_digest: input.effective_runtime_state.binding_digest().clone(),
        static_model_binding_digest: input.static_model.binding_digest().clone(),
        model_package_manifest_id: input.static_model.model_package_manifest_id.clone(),
        model_artifact_id: input.static_model.model_artifact_id.clone(),
        request_binding_digest: execution.request_digest().clone(),
        response_binding_digest: execution.response_digest().clone(),
        residency_contract_digest: input.receipt.residency_contract_digest().clone(),
        residency_observation_digest: input.receipt.residency_observation_digest().clone(),
        post_generation_process_evidence_digest: input
            .post_generation_process
            .evidence_digest()
            .clone(),
        post_generation_native_load_observation_digest: input
            .post_generation_native_load
            .native_load_observation_id()
            .digest()
            .clone(),
        managed_input_mapping_digest: input.managed_input.mapping_digest().clone(),
        managed_input_layout_digest: input.managed_input.input_layout_digest().clone(),
        input_bound_launch_spec_digest: input.input_bound_launch_spec_digest.clone(),
        generation_worker_evidence_digest: input.worker.initial.evidence_digest().clone(),
        generation_worker_native_load_observation_digest: input
            .worker
            .native_load
            .observation_digest()
            .clone(),
        generation_worker_model_mapping_observation_digest: input
            .worker
            .model_mapping
            .observation_digest()
            .clone(),
        final_isolation_evidence_digest: input.final_isolation.redacted_digest(),
        connection_observation_digest: Digest::sha256(&observation_bytes),
        connection_observation_count: u64::try_from(input.connection_observations.len())
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?,
        first_generation_response_ordinal: u64::try_from(execution.first_response_ordinal())
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?,
        last_generation_response_ordinal: u64::try_from(execution.last_response_ordinal())
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?,
        effective_context_tokens: input.receipt.context_tokens(),
        runtime_reported_accelerator_bytes: input.receipt.accelerator_bytes(),
        missing_effective_state_relationships: Vec::new(),
        static_model_package_relationship_verified: true,
        process_retained_through_generation: true,
        runtime_package_lease_retained_through_generation: true,
        model_package_lease_retained_through_generation: true,
        package_leases_revalidated_immediately_after_generation: true,
        package_leases_revalidated_after_final_observation: true,
        private_model_input_reobserved_after_generation: true,
        exact_model_weight_mapping_observed: true,
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_around_every_response: true,
        runtime_reported_residency_proven: true,
        effective_context_capacity_observed: true,
        process_retained_after_return: false,
        model_loaded_proven: true,
        model_used_proven: false,
        application_handler_proven: false,
        effective_runtime_state_proven: true,
        qualified: false,
    };
    evidence.binding_digest = bracket_observation_binding_digest(&evidence)?;
    Ok(evidence)
}

fn valid_static_relationships(input: &GenerationBracketObservationInput<'_>) -> bool {
    validate_local_ollama_model_binding_evidence(input.static_model)
        && input.static_model.preflight_plan_digest == input.managed_report.preflight.plan_digest
        && input.static_model.runtime_reference_digest
            == Digest::sha256(input.model.reference().as_bytes())
        && input.static_model.inventory_digest == *input.model.inventory_digest()
        && input.static_model.model_artifact_id == *input.model.artifact_id()
        && input.model.artifact_digest() == input.static_model.model_artifact_id.digest()
        && input.request.artifact_id == *input.model.artifact_id()
        && input.request.artifact_digest == *input.model.artifact_digest()
}

fn valid_receipt_relationships(
    input: &GenerationBracketObservationInput<'_>,
    expected_first: usize,
    expected_last: usize,
    expected_first_residency: usize,
) -> bool {
    let receipt = input.receipt;
    let execution = receipt.execution();
    execution.request_digest() == &input.request.binding_digest()
        && receipt.runtime_reference_digest() == &input.static_model.runtime_reference_digest
        && receipt.inventory_digest() == &input.static_model.inventory_digest
        && receipt.context_tokens() > 0
        && execution.first_response_ordinal() == expected_first
        && execution.last_response_ordinal() == expected_last
        && receipt.first_residency_ordinal() == expected_first_residency
        && receipt.last_residency_ordinal() == expected_last
        && receipt.runtime_reported_residency_proven()
        && !receipt.application_handler_proven()
        && !receipt.model_use_proven()
        && !receipt.resident_page_identity_proven()
        && !receipt.effective_runtime_identity_proven()
        && !receipt.qualified()
}

fn validate_relationships(
    input: &GenerationBracketObservationInput<'_>,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let report = input.managed_report;
    let receipt = input.receipt;
    let preflight_observations = report.connection_observations.len();
    let expected_observations = preflight_observations.saturating_add(GENERATION_RESPONSE_COUNT);
    let expected_first = preflight_observations;
    let expected_last = expected_observations.saturating_sub(1);
    let expected_first_residency = expected_first.saturating_add(FIRST_RESIDENCY_RESPONSE_OFFSET);
    let valid_static = valid_static_relationships(input);
    let valid_receipt = valid_receipt_relationships(
        input,
        expected_first,
        expected_last,
        expected_first_residency,
    );
    let valid_runtime = input.build_binding.managed_preflight_binding_digest()
        == &report.binding_digest
        && input.post_generation_process == &report.initial_process_witness
        && input
            .post_generation_native_load
            .runtime_package_manifest_id()
            == &report.runtime_package_manifest_id
        && input.post_generation_native_load.process_evidence_digest()
            == input.post_generation_process.evidence_digest()
        && input.final_isolation.redacted_digest() == report.final_isolation_evidence_digest;
    let valid_authority = input.admitted_runtime.runtime_package_manifest_id()
        == &report.runtime_package_manifest_id
        && input.generation_path.runtime_package_manifest_id()
            == &report.runtime_package_manifest_id
        && input.generation_path.admitted_runtime_id()
            == input.admitted_runtime.admitted_runtime_id()
        && input.generation_path.frozen_external_component_set_id()
            == input.admitted_runtime.frozen_external_component_set_id()
        && input
            .runtime_package_lease
            .evidence()
            .runtime_package_manifest_id()
            == &report.runtime_package_manifest_id
        && input
            .runtime_package_lease
            .installation_key()
            .installation_generation()
            > 0;
    let runtime_inputs = input.final_isolation.runtime_inputs();
    let valid_managed_input = input.managed_input.model_package_manifest_id()
        == &input.static_model.model_package_manifest_id
        && input.managed_input.artifact_set_id() == &input.static_model.artifact_set_id
        && input.managed_input.installation_generation()
            == input.static_model.artifact_set_installation_generation
        && input.managed_input.runtime_reference_digest()
            == &input.static_model.runtime_reference_digest
        && input.managed_input.model_artifact_id() == &input.static_model.model_artifact_id
        && runtime_inputs.member_count() == input.managed_input.member_count()
        && runtime_inputs.total_bytes() == input.managed_input.total_bytes()
        && runtime_inputs.layout_digest() == input.managed_input.input_layout_digest()
        && &runtime_inputs.input_bound_launch_digest(&report.launch_spec_digest)
            == input.input_bound_launch_spec_digest;
    let valid_worker = input.worker.initial == input.worker.final_evidence
        && input.worker.initial.runtime_package_manifest_id()
            == &report.runtime_package_manifest_id
        && input.worker.initial.model_artifact_id() == &input.static_model.model_artifact_id
        && input.worker.model_mapping.model_artifact_id() == &input.static_model.model_artifact_id
        && input.worker.native_load.component_count() > 0
        && input.worker.model_mapping.mapping_region_count() > 0;
    let valid_connections = input.connection_observations.len() == expected_observations
        && input
            .connection_observations
            .starts_with(&report.connection_observations);
    let effective_state = input.effective_runtime_state;
    let valid_effective_state = effective_state.state().runtime_build_id()
        == &input.build_binding.runtime_build().runtime_build_id()
        && effective_state.state().effective_context_tokens() == receipt.context_tokens()
        && effective_state.state().compute_backend() == ComputeBackend::NativeCpu
        && effective_state.state().placement() == ExecutionPlacement::CpuOnly
        && !effective_state.formal_placement_proven()
        && !effective_state.model_use_proven()
        && !effective_state.application_handler_proven()
        && !effective_state.qualified();
    if !valid_static
        || !valid_receipt
        || !valid_runtime
        || !valid_authority
        || !valid_managed_input
        || !valid_worker
        || !valid_connections
        || !valid_effective_state
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

fn bracket_observation_binding_digest(
    evidence: &ManagedOllamaGenerationBracketObservationV1,
) -> Result<Digest, LocalOllamaManagedPreflightError> {
    let mut canonical = evidence.clone();
    canonical.binding_digest = Digest::sha256(b"binding-field-excluded");
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut bytes = b"retonr:managed-ollama-generation-bracket-observation:v1\0".to_vec();
    bytes.extend_from_slice(&encoded);
    Ok(Digest::sha256(&bytes))
}

#[cfg(test)]
mod tests;
