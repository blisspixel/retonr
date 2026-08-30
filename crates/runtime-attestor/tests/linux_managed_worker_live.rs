#![cfg(all(target_os = "linux", feature = "worker-live-fixture"))]

use std::{
    fs::{self, File},
    net::{Ipv4Addr, TcpListener},
};

use rewrite_model::ArtifactId;
use rewrite_runtime_attestor::{
    AttachedProcessLease as _, AttachedProcessWitnessLimits, ListenerEndpoint,
    ManagedGenerationWorkerError, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerObservationRequest,
    ManagedGenerationWorkerProfile, NativeManagedLinuxProcessObserver, RetainedModelWeight,
};
use rewrite_types::{CancellationToken, Digest};

#[path = "linux_managed_worker_live/support.rs"]
mod support;
use support::{
    MODEL_BYTES, ModelSource, assert_private_model_copy, copy_distinct_worker, current_expectation,
    install_private_model_target, package, retain_code, socket_diagnostics, spawn_worker,
    wait_for_worker_ready,
};

#[test]
#[ignore = "requires controlled root with CAP_SETPCAP and complete proc visibility"]
fn live_managed_server_retains_exact_worker_native_closure_and_private_model_mapping() {
    assert_eq!(
        std::env::var("REWRITE_ATTESTOR_REQUIRE_WORKER_LIVE").as_deref(),
        Ok("1")
    );
    let cancellation = CancellationToken::new();
    let temporary = tempfile::tempdir().expect("create live fixture directory");
    let server_path = std::env::current_exe().expect("resolve live test executable");
    let worker_path = temporary.path().join("llama-server");
    copy_distinct_worker(&worker_path);

    let model_artifact = ArtifactId::from_digest(Digest::sha256(MODEL_BYTES));
    let source_path = temporary.path().join("source-model.gguf");
    fs::write(&source_path, MODEL_BYTES).expect("write retained source model");
    let target_guard = install_private_model_target(&model_artifact);
    assert_private_model_copy(&source_path, &target_guard.file);

    let server_bytes = fs::read(&server_path).expect("read server fixture bytes");
    let worker_bytes = fs::read(&worker_path).expect("read worker fixture bytes");
    let legal_bytes = b"Apache-2.0 fixture provenance";
    let package = package(&server_bytes, &worker_bytes, legal_bytes);
    let package_id = package.runtime_package_manifest_id();
    let retained_code = retain_code(&package, &server_path, &worker_path);
    let retained_worker = retained_code
        .iter()
        .find(|member| member.relative_path().as_str() == "lib/ollama/llama-server")
        .expect("retained worker member");
    let retained_weight = RetainedModelWeight::from_source(
        ModelSource {
            artifact_id: model_artifact.clone(),
            byte_size: u64::try_from(MODEL_BYTES.len()).expect("model size fits u64"),
            file: File::open(&source_path).expect("open retained source model"),
        },
        &cancellation,
    )
    .expect("retain exact source model");

    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind managed listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("valid managed endpoint");
    let mut worker = spawn_worker(&worker_path, &target_guard.file);
    wait_for_worker_ready(worker.0.id(), &target_guard.file);

    let expectation = current_expectation();
    let mut server = NativeManagedLinuxProcessObserver
        .attach(
            endpoint,
            socket_diagnostics(),
            expectation,
            AttachedProcessWitnessLimits::default(),
            &cancellation,
        )
        .expect("attach exact managed server");
    assert_eq!(server.initial_evidence().owner_pid(), std::process::id());

    let request = ManagedGenerationWorkerObservationRequest {
        package: &package,
        expected_package_id: &package_id,
        retained_worker,
        retained_model_weight: &retained_weight,
        profile: ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
        limits: ManagedGenerationWorkerLimits::default(),
    };
    let mut lease = server
        .observe_generation_worker(&request, &cancellation)
        .expect("discover and retain exact managed worker");
    assert_eq!(
        lease.initial_evidence().worker_artifact_id(),
        retained_worker.artifact_id()
    );
    assert_eq!(
        lease.initial_evidence().model_artifact_id(),
        &model_artifact
    );

    let native = lease
        .observe_native_load(
            &ManagedGenerationWorkerNativeLoadRequest {
                package: &package,
                expected_package_id: &package_id,
                retained_package_code: &retained_code,
                expected_external_components: &[],
            },
            &cancellation,
        )
        .expect("observe exact static worker native closure");
    assert_eq!(native.component_count(), 1);
    let model = lease
        .observe_model_mapping(&cancellation)
        .expect("observe byte-identical private GGUF mapping");
    assert_eq!(model.model_artifact_id(), &model_artifact);
    assert_eq!(model.mapping_region_count(), 1);
    let reobserved = lease
        .reobserve(&cancellation)
        .expect("reobserve complete worker authority chain");
    assert_eq!(&reobserved, lease.initial_evidence());

    worker.0.kill().expect("terminate exact worker fixture");
    worker.0.wait().expect("reap exact worker fixture");
    assert!(matches!(
        lease.reobserve(&cancellation),
        Err(ManagedGenerationWorkerError::WorkerChanged)
    ));
    drop(listener);
}
