use rewrite_model::{RuntimeBuildMode, RuntimePackageManifest};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_types::Digest;

use super::{
    LOCAL_OLLAMA_MANAGED_BUILD_BINDING_SCHEMA_VERSION, LocalOllamaManagedBuildEvidenceClass,
    LocalOllamaManagedPreflightOutcome, MISSING_EFFECTIVE_STATE_RELATIONSHIPS,
    bind_successful_managed_preflight,
};
use crate::{
    LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION, LocalOllamaBoundPreflightPlan,
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightReport,
    LocalOllamaManagedProcessEvidenceLevel,
    local_ollama_managed_preflight::test_support::{
        connection, native_load, package, plan, preflight, process,
    },
};

fn report(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
) -> LocalOllamaManagedPreflightReport {
    let process = process();
    let connection = connection(&process);
    let helper = &package.members()[1];
    LocalOllamaManagedPreflightReport {
        schema_version: LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION,
        plan_digest: Digest::sha256(&serde_json::to_vec(plan).expect("plan")),
        binding_digest: Digest::sha256(b"managed report"),
        runtime_package_manifest_id: package.runtime_package_manifest_id(),
        artifact_set_id: package.artifact_set_id().clone(),
        package_attestation_digest: Digest::sha256(b"package attestation"),
        isolation_policy_digest: Digest::sha256(b"isolation policy"),
        launch_spec_digest: Digest::sha256(b"launch"),
        helper_member_artifact_id: helper.artifact_id().clone(),
        helper_member_relative_path: helper.relative_path().clone(),
        helper_member_bytes: helper.byte_size(),
        initial_isolation_evidence_digest: Digest::sha256(b"isolation"),
        final_isolation_evidence_digest: Digest::sha256(b"isolation"),
        process_evidence_level:
            LocalOllamaManagedProcessEvidenceLevel::ManagedLinuxIsolationSockDiag,
        initial_process_witness: process.clone(),
        post_preflight_process_witness: process.clone(),
        final_process_witness: process.clone(),
        connection_witness: connection.clone(),
        connection_observations: std::iter::repeat_n(connection, 8).collect(),
        startup_standard_output_digest: Digest::sha256(b""),
        startup_standard_error_digest: Digest::sha256(b"marker"),
        startup_standard_output_bytes: 0,
        startup_standard_error_bytes: 6,
        cloud_disable_version_status: OllamaCloudDisableVersionStatus::Unreviewed,
        cloud_disable_environment_observed: true,
        cloud_disable_startup_marker_observed: true,
        cloud_disable_runtime_reviewed: false,
        operating_system_network_isolation_enforced: true,
        preflight: preflight(plan),
        native_load: native_load(package, &process),
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_around_every_response: true,
        application_handler_proven: false,
        exclusive_socket_owner_proven: false,
        model_loaded_or_used_proven: false,
        effective_runtime_identity_proven: false,
        qualified: false,
    }
}

#[test]
fn successful_join_constructs_only_the_point_in_time_build_identity() {
    let package = package();
    let plan = plan(&package);
    let report = report(&package, &plan);
    let binding = bind_successful_managed_preflight(&package, &plan, &report).expect("binding");

    assert_eq!(
        binding.schema_version(),
        LOCAL_OLLAMA_MANAGED_BUILD_BINDING_SCHEMA_VERSION
    );
    assert_eq!(
        binding.evidence_class(),
        LocalOllamaManagedBuildEvidenceClass::ManagedLinuxPreflightPackageProcessLoad
    );
    assert_eq!(
        binding.managed_preflight_binding_digest(),
        &report.binding_digest
    );
    assert_eq!(
        binding.native_load_observation_digest(),
        report.native_load.native_load_observation_id().digest()
    );
    assert_eq!(
        binding.runtime_build().mode(),
        RuntimeBuildMode::ManagedProcess
    );
    assert_eq!(
        binding.runtime_build().package_manifest_digest(),
        package.runtime_package_manifest_id().digest()
    );
    assert!(binding.package_declared_runtime_build_identity_constructed());
    assert!(!binding.process_retained_after_return());
    assert!(!binding.effective_runtime_state_proven());
    assert!(!binding.model_loaded_or_used_proven());
    assert!(!binding.application_handler_proven());
    assert!(!binding.qualified());
    assert_eq!(
        binding.missing_effective_state_relationships(),
        MISSING_EFFECTIVE_STATE_RELATIONSHIPS
    );
    assert_eq!(
        binding.binding_digest(),
        bind_successful_managed_preflight(&package, &plan, &report)
            .expect("repeat")
            .binding_digest()
    );

    let encoded = serde_json::to_string(&binding).expect("binding JSON");
    assert!(encoded.contains("package_declared_runtime_build_identity_constructed"));
    assert!(!encoded.contains("runtime_build_identity_proven"));
    assert!(!encoded.contains("example.invalid"));
    assert!(!encoded.contains("127.0.0.1"));
    assert!(!encoded.contains("fixture:latest"));

    let outcome = LocalOllamaManagedPreflightOutcome::new(report.clone(), binding.clone());
    assert_eq!(outcome.report(), &report);
    assert_eq!(outcome.build_binding(), &binding);
    let (returned_report, returned_binding) = outcome.into_parts();
    assert_eq!(returned_report, report);
    assert_eq!(returned_binding, binding);
}

#[test]
fn package_plan_process_and_native_load_drift_fail_closed() {
    let runtime_package = package();
    let plan = plan(&runtime_package);
    let baseline = report(&runtime_package, &plan);

    let other_package = package();
    let mut wrong_plan = plan.clone();
    wrong_plan.preflight.plan_id = "different-plan".to_owned();
    assert!(matches!(
        bind_successful_managed_preflight(&other_package, &wrong_plan, &baseline),
        Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
    ));

    let mut wrong_process = baseline.clone();
    wrong_process.final_process_witness = process_with_different_entrypoint();
    assert!(matches!(
        bind_successful_managed_preflight(&runtime_package, &plan, &wrong_process),
        Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
    ));

    let other = package_with_version("0.16.3");
    let mut wrong_load = baseline.clone();
    wrong_load.native_load = native_load(&other, &wrong_load.initial_process_witness);
    assert!(matches!(
        bind_successful_managed_preflight(&runtime_package, &plan, &wrong_load),
        Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
    ));
    assert!(matches!(
        bind_successful_managed_preflight(&other, &plan, &baseline),
        Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
    ));
}

#[test]
fn positive_claim_fabrication_and_sequence_drift_fail_closed() {
    let runtime_package = package();
    let plan = plan(&runtime_package);
    let baseline = report(&runtime_package, &plan);

    let mut cases = Vec::new();
    let mut claimed_effective = baseline.clone();
    claimed_effective.effective_runtime_identity_proven = true;
    cases.push(claimed_effective);
    let mut claimed_handler = baseline.clone();
    claimed_handler.application_handler_proven = true;
    cases.push(claimed_handler);
    let mut claimed_model = baseline.clone();
    claimed_model.model_loaded_or_used_proven = true;
    cases.push(claimed_model);
    let mut claimed_qualified = baseline.clone();
    claimed_qualified.qualified = true;
    cases.push(claimed_qualified);
    let mut incomplete = baseline.clone();
    incomplete.connection_observations.pop();
    cases.push(incomplete);
    let mut unreviewed_conflict = baseline;
    unreviewed_conflict.cloud_disable_runtime_reviewed = true;
    cases.push(unreviewed_conflict);
    let mut legacy_backend = report(&runtime_package, &plan);
    legacy_backend.preflight.observed.runtime.backend = "ollama".to_owned();
    cases.push(legacy_backend);

    for candidate in cases {
        assert!(matches!(
            bind_successful_managed_preflight(&runtime_package, &plan, &candidate),
            Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
        ));
    }
}

fn process_with_different_entrypoint() -> rewrite_runtime_attestor::AttachedProcessEvidence {
    use rewrite_runtime_attestor::{AttachedProcessEvidence, AttachedProcessEvidenceInput};

    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class:
            rewrite_runtime_attestor::AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle,
        owner_pid: 42,
        process_instance_digest: Digest::sha256(b"process"),
        ownership_snapshot_digest: Digest::sha256(b"ownership"),
        entrypoint_object_digest: Digest::sha256(b"object"),
        entrypoint_digest: Digest::sha256(b"different entrypoint"),
        entrypoint_bytes: 11,
        platform_evidence_digest: Digest::sha256(b"platform"),
    })
    .expect("process")
}

fn package_with_version(version: &str) -> RuntimePackageManifest {
    use rewrite_model::{
        ArtifactSetManifest, ArtifactSetMember, PackageSource, PackageSourceKind,
        PackageTransformation,
    };

    let baseline = package();
    let set = ArtifactSetManifest::new(
        baseline
            .members()
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
    .expect("set");
    RuntimePackageManifest::new(
        &set,
        baseline.runtime_family(),
        version,
        baseline.build_revision().map(str::to_owned),
        baseline.target(),
        PackageSource::new(
            PackageSourceKind::UpstreamRelease,
            "https://example.invalid/ollama",
            format!("v{version}"),
            Digest::sha256(version.as_bytes()),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"same"),
        },
        baseline.members().to_vec(),
    )
    .expect("package")
}
