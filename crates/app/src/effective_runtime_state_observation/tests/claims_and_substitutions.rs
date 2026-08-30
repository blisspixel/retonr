use super::*;

#[test]
fn every_leaf_accessor_preserves_bounded_claim_strength() {
    let build = runtime_build(
        "ollama",
        "0.32.15",
        RuntimeBuildMode::ManagedProcess,
        linux_target(),
    );
    let wire = OllamaWireOutputConfigurationEvidence {
        schema_version: 1,
        runtime_build_id: build.runtime_build_id(),
        model_artifact_id: ArtifactId::from_digest(digest("model")),
        request_binding_digest: digest("request"),
        response_binding_digest: digest("response"),
        effective_context_tokens: 4_096,
        configuration_digest: digest("wire configuration"),
    };
    assert_eq!(wire.schema_version(), 1);
    assert_eq!(wire.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(wire.model_artifact_id().digest(), &digest("model"));
    assert_eq!(wire.request_binding_digest(), &digest("request"));
    assert_eq!(wire.response_binding_digest(), &digest("response"));
    assert_eq!(wire.effective_context_tokens(), 4_096);
    assert_eq!(wire.configuration_digest(), &digest("wire configuration"));
    assert!(!wire.live_use_authorized());
    assert!(!wire.complete_effective_configuration());

    let platform = LinuxPlatformFrameworkEvidence {
        schema_version: 1,
        runtime_build_id: build.runtime_build_id(),
        target: linux_target(),
        kernel_observation_digest: digest("kernel"),
        worker_portable_closure_digest: digest("portable native closure"),
        worker_native_load_digest: digest("native load"),
        driver_evidence: PlatformDriverEvidenceClass::NotApplicableForReviewedNativeCpuProfile,
        platform_digest: digest("platform"),
    };
    assert_eq!(platform.schema_version(), 1);
    assert_eq!(platform.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(platform.target(), linux_target());
    assert_eq!(platform.kernel_observation_digest(), &digest("kernel"));
    assert_eq!(
        platform.worker_portable_closure_digest(),
        &digest("portable native closure")
    );
    assert_eq!(platform.worker_native_load_digest(), &digest("native load"));
    assert_eq!(
        platform.driver_evidence(),
        PlatformDriverEvidenceClass::NotApplicableForReviewedNativeCpuProfile
    );
    assert_eq!(platform.platform_digest(), &digest("platform"));
    assert!(!platform.accelerator_driver_absence_proven());

    let cpu = OllamaCpuExecutionEvidence {
        schema_version: 1,
        model_artifact_id: ArtifactId::from_digest(digest("model")),
        isolation_evidence_digest: digest("isolation"),
        worker_evidence_digest: digest("worker"),
        worker_portable_configuration_digest: digest("worker configuration"),
        worker_portable_closure_digest: digest("portable native closure"),
        worker_native_load_digest: digest("native load"),
        model_mapping_digest: digest("mapping"),
        residency_observation_digest: digest("residency"),
        effective_context_tokens: 4_096,
        compute_backend: ComputeBackend::NativeCpu,
        placement: ExecutionPlacement::CpuOnly,
        execution_class_digest: digest("execution class"),
        observation_binding_digest: digest("CPU observation"),
    };
    assert_eq!(cpu.schema_version(), 1);
    assert_eq!(cpu.model_artifact_id().digest(), &digest("model"));
    assert_eq!(
        cpu.worker_portable_configuration_digest(),
        &digest("worker configuration")
    );
    assert_eq!(
        cpu.worker_portable_closure_digest(),
        &digest("portable native closure")
    );
    assert_eq!(cpu.effective_context_tokens(), 4_096);
    assert_eq!(cpu.compute_backend(), ComputeBackend::NativeCpu);
    assert_eq!(cpu.placement(), ExecutionPlacement::CpuOnly);
    assert_eq!(cpu.execution_class_digest(), &digest("execution class"));
    assert_eq!(cpu.observation_binding_digest(), &digest("CPU observation"));
    assert!(!cpu.formal_placement_proven());
    assert!(!cpu.model_use_proven());
    assert!(!cpu.application_handler_proven());
    assert!(!cpu.qualified());

    for encoded in [
        serde_json::to_string(&wire).expect("wire JSON"),
        serde_json::to_string(&platform).expect("platform JSON"),
        serde_json::to_string(&cpu).expect("CPU JSON"),
    ] {
        assert!(!encoded.contains("\"qualified\":true"));
    }
}

#[test]
fn exact_runtime_profile_rejects_each_tuple_substitution() {
    let valid = runtime_build(
        "ollama",
        "0.32.15",
        RuntimeBuildMode::ManagedProcess,
        linux_target(),
    );
    assert!(valid_runtime_profile(&valid));

    let windows_target = RuntimeTarget::new(
        RuntimeOperatingSystem::Windows,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::WindowsMsvc,
    )
    .expect("Windows target");
    for changed in [
        runtime_build(
            "other",
            "0.32.15",
            RuntimeBuildMode::ManagedProcess,
            linux_target(),
        ),
        runtime_build(
            "ollama",
            "0.32.14",
            RuntimeBuildMode::ManagedProcess,
            linux_target(),
        ),
        runtime_build(
            "ollama",
            "0.32.15",
            RuntimeBuildMode::AttachedAttestedProcess,
            linux_target(),
        ),
        runtime_build(
            "ollama",
            "0.32.15",
            RuntimeBuildMode::ManagedProcess,
            windows_target,
        ),
    ] {
        assert!(!valid_runtime_profile(&changed));
    }
}

#[test]
fn provider_relationship_substitution_matrix_fails_closed() {
    let cases = (0..18).map(|case| {
        let mut facts = common_facts();
        match case {
            0 => facts.runtime_profile_valid = false,
            1 => facts.model_input_schema_valid = false,
            2 => facts.model_installation_generation = 0,
            3 => facts.input_member_count = 0,
            4 => facts.input_total_bytes = 0,
            5 => facts.request_model_digest = digest("other model"),
            6 => facts.binding_model_digest = digest("other model"),
            7 => facts.binding_reference_digest = digest("other reference"),
            8 => facts.receipt_reference_digest = digest("other reference"),
            9 => facts.receipt_inventory_digest = digest("other inventory"),
            10 => facts.request_valid = false,
            11 => facts.receipt_request_digest = digest("other request"),
            12 => facts.request_context_tokens = 0,
            13 => facts.receipt_context_tokens = 8_192,
            14 => facts.receipt_first_residency = facts.receipt_first_response,
            15 => facts.receipt_last_residency -= 1,
            16 => facts.receipt_first_residency = facts.receipt_last_residency,
            17 => facts.residency_claims_valid = false,
            _ => unreachable!("bounded substitution case"),
        }
        facts
    });
    for facts in cases {
        assert!(validate_common(&facts).is_err());
    }
}

#[test]
fn cpu_execution_substitution_matrix_fails_closed() {
    let cases = (0..20).map(|case| {
        let mut facts = cpu_facts();
        match case {
            0 => facts.request_model_digest = digest("other model"),
            1 => facts.worker_model_digest = digest("other model"),
            2 => facts.mapping_model_digest = digest("other model"),
            3 => facts.isolation_layout_digest = digest("other layout"),
            4 => facts.input_member_count = 0,
            5 => facts.isolation_member_count += 1,
            6 => facts.input_total_bytes = 0,
            7 => facts.isolation_total_bytes += 1,
            8 => facts.stable_isolation = false,
            9 => facts.isolation_canaries_valid = false,
            10 => facts.stable_worker = false,
            11 => facts.worker_profile_valid = false,
            12 => facts.worker_native_components = 0,
            13 => facts.model_mapping_regions = 0,
            14 => facts.receipt_request_digest = digest("other request"),
            15 => facts.request_context_tokens = 0,
            16 => facts.receipt_context_tokens = 8_192,
            17 => facts.runtime_reported_accelerator_bytes = 1,
            18 => facts.residency_claims_valid = false,
            19 => facts.isolation_member_count = 0,
            _ => unreachable!("bounded substitution case"),
        }
        facts
    });
    for facts in cases {
        assert!(validate_cpu(&facts).is_err());
    }
}

#[test]
fn kernel_fields_are_bounded_canonical_text() {
    assert!(fields_valid(&[
        b"Linux".to_vec(),
        b"6.12.0".to_vec(),
        b"#1 SMP PREEMPT_DYNAMIC".to_vec(),
    ]));
    assert!(!field_valid(b""));
    assert!(!field_valid(b"Linux\n"));
    assert!(!field_valid(&vec![b'x'; 4_097]));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one platform matrix covers kernel, closure, target, and malformed field substitutions"
)]
fn platform_constructor_binds_kernel_target_and_native_framework() {
    let build = runtime_build(
        "ollama",
        "0.32.15",
        RuntimeBuildMode::ManagedProcess,
        linux_target(),
    );
    let fields = vec![
        b"Linux".to_vec(),
        b"6.12.0".to_vec(),
        b"#1 SMP PREEMPT_DYNAMIC".to_vec(),
    ];
    let evidence = build_fixture(
        &build,
        fields.clone(),
        digest("portable native closure"),
        digest("native load"),
        4,
    )
    .expect("platform evidence");
    let other_attempt = build_fixture(
        &build,
        fields.clone(),
        digest("portable native closure"),
        digest("other native load"),
        5,
    )
    .expect("other attempt");
    assert_eq!(evidence.runtime_build_id(), &build.runtime_build_id());
    assert_eq!(evidence.target(), linux_target());
    assert_eq!(
        evidence.worker_portable_closure_digest(),
        &digest("portable native closure")
    );
    assert_eq!(evidence.worker_native_load_digest(), &digest("native load"));
    assert_eq!(
        evidence.driver_evidence(),
        PlatformDriverEvidenceClass::NotApplicableForReviewedNativeCpuProfile
    );
    assert!(!evidence.accelerator_driver_absence_proven());
    assert_eq!(evidence.platform_digest(), other_attempt.platform_digest());
    let other_kernel = build_fixture(
        &build,
        vec![
            b"Linux".to_vec(),
            b"6.13.0".to_vec(),
            b"#2 SMP PREEMPT_DYNAMIC".to_vec(),
        ],
        digest("portable native closure"),
        digest("other native load"),
        5,
    )
    .expect("other kernel observation");
    assert_eq!(evidence.platform_digest(), other_kernel.platform_digest());
    assert_ne!(
        evidence.kernel_observation_digest(),
        other_kernel.kernel_observation_digest()
    );
    let other_closure = build_fixture(
        &build,
        fields.clone(),
        digest("other portable closure"),
        digest("other native load"),
        5,
    )
    .expect("other closure");
    assert_ne!(evidence.platform_digest(), other_closure.platform_digest());

    assert!(
        build_fixture(
            &build,
            fields.clone(),
            digest("portable native closure"),
            digest("native load"),
            0,
        )
        .is_err()
    );
    let alternate_targets = [
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
            RuntimeArchitecture::Aarch64,
            RuntimeAbi::LinuxMusl,
        ),
    ];
    for (operating_system, architecture, abi) in alternate_targets {
        let target = RuntimeTarget::new(operating_system, architecture, abi).expect("target");
        let alternate_build = runtime_build(
            "ollama",
            "0.32.15",
            RuntimeBuildMode::ManagedProcess,
            target,
        );
        let alternate = build_fixture(
            &alternate_build,
            fields.clone(),
            digest("portable native closure"),
            digest("native load"),
            4,
        )
        .expect("bounded alternate fixture");
        assert_ne!(evidence.platform_digest(), alternate.platform_digest());
    }
    let mut changed = fields;
    changed[0].clear();
    assert!(
        build_fixture(
            &build,
            changed,
            digest("portable native closure"),
            digest("native load"),
            4,
        )
        .is_err()
    );
}
