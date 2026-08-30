use std::time::Instant;

use rewrite_inference::{
    OutputContract, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest, StructuredCompletionResponse,
    UsageObservation,
};
use rewrite_model::{ArtifactId, RuntimeIdentity, RuntimePackageMemberRole};
use rewrite_ollama::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
    OllamaResidentResourceObservedCompletion, OllamaResidentSessionExecutionReceipt,
    OllamaRunningModel, OllamaSessionExecutionReceipt,
};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerProfile,
    ManagedGenerationWorkerResourceObservation,
};
use rewrite_types::{CancellationToken, Digest};

use crate::candidate_attempt_precursor::tests::support::{
    Fixture as PackageFixture, characterized, launch,
};
use crate::generation_api::generation_qualification_phase_denial::tests::support::with_fixture;

use super::validation::{
    fresh_package_revalidation, validate_package_binding, validate_response_binding,
    validate_worker_binding,
};
use super::*;

mod api_surface;
mod assertions;
mod recombination;

const FIRST_RESPONSE_ORDINAL: usize = 8;
const GENERATE_RESPONSE_ORDINAL: usize = 11;
const LAST_RESPONSE_ORDINAL: usize = 16;

struct CompletionFixture {
    request: StructuredCompletionRequest,
    completion: OllamaResidentResourceObservedCompletion,
}

fn completion(artifact_id: ArtifactId, input: &str, response_json: &str) -> CompletionFixture {
    let request = request(artifact_id, input);
    let response = StructuredCompletionResponse::complete(
        &request,
        runtime_identity(),
        request.artifact_id.clone(),
        request.artifact_digest.clone(),
        response_json.to_owned(),
        UsageObservation {
            input_tokens: Some(11),
            output_tokens: Some(3),
            generation_micros: Some(0),
        },
    )
    .expect("structured response");
    let running = running_model();
    let execution = OllamaSessionExecutionReceipt::for_test(
        &preflight(&running),
        &response,
        FIRST_RESPONSE_ORDINAL,
        LAST_RESPONSE_ORDINAL,
    )
    .expect("execution receipt");
    let receipt = OllamaResidentSessionExecutionReceipt::for_test(
        execution,
        &running,
        FIRST_RESPONSE_ORDINAL + 4,
        LAST_RESPONSE_ORDINAL,
    );
    let completion = OllamaResidentResourceObservedCompletion::for_test(
        response,
        receipt,
        GENERATE_RESPONSE_ORDINAL,
        Instant::now(),
        0,
        0,
        11,
        0,
        3,
        0,
    )
    .expect("resource-observed completion");
    CompletionFixture {
        request,
        completion,
    }
}

fn request(artifact_id: ArtifactId, input: &str) -> StructuredCompletionRequest {
    let schema_json = r#"{"type":"object"}"#.to_owned();
    StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_digest: artifact_id.digest().clone(),
        artifact_id,
        input: input.to_owned(),
        output: OutputContract {
            schema_digest: Digest::sha256(schema_json.as_bytes()),
            schema_json,
        },
        source_byte_count: 5,
        source_byte_limit: 1_024,
        input_byte_limit: 2_048,
        context_token_limit: 4_096,
        output_token_limit: 256,
        output_byte_limit: 1_024,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

fn runtime_identity() -> RuntimeIdentity {
    RuntimeIdentity {
        backend: "ollama_native".to_owned(),
        version: "0.32.15".to_owned(),
        digest: Some(Digest::sha256(b"resource runtime")),
    }
}

fn running_model() -> OllamaRunningModel {
    OllamaRunningModel {
        reference: "fixture:latest".to_owned(),
        inventory_digest: Digest::sha256(b"resource inventory"),
        byte_size: 4_096,
        accelerator_bytes: 0,
        context_tokens: 4_096,
    }
}

fn preflight(running: &OllamaRunningModel) -> OllamaPreflight {
    OllamaPreflight {
        runtime: runtime_identity(),
        inventory: vec![OllamaInventoryEntry {
            reference: running.reference.clone(),
            inventory_digest: running.inventory_digest.clone(),
            byte_size: running.byte_size,
        }],
        bindings: vec![OllamaPreflightBinding {
            reference: running.reference.clone(),
            inventory_digest: running.inventory_digest.clone(),
            details: OllamaModelDetails {
                format: "gguf".to_owned(),
                family: "llama".to_owned(),
                quantization: "Q4_K_M".to_owned(),
                capabilities: vec!["completion".to_owned()],
                license_digest: Digest::sha256(b"license"),
                template_digest: Digest::sha256(b"template"),
                metadata_digest: Digest::sha256(b"metadata"),
            },
        }],
        running: vec![running.clone()],
    }
}

fn worker(
    packages: &PackageFixture,
    model_artifact_id: ArtifactId,
    tag: &str,
) -> ManagedGenerationWorkerEvidence {
    let worker_artifact_id = packages
        .runtime
        .runtime_manifest
        .members()
        .iter()
        .find(|member| {
            member
                .roles()
                .contains(&RuntimePackageMemberRole::WorkerExecutable)
        })
        .expect("worker member")
        .artifact_id()
        .clone();
    ManagedGenerationWorkerEvidence::for_test(
        ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
        packages
            .runtime
            .runtime_manifest
            .runtime_package_manifest_id(),
        worker_artifact_id,
        model_artifact_id,
        tag,
    )
}

#[test]
fn exact_finish_returns_a_self_contained_revalidatable_snapshot() {
    with_fixture(|policy_fixture| {
        let mut packages = PackageFixture::new();
        let launch_plan = launch(&packages.model_lease, "resource");
        let approved = policy_fixture.resource_policy(true);
        let clock = GenerationQualificationResourceAttemptClock::start(
            &approved,
            &policy_fixture.operation,
            &CancellationToken::new(),
        )
        .expect("approved clock");
        let completion = completion(
            launch_plan.model_target().artifact_id().clone(),
            "resource input",
            r#"{"candidates":[{"text":"ok"}]}"#,
        );
        let observed_worker = worker(
            &packages,
            launch_plan.model_target().artifact_id().clone(),
            "resource worker",
        );
        let worker_resource =
            ManagedGenerationWorkerResourceObservation::for_test(&observed_worker, 8_192);
        let mut released = characterized(&packages, &launch_plan, &packages.runtime.runtime_state);
        released.retain_resource_attempt_test_subject(
            &completion.completion,
            &observed_worker,
            &packages.runtime.runtime_package,
            &packages.model_lease,
        );
        let mut observed = clock
            .finish(GenerationQualificationResourceAttemptObservationInput {
                policy: &approved,
                operation_policy: &policy_fixture.operation,
                completion: completion.completion,
                worker_observation: &worker_resource,
                worker_evidence: &observed_worker,
                released_package: &released,
                runtime_package: &mut packages.runtime.runtime_package,
                model_package: &packages.model_lease,
                cancellation: &CancellationToken::new(),
            })
            .expect("resource snapshot");

        assert_eq!(observed.prompt_token_count(), 11);
        assert_eq!(observed.generated_token_count(), 3);
        assertions::assert_complete_getter_surface(&observed);
        assert_eq!(
            observed.response_id(),
            &rewrite_ollama::derive_ollama_retained_session_response_id(observed.response())
        );
        assert_eq!(
            observed.receipt_binding_digest(),
            &observed
                .resident_execution_receipt()
                .complete_binding_digest()
        );
        assert_eq!(observed.worker_high_water_resident_bytes(), 8_192);
        assert_eq!(
            observed.runtime_installed_payload_bytes(),
            packages
                .runtime
                .runtime_package
                .evidence()
                .payload_byte_size()
        );
        assert_eq!(
            observed.model_installed_payload_bytes(),
            packages.model_lease.byte_size()
        );
        assert_eq!(
            observed.installed_footprint_bytes(),
            observed.runtime_installed_payload_bytes() + observed.model_installed_payload_bytes()
        );
        assert!(observed.exceeded_limits().is_empty());
        assert_eq!(observed.revalidate(), Ok(()));
        let debug = format!("{observed:?}");
        assert_eq!(
            debug,
            "VerifiedGenerationQualificationResourceAttemptObservation { content: \"redacted\" }"
        );
        assert!(!debug.contains(observed.snapshot_digest().as_str()));
        observed.values.installed_footprint_bytes += 1;
        assert_eq!(
            observed.revalidate(),
            Err(GenerationQualificationResourceAttemptObservationError::ObservationInconsistent)
        );
    });
}

#[test]
fn denied_and_foreign_operation_policies_cannot_start_a_clock() {
    with_fixture(|fixture| {
        let denied = fixture.resource_policy(false);
        assert!(matches!(
            GenerationQualificationResourceAttemptClock::start(
                &denied,
                &fixture.operation,
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationResourceAttemptObservationError::PolicyDenied)
        ));
        let approved = fixture.resource_policy(true);
        let debug_clock = GenerationQualificationResourceAttemptClock::start(
            &approved,
            &fixture.operation,
            &CancellationToken::new(),
        )
        .expect("debug clock");
        assert_eq!(
            format!("{debug_clock:?}"),
            "GenerationQualificationResourceAttemptClock { content: \"redacted\" }"
        );
        let (_plan, foreign_operation) = fixture.swapped_operation();
        assert!(matches!(
            GenerationQualificationResourceAttemptClock::start(
                &approved,
                &foreign_operation,
                &CancellationToken::new(),
            ),
            Err(GenerationQualificationResourceAttemptObservationError::PolicyBindingMismatch)
        ));
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            GenerationQualificationResourceAttemptClock::start(
                &approved,
                &fixture.operation,
                &cancelled,
            ),
            Err(GenerationQualificationResourceAttemptObservationError::Cancelled)
        ));
    });
}

#[test]
fn response_request_receipt_and_ordinal_substitution_are_rejected() {
    let artifact = ArtifactId::from_digest(Digest::sha256(b"resource model"));
    let exact = completion(
        artifact.clone(),
        "exact request",
        r#"{"candidates":[{"text":"exact"}]}"#,
    );
    let foreign_response = completion(
        artifact,
        "foreign request",
        r#"{"candidates":[{"text":"foreign"}]}"#,
    );
    assert!(validate_response_binding(&exact.completion).is_ok());
    assert!(
        OllamaResidentResourceObservedCompletion::for_test(
            foreign_response.completion.response().clone(),
            exact.completion.resident_execution_receipt().clone(),
            GENERATE_RESPONSE_ORDINAL,
            Instant::now(),
            0,
            0,
            11,
            0,
            3,
            0,
        )
        .is_err()
    );
    let changed_response = StructuredCompletionResponse::complete(
        &exact.request,
        runtime_identity(),
        exact.request.artifact_id.clone(),
        exact.request.artifact_digest.clone(),
        r#"{"candidates":[{"text":"changed"}]}"#.to_owned(),
        UsageObservation {
            input_tokens: Some(11),
            output_tokens: Some(3),
            generation_micros: Some(0),
        },
    )
    .expect("changed response");
    assert!(
        OllamaResidentResourceObservedCompletion::for_test(
            changed_response,
            exact.completion.resident_execution_receipt().clone(),
            GENERATE_RESPONSE_ORDINAL,
            Instant::now(),
            0,
            0,
            11,
            0,
            3,
            0,
        )
        .is_err()
    );
    let wrong_ordinal = OllamaResidentResourceObservedCompletion::for_test(
        exact.completion.response().clone(),
        exact.completion.resident_execution_receipt().clone(),
        GENERATE_RESPONSE_ORDINAL + 1,
        Instant::now(),
        0,
        0,
        11,
        0,
        3,
        0,
    );
    assert!(wrong_ordinal.is_err());
    let running = running_model();
    let wrong_execution = OllamaSessionExecutionReceipt::for_test(
        &preflight(&running),
        exact.completion.response(),
        FIRST_RESPONSE_ORDINAL + 1,
        LAST_RESPONSE_ORDINAL + 1,
    )
    .expect("substituted receipt");
    let wrong_receipt = OllamaResidentSessionExecutionReceipt::for_test(
        wrong_execution,
        &running,
        FIRST_RESPONSE_ORDINAL + 5,
        LAST_RESPONSE_ORDINAL + 1,
    );
    let wrong_receipt = OllamaResidentResourceObservedCompletion::for_test(
        exact.completion.response().clone(),
        wrong_receipt,
        GENERATE_RESPONSE_ORDINAL,
        Instant::now(),
        0,
        0,
        11,
        0,
        3,
        0,
    );
    assert!(wrong_receipt.is_err());
}

#[test]
fn worker_and_package_substitution_are_rejected() {
    let first = PackageFixture::new();
    let second = PackageFixture::new_model_variant(true);
    let first_launch = launch(&first.model_lease, "first-resource");
    let second_launch = launch(&second.model_lease, "second-resource");
    let first_worker = worker(
        &first,
        first_launch.model_target().artifact_id().clone(),
        "first worker",
    );
    let second_worker = worker(
        &first,
        first_launch.model_target().artifact_id().clone(),
        "second worker",
    );
    let resource = ManagedGenerationWorkerResourceObservation::for_test(&first_worker, 4_096);
    assert!(validate_worker_binding(&resource, &first_worker).is_ok());
    assert_eq!(
        validate_worker_binding(&resource, &second_worker),
        Err(GenerationQualificationResourceAttemptObservationError::WorkerBindingMismatch)
    );

    let released = characterized(&first, &first_launch, &first.runtime.runtime_state);
    assert!(
        validate_package_binding(
            &released,
            &first.runtime.runtime_package,
            &first.model_lease,
        )
        .is_ok()
    );
    assert_eq!(
        validate_package_binding(
            &released,
            &second.runtime.runtime_package,
            &second.model_lease,
        ),
        Err(GenerationQualificationResourceAttemptObservationError::PackageBindingMismatch)
    );
    drop(second_launch);
}

#[test]
fn final_package_revalidation_observes_cancellation_and_drift() {
    let mut cancelled_packages = PackageFixture::new();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert_eq!(
        fresh_package_revalidation(
            &mut cancelled_packages.runtime.runtime_package,
            &cancelled_packages.model_lease,
            &cancelled,
        ),
        Err(GenerationQualificationResourceAttemptObservationError::Cancelled)
    );

    let mut drifted_packages = PackageFixture::new();
    drifted_packages.add_runtime_member();
    assert_eq!(
        fresh_package_revalidation(
            &mut drifted_packages.runtime.runtime_package,
            &drifted_packages.model_lease,
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationResourceAttemptObservationError::PackageRevalidationFailed)
    );

    let mut model_drifted_packages = PackageFixture::new();
    model_drifted_packages.add_model_member();
    assert_eq!(
        fresh_package_revalidation(
            &mut model_drifted_packages.runtime.runtime_package,
            &model_drifted_packages.model_lease,
            &CancellationToken::new(),
        ),
        Err(GenerationQualificationResourceAttemptObservationError::PackageRevalidationFailed)
    );
}
