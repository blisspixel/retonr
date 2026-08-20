use std::fs;

use rewrite_model::{
    ComputeBackend, ExecutionPlacement, RuntimeAbi, RuntimeArchitecture, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_model_store::{ArtifactStateStore, WriteDisposition};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::{
    ManagedRuntimeAttestationRequest, ManagedRuntimeIdentityFacts, ManagedRuntimeStateFacts,
    RuntimeAttestationError, RuntimeAttestationLimits, RuntimeAttestationPersistence,
    RuntimeAttestationService, host_runtime_target,
};

const ENTRYPOINT_BYTES: &[u8] = b"managed-runtime-fixture-entrypoint";

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn limits() -> RuntimeAttestationLimits {
    RuntimeAttestationLimits {
        maximum_entrypoint_bytes: 4_096,
    }
}

fn request(entrypoint: std::path::PathBuf) -> ManagedRuntimeAttestationRequest {
    ManagedRuntimeAttestationRequest {
        entrypoint,
        expected_entrypoint_digest: None,
        identity: ManagedRuntimeIdentityFacts {
            runtime_family: "fixture-runtime".to_owned(),
            reported_version: "0.0.1".to_owned(),
            build_revision: Some("fixture-revision".to_owned()),
            target: host_runtime_target().unwrap_or_else(|| {
                RuntimeTarget::new(
                    RuntimeOperatingSystem::Windows,
                    RuntimeArchitecture::X86_64,
                    RuntimeAbi::WindowsMsvc,
                )
                .expect("windows target")
            }),
            package_manifest_digest: digest("package"),
            packaged_dependencies_digest: digest("dependencies"),
            build_configuration_digest: digest("build configuration"),
        },
        state: ManagedRuntimeStateFacts {
            provider_snapshot_contract: "fixture-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("provider snapshot"),
            launch_policy_digest: digest("launch"),
            loaded_components_digest: digest("loaded components"),
            effective_configuration_digest: digest("effective configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("execution class"),
            isolation_policy_digest: digest("isolation"),
            effective_context_tokens: 2_048,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
        },
    }
}

fn write_entrypoint(root: &std::path::Path) -> std::path::PathBuf {
    let path = root.join("runtime.bin");
    fs::write(&path, ENTRYPOINT_BYTES).expect("write entrypoint");
    path
}

#[test]
fn attests_a_managed_entrypoint_without_granting_authority() {
    let directory = tempdir().expect("temporary directory");
    let entrypoint = write_entrypoint(directory.path());
    let result = RuntimeAttestationService::attest_managed(
        &request(entrypoint),
        limits(),
        None,
        &CancellationToken::new(),
    )
    .expect("attest fixture");
    assert_eq!(result.build.mode(), RuntimeBuildMode::ManagedProcess);
    assert_eq!(
        result.build.entrypoint_digest(),
        &Digest::sha256(ENTRYPOINT_BYTES)
    );
    assert_eq!(result.entrypoint_bytes, ENTRYPOINT_BYTES.len() as u64);
    assert_eq!(
        result.persistence,
        RuntimeAttestationPersistence::NotRequested
    );
    assert_eq!(
        result.state.runtime_build_id(),
        &result.build.runtime_build_id()
    );
}

#[test]
fn loaded_component_evidence_is_caller_supplied() {
    let directory = tempdir().expect("temporary directory");
    let entrypoint = write_entrypoint(directory.path());
    let baseline = RuntimeAttestationService::attest_managed(
        &request(entrypoint.clone()),
        limits(),
        None,
        &CancellationToken::new(),
    )
    .expect("attest baseline");
    let mut changed = request(entrypoint);
    changed.state.loaded_components_digest = digest("different loaded components");
    let changed = RuntimeAttestationService::attest_managed(
        &changed,
        limits(),
        None,
        &CancellationToken::new(),
    )
    .expect("attest changed loaded components");

    assert_eq!(baseline.build, changed.build);
    assert_ne!(baseline.state, changed.state);
}

#[test]
fn persists_and_reloads_inert_attestation_records() {
    let directory = tempdir().expect("temporary directory");
    let entrypoint = write_entrypoint(directory.path());
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("open store");
    let first = RuntimeAttestationService::attest_managed(
        &request(entrypoint.clone()),
        limits(),
        Some(&mut store),
        &CancellationToken::new(),
    )
    .expect("persist attestation");
    assert_eq!(
        first.persistence,
        RuntimeAttestationPersistence::Stored {
            build: WriteDisposition::Inserted,
            state: WriteDisposition::Inserted,
        }
    );
    let second = RuntimeAttestationService::attest_managed(
        &request(entrypoint),
        limits(),
        Some(&mut store),
        &CancellationToken::new(),
    )
    .expect("repeat attestation");
    assert_eq!(
        second.persistence,
        RuntimeAttestationPersistence::Stored {
            build: WriteDisposition::AlreadyPresent,
            state: WriteDisposition::AlreadyPresent,
        }
    );
    assert_eq!(first.build, second.build);
    assert_eq!(first.state, second.state);
}

#[test]
fn expected_digest_mismatch_and_cancellation_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let entrypoint = write_entrypoint(directory.path());
    let mut mismatched = request(entrypoint.clone());
    mismatched.expected_entrypoint_digest = Some(digest("other entrypoint"));
    assert!(matches!(
        RuntimeAttestationService::attest_managed(
            &mismatched,
            limits(),
            None,
            &CancellationToken::new(),
        ),
        Err(RuntimeAttestationError::DigestMismatch)
    ));

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        RuntimeAttestationService::attest_managed(&request(entrypoint), limits(), None, &cancelled,),
        Err(RuntimeAttestationError::Cancelled)
    ));
}

#[test]
fn directory_source_and_zero_limits_are_rejected() {
    let directory = tempdir().expect("temporary directory");
    assert!(matches!(
        RuntimeAttestationService::attest_managed(
            &request(directory.path().to_path_buf()),
            limits(),
            None,
            &CancellationToken::new(),
        ),
        Err(RuntimeAttestationError::EntrypointNotFile)
    ));
    let entrypoint = write_entrypoint(directory.path());
    assert!(matches!(
        RuntimeAttestationService::attest_managed(
            &request(entrypoint),
            RuntimeAttestationLimits {
                maximum_entrypoint_bytes: 0,
            },
            None,
            &CancellationToken::new(),
        ),
        Err(RuntimeAttestationError::InvalidLimits)
    ));
}

#[test]
fn oversized_entrypoint_is_rejected_before_identity_construction() {
    let directory = tempdir().expect("temporary directory");
    let entrypoint = write_entrypoint(directory.path());
    assert!(matches!(
        RuntimeAttestationService::attest_managed(
            &request(entrypoint),
            RuntimeAttestationLimits {
                maximum_entrypoint_bytes: 4,
            },
            None,
            &CancellationToken::new(),
        ),
        Err(RuntimeAttestationError::EntrypointTooLarge { actual: 34, .. })
    ));
}
