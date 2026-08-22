use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{ArtifactId, ModelPackageManifestId, NativeLoadObservation, RuntimeBuildId};
use rewrite_ollama::{OllamaModelBinding, OllamaResidentSessionExecutionReceipt};
use rewrite_runtime_attestor::{AttachedProcessEvidence, RetainedTcpConnectionEvidence};
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::Digest;
use serde::Serialize;

use crate::{
    LocalOllamaModelBindingEvidence,
    local_ollama_model_binding::validate_local_ollama_model_binding_evidence,
};

use super::super::{
    LocalOllamaEffectiveStateMissingRelationship, LocalOllamaManagedBuildBinding,
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightReport,
};

/// Current retained managed-generation evidence contract version.
pub const LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION: u32 = 1;

const GENERATION_RESPONSE_COUNT: usize = 9;
const FIRST_RESIDENCY_RESPONSE_OFFSET: usize = 4;
const MISSING_RELATIONSHIPS: [LocalOllamaEffectiveStateMissingRelationship; 4] = [
    LocalOllamaEffectiveStateMissingRelationship::GenerationBoundProviderSnapshot,
    LocalOllamaEffectiveStateMissingRelationship::EffectiveOutputConfiguration,
    LocalOllamaEffectiveStateMissingRelationship::PlatformFrameworkAndDriver,
    LocalOllamaEffectiveStateMissingRelationship::ComputeBackendAndPlacement,
];

/// Redacted evidence for one completion bracketed by the retained managed runtime.
///
/// This record proves that one process, isolated namespace, package lease, native
/// load observer, and direct HTTP/1 connection remained stable through the exact
/// response sequence. The Ollama API additionally reported two equal residency
/// snapshots and an effective context length. It does not prove handler execution,
/// model weight use, resident-page identity, complete effective runtime identity,
/// semantic correctness, or qualification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each evidence strength and limitation remains independently explicit"
)]
pub struct LocalOllamaManagedGenerationEvidence {
    schema_version: u32,
    binding_digest: Digest,
    managed_preflight_binding_digest: Digest,
    managed_build_binding_digest: Digest,
    runtime_build_id: RuntimeBuildId,
    static_model_binding_digest: Digest,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    request_binding_digest: Digest,
    response_binding_digest: Digest,
    residency_contract_digest: Digest,
    residency_observation_digest: Digest,
    post_generation_process_evidence_digest: Digest,
    post_generation_native_load_observation_digest: Digest,
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
    package_lease_retained_through_generation: bool,
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

impl LocalOllamaManagedGenerationEvidence {
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

    /// Returns the exact package-declared runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
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

    /// Always false until every remaining effective-state relationship is observed.
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

pub(super) struct GenerationEvidenceInput<'a> {
    pub(super) managed_report: &'a LocalOllamaManagedPreflightReport,
    pub(super) build_binding: &'a LocalOllamaManagedBuildBinding,
    pub(super) static_model: &'a LocalOllamaModelBindingEvidence,
    pub(super) model: &'a OllamaModelBinding,
    pub(super) request: &'a StructuredCompletionRequest,
    pub(super) receipt: &'a OllamaResidentSessionExecutionReceipt,
    pub(super) post_generation_process: &'a AttachedProcessEvidence,
    pub(super) post_generation_native_load: &'a NativeLoadObservation,
    pub(super) final_isolation: &'a IsolationEvidence,
    pub(super) connection_observations: &'a [RetainedTcpConnectionEvidence],
}

pub(super) fn build_generation_evidence(
    input: &GenerationEvidenceInput<'_>,
) -> Result<LocalOllamaManagedGenerationEvidence, LocalOllamaManagedPreflightError> {
    validate_relationships(input)?;
    let execution = input.receipt.execution();
    let observation_bytes = serde_json::to_vec(input.connection_observations)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut evidence = LocalOllamaManagedGenerationEvidence {
        schema_version: LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION,
        binding_digest: Digest::sha256(b"pending"),
        managed_preflight_binding_digest: input.managed_report.binding_digest.clone(),
        managed_build_binding_digest: input.build_binding.binding_digest().clone(),
        runtime_build_id: input.build_binding.runtime_build().runtime_build_id(),
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
        missing_effective_state_relationships: MISSING_RELATIONSHIPS.to_vec(),
        static_model_package_relationship_verified: true,
        process_retained_through_generation: true,
        package_lease_retained_through_generation: true,
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_around_every_response: true,
        runtime_reported_residency_proven: true,
        effective_context_capacity_observed: true,
        process_retained_after_return: false,
        model_loaded_proven: false,
        model_used_proven: false,
        application_handler_proven: false,
        effective_runtime_state_proven: false,
        qualified: false,
    };
    evidence.binding_digest = evidence_binding_digest(&evidence)?;
    Ok(evidence)
}

fn validate_relationships(
    input: &GenerationEvidenceInput<'_>,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let report = input.managed_report;
    let receipt = input.receipt;
    let execution = receipt.execution();
    let preflight_observations = report.connection_observations.len();
    let expected_observations = preflight_observations.saturating_add(GENERATION_RESPONSE_COUNT);
    let expected_first = preflight_observations;
    let expected_last = expected_observations.saturating_sub(1);
    let expected_first_residency = expected_first.saturating_add(FIRST_RESIDENCY_RESPONSE_OFFSET);
    let valid_static = validate_local_ollama_model_binding_evidence(input.static_model)
        && input.static_model.preflight_plan_digest == report.preflight.plan_digest
        && input.static_model.runtime_reference_digest
            == Digest::sha256(input.model.reference().as_bytes())
        && input.static_model.inventory_digest == *input.model.inventory_digest()
        && input.static_model.model_artifact_id == *input.model.artifact_id()
        && input.model.artifact_digest() == input.static_model.model_artifact_id.digest()
        && input.request.artifact_id == *input.model.artifact_id()
        && input.request.artifact_digest == *input.model.artifact_digest();
    let valid_receipt = execution.request_digest() == &input.request.binding_digest()
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
        && !receipt.qualified();
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
    let valid_connections = input.connection_observations.len() == expected_observations
        && input
            .connection_observations
            .starts_with(&report.connection_observations);
    if !valid_static || !valid_receipt || !valid_runtime || !valid_connections {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

fn evidence_binding_digest(
    evidence: &LocalOllamaManagedGenerationEvidence,
) -> Result<Digest, LocalOllamaManagedPreflightError> {
    let mut canonical = evidence.clone();
    canonical.binding_digest = Digest::sha256(b"binding-field-excluded");
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut bytes = b"retonr:local-ollama-managed-generation-evidence:v1\0".to_vec();
    bytes.extend_from_slice(&encoded);
    Ok(Digest::sha256(&bytes))
}

#[cfg(test)]
mod tests {
    use rewrite_model::{RuntimeBuildIdentity, RuntimeBuildMode};
    use rewrite_types::Digest;

    use super::{
        LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION,
        LocalOllamaManagedGenerationEvidence, MISSING_RELATIONSHIPS, evidence_binding_digest,
    };
    use crate::{
        local_ollama_managed_preflight::test_support::package_for_version,
        local_ollama_model_binding::{
            LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, tests::exact_binding_fixture,
        },
    };

    fn fixture_evidence(accelerator_bytes: u64) -> LocalOllamaManagedGenerationEvidence {
        let package = package_for_version(LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION);
        let runtime_build = RuntimeBuildIdentity::new_from_package_manifest(
            RuntimeBuildMode::ManagedProcess,
            &package,
        )
        .expect("runtime build");
        let (_plan, model, _binding) = exact_binding_fixture();
        let mut evidence = LocalOllamaManagedGenerationEvidence {
            schema_version: LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION,
            binding_digest: Digest::sha256(b"pending"),
            managed_preflight_binding_digest: Digest::sha256(b"managed preflight"),
            managed_build_binding_digest: Digest::sha256(b"managed build"),
            runtime_build_id: runtime_build.runtime_build_id(),
            static_model_binding_digest: model.binding_digest().clone(),
            model_package_manifest_id: model.model_package_manifest_id.clone(),
            model_artifact_id: model.model_artifact_id.clone(),
            request_binding_digest: Digest::sha256(b"request"),
            response_binding_digest: Digest::sha256(b"response"),
            residency_contract_digest: Digest::sha256(b"residency contract"),
            residency_observation_digest: Digest::sha256(b"residency observation"),
            post_generation_process_evidence_digest: Digest::sha256(b"process"),
            post_generation_native_load_observation_digest: Digest::sha256(b"native load"),
            final_isolation_evidence_digest: Digest::sha256(b"isolation"),
            connection_observation_digest: Digest::sha256(b"connections"),
            connection_observation_count: 17,
            first_generation_response_ordinal: 8,
            last_generation_response_ordinal: 16,
            effective_context_tokens: 2048,
            runtime_reported_accelerator_bytes: accelerator_bytes,
            missing_effective_state_relationships: MISSING_RELATIONSHIPS.to_vec(),
            static_model_package_relationship_verified: true,
            process_retained_through_generation: true,
            package_lease_retained_through_generation: true,
            all_responses_used_retained_transport: true,
            kernel_attribution_checked_around_every_response: true,
            runtime_reported_residency_proven: true,
            effective_context_capacity_observed: true,
            process_retained_after_return: false,
            model_loaded_proven: false,
            model_used_proven: false,
            application_handler_proven: false,
            effective_runtime_state_proven: false,
            qualified: false,
        };
        evidence.binding_digest = evidence_binding_digest(&evidence).expect("binding digest");
        evidence
    }

    #[test]
    fn redacted_contract_binds_positive_and_negative_claims() {
        let evidence = fixture_evidence(0);
        assert_eq!(
            evidence.schema_version(),
            LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION
        );
        assert_eq!(evidence.binding_digest(), &evidence.binding_digest);
        assert_eq!(evidence.runtime_build_id(), &evidence.runtime_build_id);
        assert_eq!(
            evidence.model_package_manifest_id(),
            &evidence.model_package_manifest_id
        );
        assert_eq!(evidence.model_artifact_id(), &evidence.model_artifact_id);
        assert_eq!(
            evidence.response_binding_digest(),
            &evidence.response_binding_digest
        );
        assert_eq!(evidence.effective_context_tokens(), 2048);
        assert_eq!(
            evidence.missing_effective_state_relationships(),
            MISSING_RELATIONSHIPS
        );
        assert!(evidence.process_retained_through_generation());
        assert!(evidence.runtime_reported_residency_proven());
        assert!(evidence.effective_context_capacity_observed());
        assert!(!evidence.process_retained_after_return());
        assert!(!evidence.model_used_proven());
        assert!(!evidence.application_handler_proven());
        assert!(!evidence.effective_runtime_state_proven());
        assert!(!evidence.qualified());
        assert_ne!(
            evidence.binding_digest(),
            fixture_evidence(1).binding_digest()
        );

        let encoded = serde_json::to_string(&evidence).expect("serialize evidence");
        assert!(!encoded.contains("bounded fixture"));
        assert!(!encoded.contains("model:exact"));
    }
}
