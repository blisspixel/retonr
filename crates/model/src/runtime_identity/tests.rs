use rewrite_types::Digest;

use super::state::{compute_backend_byte, execution_placement_byte};
use super::{
    ComputeBackend, EffectiveRuntimeState, EffectiveRuntimeStateError, EffectiveRuntimeStateInput,
    ExecutionPlacement, MAX_RUNTIME_IDENTITY_JSON_BYTES, RuntimeAbi, RuntimeArchitecture,
    RuntimeBuildIdentity, RuntimeBuildIdentityError, RuntimeBuildIdentityInput, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimeTarget, abi_byte, architecture_byte, build_mode_byte,
    operating_system_byte,
};

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn build_input() -> RuntimeBuildIdentityInput {
    RuntimeBuildIdentityInput {
        mode: RuntimeBuildMode::ManagedProcess,
        runtime_family: "llama-server".to_owned(),
        reported_version: "b10417".to_owned(),
        build_revision: Some("0123456789abcdef".to_owned()),
        target: RuntimeTarget::new(
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsMsvc,
        )
        .expect("target"),
        package_manifest_digest: digest("package"),
        entrypoint_digest: digest("entrypoint"),
        packaged_dependencies_digest: digest("dependencies"),
        build_configuration_digest: digest("build configuration"),
    }
}

fn build() -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(build_input()).expect("runtime build")
}

fn state_input() -> EffectiveRuntimeStateInput {
    EffectiveRuntimeStateInput {
        provider_snapshot_contract: "llama-server-snapshot".to_owned(),
        provider_snapshot_schema_version: 1,
        provider_snapshot_digest: digest("provider snapshot"),
        launch_policy_digest: digest("launch"),
        loaded_components_digest: digest("loaded components"),
        effective_configuration_digest: digest("effective configuration"),
        platform_digest: digest("platform"),
        execution_class_digest: digest("execution class"),
        isolation_policy_digest: digest("isolation"),
        effective_context_tokens: 32_768,
        compute_backend: ComputeBackend::Cuda,
        placement: ExecutionPlacement::AcceleratorOnly,
    }
}

#[test]
fn freezes_runtime_build_and_effective_state_identities() {
    let build = build();
    assert_eq!(
        build.runtime_build_id().digest().as_str(),
        "3be1e0228cc02924beaa32ba47c309edd02fe66dbde49b32b63f8621f77f8475"
    );
    let state = EffectiveRuntimeState::new(&build, state_input()).expect("runtime state");
    assert_eq!(
        state.effective_runtime_state_id().digest().as_str(),
        "439bf6fd3c2cad92a3b0a6ea6a0f03950c8e35701d42273501c7b6783af409be"
    );
    assert_eq!(state.runtime_build_id(), &build.runtime_build_id());
}

#[test]
fn strict_round_trips_revalidate_private_identity_fields() {
    let build = build();
    let encoded = serde_json::to_string(&build).expect("serialize build");
    assert_eq!(
        RuntimeBuildIdentity::from_json_bytes(encoded.as_bytes()).expect("parse build"),
        build
    );
    let state = EffectiveRuntimeState::new(&build, state_input()).expect("runtime state");
    let encoded = serde_json::to_string(&state).expect("serialize state");
    assert_eq!(
        EffectiveRuntimeState::from_json_bytes(encoded.as_bytes()).expect("parse state"),
        state
    );

    let mut unknown = serde_json::to_value(&build).expect("build value");
    unknown["unknown"] = serde_json::json!(true);
    assert!(
        RuntimeBuildIdentity::from_json_bytes(
            serde_json::to_string(&unknown)
                .expect("encode unknown")
                .as_bytes()
        )
        .is_err()
    );
    let mut future = serde_json::to_value(&state).expect("state value");
    future["schema_version"] = serde_json::json!(2);
    assert!(
        EffectiveRuntimeState::from_json_bytes(
            serde_json::to_string(&future)
                .expect("encode future")
                .as_bytes()
        )
        .is_err()
    );
}

#[test]
fn encoded_identity_limit_precedes_json_allocation() {
    assert_eq!(
        RuntimeBuildIdentity::from_json_bytes(&vec![b' '; MAX_RUNTIME_IDENTITY_JSON_BYTES]),
        Err(RuntimeBuildIdentityError::InvalidEncoding)
    );
    assert_eq!(
        RuntimeBuildIdentity::from_json_bytes(&vec![b' '; MAX_RUNTIME_IDENTITY_JSON_BYTES + 1]),
        Err(RuntimeBuildIdentityError::EncodedIdentityTooLarge)
    );
    assert_eq!(
        EffectiveRuntimeState::from_json_bytes(&vec![b' '; MAX_RUNTIME_IDENTITY_JSON_BYTES + 1]),
        Err(EffectiveRuntimeStateError::EncodedIdentityTooLarge)
    );
}

#[test]
fn validates_target_matrix_and_portable_metadata() {
    for target in [
        (
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsMsvc,
        ),
        (
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsGnu,
        ),
        (
            RuntimeOperatingSystem::MacOs,
            RuntimeArchitecture::Aarch64,
            RuntimeAbi::Darwin,
        ),
        (
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        ),
        (
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::Aarch64,
            RuntimeAbi::LinuxMusl,
        ),
    ] {
        RuntimeTarget::new(target.0, target.1, target.2).expect("valid target");
    }
    for invalid in [
        (RuntimeOperatingSystem::Windows, RuntimeAbi::LinuxGnuLibc),
        (RuntimeOperatingSystem::MacOs, RuntimeAbi::WindowsMsvc),
        (RuntimeOperatingSystem::Linux, RuntimeAbi::Darwin),
    ] {
        assert!(RuntimeTarget::new(invalid.0, RuntimeArchitecture::X86_64, invalid.1).is_err());
    }

    for family in ["", "Ollama", "has space", "a/b", &"a".repeat(65)] {
        let mut input = build_input();
        input.runtime_family = family.to_owned();
        assert_eq!(
            RuntimeBuildIdentity::new(input),
            Err(RuntimeBuildIdentityError::InvalidMetadata)
        );
    }
    let mut input = build_input();
    input.reported_version = "v\n1".to_owned();
    assert_eq!(
        RuntimeBuildIdentity::new(input),
        Err(RuntimeBuildIdentityError::InvalidMetadata)
    );
}

#[test]
fn every_runtime_build_field_changes_its_identity() {
    let baseline = build();
    let mut variants = Vec::new();
    let mut input = build_input();
    input.mode = RuntimeBuildMode::AttachedAttestedProcess;
    variants.push(input);
    let mut input = build_input();
    input.runtime_family = "other".to_owned();
    variants.push(input);
    let mut input = build_input();
    input.reported_version = "other".to_owned();
    variants.push(input);
    let mut input = build_input();
    input.build_revision = None;
    variants.push(input);
    let mut input = build_input();
    input.target = RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("target");
    variants.push(input);
    let mut input = build_input();
    input.target = RuntimeTarget::new(
        RuntimeOperatingSystem::Windows,
        RuntimeArchitecture::Aarch64,
        RuntimeAbi::WindowsMsvc,
    )
    .expect("target");
    variants.push(input);
    for field in 0..4 {
        let mut input = build_input();
        let changed = digest(&format!("changed-{field}"));
        match field {
            0 => input.package_manifest_digest = changed,
            1 => input.entrypoint_digest = changed,
            2 => input.packaged_dependencies_digest = changed,
            _ => input.build_configuration_digest = changed,
        }
        variants.push(input);
    }
    for input in variants {
        let variant = RuntimeBuildIdentity::new(input).expect("variant");
        assert_ne!(variant.runtime_build_id(), baseline.runtime_build_id());
    }
}

#[test]
fn validates_execution_class_combinations() {
    let build = build();
    let backends = [
        ComputeBackend::NativeCpu,
        ComputeBackend::Cuda,
        ComputeBackend::Rocm,
        ComputeBackend::Metal,
        ComputeBackend::Vulkan,
        ComputeBackend::Sycl,
        ComputeBackend::OpenVino,
    ];
    let placements = [
        ExecutionPlacement::CpuOnly,
        ExecutionPlacement::AcceleratorOnly,
        ExecutionPlacement::Hybrid,
    ];
    for backend in backends {
        for placement in placements {
            let mut input = state_input();
            input.compute_backend = backend;
            input.placement = placement;
            let result = EffectiveRuntimeState::new(&build, input);
            let valid =
                backend != ComputeBackend::NativeCpu || placement == ExecutionPlacement::CpuOnly;
            assert_eq!(result.is_ok(), valid);
            if !valid {
                assert_eq!(
                    result,
                    Err(EffectiveRuntimeStateError::InvalidExecutionClass)
                );
            }
        }
    }
}

#[test]
fn canonical_enum_tags_are_append_only() {
    assert_eq!(
        [
            RuntimeBuildMode::ManagedProcess,
            RuntimeBuildMode::AttachedAttestedProcess,
            RuntimeBuildMode::AttachedAttestedContainer,
        ]
        .map(build_mode_byte),
        [0, 1, 2]
    );
    assert_eq!(
        [
            RuntimeOperatingSystem::Windows,
            RuntimeOperatingSystem::MacOs,
            RuntimeOperatingSystem::Linux,
        ]
        .map(operating_system_byte),
        [0, 1, 2]
    );
    assert_eq!(
        [RuntimeArchitecture::X86_64, RuntimeArchitecture::Aarch64].map(architecture_byte),
        [0, 1]
    );
    assert_eq!(
        [
            RuntimeAbi::WindowsMsvc,
            RuntimeAbi::WindowsGnu,
            RuntimeAbi::LinuxGnuLibc,
            RuntimeAbi::LinuxMusl,
            RuntimeAbi::Darwin,
        ]
        .map(abi_byte),
        [0, 1, 2, 3, 4]
    );
    assert_eq!(
        [
            ComputeBackend::NativeCpu,
            ComputeBackend::Cuda,
            ComputeBackend::Rocm,
            ComputeBackend::Metal,
            ComputeBackend::Vulkan,
            ComputeBackend::Sycl,
            ComputeBackend::OpenVino,
        ]
        .map(compute_backend_byte),
        [0, 1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        [
            ExecutionPlacement::CpuOnly,
            ExecutionPlacement::AcceleratorOnly,
            ExecutionPlacement::Hybrid,
        ]
        .map(execution_placement_byte),
        [0, 1, 2]
    );
}

#[test]
fn canonical_enum_json_names_are_append_only() {
    assert_eq!(
        serde_json::to_string(&[
            RuntimeBuildMode::ManagedProcess,
            RuntimeBuildMode::AttachedAttestedProcess,
            RuntimeBuildMode::AttachedAttestedContainer,
        ])
        .expect("mode names"),
        "[\"managed_process\",\"attached_attested_process\",\"attached_attested_container\"]"
    );
    assert_eq!(
        serde_json::to_string(&[
            RuntimeOperatingSystem::Windows,
            RuntimeOperatingSystem::MacOs,
            RuntimeOperatingSystem::Linux,
        ])
        .expect("OS names"),
        "[\"windows\",\"mac_os\",\"linux\"]"
    );
    assert_eq!(
        serde_json::to_string(&[RuntimeArchitecture::X86_64, RuntimeArchitecture::Aarch64])
            .expect("architecture names"),
        "[\"x86_64\",\"aarch64\"]"
    );
    assert_eq!(
        serde_json::to_string(&[
            RuntimeAbi::WindowsMsvc,
            RuntimeAbi::WindowsGnu,
            RuntimeAbi::LinuxGnuLibc,
            RuntimeAbi::LinuxMusl,
            RuntimeAbi::Darwin,
        ])
        .expect("ABI names"),
        "[\"windows_msvc\",\"windows_gnu\",\"linux_gnu_libc\",\"linux_musl\",\"darwin\"]"
    );
    assert_eq!(
        serde_json::to_string(&[
            ComputeBackend::NativeCpu,
            ComputeBackend::Cuda,
            ComputeBackend::Rocm,
            ComputeBackend::Metal,
            ComputeBackend::Vulkan,
            ComputeBackend::Sycl,
            ComputeBackend::OpenVino,
        ])
        .expect("backend names"),
        "[\"native_cpu\",\"cuda\",\"rocm\",\"metal\",\"vulkan\",\"sycl\",\"open_vino\"]"
    );
    assert_eq!(
        serde_json::to_string(&[
            ExecutionPlacement::CpuOnly,
            ExecutionPlacement::AcceleratorOnly,
            ExecutionPlacement::Hybrid,
        ])
        .expect("placement names"),
        "[\"cpu_only\",\"accelerator_only\",\"hybrid\"]"
    );
}

#[test]
fn every_effective_state_field_changes_its_identity() {
    let build = build();
    let baseline = EffectiveRuntimeState::new(&build, state_input()).expect("baseline");
    let mut variants = Vec::new();
    let mut input = state_input();
    input.provider_snapshot_contract = "other".to_owned();
    variants.push(input);
    let mut input = state_input();
    input.provider_snapshot_schema_version = 2;
    variants.push(input);
    let mut input = state_input();
    input.effective_context_tokens += 1;
    variants.push(input);
    let mut input = state_input();
    input.compute_backend = ComputeBackend::Vulkan;
    input.placement = ExecutionPlacement::Hybrid;
    variants.push(input);
    let mut input = state_input();
    input.placement = ExecutionPlacement::Hybrid;
    variants.push(input);
    for field in 0..7 {
        let mut input = state_input();
        let changed = digest(&format!("state-change-{field}"));
        match field {
            0 => input.provider_snapshot_digest = changed,
            1 => input.launch_policy_digest = changed,
            2 => input.loaded_components_digest = changed,
            3 => input.effective_configuration_digest = changed,
            4 => input.platform_digest = changed,
            5 => input.execution_class_digest = changed,
            _ => input.isolation_policy_digest = changed,
        }
        variants.push(input);
    }
    for input in variants {
        let variant = EffectiveRuntimeState::new(&build, input).expect("variant");
        assert_ne!(
            variant.effective_runtime_state_id(),
            baseline.effective_runtime_state_id()
        );
    }

    let other_build = RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        runtime_family: "other".to_owned(),
        ..build_input()
    })
    .expect("other build");
    let variant = EffectiveRuntimeState::new(&other_build, state_input()).expect("variant");
    assert_ne!(
        variant.effective_runtime_state_id(),
        baseline.effective_runtime_state_id()
    );
}

#[test]
fn rejects_empty_provider_identity_and_zero_context() {
    let build = build();
    let mut input = state_input();
    input.provider_snapshot_contract.clear();
    assert_eq!(
        EffectiveRuntimeState::new(&build, input),
        Err(EffectiveRuntimeStateError::InvalidMetadata)
    );
    let mut input = state_input();
    input.effective_context_tokens = 0;
    assert_eq!(
        EffectiveRuntimeState::new(&build, input),
        Err(EffectiveRuntimeStateError::InvalidMetadata)
    );
    let mut input = state_input();
    input.provider_snapshot_schema_version = 0;
    assert_eq!(
        EffectiveRuntimeState::new(&build, input),
        Err(EffectiveRuntimeStateError::InvalidMetadata)
    );
}

#[test]
fn accepts_exact_metadata_bounds_and_rejects_one_byte_more() {
    let mut input = build_input();
    input.runtime_family = "a".repeat(64);
    input.reported_version = "v".repeat(128);
    input.build_revision = Some("r".repeat(128));
    RuntimeBuildIdentity::new(input).expect("exact metadata bounds");

    let mut input = build_input();
    input.reported_version = "v".repeat(129);
    assert_eq!(
        RuntimeBuildIdentity::new(input),
        Err(RuntimeBuildIdentityError::InvalidMetadata)
    );
    let mut input = build_input();
    input.build_revision = Some(String::new());
    assert_eq!(
        RuntimeBuildIdentity::new(input),
        Err(RuntimeBuildIdentityError::InvalidMetadata)
    );
}
