use rewrite_inference::{
    OutputContract, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest,
};
use rewrite_model::{
    ArtifactId, ArtifactSetId, ComputeBackend, ExecutionPlacement, ModelPackageManifestId,
    RuntimeAbi, RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildIdentityInput,
    RuntimeBuildMode, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::platform::test_support::{build_fixture, field_valid, fields_valid};
use super::validation::{
    CommonFacts, CpuFacts, CpuRecordFields, ProviderRecordFields, WireRecordFields,
    build_cpu_record, build_provider_record, build_wire_record, valid_runtime_profile,
    validate_common, validate_cpu, wire_contract_digest, wire_request_configuration_digest,
};
use super::{
    LinuxPlatformFrameworkEvidence, OllamaCpuExecutionEvidence,
    OllamaWireOutputConfigurationEvidence, PlatformDriverEvidenceClass,
};

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn common_facts() -> CommonFacts {
    CommonFacts {
        runtime_profile_valid: true,
        model_input_schema_valid: true,
        model_installation_generation: 7,
        input_member_count: 6,
        input_total_bytes: 1_024,
        input_model_digest: digest("model"),
        request_model_digest: digest("model"),
        binding_model_digest: digest("model"),
        input_reference_digest: digest("reference"),
        binding_reference_digest: digest("reference"),
        receipt_reference_digest: digest("reference"),
        binding_inventory_digest: digest("inventory"),
        receipt_inventory_digest: digest("inventory"),
        request_valid: true,
        request_binding_digest: digest("request"),
        receipt_request_digest: digest("request"),
        request_context_tokens: 4_096,
        receipt_context_tokens: 4_096,
        receipt_first_response: 9,
        receipt_last_response: 17,
        receipt_first_residency: 13,
        receipt_last_residency: 17,
        residency_claims_valid: true,
    }
}

fn cpu_facts() -> CpuFacts {
    CpuFacts {
        input_model_digest: digest("model"),
        request_model_digest: digest("model"),
        worker_model_digest: digest("model"),
        mapping_model_digest: digest("model"),
        input_layout_digest: digest("layout"),
        isolation_layout_digest: digest("layout"),
        input_member_count: 6,
        isolation_member_count: 6,
        input_total_bytes: 1_024,
        isolation_total_bytes: 1_024,
        stable_isolation: true,
        isolation_canaries_valid: true,
        stable_worker: true,
        worker_profile_valid: true,
        worker_native_components: 4,
        model_mapping_regions: 2,
        request_binding_digest: digest("request"),
        receipt_request_digest: digest("request"),
        request_context_tokens: 4_096,
        receipt_context_tokens: 4_096,
        runtime_reported_accelerator_bytes: 0,
        residency_claims_valid: true,
    }
}

fn runtime_build(
    family: &str,
    version: &str,
    mode: RuntimeBuildMode,
    target: RuntimeTarget,
) -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode,
        runtime_family: family.to_owned(),
        reported_version: version.to_owned(),
        build_revision: Some("reviewed-revision".to_owned()),
        target,
        package_manifest_digest: digest("runtime package"),
        entrypoint_digest: digest("entrypoint"),
        packaged_dependencies_digest: digest("dependencies"),
        build_configuration_digest: digest("build configuration"),
    })
    .expect("runtime build")
}

fn linux_target() -> RuntimeTarget {
    RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("Linux target")
}

fn model_package_id() -> ModelPackageManifestId {
    serde_json::from_value(serde_json::Value::String(
        digest("model package").as_str().to_owned(),
    ))
    .expect("model package ID")
}

fn provider_fields(build: &RuntimeBuildIdentity) -> ProviderRecordFields {
    ProviderRecordFields {
        runtime_build_id: build.runtime_build_id(),
        runtime_package_manifest_digest: build.package_manifest_digest().clone(),
        model_artifact_set_id: ArtifactSetId::from_digest(digest("model set")),
        model_package_manifest_id: model_package_id(),
        model_artifact_id: ArtifactId::from_digest(digest("model")),
        model_installation_generation: 7,
        request_binding_digest: digest("request"),
        response_binding_digest: digest("response"),
        snapshot_components: std::array::from_fn(|index| {
            Digest::sha256(format!("snapshot component {index}").as_bytes())
        }),
        snapshot_numbers: [7, 4_096, 0],
    }
}

fn wire_fields(build: &RuntimeBuildIdentity) -> WireRecordFields {
    WireRecordFields {
        runtime_build_id: build.runtime_build_id(),
        model_artifact_id: ArtifactId::from_digest(digest("model")),
        request_binding_digest: digest("request"),
        response_binding_digest: digest("response"),
        effective_context_tokens: 4_096,
        configuration_components: [
            build.runtime_build_id().digest().clone(),
            digest("model"),
            digest("request"),
            digest("response"),
            digest("residency"),
            wire_contract_digest(),
        ],
    }
}

fn cpu_record_fields() -> CpuRecordFields {
    CpuRecordFields {
        model_artifact_id: ArtifactId::from_digest(digest("model")),
        isolation_evidence_digest: digest("isolation"),
        worker_evidence_digest: digest("worker"),
        worker_portable_configuration_digest: digest("worker configuration"),
        worker_portable_closure_digest: digest("portable native closure"),
        worker_native_load_digest: digest("native load"),
        model_mapping_digest: digest("mapping"),
        residency_observation_digest: digest("residency"),
        request_binding_digest: digest("request"),
        response_binding_digest: digest("response"),
        effective_context_tokens: 4_096,
        runtime_reported_accelerator_bytes: 0,
    }
}

#[test]
fn normalized_leaf_relationships_accept_one_exact_bracket() {
    validate_common(&common_facts()).expect("provider facts");
    validate_cpu(&cpu_facts()).expect("CPU facts");
}

#[test]
fn provider_constructor_binds_exact_identities_and_remains_inert() {
    let build = runtime_build(
        "ollama",
        "0.32.15",
        RuntimeBuildMode::ManagedProcess,
        linux_target(),
    );
    let evidence =
        build_provider_record(&common_facts(), provider_fields(&build)).expect("provider evidence");
    assert_eq!(evidence.schema_version(), 1);
    assert_eq!(evidence.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(
        evidence.runtime_package_manifest_digest(),
        build.package_manifest_digest()
    );
    assert_eq!(
        evidence.model_artifact_set_id(),
        &ArtifactSetId::from_digest(digest("model set"))
    );
    assert_eq!(evidence.model_package_manifest_id(), &model_package_id());
    assert_eq!(
        evidence.model_artifact_id(),
        &ArtifactId::from_digest(digest("model"))
    );
    assert_eq!(evidence.model_installation_generation(), 7);
    assert_eq!(evidence.request_binding_digest(), &digest("request"));
    assert_eq!(evidence.response_binding_digest(), &digest("response"));
    assert_ne!(evidence.snapshot_digest(), &digest("snapshot component 0"));
    assert_ne!(
        evidence.observation_binding_digest(),
        evidence.snapshot_digest()
    );
    assert!(!evidence.qualified());

    let mut changed = common_facts();
    changed.receipt_request_digest = digest("substituted request");
    assert!(build_provider_record(&changed, provider_fields(&build)).is_err());

    let mut changed_fields = provider_fields(&build);
    changed_fields.snapshot_components[3] = digest("substituted package");
    let changed_evidence =
        build_provider_record(&common_facts(), changed_fields).expect("changed inert evidence");
    assert_ne!(
        evidence.snapshot_digest(),
        changed_evidence.snapshot_digest()
    );
    let mut next_generation_facts = common_facts();
    next_generation_facts.model_installation_generation = 8;
    let mut next_generation_fields = provider_fields(&build);
    next_generation_fields.model_installation_generation = 8;
    next_generation_fields.snapshot_numbers[0] = 8;
    let next_generation = build_provider_record(&next_generation_facts, next_generation_fields)
        .expect("next installation generation");
    assert_eq!(
        evidence.snapshot_digest(),
        next_generation.snapshot_digest()
    );
    assert_ne!(
        evidence.observation_binding_digest(),
        next_generation.observation_binding_digest()
    );
    let mut repeated_preflight_fields = provider_fields(&build);
    repeated_preflight_fields.snapshot_components[7] = digest("other live preflight");
    let repeated_preflight = build_provider_record(&common_facts(), repeated_preflight_fields)
        .expect("other live preflight");
    assert_eq!(
        evidence.snapshot_digest(),
        repeated_preflight.snapshot_digest()
    );
    assert_ne!(
        evidence.observation_binding_digest(),
        repeated_preflight.observation_binding_digest()
    );
}

#[test]
fn portable_wire_configuration_excludes_exact_seed_but_binds_other_settings() {
    let request = wire_request(Some(7));
    let mut other_seed = request.clone();
    other_seed.sampling.seed = Some(8);
    assert_eq!(
        wire_request_configuration_digest(&request).expect("wire configuration"),
        wire_request_configuration_digest(&other_seed).expect("other seed")
    );
    let mut other_top_p = request;
    other_top_p.sampling.top_p = 0.9;
    assert_ne!(
        wire_request_configuration_digest(&other_top_p).expect("other top p"),
        wire_request_configuration_digest(&other_seed).expect("seed-independent configuration")
    );
}

fn wire_request(seed: Option<u64>) -> StructuredCompletionRequest {
    let schema_json = "{\"type\":\"object\"}".to_owned();
    let model = digest("model");
    StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(model.clone()),
        artifact_digest: model,
        input: "bounded input".to_owned(),
        output: OutputContract {
            schema_digest: Digest::sha256(schema_json.as_bytes()),
            schema_json,
        },
        source_byte_count: 13,
        source_byte_limit: 1_024,
        input_byte_limit: 2_048,
        context_token_limit: 4_096,
        output_token_limit: 256,
        output_byte_limit: 4_096,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed,
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

#[test]
fn wire_and_cpu_constructors_bind_every_component_and_reject_weakening() {
    let build = runtime_build(
        "ollama",
        "0.32.15",
        RuntimeBuildMode::ManagedProcess,
        linux_target(),
    );
    let wire =
        build_wire_record(&common_facts(), true, wire_fields(&build)).expect("wire evidence");
    assert_eq!(wire.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(wire.effective_context_tokens(), 4_096);
    assert!(!wire.complete_effective_configuration());
    assert_eq!(
        wire_contract_digest(),
        Digest::sha256(
            b"stream=false;format=exact-schema;think=false;raw=false;keep_alive=5m;temperature=0;top_p=explicit;seed=explicit;num_ctx=explicit;num_predict=explicit;num_gpu=0;stop=empty"
        )
    );
    assert!(build_wire_record(&common_facts(), false, wire_fields(&build)).is_err());
    let mut wrong_context = wire_fields(&build);
    wrong_context.effective_context_tokens = 2_048;
    assert!(build_wire_record(&common_facts(), true, wrong_context).is_err());

    let cpu = build_cpu_record(&cpu_facts(), cpu_record_fields()).expect("CPU evidence");
    assert_eq!(cpu.compute_backend(), ComputeBackend::NativeCpu);
    assert_eq!(cpu.placement(), ExecutionPlacement::CpuOnly);
    assert!(!cpu.formal_placement_proven());
    let mut changed_fields = cpu_record_fields();
    changed_fields.model_mapping_digest = digest("other mapping");
    let changed = build_cpu_record(&cpu_facts(), changed_fields).expect("changed CPU evidence");
    assert_eq!(
        cpu.execution_class_digest(),
        changed.execution_class_digest()
    );
    assert_ne!(
        cpu.observation_binding_digest(),
        changed.observation_binding_digest()
    );
    let mut changed_configuration = cpu_record_fields();
    changed_configuration.worker_portable_configuration_digest = digest("other configuration");
    let changed_configuration = build_cpu_record(&cpu_facts(), changed_configuration)
        .expect("changed worker configuration");
    assert_ne!(
        cpu.execution_class_digest(),
        changed_configuration.execution_class_digest()
    );
    let mut changed_closure = cpu_record_fields();
    changed_closure.worker_portable_closure_digest = digest("other portable closure");
    let changed_closure =
        build_cpu_record(&cpu_facts(), changed_closure).expect("changed portable closure");
    assert_ne!(
        cpu.execution_class_digest(),
        changed_closure.execution_class_digest()
    );
    let mut accelerated = cpu_facts();
    accelerated.runtime_reported_accelerator_bytes = 1;
    assert!(build_cpu_record(&accelerated, cpu_record_fields()).is_err());
}

mod claims_and_substitutions;
