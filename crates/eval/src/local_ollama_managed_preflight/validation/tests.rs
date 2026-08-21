use std::time::Duration;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeMappingClass, PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageMember, RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_ollama::OllamaModelDetails;
use rewrite_runtime_attestor::{
    AttachedProcessWitnessLimits, ExpectedExternalNativeComponent, NativeLoadObservationLimits,
};
use rewrite_types::Digest;

use super::{
    exact_helper_member, helper_matches, valid_external_components, valid_managed_plan_binding,
    valid_native_limits, valid_process_limits, validate_cloud_observations,
};
use crate::{
    LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION, LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
    LocalOllamaBoundPreflightPlan, LocalOllamaManagedPreflightError, LocalOllamaModelPlan,
    LocalOllamaPreflightMode, LocalOllamaPreflightPlan,
};

fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn helper(
    roles: Vec<RuntimePackageMemberRole>,
    policy: RuntimePackageLoadPolicy,
) -> RuntimePackageMember {
    RuntimePackageMember::new(
        artifact("helper"),
        7,
        ArtifactSetRelativePath::new("bin/helper").expect("path"),
        roles,
        policy,
    )
}

fn package() -> RuntimePackageManifest {
    let members = vec![
        ArtifactSetMember::new(
            artifact("entrypoint"),
            11,
            ArtifactSetRelativePath::new("bin/ollama").expect("path"),
        ),
        ArtifactSetMember::new(
            artifact("helper"),
            7,
            ArtifactSetRelativePath::new("helper/isolation").expect("path"),
        ),
        ArtifactSetMember::new(
            artifact("license"),
            5,
            ArtifactSetRelativePath::new("legal/license").expect("path"),
        ),
        ArtifactSetMember::new(
            artifact("provenance"),
            9,
            ArtifactSetRelativePath::new("legal/provenance").expect("path"),
        ),
    ];
    let set = ArtifactSetManifest::new(members).expect("set");
    RuntimePackageManifest::new(
        &set,
        "ollama",
        "0.16.2",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        PackageSource::new(
            PackageSourceKind::UpstreamRelease,
            "https://example.invalid/ollama",
            "v0.16.2",
            Digest::sha256(b"source"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"same"),
        },
        vec![
            RuntimePackageMember::new(
                artifact("entrypoint"),
                11,
                ArtifactSetRelativePath::new("bin/ollama").expect("path"),
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                artifact("helper"),
                7,
                ArtifactSetRelativePath::new("helper/isolation").expect("path"),
                vec![RuntimePackageMemberRole::HelperExecutable],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            RuntimePackageMember::new(
                artifact("license"),
                5,
                ArtifactSetRelativePath::new("legal/license").expect("path"),
                vec![RuntimePackageMemberRole::LicenseText],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            RuntimePackageMember::new(
                artifact("provenance"),
                9,
                ArtifactSetRelativePath::new("legal/provenance").expect("path"),
                vec![RuntimePackageMemberRole::ProvenanceRecord],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("package")
}

fn plan(
    package: &RuntimePackageManifest,
    mode: LocalOllamaPreflightMode,
) -> LocalOllamaBoundPreflightPlan {
    let expected_details = (mode == LocalOllamaPreflightMode::Verify).then(|| OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        quantization: "Q4_K_M".to_owned(),
        capabilities: vec!["completion".to_owned()],
        license_digest: Digest::sha256(b"license"),
        template_digest: Digest::sha256(b"template"),
        metadata_digest: Digest::sha256(b"metadata"),
    });
    LocalOllamaBoundPreflightPlan {
        schema_version: LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION,
        preflight: LocalOllamaPreflightPlan {
            schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
            plan_id: "managed-fixture".to_owned(),
            mode,
            endpoint: "http://127.0.0.1:11434".to_owned(),
            expected_runtime_version: "0.16.2".to_owned(),
            require_idle: true,
            models: vec![LocalOllamaModelPlan {
                reference: "fixture:latest".to_owned(),
                inventory_digest: Digest::sha256(b"inventory"),
                expected_details,
            }],
        },
        maximum_entrypoint_bytes: 1024,
        maximum_session_body_bytes: 1024 * 1024,
        expected_entrypoint_digest: (mode == LocalOllamaPreflightMode::Verify)
            .then(|| package.entrypoint().artifact_id().digest().clone()),
    }
}

#[test]
fn helper_binding_rejects_entrypoint_alias_and_loadable_policy() {
    let digest = artifact("helper").digest().clone();
    assert!(helper_matches(
        &helper(
            vec![RuntimePackageMemberRole::HelperExecutable],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        &digest,
        7,
    ));
    assert!(!helper_matches(
        &helper(
            vec![
                RuntimePackageMemberRole::Entrypoint,
                RuntimePackageMemberRole::HelperExecutable,
            ],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        &digest,
        7,
    ));
    assert!(!helper_matches(
        &helper(
            vec![RuntimePackageMemberRole::HelperExecutable],
            RuntimePackageLoadPolicy::BackendConditional,
        ),
        &digest,
        7,
    ));
}

#[test]
fn exact_helper_and_verify_plan_bind_to_the_package() {
    let package = package();
    let helper =
        exact_helper_member(&package, artifact("helper").digest(), 7).expect("exact helper");
    assert_eq!(helper.relative_path().as_str(), "helper/isolation");
    assert!(matches!(
        exact_helper_member(&package, artifact("other").digest(), 7),
        Err(LocalOllamaManagedPreflightError::InvalidHelperBinding)
    ));
    let limits = AttachedProcessWitnessLimits {
        maximum_entrypoint_bytes: 1024,
        ..AttachedProcessWitnessLimits::default()
    };
    assert!(valid_managed_plan_binding(
        &package,
        &plan(&package, LocalOllamaPreflightMode::Verify),
        limits,
    ));
    assert!(!valid_managed_plan_binding(
        &package,
        &plan(&package, LocalOllamaPreflightMode::Observe),
        limits,
    ));
}

#[test]
fn cloud_observation_requires_exact_environment_complete_output_and_one_marker() {
    assert!(
        validate_cloud_observations(
            Some("1"),
            b"",
            b"Ollama cloud disabled: true\n",
            false,
            false,
        )
        .is_ok()
    );
    assert!(matches!(
        validate_cloud_observations(None, b"", b"", false, false),
        Err(LocalOllamaManagedPreflightError::CloudDisable(_))
    ));
    assert!(matches!(
        validate_cloud_observations(
            Some("1"),
            b"Ollama cloud disabled: true\n",
            b"",
            true,
            false,
        ),
        Err(LocalOllamaManagedPreflightError::TruncatedStartupOutput)
    ));
    assert!(matches!(
        validate_cloud_observations(
            Some("1"),
            b"Ollama cloud disabled: true\n",
            b"Ollama cloud disabled: true\n",
            false,
            false,
        ),
        Err(LocalOllamaManagedPreflightError::CloudDisable(_))
    ));
}

#[test]
fn public_limit_mirrors_fail_closed() {
    assert!(valid_process_limits(AttachedProcessWitnessLimits::default()));
    assert!(valid_native_limits(NativeLoadObservationLimits::default()));
    let process = AttachedProcessWitnessLimits {
        maximum_elapsed: Duration::ZERO,
        ..AttachedProcessWitnessLimits::default()
    };
    assert!(!valid_process_limits(process));
    let native = NativeLoadObservationLimits {
        maximum_components: 0,
        ..NativeLoadObservationLimits::default()
    };
    assert!(!valid_native_limits(native));
}

#[test]
fn external_component_policy_requires_canonical_executable_mappings() {
    let mut components = ["one", "two"]
        .map(|value| {
            ExpectedExternalNativeComponent::new(
                artifact(value),
                8,
                NativeMappingClass::ExecutableMapped,
            )
        })
        .to_vec();
    components.sort_by(|left, right| {
        left.artifact_id()
            .digest()
            .as_str()
            .cmp(right.artifact_id().digest().as_str())
    });
    assert!(valid_external_components(&components, 2));
    components.reverse();
    assert!(!valid_external_components(&components, 2));
    let invalid = [ExpectedExternalNativeComponent::new(
        artifact("data"),
        8,
        NativeMappingClass::DataMapped,
    )];
    assert!(!valid_external_components(&invalid, 2));
}
