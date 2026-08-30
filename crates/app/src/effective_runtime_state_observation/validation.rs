use rewrite_inference::{ReasoningPolicy, StructuredCompletionRequest};
use rewrite_model::{
    ArtifactId, ArtifactSetId, ComputeBackend, ExecutionPlacement, ModelPackageManifestId,
    RuntimeAbi, RuntimeArchitecture, RuntimeBuildId, RuntimeBuildIdentity, RuntimeBuildMode,
    RuntimeOperatingSystem,
};
use rewrite_ollama::{OllamaModelBinding, OllamaResidentSessionExecutionReceipt};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerModelMappingEvidence,
    ManagedGenerationWorkerNativeLoadEvidence,
};
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::Digest;

use crate::{MANAGED_OLLAMA_INPUT_SCHEMA_VERSION, ManagedOllamaInputEvidence};

use super::contract::{
    EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION, EffectiveRuntimeStateObservationError,
    OllamaCpuExecutionEvidence, OllamaProviderSnapshotEvidence,
    OllamaWireOutputConfigurationEvidence,
};

mod facts;
use facts::{common_facts, cpu_facts};

const OLLAMA_RUNTIME_FAMILY: &str = "ollama";
const OLLAMA_RUNTIME_VERSION: &str = "0.32.15";
const PROVIDER_SNAPSHOT_CONTRACT: &[u8] = b"ollama/retained-provider-snapshot/v1";
const PROVIDER_OBSERVATION_CONTRACT: &[u8] = b"ollama/retained-provider-observation/v1";
const WIRE_CONFIGURATION_CONTRACT: &[u8] =
    b"ollama/v0.32.15/structured-output-wire-configuration/v1";
const CPU_EXECUTION_CLASS_CONTRACT: &[u8] =
    b"ollama/v0.32.15/portable-native-cpu-execution-class/v1";
const CPU_EXECUTION_OBSERVATION_CONTRACT: &[u8] =
    b"ollama/v0.32.15/bounded-native-cpu-execution-observation/v1";

pub(super) struct ProviderSources<'a> {
    pub(super) runtime_build: &'a RuntimeBuildIdentity,
    pub(super) model_input: &'a ManagedOllamaInputEvidence,
    pub(super) model: &'a OllamaModelBinding,
    pub(super) request: &'a StructuredCompletionRequest,
    pub(super) receipt: &'a OllamaResidentSessionExecutionReceipt,
}

pub(super) struct CpuExecutionSources<'a> {
    pub(super) model_input: &'a ManagedOllamaInputEvidence,
    pub(super) request: &'a StructuredCompletionRequest,
    pub(super) receipt: &'a OllamaResidentSessionExecutionReceipt,
    pub(super) initial_isolation: &'a IsolationEvidence,
    pub(super) final_isolation: &'a IsolationEvidence,
    pub(super) initial_worker: &'a ManagedGenerationWorkerEvidence,
    pub(super) final_worker: &'a ManagedGenerationWorkerEvidence,
    pub(super) worker_native_load: &'a ManagedGenerationWorkerNativeLoadEvidence,
    pub(super) model_mapping: &'a ManagedGenerationWorkerModelMappingEvidence,
}

#[derive(Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent observation validity conditions remain separately mutable in tests"
)]
pub(super) struct CommonFacts {
    pub(super) runtime_profile_valid: bool,
    pub(super) model_input_schema_valid: bool,
    pub(super) model_installation_generation: u64,
    pub(super) input_member_count: u32,
    pub(super) input_total_bytes: u64,
    pub(super) input_model_digest: Digest,
    pub(super) request_model_digest: Digest,
    pub(super) binding_model_digest: Digest,
    pub(super) input_reference_digest: Digest,
    pub(super) binding_reference_digest: Digest,
    pub(super) receipt_reference_digest: Digest,
    pub(super) binding_inventory_digest: Digest,
    pub(super) receipt_inventory_digest: Digest,
    pub(super) request_valid: bool,
    pub(super) request_binding_digest: Digest,
    pub(super) receipt_request_digest: Digest,
    pub(super) request_context_tokens: u32,
    pub(super) receipt_context_tokens: u32,
    pub(super) receipt_first_response: usize,
    pub(super) receipt_last_response: usize,
    pub(super) receipt_first_residency: usize,
    pub(super) receipt_last_residency: usize,
    pub(super) residency_claims_valid: bool,
}

#[derive(Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent CPU evidence conditions remain separately mutable in tests"
)]
pub(super) struct CpuFacts {
    pub(super) input_model_digest: Digest,
    pub(super) request_model_digest: Digest,
    pub(super) worker_model_digest: Digest,
    pub(super) mapping_model_digest: Digest,
    pub(super) input_layout_digest: Digest,
    pub(super) isolation_layout_digest: Digest,
    pub(super) input_member_count: u32,
    pub(super) isolation_member_count: u32,
    pub(super) input_total_bytes: u64,
    pub(super) isolation_total_bytes: u64,
    pub(super) stable_isolation: bool,
    pub(super) isolation_canaries_valid: bool,
    pub(super) stable_worker: bool,
    pub(super) worker_profile_valid: bool,
    pub(super) worker_native_components: u32,
    pub(super) model_mapping_regions: u32,
    pub(super) request_binding_digest: Digest,
    pub(super) receipt_request_digest: Digest,
    pub(super) request_context_tokens: u32,
    pub(super) receipt_context_tokens: u32,
    pub(super) runtime_reported_accelerator_bytes: u64,
    pub(super) residency_claims_valid: bool,
}

#[derive(Clone)]
pub(super) struct ProviderRecordFields {
    pub(super) runtime_build_id: RuntimeBuildId,
    pub(super) runtime_package_manifest_digest: Digest,
    pub(super) model_artifact_set_id: ArtifactSetId,
    pub(super) model_package_manifest_id: ModelPackageManifestId,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) model_installation_generation: u64,
    pub(super) request_binding_digest: Digest,
    pub(super) response_binding_digest: Digest,
    pub(super) snapshot_components: [Digest; 14],
    pub(super) snapshot_numbers: [u64; 3],
}

#[derive(Clone)]
pub(super) struct WireRecordFields {
    pub(super) runtime_build_id: RuntimeBuildId,
    pub(super) model_artifact_id: ArtifactId,
    pub(super) request_binding_digest: Digest,
    pub(super) response_binding_digest: Digest,
    pub(super) effective_context_tokens: u32,
    pub(super) configuration_components: [Digest; 6],
}

#[derive(Clone)]
pub(super) struct CpuRecordFields {
    pub(super) model_artifact_id: ArtifactId,
    pub(super) isolation_evidence_digest: Digest,
    pub(super) worker_evidence_digest: Digest,
    pub(super) worker_portable_configuration_digest: Digest,
    pub(super) worker_portable_closure_digest: Digest,
    pub(super) worker_native_load_digest: Digest,
    pub(super) model_mapping_digest: Digest,
    pub(super) residency_observation_digest: Digest,
    pub(super) request_binding_digest: Digest,
    pub(super) response_binding_digest: Digest,
    pub(super) effective_context_tokens: u32,
    pub(super) runtime_reported_accelerator_bytes: u64,
}

pub(super) fn build_provider_snapshot(
    sources: &ProviderSources<'_>,
) -> Result<OllamaProviderSnapshotEvidence, EffectiveRuntimeStateObservationError> {
    let facts = common_facts(sources);
    let execution = sources.receipt.execution();
    build_provider_record(
        &facts,
        ProviderRecordFields {
            runtime_build_id: sources.runtime_build.runtime_build_id(),
            runtime_package_manifest_digest: sources
                .runtime_build
                .package_manifest_digest()
                .clone(),
            model_artifact_set_id: sources.model_input.artifact_set_id().clone(),
            model_package_manifest_id: sources.model_input.model_package_manifest_id().clone(),
            model_artifact_id: sources.model_input.model_artifact_id().clone(),
            model_installation_generation: sources.model_input.installation_generation(),
            request_binding_digest: execution.request_digest().clone(),
            response_binding_digest: execution.response_digest().clone(),
            snapshot_components: [
                sources.runtime_build.runtime_build_id().digest().clone(),
                sources.runtime_build.package_manifest_digest().clone(),
                sources.model_input.artifact_set_id().digest().clone(),
                sources
                    .model_input
                    .model_package_manifest_id()
                    .digest()
                    .clone(),
                sources.model_input.model_artifact_id().digest().clone(),
                sources.model_input.reference_digest().clone(),
                sources.model_input.mapping_digest().clone(),
                execution.preflight_digest().clone(),
                sources.receipt.residency_contract_digest().clone(),
                sources.receipt.residency_observation_digest().clone(),
                sources.receipt.runtime_reference_digest().clone(),
                sources.receipt.inventory_digest().clone(),
                execution.request_digest().clone(),
                execution.response_digest().clone(),
            ],
            snapshot_numbers: [
                sources.model_input.installation_generation(),
                u64::from(sources.receipt.context_tokens()),
                sources.receipt.accelerator_bytes(),
            ],
        },
    )
}

pub(super) fn build_provider_record(
    facts: &CommonFacts,
    fields: ProviderRecordFields,
) -> Result<OllamaProviderSnapshotEvidence, EffectiveRuntimeStateObservationError> {
    validate_common(facts)?;
    let portable_components =
        [0_usize, 1, 2, 3, 4, 5, 6, 8, 10, 11].map(|index| &fields.snapshot_components[index]);
    let snapshot_digest = domain_digest(
        PROVIDER_SNAPSHOT_CONTRACT,
        &portable_components,
        &[fields.snapshot_numbers[1], fields.snapshot_numbers[2]],
    );
    let mut observation_components = fields.snapshot_components.iter().collect::<Vec<_>>();
    observation_components.push(&snapshot_digest);
    let observation_binding_digest = domain_digest(
        PROVIDER_OBSERVATION_CONTRACT,
        &observation_components,
        &fields.snapshot_numbers,
    );
    Ok(OllamaProviderSnapshotEvidence {
        schema_version: EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION,
        runtime_build_id: fields.runtime_build_id,
        runtime_package_manifest_digest: fields.runtime_package_manifest_digest,
        model_artifact_set_id: fields.model_artifact_set_id,
        model_package_manifest_id: fields.model_package_manifest_id,
        model_artifact_id: fields.model_artifact_id,
        model_installation_generation: fields.model_installation_generation,
        request_binding_digest: fields.request_binding_digest,
        response_binding_digest: fields.response_binding_digest,
        snapshot_digest,
        observation_binding_digest,
    })
}

pub(super) fn build_effective_configuration(
    sources: &ProviderSources<'_>,
) -> Result<OllamaWireOutputConfigurationEvidence, EffectiveRuntimeStateObservationError> {
    let facts = common_facts(sources);
    let execution = sources.receipt.execution();
    build_wire_record(
        &facts,
        sources.request.sampling.temperature == 0.0
            && sources.request.reasoning == ReasoningPolicy::Disabled,
        WireRecordFields {
            runtime_build_id: sources.runtime_build.runtime_build_id(),
            model_artifact_id: sources.model_input.model_artifact_id().clone(),
            request_binding_digest: execution.request_digest().clone(),
            response_binding_digest: execution.response_digest().clone(),
            effective_context_tokens: sources.receipt.context_tokens(),
            configuration_components: [
                sources.runtime_build.runtime_build_id().digest().clone(),
                sources.model_input.model_artifact_id().digest().clone(),
                wire_request_configuration_digest(sources.request)?,
                execution.response_digest().clone(),
                sources.receipt.residency_observation_digest().clone(),
                wire_contract_digest(),
            ],
        },
    )
}

pub(super) fn build_wire_record(
    facts: &CommonFacts,
    deterministic_wire_profile: bool,
    fields: WireRecordFields,
) -> Result<OllamaWireOutputConfigurationEvidence, EffectiveRuntimeStateObservationError> {
    validate_common(facts)?;
    if !deterministic_wire_profile
        || fields.effective_context_tokens == 0
        || fields.effective_context_tokens != facts.receipt_context_tokens
    {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    let component_refs = [
        &fields.configuration_components[0],
        &fields.configuration_components[1],
        &fields.configuration_components[2],
        &fields.configuration_components[5],
    ];
    let configuration_digest = domain_digest(
        WIRE_CONFIGURATION_CONTRACT,
        &component_refs,
        &[u64::from(fields.effective_context_tokens)],
    );
    Ok(OllamaWireOutputConfigurationEvidence {
        schema_version: EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION,
        runtime_build_id: fields.runtime_build_id,
        model_artifact_id: fields.model_artifact_id,
        request_binding_digest: fields.request_binding_digest,
        response_binding_digest: fields.response_binding_digest,
        effective_context_tokens: fields.effective_context_tokens,
        configuration_digest,
    })
}

pub(super) fn build_cpu_execution(
    sources: &CpuExecutionSources<'_>,
) -> Result<OllamaCpuExecutionEvidence, EffectiveRuntimeStateObservationError> {
    let facts = cpu_facts(sources);
    let isolation_evidence_digest = sources.initial_isolation.redacted_digest();
    build_cpu_record(
        &facts,
        CpuRecordFields {
            model_artifact_id: sources.model_input.model_artifact_id().clone(),
            isolation_evidence_digest,
            worker_evidence_digest: sources.initial_worker.evidence_digest().clone(),
            worker_portable_configuration_digest: sources
                .initial_worker
                .portable_configuration_digest()
                .clone(),
            worker_portable_closure_digest: sources
                .worker_native_load
                .portable_closure_digest()
                .clone(),
            worker_native_load_digest: sources.worker_native_load.observation_digest().clone(),
            model_mapping_digest: sources.model_mapping.observation_digest().clone(),
            residency_observation_digest: sources.receipt.residency_observation_digest().clone(),
            request_binding_digest: sources.receipt.execution().request_digest().clone(),
            response_binding_digest: sources.receipt.execution().response_digest().clone(),
            effective_context_tokens: sources.receipt.context_tokens(),
            runtime_reported_accelerator_bytes: sources.receipt.accelerator_bytes(),
        },
    )
}

pub(super) fn build_cpu_record(
    facts: &CpuFacts,
    fields: CpuRecordFields,
) -> Result<OllamaCpuExecutionEvidence, EffectiveRuntimeStateObservationError> {
    validate_cpu(facts)?;
    let execution_class_digest = domain_digest(
        CPU_EXECUTION_CLASS_CONTRACT,
        &[
            fields.model_artifact_id.digest(),
            &fields.worker_portable_configuration_digest,
            &fields.worker_portable_closure_digest,
            &wire_contract_digest(),
        ],
        &[u64::from(fields.effective_context_tokens), 0],
    );
    let observation_binding_digest = domain_digest(
        CPU_EXECUTION_OBSERVATION_CONTRACT,
        &[
            fields.model_artifact_id.digest(),
            &fields.isolation_evidence_digest,
            &fields.worker_evidence_digest,
            &fields.worker_native_load_digest,
            &fields.model_mapping_digest,
            &fields.residency_observation_digest,
            &fields.request_binding_digest,
            &fields.response_binding_digest,
        ],
        &[
            u64::from(fields.effective_context_tokens),
            fields.runtime_reported_accelerator_bytes,
        ],
    );
    Ok(OllamaCpuExecutionEvidence {
        schema_version: EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION,
        model_artifact_id: fields.model_artifact_id,
        isolation_evidence_digest: fields.isolation_evidence_digest,
        worker_evidence_digest: fields.worker_evidence_digest,
        worker_portable_configuration_digest: fields.worker_portable_configuration_digest,
        worker_portable_closure_digest: fields.worker_portable_closure_digest,
        worker_native_load_digest: fields.worker_native_load_digest,
        model_mapping_digest: fields.model_mapping_digest,
        residency_observation_digest: fields.residency_observation_digest,
        effective_context_tokens: fields.effective_context_tokens,
        compute_backend: ComputeBackend::NativeCpu,
        placement: ExecutionPlacement::CpuOnly,
        execution_class_digest,
        observation_binding_digest,
    })
}

pub(super) fn wire_contract_digest() -> Digest {
    Digest::sha256(
        b"stream=false;format=exact-schema;think=false;raw=false;keep_alive=5m;temperature=0;top_p=explicit;seed=explicit;num_ctx=explicit;num_predict=explicit;num_gpu=0;stop=empty",
    )
}

pub(super) fn wire_request_configuration_digest(
    request: &StructuredCompletionRequest,
) -> Result<Digest, EffectiveRuntimeStateObservationError> {
    let mut bytes = Vec::new();
    push_bytes(&mut bytes, b"ollama/v0.32.15/wire-request-configuration/v1");
    push_bytes(
        &mut bytes,
        &serde_json::to_vec(&request.output)
            .map_err(|_error| EffectiveRuntimeStateObservationError::RelationshipMismatch)?,
    );
    bytes.extend_from_slice(&request.context_token_limit.to_be_bytes());
    bytes.extend_from_slice(&request.output_token_limit.to_be_bytes());
    bytes.extend_from_slice(&request.output_byte_limit.to_be_bytes());
    bytes.extend_from_slice(&request.sampling.temperature.to_bits().to_be_bytes());
    bytes.extend_from_slice(&request.sampling.top_p.to_bits().to_be_bytes());
    bytes.push(u8::from(request.reasoning == ReasoningPolicy::Disabled));
    Ok(Digest::sha256(&bytes))
}

pub(super) fn validate_common(
    facts: &CommonFacts,
) -> Result<(), EffectiveRuntimeStateObservationError> {
    let ordered_responses = facts.receipt_first_response < facts.receipt_first_residency
        && facts.receipt_first_residency < facts.receipt_last_residency
        && facts.receipt_last_residency == facts.receipt_last_response;
    if !facts.runtime_profile_valid
        || !facts.model_input_schema_valid
        || facts.model_installation_generation == 0
        || facts.input_member_count == 0
        || facts.input_total_bytes == 0
        || facts.input_model_digest != facts.request_model_digest
        || facts.input_model_digest != facts.binding_model_digest
        || facts.input_reference_digest != facts.binding_reference_digest
        || facts.input_reference_digest != facts.receipt_reference_digest
        || facts.binding_inventory_digest != facts.receipt_inventory_digest
        || !facts.request_valid
        || facts.request_binding_digest != facts.receipt_request_digest
        || facts.request_context_tokens == 0
        || facts.request_context_tokens != facts.receipt_context_tokens
        || !ordered_responses
        || !facts.residency_claims_valid
    {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    Ok(())
}

pub(super) fn validate_cpu(facts: &CpuFacts) -> Result<(), EffectiveRuntimeStateObservationError> {
    if facts.input_model_digest != facts.request_model_digest
        || facts.input_model_digest != facts.worker_model_digest
        || facts.input_model_digest != facts.mapping_model_digest
        || facts.input_layout_digest != facts.isolation_layout_digest
        || facts.input_member_count == 0
        || facts.input_member_count != facts.isolation_member_count
        || facts.input_total_bytes == 0
        || facts.input_total_bytes != facts.isolation_total_bytes
        || !facts.stable_isolation
        || !facts.isolation_canaries_valid
        || !facts.stable_worker
        || !facts.worker_profile_valid
        || facts.worker_native_components == 0
        || facts.model_mapping_regions == 0
        || facts.request_binding_digest != facts.receipt_request_digest
        || facts.request_context_tokens == 0
        || facts.request_context_tokens != facts.receipt_context_tokens
        || facts.runtime_reported_accelerator_bytes != 0
        || !facts.residency_claims_valid
    {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    Ok(())
}

pub(super) fn valid_runtime_profile(build: &RuntimeBuildIdentity) -> bool {
    let target = build.target();
    build.mode() == RuntimeBuildMode::ManagedProcess
        && build.runtime_family() == OLLAMA_RUNTIME_FAMILY
        && build.reported_version() == OLLAMA_RUNTIME_VERSION
        && target.operating_system() == RuntimeOperatingSystem::Linux
        && target.architecture() == RuntimeArchitecture::X86_64
        && target.abi() == RuntimeAbi::LinuxGnuLibc
}

pub(super) fn domain_digest(domain: &[u8], digests: &[&Digest], numbers: &[u64]) -> Digest {
    let mut material = Vec::with_capacity(domain.len() + digests.len() * 72 + numbers.len() * 8);
    push_bytes(&mut material, domain);
    for digest in digests {
        push_bytes(&mut material, digest.as_str().as_bytes());
    }
    for number in numbers {
        material.extend_from_slice(&number.to_be_bytes());
    }
    Digest::sha256(&material)
}

pub(super) fn push_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_be_bytes());
    target.extend_from_slice(value);
}
