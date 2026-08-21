use rewrite_app::RuntimePackageLease;
use rewrite_model::{
    ArtifactId, NativeLoadObservation, NativeMappingClass, RuntimePackageManifest,
};
use rewrite_ollama::OllamaCloudDisableVersionStatus;
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, ExpectedExternalNativeComponent, RetainedTcpConnectionEvidence,
};
use rewrite_runtime_isolation::{
    IsolationEvidence, LaunchSpec, ManagedStartupOutput, PreparedIsolation,
};
use rewrite_types::Digest;
use serde::Serialize;

use crate::{LocalOllamaBoundPreflightPlan, LocalOllamaPreflightReport};

use super::{
    LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION, LocalOllamaManagedPreflightError,
    LocalOllamaManagedPreflightLimits, LocalOllamaManagedPreflightReport,
    LocalOllamaManagedProcessEvidenceLevel,
};

pub(super) struct ManagedReportEvidenceDigests {
    package_attestation: Digest,
    isolation_policy: Digest,
    launch_spec: Digest,
    initial_isolation: Digest,
    final_isolation: Digest,
    startup_standard_output: Digest,
    startup_standard_error: Digest,
    startup_standard_output_bytes: u64,
    startup_standard_error_bytes: u64,
}

pub(super) fn report_evidence_digests(
    package_lease: &RuntimePackageLease,
    isolation: &PreparedIsolation,
    launch: &LaunchSpec,
    initial_isolation: &IsolationEvidence,
    final_isolation: &IsolationEvidence,
    startup_output: &ManagedStartupOutput,
) -> Result<ManagedReportEvidenceDigests, LocalOllamaManagedPreflightError> {
    Ok(ManagedReportEvidenceDigests {
        package_attestation: package_attestation_digest(package_lease),
        isolation_policy: isolation.policy_digest(),
        launch_spec: launch.redacted_digest(),
        initial_isolation: initial_isolation.redacted_digest(),
        final_isolation: final_isolation.redacted_digest(),
        startup_standard_output: Digest::sha256(startup_output.standard_output()),
        startup_standard_error: Digest::sha256(startup_output.standard_error()),
        startup_standard_output_bytes: u64::try_from(startup_output.standard_output().len())
            .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?,
        startup_standard_error_bytes: u64::try_from(startup_output.standard_error().len())
            .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "report construction binds the full ordered evidence join"
)]
pub(super) fn build_report(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    helper: &rewrite_model::RuntimePackageMember,
    external_components: &[ExpectedExternalNativeComponent],
    limits: LocalOllamaManagedPreflightLimits,
    digests: ManagedReportEvidenceDigests,
    initial_process_witness: AttachedProcessEvidence,
    post_preflight_process_witness: AttachedProcessEvidence,
    final_process_witness: AttachedProcessEvidence,
    connection_witness: RetainedTcpConnectionEvidence,
    connection_observations: Vec<RetainedTcpConnectionEvidence>,
    cloud_status: OllamaCloudDisableVersionStatus,
    preflight: LocalOllamaPreflightReport,
    native_load: NativeLoadObservation,
) -> Result<LocalOllamaManagedPreflightReport, LocalOllamaManagedPreflightError> {
    let plan_bytes = serde_json::to_vec(plan)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let plan_digest = Digest::sha256(&plan_bytes);
    let external_components_digest = external_components_digest(external_components)?;
    let limits_digest = limits_digest(limits);
    let runtime_package_manifest_id = package.runtime_package_manifest_id();
    let artifact_set_id = package.artifact_set_id().clone();
    let cloud_disable_runtime_reviewed = cloud_status == OllamaCloudDisableVersionStatus::Reviewed;

    let mut report = LocalOllamaManagedPreflightReport {
        schema_version: LOCAL_OLLAMA_MANAGED_PREFLIGHT_REPORT_SCHEMA_VERSION,
        plan_digest,
        binding_digest: Digest::sha256(b"pending"),
        runtime_package_manifest_id,
        artifact_set_id,
        package_attestation_digest: digests.package_attestation,
        isolation_policy_digest: digests.isolation_policy,
        launch_spec_digest: digests.launch_spec,
        helper_member_artifact_id: helper.artifact_id().clone(),
        helper_member_relative_path: helper.relative_path().clone(),
        helper_member_bytes: helper.byte_size(),
        initial_isolation_evidence_digest: digests.initial_isolation,
        final_isolation_evidence_digest: digests.final_isolation,
        process_evidence_level:
            LocalOllamaManagedProcessEvidenceLevel::ManagedLinuxIsolationSockDiag,
        initial_process_witness,
        post_preflight_process_witness,
        final_process_witness,
        connection_witness,
        connection_observations,
        startup_standard_output_digest: digests.startup_standard_output,
        startup_standard_error_digest: digests.startup_standard_error,
        startup_standard_output_bytes: digests.startup_standard_output_bytes,
        startup_standard_error_bytes: digests.startup_standard_error_bytes,
        cloud_disable_version_status: cloud_status,
        cloud_disable_environment_observed: true,
        cloud_disable_startup_marker_observed: true,
        cloud_disable_runtime_reviewed,
        operating_system_network_isolation_enforced: true,
        preflight,
        native_load,
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_around_every_response: true,
        application_handler_proven: false,
        exclusive_socket_owner_proven: false,
        model_loaded_or_used_proven: false,
        effective_runtime_identity_proven: false,
        qualified: false,
    };
    report.binding_digest =
        report_binding_digest(&report, &external_components_digest, &limits_digest)?;
    Ok(report)
}

fn package_attestation_digest(lease: &RuntimePackageLease) -> Digest {
    let evidence = lease.evidence();
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(b"retonr:local-ollama-managed-package-attestation:v1\0");
    bytes.extend_from_slice(evidence.artifact_set_id().digest().as_str().as_bytes());
    bytes.extend_from_slice(
        evidence
            .runtime_package_manifest_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    bytes.extend_from_slice(
        evidence
            .entrypoint_artifact_id()
            .digest()
            .as_str()
            .as_bytes(),
    );
    bytes.extend_from_slice(&evidence.code_member_count().to_be_bytes());
    bytes.extend_from_slice(&evidence.code_byte_size().to_be_bytes());
    Digest::sha256(&bytes)
}

fn external_components_digest(
    components: &[ExpectedExternalNativeComponent],
) -> Result<Digest, LocalOllamaManagedPreflightError> {
    #[derive(Serialize)]
    struct External<'a> {
        artifact_id: &'a ArtifactId,
        byte_size: u64,
        mapping_class: NativeMappingClass,
    }
    let material = components
        .iter()
        .map(|component| External {
            artifact_id: component.artifact_id(),
            byte_size: component.byte_size(),
            mapping_class: component.mapping_class(),
        })
        .collect::<Vec<_>>();
    let encoded = serde_json::to_vec(&material)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut bytes = b"retonr:local-ollama-managed-external-components:v1\0".to_vec();
    bytes.extend_from_slice(&encoded);
    Ok(Digest::sha256(&bytes))
}

fn limits_digest(limits: LocalOllamaManagedPreflightLimits) -> Digest {
    let mut bytes = Vec::with_capacity(160);
    bytes.extend_from_slice(b"retonr:local-ollama-managed-limits:v1\0");
    for value in [
        u64::try_from(limits.process.maximum_socket_table_bytes).unwrap_or(u64::MAX),
        u64::try_from(limits.process.maximum_socket_table_entries).unwrap_or(u64::MAX),
        u64::try_from(limits.process.maximum_processes).unwrap_or(u64::MAX),
        u64::try_from(limits.process.maximum_descriptors_per_process).unwrap_or(u64::MAX),
        limits.process.maximum_entrypoint_bytes,
        u64::try_from(limits.native_load.maximum_mapping_regions).unwrap_or(u64::MAX),
        u64::try_from(limits.native_load.maximum_mapping_metadata_bytes).unwrap_or(u64::MAX),
        u64::try_from(limits.native_load.maximum_components).unwrap_or(u64::MAX),
        limits.native_load.maximum_aggregate_hash_bytes,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    for elapsed in [
        limits.process.maximum_elapsed,
        limits.native_load.maximum_elapsed,
    ] {
        bytes.extend_from_slice(&elapsed.as_secs().to_be_bytes());
        bytes.extend_from_slice(&elapsed.subsec_nanos().to_be_bytes());
    }
    Digest::sha256(&bytes)
}

fn report_binding_digest(
    report: &LocalOllamaManagedPreflightReport,
    external_components_digest: &Digest,
    limits_digest: &Digest,
) -> Result<Digest, LocalOllamaManagedPreflightError> {
    let mut canonical = report.clone();
    canonical.binding_digest = Digest::sha256(b"binding-field-excluded");
    let encoded = serde_json::to_vec(&canonical)
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let mut bytes = b"retonr:local-ollama-managed-preflight-binding:v1\0".to_vec();
    bytes.extend_from_slice(external_components_digest.as_str().as_bytes());
    bytes.extend_from_slice(limits_digest.as_str().as_bytes());
    bytes.extend_from_slice(&encoded);
    Ok(Digest::sha256(&bytes))
}

#[cfg(test)]
mod tests {
    use rewrite_ollama::OllamaCloudDisableVersionStatus;
    use rewrite_runtime_attestor::{AttachedProcessWitnessLimits, NativeLoadObservationLimits};
    use rewrite_types::Digest;

    use super::{ManagedReportEvidenceDigests, build_report, limits_digest};
    use crate::{
        LocalOllamaManagedPreflightLimits,
        local_ollama_managed_preflight::test_support::{
            connection, native_load, package, plan, preflight, process,
        },
    };

    #[test]
    fn limit_digest_is_deterministic_and_sensitive() {
        let baseline = LocalOllamaManagedPreflightLimits::default();
        assert_eq!(limits_digest(baseline), limits_digest(baseline));
        let changed = LocalOllamaManagedPreflightLimits {
            process: AttachedProcessWitnessLimits {
                maximum_processes: baseline.process.maximum_processes - 1,
                ..baseline.process
            },
            native_load: NativeLoadObservationLimits {
                maximum_components: baseline.native_load.maximum_components - 1,
                ..baseline.native_load
            },
        };
        assert_ne!(limits_digest(baseline), limits_digest(changed));
    }

    fn evidence(label: &str) -> ManagedReportEvidenceDigests {
        ManagedReportEvidenceDigests {
            package_attestation: Digest::sha256(format!("package {label}").as_bytes()),
            isolation_policy: Digest::sha256(b"isolation policy"),
            launch_spec: Digest::sha256(b"launch"),
            initial_isolation: Digest::sha256(b"initial isolation"),
            final_isolation: Digest::sha256(b"initial isolation"),
            startup_standard_output: Digest::sha256(b""),
            startup_standard_error: Digest::sha256(b"Ollama cloud disabled: true\n"),
            startup_standard_output_bytes: 0,
            startup_standard_error_bytes: 28,
        }
    }

    #[test]
    fn complete_report_is_inert_and_binds_every_material_digest() {
        let package = package();
        let plan = plan(&package);
        let helper = &package.members()[1];
        let process = process();
        let connection = connection(&process);
        let native_load = native_load(&package, &process);
        let build = |label| {
            build_report(
                &package,
                &plan,
                helper,
                &[],
                LocalOllamaManagedPreflightLimits::default(),
                evidence(label),
                process.clone(),
                process.clone(),
                process.clone(),
                connection.clone(),
                vec![connection.clone()],
                OllamaCloudDisableVersionStatus::Unreviewed,
                preflight(&plan),
                native_load.clone(),
            )
            .expect("report")
        };
        let report = build("one");
        assert!(!report.cloud_disable_runtime_reviewed);
        assert!(report.operating_system_network_isolation_enforced);
        assert!(report.all_responses_used_retained_transport);
        assert!(report.kernel_attribution_checked_around_every_response);
        assert!(!report.application_handler_proven);
        assert!(!report.exclusive_socket_owner_proven);
        assert!(!report.model_loaded_or_used_proven);
        assert!(!report.effective_runtime_identity_proven);
        assert!(!report.qualified);
        assert_eq!(
            report.helper_member_relative_path.as_str(),
            "helper/isolation"
        );
        assert_ne!(report.binding_digest, build("two").binding_digest);
        let encoded = serde_json::to_string(&report).expect("serialized report");
        assert!(!encoded.contains("example.invalid"));
        assert!(!encoded.contains("Ollama cloud disabled"));
    }
}
