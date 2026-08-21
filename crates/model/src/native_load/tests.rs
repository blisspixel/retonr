use rewrite_types::Digest;

use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend,
    EffectiveRuntimeState, EffectiveRuntimeStateError, EffectiveRuntimeStateFromLoadInput,
    ExecutionPlacement, PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi,
    RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildMode, RuntimeOperatingSystem,
    RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};

use super::{
    MAX_NATIVE_LOAD_COMPONENTS, MAX_NATIVE_LOAD_OBSERVATION_JSON_BYTES, NativeLoadEvidenceClass,
    NativeLoadObservation, NativeLoadObservationError, NativeLoadObservationInput,
    NativeLoadOrigin, NativeLoadVisibilityScope, NativeLoadedComponent, NativeMappingClass,
};

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn runtime_members() -> Vec<RuntimePackageMember> {
    vec![
        RuntimePackageMember::new(
            artifact("entrypoint"),
            10,
            path("bin/runtime"),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        RuntimePackageMember::new(
            artifact("license"),
            11,
            path("legal/license.txt"),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("provenance"),
            12,
            path("legal/provenance.txt"),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("conditional"),
            13,
            path("lib/conditional.so"),
            vec![RuntimePackageMemberRole::NativeDependency],
            RuntimePackageLoadPolicy::BackendConditional,
        ),
        RuntimePackageMember::new(
            artifact("required"),
            14,
            path("lib/required.so"),
            vec![RuntimePackageMemberRole::NativeDependency],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
    ]
}

fn runtime_package(version: &str) -> RuntimePackageManifest {
    runtime_package_for(
        version,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("valid target"),
    )
}

fn runtime_package_for(version: &str, target: RuntimeTarget) -> RuntimePackageManifest {
    let members = runtime_members();
    let artifact_set = ArtifactSetManifest::new(
        members
            .iter()
            .map(|member| {
                ArtifactSetMember::new(
                    member.artifact_id().clone(),
                    member.byte_size(),
                    member.relative_path().clone(),
                )
            })
            .collect(),
    )
    .expect("valid set");
    RuntimePackageManifest::new(
        &artifact_set,
        "fixture-runtime",
        version,
        None,
        target,
        PackageSource::new(
            PackageSourceKind::UpstreamRelease,
            "https://example.invalid/runtime",
            version,
            Digest::sha256(b"provenance"),
        )
        .expect("valid source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"untransformed"),
        },
        members,
    )
    .expect("valid runtime package")
}

fn packaged(
    name: &str,
    byte_size: u64,
    relative_path: &str,
    mapping_class: NativeMappingClass,
) -> NativeLoadedComponent {
    NativeLoadedComponent::new(
        artifact(name),
        byte_size,
        NativeLoadOrigin::PackagedMember {
            relative_path: path(relative_path),
        },
        mapping_class,
        Digest::sha256(format!("evidence:{name}").as_bytes()),
    )
}

fn components() -> Vec<NativeLoadedComponent> {
    vec![
        packaged(
            "entrypoint",
            10,
            "bin/runtime",
            NativeMappingClass::ExecutableImage,
        ),
        packaged(
            "required",
            14,
            "lib/required.so",
            NativeMappingClass::ExecutableMapped,
        ),
        NativeLoadedComponent::new(
            artifact("libc"),
            15,
            NativeLoadOrigin::ExternalPlatformComponent,
            NativeMappingClass::ExecutableMapped,
            Digest::sha256(b"external evidence"),
        ),
    ]
}

fn input(components: Vec<NativeLoadedComponent>) -> NativeLoadObservationInput {
    NativeLoadObservationInput {
        evidence_class: NativeLoadEvidenceClass::LinuxProcMapFiles,
        visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
        process_evidence_digest: Digest::sha256(b"process"),
        observation_contract_id: "linux-proc-map-files".to_owned(),
        observation_contract_schema_version: 1,
        components,
    }
}

fn observation(package: &RuntimePackageManifest) -> NativeLoadObservation {
    NativeLoadObservation::new(package, input(components())).expect("valid observation")
}

#[test]
fn observation_has_stable_identity_and_complete_accessors() {
    let package = runtime_package("1.0.0");
    let observation = observation(&package);
    assert_eq!(observation.schema_version(), 1);
    assert_eq!(
        observation.runtime_package_manifest_id(),
        &package.runtime_package_manifest_id()
    );
    assert_eq!(
        observation.evidence_class(),
        NativeLoadEvidenceClass::LinuxProcMapFiles
    );
    assert_eq!(
        observation.visibility_scope(),
        NativeLoadVisibilityScope::FileBackedExecutableMappings
    );
    assert_eq!(
        observation.process_evidence_digest(),
        &Digest::sha256(b"process")
    );
    assert_eq!(
        observation.observation_contract_id(),
        "linux-proc-map-files"
    );
    assert_eq!(observation.observation_contract_schema_version(), 1);
    assert_eq!(observation.components().len(), 3);
    let component = &observation.components()[0];
    assert_eq!(component.artifact_id(), &artifact("entrypoint"));
    assert_eq!(component.byte_size(), 10);
    assert_eq!(
        component.mapping_class(),
        NativeMappingClass::ExecutableImage
    );
    assert_eq!(
        component.object_evidence_digest(),
        &Digest::sha256(b"evidence:entrypoint")
    );
    assert_eq!(
        observation.native_load_observation_id().digest().as_str(),
        "1b639229c39371c7538107d3cc683bb6ef51cb8caa8e4f5f2dc7c9b5b59b20ef"
    );
}

#[test]
fn observation_round_trips_and_rejects_bounded_encoding_drift() {
    let package = runtime_package("1.0.0");
    let observation = observation(&package);
    let encoded = serde_json::to_vec(&observation).expect("serialize");
    assert_eq!(
        NativeLoadObservation::from_json_bytes(&encoded, &package).expect("decode"),
        observation
    );
    let mut value = serde_json::to_value(&observation).expect("value");
    value["schema_version"] = serde_json::json!(2);
    assert_eq!(
        NativeLoadObservation::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &package
        ),
        Err(NativeLoadObservationError::UnsupportedSchema(2))
    );
    value["schema_version"] = serde_json::json!(1);
    value["extra"] = serde_json::json!(true);
    assert_eq!(
        NativeLoadObservation::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &package
        ),
        Err(NativeLoadObservationError::InvalidEncoding)
    );
    let mut unsupported = serde_json::to_value(&observation).expect("value");
    unsupported["evidence_class"] = serde_json::json!("windows_virtual_memory_map");
    assert_eq!(
        NativeLoadObservation::from_json_bytes(
            &serde_json::to_vec(&unsupported).expect("encode"),
            &package
        ),
        Err(NativeLoadObservationError::InvalidEncoding)
    );
    assert_eq!(
        NativeLoadObservation::from_json_bytes(
            &vec![b' '; MAX_NATIVE_LOAD_OBSERVATION_JSON_BYTES + 1],
            &package
        ),
        Err(NativeLoadObservationError::EncodedObservationTooLarge)
    );
    let other = runtime_package("2.0.0");
    assert_eq!(
        NativeLoadObservation::from_json_bytes(&encoded, &other),
        Err(NativeLoadObservationError::PackagedComponentMismatch)
    );
}

#[test]
fn observation_rejects_missing_mismatched_forbidden_and_unordered_components() {
    let package = runtime_package("1.0.0");
    assert_eq!(
        NativeLoadObservation::new(&package, input(Vec::new())),
        Err(NativeLoadObservationError::InvalidComponentCount)
    );
    let excessive = (0..=MAX_NATIVE_LOAD_COMPONENTS)
        .map(|index| {
            NativeLoadedComponent::new(
                artifact(&format!("external-{index}")),
                index as u64,
                NativeLoadOrigin::ExternalPlatformComponent,
                NativeMappingClass::ExecutableMapped,
                Digest::sha256(&index.to_be_bytes()),
            )
        })
        .collect();
    assert_eq!(
        NativeLoadObservation::new(&package, input(excessive)),
        Err(NativeLoadObservationError::InvalidComponentCount)
    );
    let mut missing = components();
    missing.remove(1);
    assert_eq!(
        NativeLoadObservation::new(&package, input(missing)),
        Err(NativeLoadObservationError::MissingRequiredComponent)
    );
    let mut mismatch = components();
    mismatch[1] = packaged(
        "wrong",
        14,
        "lib/required.so",
        NativeMappingClass::ExecutableMapped,
    );
    assert_eq!(
        NativeLoadObservation::new(&package, input(mismatch)),
        Err(NativeLoadObservationError::PackagedComponentMismatch)
    );
    let mut unordered = components();
    unordered.swap(0, 1);
    assert_eq!(
        NativeLoadObservation::new(&package, input(unordered)),
        Err(NativeLoadObservationError::InvalidComponentOrder)
    );
    let mut forbidden = components();
    forbidden.insert(
        1,
        packaged(
            "license",
            11,
            "legal/license.txt",
            NativeMappingClass::ExecutableMapped,
        ),
    );
    assert_eq!(
        NativeLoadObservation::new(&package, input(forbidden)),
        Err(NativeLoadObservationError::LoadPolicyViolation)
    );
}

#[test]
fn observation_rejects_duplicate_origin_mapping_and_contract_drift() {
    let package = runtime_package("1.0.0");
    let mut duplicate = components();
    duplicate.insert(
        1,
        packaged(
            "entrypoint",
            10,
            "bin/runtime",
            NativeMappingClass::ExecutableMapped,
        ),
    );
    assert_eq!(
        NativeLoadObservation::new(&package, input(duplicate)),
        Err(NativeLoadObservationError::InvalidComponentOrder)
    );
    let mut wrong_mapping = components();
    wrong_mapping[0] = packaged(
        "entrypoint",
        10,
        "bin/runtime",
        NativeMappingClass::ExecutableMapped,
    );
    assert_eq!(
        NativeLoadObservation::new(&package, input(wrong_mapping)),
        Err(NativeLoadObservationError::LoadPolicyViolation)
    );
    let mut invalid_metadata = input(components());
    invalid_metadata.observation_contract_id = "Bad Contract".to_owned();
    invalid_metadata.observation_contract_schema_version = 0;
    assert_eq!(
        NativeLoadObservation::new(&package, invalid_metadata),
        Err(NativeLoadObservationError::InvalidMetadata)
    );
}

#[test]
fn evidence_class_scope_and_executable_image_rules_are_exact() {
    let linux_package = runtime_package("1.0.0");
    let windows_package = runtime_package_for(
        "1.0.0",
        RuntimeTarget::new(
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsMsvc,
        )
        .expect("valid target"),
    );
    assert_eq!(
        NativeLoadObservation::new(&windows_package, input(components())),
        Err(NativeLoadObservationError::EvidenceClassTargetMismatch)
    );
    let mut outside_scope = components();
    outside_scope[2] = NativeLoadedComponent::new(
        artifact("libc"),
        15,
        NativeLoadOrigin::ExternalPlatformComponent,
        NativeMappingClass::DataMapped,
        Digest::sha256(b"external evidence"),
    );
    assert_eq!(
        NativeLoadObservation::new(&linux_package, input(outside_scope)),
        Err(NativeLoadObservationError::VisibilityScopeViolation)
    );
    let mut second_image = components();
    second_image[1] = packaged(
        "required",
        14,
        "lib/required.so",
        NativeMappingClass::ExecutableImage,
    );
    assert_eq!(
        NativeLoadObservation::new(&linux_package, input(second_image)),
        Err(NativeLoadObservationError::LoadPolicyViolation)
    );
    let mut data_dependency = components();
    data_dependency[1] = packaged(
        "required",
        14,
        "lib/required.so",
        NativeMappingClass::DataMapped,
    );
    let mut data_input = input(data_dependency);
    data_input.visibility_scope = NativeLoadVisibilityScope::FileBackedMappings;
    assert_eq!(
        NativeLoadObservation::new(&linux_package, data_input),
        Err(NativeLoadObservationError::LoadPolicyViolation)
    );
    let mut external_image = components();
    external_image[2] = NativeLoadedComponent::new(
        artifact("libc"),
        15,
        NativeLoadOrigin::ExternalPlatformComponent,
        NativeMappingClass::ExecutableImage,
        Digest::sha256(b"external evidence"),
    );
    assert_eq!(
        NativeLoadObservation::new(&linux_package, input(external_image)),
        Err(NativeLoadObservationError::LoadPolicyViolation)
    );
}

#[test]
fn observation_decoder_rejects_invalid_packaged_paths_and_absent_members() {
    let package = runtime_package("1.0.0");
    let observation = observation(&package);
    let mut value = serde_json::to_value(&observation).expect("value");
    value["components"][0]["origin"]["relative_path"] = serde_json::json!("../runtime");
    assert_eq!(
        NativeLoadObservation::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &package
        ),
        Err(NativeLoadObservationError::InvalidMemberPath)
    );

    let mut absent = components();
    absent.insert(
        1,
        packaged(
            "missing",
            1,
            "lib/missing.so",
            NativeMappingClass::ExecutableMapped,
        ),
    );
    assert_eq!(
        NativeLoadObservation::new(&package, input(absent)),
        Err(NativeLoadObservationError::PackagedComponentMismatch)
    );
}

#[test]
fn typed_builders_bind_package_load_and_existing_v1_state_fields() {
    let package = runtime_package("1.0.0");
    let load_observation = observation(&package);
    let build =
        RuntimeBuildIdentity::new_from_package_manifest(RuntimeBuildMode::ManagedProcess, &package)
            .expect("typed build");
    let state = EffectiveRuntimeState::new_from_load_observation(
        &build,
        &load_observation,
        effective_input(),
    )
    .expect("typed state");
    assert_eq!(
        state.loaded_components_digest(),
        load_observation.native_load_observation_id().digest()
    );

    let other_package = runtime_package("2.0.0");
    let other_observation = observation(&other_package);
    assert_eq!(
        EffectiveRuntimeState::new_from_load_observation(
            &build,
            &other_observation,
            effective_input()
        ),
        Err(EffectiveRuntimeStateError::LoadObservationBuildMismatch)
    );
}

fn effective_input() -> EffectiveRuntimeStateFromLoadInput {
    EffectiveRuntimeStateFromLoadInput {
        provider_snapshot_contract: "ollama-provider-snapshot".to_owned(),
        provider_snapshot_schema_version: 1,
        provider_snapshot_digest: Digest::sha256(b"provider"),
        launch_policy_digest: Digest::sha256(b"launch"),
        effective_configuration_digest: Digest::sha256(b"configuration"),
        platform_digest: Digest::sha256(b"platform"),
        execution_class_digest: Digest::sha256(b"execution"),
        isolation_policy_digest: Digest::sha256(b"isolation"),
        effective_context_tokens: 4_096,
        compute_backend: ComputeBackend::NativeCpu,
        placement: ExecutionPlacement::CpuOnly,
    }
}
