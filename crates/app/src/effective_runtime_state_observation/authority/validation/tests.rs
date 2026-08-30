use rewrite_model::{
    ArtifactId, ArtifactSetId, ComputeBackend, ExecutionPlacement, ModelPackageManifestId,
    RuntimeAbi, RuntimeArchitecture, RuntimeBuildId, RuntimeBuildIdentity,
    RuntimeBuildIdentityInput, RuntimeBuildMode, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::{
    BracketBindings, ExecutionBindings, LiveEffectiveStateFacts, ModelBindings, RuntimeBindings,
    build_effective_state,
};
use crate::effective_runtime_state_observation::{
    EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION, MANAGED_OLLAMA_PROVIDER_SNAPSHOT_CONTRACT,
};

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn model_package_id(label: &str) -> ModelPackageManifestId {
    serde_json::from_value(serde_json::Value::String(digest(label).as_str().to_owned()))
        .expect("model package ID")
}

fn runtime_build_id(label: &str) -> RuntimeBuildId {
    serde_json::from_value(serde_json::Value::String(digest(label).as_str().to_owned()))
        .expect("runtime build ID")
}

fn build() -> RuntimeBuildIdentity {
    let target = RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("target");
    RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode: RuntimeBuildMode::ManagedProcess,
        runtime_family: "ollama".to_owned(),
        reported_version: "0.32.15".to_owned(),
        build_revision: Some("reviewed".to_owned()),
        target,
        package_manifest_digest: digest("runtime package"),
        entrypoint_digest: digest("entrypoint"),
        packaged_dependencies_digest: digest("dependencies"),
        build_configuration_digest: digest("build configuration"),
    })
    .expect("runtime build")
}

#[expect(
    clippy::too_many_lines,
    reason = "the full positive authority fixture names every independent binding"
)]
fn facts(build: &RuntimeBuildIdentity) -> LiveEffectiveStateFacts {
    let runtime_id = build.runtime_build_id();
    let package_digest = build.package_manifest_digest().clone();
    let artifact_set = ArtifactSetId::from_digest(digest("model set"));
    let model_package = model_package_id("model package");
    let model = ArtifactId::from_digest(digest("model"));
    LiveEffectiveStateFacts {
        runtime: RuntimeBindings {
            runtime_build_id: runtime_id.clone(),
            provider_runtime_build_id: runtime_id.clone(),
            wire_runtime_build_id: runtime_id.clone(),
            platform_runtime_build_id: runtime_id,
            runtime_package_digest: package_digest.clone(),
            runtime_installation_generation: 11,
            lease_runtime_package_digest: package_digest.clone(),
            runtime_entrypoint_digest: digest("entrypoint"),
            packaged_dependencies_digest: digest("dependencies"),
            provider_runtime_package_digest: package_digest.clone(),
            admitted_runtime_package_digest: package_digest.clone(),
            path_runtime_package_digest: package_digest,
            admitted_frozen_component_set_digest: digest("frozen components"),
            path_frozen_component_set_digest: digest("frozen components"),
            admitted_runtime_id: digest("admission"),
            path_admitted_runtime_id: digest("admission"),
            generation_path_id: digest("generation path"),
            admitted_plain_launch_digest: digest("plain launch"),
            retained_plain_launch_digest: digest("plain launch"),
        },
        model: ModelBindings {
            input_artifact_set_id: artifact_set.clone(),
            provider_artifact_set_id: artifact_set,
            input_model_package_id: model_package.clone(),
            provider_model_package_id: model_package,
            input_installation_generation: 7,
            provider_installation_generation: 7,
            input_model_artifact_id: model.clone(),
            provider_model_artifact_id: model.clone(),
            wire_model_artifact_id: model.clone(),
            cpu_model_artifact_id: model.clone(),
            worker_model_artifact_id: model.clone(),
            mapping_model_artifact_id: model.clone(),
            model_target_artifact_id: model,
            input_model_target_digest: digest("model target"),
            retained_model_target_digest: digest("model target"),
        },
        bracket: BracketBindings {
            request_digest: digest("request"),
            receipt_request_digest: digest("request"),
            provider_request_digest: digest("request"),
            wire_request_digest: digest("request"),
            receipt_response_digest: digest("response"),
            provider_response_digest: digest("response"),
            wire_response_digest: digest("response"),
            receipt_complete_binding_digest: digest("complete receipt"),
            receipt_preflight_digest: digest("preflight"),
            receipt_first_response_ordinal: 8,
            receipt_last_response_ordinal: 16,
            receipt_first_residency_ordinal: 12,
            receipt_last_residency_ordinal: 16,
            request_context_tokens: 4_096,
            receipt_context_tokens: 4_096,
            wire_context_tokens: 4_096,
            cpu_context_tokens: 4_096,
            input_layout_digest: digest("input layout"),
            isolation_layout_digest: digest("input layout"),
            input_member_count: 6,
            isolation_member_count: 6,
            input_total_bytes: 1_024,
            isolation_total_bytes: 1_024,
            retained_input_bound_launch_digest: digest("input bound launch"),
            observed_input_bound_launch_digest: digest("input bound launch"),
            input_mapping_digest: digest("input mapping"),
        },
        execution: ExecutionBindings {
            initial_isolation_digest: digest("isolation"),
            final_isolation_digest: digest("isolation"),
            cpu_isolation_digest: digest("isolation"),
            initial_worker_digest: digest("worker"),
            final_worker_digest: digest("worker"),
            cpu_worker_digest: digest("worker"),
            reviewed_worker_artifact_id: ArtifactId::from_digest(digest("worker artifact")),
            initial_worker_artifact_id: ArtifactId::from_digest(digest("worker artifact")),
            final_worker_artifact_id: ArtifactId::from_digest(digest("worker artifact")),
            initial_worker_runtime_package_digest: digest("runtime package"),
            final_worker_runtime_package_digest: digest("runtime package"),
            worker_portable_configuration_digest: digest("worker configuration"),
            final_worker_portable_configuration_digest: digest("worker configuration"),
            cpu_portable_configuration_digest: digest("worker configuration"),
            worker_portable_closure_digest: digest("portable native closure"),
            platform_portable_closure_digest: digest("portable native closure"),
            cpu_portable_closure_digest: digest("portable native closure"),
            server_portable_component_set_digest: digest("portable server closure"),
            server_native_load_observation_digest: digest("server native observation"),
            server_process_evidence_digest: digest("server process"),
            server_native_process_evidence_digest: digest("server process"),
            server_entrypoint_digest: digest("entrypoint"),
            server_process_profile_valid: true,
            server_native_load_runtime_package_digest: digest("runtime package"),
            server_native_component_count: 3,
            worker_native_load_digest: digest("worker native load"),
            platform_native_load_digest: digest("worker native load"),
            cpu_native_load_digest: digest("worker native load"),
            model_mapping_digest: digest("model mapping"),
            cpu_model_mapping_digest: digest("model mapping"),
            residency_digest: digest("residency"),
            cpu_residency_digest: digest("residency"),
            wire_configuration_digest: digest("wire configuration"),
            platform_digest: digest("platform"),
            platform_kernel_observation_digest: digest("kernel observation"),
            execution_class_digest: digest("execution class"),
            isolation_policy_digest: digest("isolation policy"),
            worker_profile_valid: true,
            worker_native_component_count: 4,
            model_mapping_region_count: 2,
            runtime_reported_accelerator_bytes: 0,
            isolation_canaries_valid: true,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
        },
        leaf_schema_versions: [EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION; 4],
        provider_snapshot_digest: digest("provider snapshot"),
        provider_observation_binding_digest: digest("provider observation"),
        cpu_observation_binding_digest: digest("CPU observation"),
    }
}

fn assert_rejected(
    build: &RuntimeBuildIdentity,
    base: &LiveEffectiveStateFacts,
    mutations: &[fn(&mut LiveEffectiveStateFacts)],
) {
    for mutate in mutations {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert!(build_effective_state(build, &changed).is_err());
    }
}

#[test]
fn exact_live_relationships_construct_only_inert_state() {
    let build = build();
    let observed = build_effective_state(&build, &facts(&build)).expect("effective state");
    assert_eq!(
        observed.state().runtime_build_id(),
        &build.runtime_build_id()
    );
    assert_ne!(
        observed.state().loaded_components_digest(),
        &digest("portable native closure")
    );
    assert_eq!(observed.state().effective_context_tokens(), 4_096);
    assert_eq!(
        observed.state().compute_backend(),
        ComputeBackend::NativeCpu
    );
    assert_eq!(observed.state().placement(), ExecutionPlacement::CpuOnly);
    assert!(!observed.qualified());
    assert!(!observed.formal_placement_proven());
    assert!(!observed.model_use_proven());
    assert!(!observed.application_handler_proven());
    assert_eq!(
        observed.effective_runtime_state_id(),
        observed.state().effective_runtime_state_id()
    );
    assert_ne!(observed.binding_digest(), &digest("provider snapshot"));
    assert_eq!(
        observed.effective_runtime_state_join_id().digest(),
        observed.binding_digest()
    );
    let state = observed.into_state();
    let encoded = serde_json::to_string(&state).expect("state JSON");
    assert!(encoded.contains(MANAGED_OLLAMA_PROVIDER_SNAPSHOT_CONTRACT));
    assert!(encoded.contains("native_cpu"));
    assert!(encoded.contains("cpu_only"));
}

#[test]
fn equal_portable_states_retain_distinct_effective_package_subjects() {
    let build = build();
    let base = facts(&build);
    let original = build_effective_state(&build, &base).expect("original state");
    for mutate in [
        (|facts: &mut LiveEffectiveStateFacts| {
            facts.runtime.admitted_runtime_id = digest("other admission");
            facts.runtime.path_admitted_runtime_id = digest("other admission");
        }) as fn(&mut LiveEffectiveStateFacts),
        |facts| facts.runtime.generation_path_id = digest("other generation path"),
        |facts| facts.runtime.runtime_installation_generation = 12,
        |facts| {
            facts.model.input_installation_generation = 8;
            facts.model.provider_installation_generation = 8;
        },
        |facts| {
            facts.bracket.retained_input_bound_launch_digest = digest("other bound launch");
            facts.bracket.observed_input_bound_launch_digest = digest("other bound launch");
        },
    ] {
        let mut changed = base.clone();
        mutate(&mut changed);
        let changed = build_effective_state(&build, &changed).expect("changed live subject");
        assert_eq!(
            original.effective_runtime_state_id(),
            changed.effective_runtime_state_id()
        );
        assert!(!original.has_same_effective_package_subject(&changed));
    }
}

#[test]
fn runtime_and_model_substitutions_fail_closed() {
    let build = build();
    let base = facts(&build);
    assert_rejected(
        &build,
        &base,
        &[
            |f| f.runtime.provider_runtime_build_id = runtime_build_id("other build"),
            |f| f.runtime.wire_runtime_build_id = runtime_build_id("other build"),
            |f| f.runtime.platform_runtime_build_id = runtime_build_id("other build"),
            |f| f.runtime.provider_runtime_package_digest = digest("other package"),
            |f| f.runtime.lease_runtime_package_digest = digest("other package"),
            |f| f.runtime.admitted_runtime_package_digest = digest("other package"),
            |f| f.runtime.path_runtime_package_digest = digest("other package"),
            |f| f.runtime.runtime_installation_generation = 0,
            |f| f.runtime.path_frozen_component_set_digest = digest("other frozen set"),
            |f| f.runtime.path_admitted_runtime_id = digest("other admission"),
            |f| f.runtime.retained_plain_launch_digest = digest("other launch"),
            |f| f.model.provider_artifact_set_id = ArtifactSetId::from_digest(digest("other set")),
            |f| f.model.provider_model_package_id = model_package_id("other package"),
            |f| f.model.provider_installation_generation = 8,
            |f| f.model.provider_model_artifact_id = ArtifactId::from_digest(digest("other model")),
            |f| f.model.wire_model_artifact_id = ArtifactId::from_digest(digest("other model")),
            |f| f.model.cpu_model_artifact_id = ArtifactId::from_digest(digest("other model")),
            |f| f.model.worker_model_artifact_id = ArtifactId::from_digest(digest("other model")),
            |f| f.model.mapping_model_artifact_id = ArtifactId::from_digest(digest("other model")),
            |f| f.model.model_target_artifact_id = ArtifactId::from_digest(digest("other model")),
            |f| f.model.retained_model_target_digest = digest("other target"),
        ],
    );
    let mut wrong_build = base;
    wrong_build.runtime.runtime_build_id = runtime_build_id("other build");
    assert!(build_effective_state(&build, &wrong_build).is_err());
}

#[test]
fn bracket_and_execution_substitutions_fail_closed() {
    let build = build();
    let base = facts(&build);
    assert_rejected(
        &build,
        &base,
        &[
            |f| f.bracket.receipt_request_digest = digest("other request"),
            |f| f.bracket.provider_request_digest = digest("other request"),
            |f| f.bracket.wire_request_digest = digest("other request"),
            |f| f.bracket.provider_response_digest = digest("other response"),
            |f| f.bracket.wire_response_digest = digest("other response"),
            |f| f.bracket.request_context_tokens = 0,
            |f| f.bracket.receipt_context_tokens = 2_048,
            |f| f.bracket.wire_context_tokens = 2_048,
            |f| f.bracket.cpu_context_tokens = 2_048,
            |f| f.bracket.isolation_layout_digest = digest("other layout"),
            |f| f.bracket.input_member_count = 0,
            |f| f.bracket.isolation_member_count = 5,
            |f| f.bracket.input_total_bytes = 0,
            |f| f.bracket.isolation_total_bytes = 2_048,
            |f| f.bracket.observed_input_bound_launch_digest = digest("other launch"),
            |f| f.execution.final_isolation_digest = digest("other isolation"),
            |f| f.execution.cpu_isolation_digest = digest("other isolation"),
            |f| f.execution.final_worker_digest = digest("other worker"),
            |f| f.execution.cpu_worker_digest = digest("other worker"),
            |f| {
                f.execution.initial_worker_artifact_id =
                    ArtifactId::from_digest(digest("other worker"));
            },
            |f| {
                f.execution.final_worker_artifact_id =
                    ArtifactId::from_digest(digest("other worker"));
            },
            |f| f.execution.initial_worker_runtime_package_digest = digest("other package"),
            |f| f.execution.final_worker_runtime_package_digest = digest("other package"),
            |f| {
                f.execution.final_worker_portable_configuration_digest =
                    digest("other worker configuration");
            },
            |f| {
                f.execution.cpu_portable_configuration_digest =
                    digest("other worker configuration");
            },
            |f| f.execution.platform_portable_closure_digest = digest("other portable closure"),
            |f| f.execution.cpu_portable_closure_digest = digest("other portable closure"),
            |f| f.execution.server_native_process_evidence_digest = digest("other server process"),
            |f| f.execution.server_entrypoint_digest = digest("other entrypoint"),
            |f| f.execution.server_process_profile_valid = false,
            |f| f.execution.server_native_load_runtime_package_digest = digest("other package"),
            |f| f.execution.server_native_component_count = 0,
            |f| f.execution.platform_native_load_digest = digest("other native load"),
            |f| f.execution.cpu_native_load_digest = digest("other native load"),
            |f| f.execution.cpu_model_mapping_digest = digest("other mapping"),
            |f| f.execution.cpu_residency_digest = digest("other residency"),
            |f| f.execution.worker_profile_valid = false,
            |f| f.execution.worker_native_component_count = 0,
            |f| f.execution.model_mapping_region_count = 0,
            |f| f.execution.runtime_reported_accelerator_bytes = 1,
            |f| f.execution.isolation_canaries_valid = false,
            |f| f.execution.compute_backend = ComputeBackend::Cuda,
            |f| f.execution.placement = ExecutionPlacement::Hybrid,
            |f| f.leaf_schema_versions[2] = 0,
        ],
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one matrix contrasts portable identity inputs with attempt-only provenance"
)]
fn every_state_identity_component_changes_the_result() {
    let build = build();
    let base = facts(&build);
    let original = build_effective_state(&build, &base).expect("original");
    for mutate in [
        (|f: &mut LiveEffectiveStateFacts| {
            f.runtime.admitted_plain_launch_digest = digest("other launch");
            f.runtime.retained_plain_launch_digest = digest("other launch");
        }) as fn(&mut LiveEffectiveStateFacts),
        |f| {
            f.execution.worker_portable_closure_digest = digest("other portable closure");
            f.execution.platform_portable_closure_digest = digest("other portable closure");
            f.execution.cpu_portable_closure_digest = digest("other portable closure");
        },
        |f| {
            f.execution.server_portable_component_set_digest = digest("other server closure");
        },
        |f| {
            f.execution.worker_portable_configuration_digest = digest("other configuration");
            f.execution.final_worker_portable_configuration_digest = digest("other configuration");
            f.execution.cpu_portable_configuration_digest = digest("other configuration");
            f.execution.execution_class_digest = digest("other execution class");
        },
        |f| f.execution.wire_configuration_digest = digest("other wire"),
        |f| f.bracket.input_mapping_digest = digest("other mapping"),
        |f| f.execution.isolation_policy_digest = digest("other policy"),
        |f| f.execution.platform_digest = digest("other platform"),
        |f| f.execution.execution_class_digest = digest("other execution"),
        |f| f.provider_snapshot_digest = digest("other snapshot"),
    ] {
        let mut changed = base.clone();
        mutate(&mut changed);
        let changed = build_effective_state(&build, &changed).expect("changed state");
        assert_ne!(
            original.effective_runtime_state_id(),
            changed.effective_runtime_state_id()
        );
        assert_ne!(original.binding_digest(), changed.binding_digest());
    }

    for mutate in [
        (|f: &mut LiveEffectiveStateFacts| {
            f.runtime.admitted_runtime_id = digest("other admission");
            f.runtime.path_admitted_runtime_id = digest("other admission");
        }) as fn(&mut LiveEffectiveStateFacts),
        |f| f.runtime.generation_path_id = digest("other path"),
        |f| f.runtime.runtime_installation_generation = 12,
        |f| {
            f.runtime.admitted_frozen_component_set_digest = digest("other frozen set");
            f.runtime.path_frozen_component_set_digest = digest("other frozen set");
        },
        |f| {
            f.model.input_installation_generation = 8;
            f.model.provider_installation_generation = 8;
            f.provider_observation_binding_digest = digest("other provider observation");
        },
        |f| {
            f.bracket.request_digest = digest("other request");
            f.bracket.receipt_request_digest = digest("other request");
            f.bracket.provider_request_digest = digest("other request");
            f.bracket.wire_request_digest = digest("other request");
            f.bracket.receipt_response_digest = digest("other response");
            f.bracket.provider_response_digest = digest("other response");
            f.bracket.wire_response_digest = digest("other response");
            f.provider_observation_binding_digest = digest("other provider observation");
        },
        |f| {
            f.bracket.input_layout_digest = digest("other live layout");
            f.bracket.isolation_layout_digest = digest("other live layout");
            f.bracket.retained_input_bound_launch_digest = digest("other input bound launch");
            f.bracket.observed_input_bound_launch_digest = digest("other input bound launch");
        },
        |f| {
            f.execution.initial_isolation_digest = digest("other isolation");
            f.execution.final_isolation_digest = digest("other isolation");
            f.execution.cpu_isolation_digest = digest("other isolation");
            f.execution.initial_worker_digest = digest("other worker");
            f.execution.final_worker_digest = digest("other worker");
            f.execution.cpu_worker_digest = digest("other worker");
            f.execution.worker_native_load_digest = digest("other native load");
            f.execution.platform_native_load_digest = digest("other native load");
            f.execution.cpu_native_load_digest = digest("other native load");
            f.execution.model_mapping_digest = digest("other live mapping");
            f.execution.cpu_model_mapping_digest = digest("other live mapping");
            f.execution.residency_digest = digest("other residency");
            f.execution.cpu_residency_digest = digest("other residency");
            f.cpu_observation_binding_digest = digest("other CPU observation");
        },
        |f| {
            f.execution.server_process_evidence_digest = digest("other server process");
            f.execution.server_native_process_evidence_digest = digest("other server process");
            f.execution.server_native_load_observation_digest =
                digest("other server native observation");
        },
        |f| {
            f.execution.platform_kernel_observation_digest = digest("other kernel observation");
        },
    ] {
        let mut changed = base.clone();
        mutate(&mut changed);
        let changed = build_effective_state(&build, &changed).expect("provenance-only change");
        assert_eq!(
            original.effective_runtime_state_id(),
            changed.effective_runtime_state_id()
        );
        assert_ne!(original.binding_digest(), changed.binding_digest());
    }
}
