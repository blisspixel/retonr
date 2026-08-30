use std::{
    fmt::Write as _,
    fs::File,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use rewrite_inference::{
    OutputContract, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest, StructuredCompletionResponse,
    UsageObservation,
};
use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    CandidateJudgeScheduleId, NativeLoadEvidenceClass, NativeLoadObservation,
    NativeLoadObservationInput, NativeLoadOrigin, NativeLoadVisibilityScope, NativeLoadedComponent,
    NativeMappingClass, PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi,
    RuntimeArchitecture, RuntimeIdentity, RuntimeOperatingSystem, RuntimePackageLoadPolicy,
    RuntimePackageManifest, RuntimePackageMember, RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_ollama::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
    OllamaResidentSessionExecutionReceipt, OllamaResponseObservationPhase, OllamaRunningModel,
    OllamaSessionExecutionReceipt,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessWitnessError, ManagedGenerationWorkerError, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerLimits, ManagedGenerationWorkerModelMappingEvidence,
    ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerNativeLoadRequest,
    ManagedGenerationWorkerObservationRequest, ManagedGenerationWorkerProfile,
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeLoadObserverError,
    RetainedModelWeight, RetainedModelWeightSink, RetainedModelWeightSource,
    RetainedNativePackageMember, RetainedTcpConnection, RetainedTcpConnectionEvidence,
    RetainedTcpConnectionEvidenceInput, TcpConnectionAttributionKind,
    TcpConnectionSharingLimitation,
};
use rewrite_types::{CancellationToken, Digest};
use tempfile::TempDir;

use super::super::super::{
    ManagedOllamaEffectiveRuntimeState, authority::ManagedOllamaManagedJudgeAttemptFacts,
};
use super::{
    EffectiveStateObservationSubject, MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT,
    MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT, ManagedJudgeAttemptObservation,
    ManagedJudgeObservationError, ManagedJudgeObservationSequence, ManagedJudgeProcessAuthority,
    ManagedJudgeWorkerAuthority, connection_span_digest,
};

mod authority_support;
mod closure;
mod coverage;
mod deadline;
mod helpers;
mod protocol;
mod substitutions;

pub(crate) use authority_support::*;
use helpers::*;

struct FakeAuthority {
    process: AttachedProcessEvidence,
    native_load: NativeLoadObservation,
    worker: ManagedGenerationWorkerEvidence,
    worker_native_load: ManagedGenerationWorkerNativeLoadEvidence,
    model_mapping: ManagedGenerationWorkerModelMappingEvidence,
    process_error: bool,
    native_error: bool,
    worker_reobserve_error: bool,
}

struct FakeWorkerAuthority {
    initial: ManagedGenerationWorkerEvidence,
    native_load: ManagedGenerationWorkerNativeLoadEvidence,
    model_mapping: ManagedGenerationWorkerModelMappingEvidence,
    reobserve_error: bool,
}

impl ManagedJudgeWorkerAuthority for FakeWorkerAuthority {
    fn initial_evidence(&self) -> &ManagedGenerationWorkerEvidence {
        &self.initial
    }

    fn observe_native_load(
        &mut self,
        _request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        Ok(self.native_load.clone())
    }

    fn observe_model_mapping(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        Ok(self.model_mapping.clone())
    }

    fn reobserve(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        if self.reobserve_error {
            return Err(ManagedGenerationWorkerError::ObservationChanged);
        }
        Ok(self.initial.clone())
    }
}

impl ManagedJudgeProcessAuthority for FakeAuthority {
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.process
    }

    fn reobserve(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        if self.process_error {
            return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
        }
        Ok(self.process.clone())
    }

    fn observe_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        connection_evidence(&self.process)
    }

    fn reobserve_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _initial: &RetainedTcpConnectionEvidence,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        connection_evidence(&self.process)
    }

    fn observe_native_load(
        &mut self,
        _request: &NativeLoadObservationRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError> {
        if self.native_error {
            return Err(NativeLoadObserverError::ObservationChanged);
        }
        Ok(self.native_load.clone())
    }

    fn observe_generation_worker(
        &mut self,
        _request: &ManagedGenerationWorkerObservationRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<Box<dyn ManagedJudgeWorkerAuthority>, ManagedGenerationWorkerError> {
        Ok(Box::new(FakeWorkerAuthority {
            initial: self.worker.clone(),
            native_load: self.worker_native_load.clone(),
            model_mapping: self.model_mapping.clone(),
            reobserve_error: self.worker_reobserve_error,
        }))
    }
}

struct FakeState {
    binding: Digest,
    process: Digest,
    native_load: Digest,
    worker: Digest,
    worker_native_load: Digest,
    model_mapping: Digest,
    request: Digest,
    response: Digest,
    receipt: Digest,
    preflight: Digest,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
    first_residency_ordinal: u64,
    last_residency_ordinal: u64,
    managed: Option<ManagedOllamaEffectiveRuntimeState>,
}

impl FakeState {
    fn new(
        observation: &ManagedJudgeAttemptObservation,
        request: &StructuredCompletionRequest,
        receipt: &OllamaResidentSessionExecutionReceipt,
        tag: &str,
    ) -> Self {
        let execution = receipt.execution();
        Self {
            binding: digest(tag),
            process: observation.process.evidence_digest().clone(),
            native_load: observation
                .native_load
                .native_load_observation_id()
                .digest()
                .clone(),
            worker: observation.initial_worker.evidence_digest().clone(),
            worker_native_load: observation.worker_native_load.observation_digest().clone(),
            model_mapping: observation.model_mapping.observation_digest().clone(),
            request: request.binding_digest(),
            response: execution.response_digest().clone(),
            receipt: receipt.complete_binding_digest(),
            preflight: execution.preflight_digest().clone(),
            first_response_ordinal: u64::try_from(execution.first_response_ordinal())
                .expect("ordinal"),
            last_response_ordinal: u64::try_from(execution.last_response_ordinal())
                .expect("ordinal"),
            first_residency_ordinal: u64::try_from(receipt.first_residency_ordinal())
                .expect("ordinal"),
            last_residency_ordinal: u64::try_from(receipt.last_residency_ordinal())
                .expect("ordinal"),
            managed: None,
        }
    }

    fn new_for_batch(
        observation: &ManagedJudgeAttemptObservation,
        request: &StructuredCompletionRequest,
        receipt: &OllamaResidentSessionExecutionReceipt,
        context: authority_support::BatchEffectiveStateFixture<'_, '_>,
    ) -> Self {
        let mut value = Self::new(observation, request, receipt, context.tag);
        let execution = receipt.execution();
        let request_digest = request.binding_digest();
        let response_digest = execution.response_digest().clone();
        let receipt_digest = receipt.complete_binding_digest();
        let preflight_digest = execution.preflight_digest().clone();
        let facts = ManagedOllamaManagedJudgeAttemptFacts {
            server_process: observation.process(),
            server_native_load: observation.native_load(),
            initial_worker: observation.initial_worker(),
            final_worker: observation.final_worker(),
            worker_native_load: observation.worker_native_load(),
            model_mapping: observation.model_mapping(),
            request: &request_digest,
            response: &response_digest,
            receipt: &receipt_digest,
            preflight: &preflight_digest,
            first_response_ordinal: value.first_response_ordinal,
            last_response_ordinal: value.last_response_ordinal,
            first_residency_ordinal: value.first_residency_ordinal,
            last_residency_ordinal: value.last_residency_ordinal,
        };
        value.managed = Some(
            ManagedOllamaEffectiveRuntimeState::managed_judge_batch_test_fixture(
                context.state,
                value.binding.clone(),
                &facts,
                context.admitted_runtime,
                context.generation_path,
                context.runtime_package,
                context.managed_ollama,
            ),
        );
        value
    }
}

impl EffectiveStateObservationSubject for FakeState {
    fn binds_observations(&self, facts: &ManagedOllamaManagedJudgeAttemptFacts<'_>) -> bool {
        self.process == *facts.server_process.evidence_digest()
            && self.native_load
                == *facts
                    .server_native_load
                    .native_load_observation_id()
                    .digest()
            && facts.server_native_load.process_evidence_digest()
                == facts.server_process.evidence_digest()
            && self.worker == *facts.initial_worker.evidence_digest()
            && facts.initial_worker == facts.final_worker
            && self.worker_native_load == *facts.worker_native_load.observation_digest()
            && self.model_mapping == *facts.model_mapping.observation_digest()
            && self.request == *facts.request
            && self.response == *facts.response
            && self.receipt == *facts.receipt
            && self.preflight == *facts.preflight
            && self.first_response_ordinal == facts.first_response_ordinal
            && self.last_response_ordinal == facts.last_response_ordinal
            && self.first_residency_ordinal == facts.first_residency_ordinal
            && self.last_residency_ordinal == facts.last_residency_ordinal
    }

    fn into_retained(self) -> super::super::contract::RetainedManagedJudgeEffectiveState {
        match self.managed {
            Some(state) => {
                super::super::contract::RetainedManagedJudgeEffectiveState::Managed(Box::new(state))
            }
            None => {
                super::super::contract::RetainedManagedJudgeEffectiveState::Offline(self.binding)
            }
        }
    }
}

struct WeightSource {
    artifact_id: ArtifactId,
    byte_size: u64,
    file: File,
}

impl<'lease> RetainedModelWeightSource<'lease> for WeightSource {
    fn transfer(
        self,
        sink: &mut RetainedModelWeightSink<'_, 'lease>,
    ) -> Result<(), ManagedGenerationWorkerError> {
        sink.retain(self.artifact_id, self.byte_size, self.file)
    }
}

struct Fixture {
    _temporary: TempDir,
    package: RuntimePackageManifest,
    package_id: rewrite_model::RuntimePackageManifestId,
    process: AttachedProcessEvidence,
    native_load: NativeLoadObservation,
    retained_worker: RetainedNativePackageMember,
    retained_weight: RetainedModelWeight<'static>,
    worker: ManagedGenerationWorkerEvidence,
    worker_native_load: ManagedGenerationWorkerNativeLoadEvidence,
    model_mapping: ManagedGenerationWorkerModelMappingEvidence,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let process = process(tag);
        let temporary = tempfile::tempdir().expect("temporary directory");
        let package = package();
        let entrypoint = package.entrypoint();
        let native_load = NativeLoadObservation::new(
            &package,
            NativeLoadObservationInput {
                evidence_class: NativeLoadEvidenceClass::LinuxProcMapFiles,
                visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
                process_evidence_digest: process.evidence_digest().clone(),
                observation_contract_id: "managed-judge-test".to_owned(),
                observation_contract_schema_version: 1,
                components: vec![NativeLoadedComponent::new(
                    entrypoint.artifact_id().clone(),
                    entrypoint.byte_size(),
                    NativeLoadOrigin::PackagedMember {
                        relative_path: entrypoint.relative_path().clone(),
                    },
                    NativeMappingClass::ExecutableImage,
                    digest("object"),
                )],
            },
        )
        .expect("valid native load");
        let worker_member = package
            .members()
            .iter()
            .find(|member| member.roles() == [RuntimePackageMemberRole::WorkerExecutable])
            .expect("worker member");
        let worker_path = temporary.path().join("worker");
        std::fs::write(&worker_path, b"worker").expect("write worker");
        let retained_worker = RetainedNativePackageMember::new(
            worker_member.relative_path().clone(),
            worker_member.artifact_id().clone(),
            worker_member.byte_size(),
            File::open(&worker_path).expect("open worker"),
        )
        .expect("retained worker");
        let weight_path = temporary.path().join("weight.gguf");
        std::fs::write(&weight_path, b"weight").expect("write weight");
        let weight_artifact = ArtifactId::from_digest(Digest::sha256(b"weight"));
        let retained_weight = RetainedModelWeight::from_source(
            WeightSource {
                artifact_id: weight_artifact.clone(),
                byte_size: 6,
                file: File::open(&weight_path).expect("open weight"),
            },
            &CancellationToken::new(),
        )
        .expect("retained weight");
        let worker = ManagedGenerationWorkerEvidence::for_test(
            ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
            package.runtime_package_manifest_id(),
            worker_member.artifact_id().clone(),
            weight_artifact.clone(),
            tag,
        );
        let worker_native_load = ManagedGenerationWorkerNativeLoadEvidence::for_test(tag);
        let model_mapping =
            ManagedGenerationWorkerModelMappingEvidence::for_test(weight_artifact, tag);
        let package_id = package.runtime_package_manifest_id();
        Self {
            _temporary: temporary,
            package,
            package_id,
            process,
            native_load,
            retained_worker,
            retained_weight,
            worker,
            worker_native_load,
            model_mapping,
        }
    }

    fn sequence(&self, attempts: u32) -> ManagedJudgeObservationSequence {
        ManagedJudgeObservationSequence::for_test(
            self.authority(),
            schedule_id("schedule"),
            attempts,
            &CancellationToken::new(),
        )
        .expect("valid test sequence")
    }

    fn authority(&self) -> Box<dyn ManagedJudgeProcessAuthority> {
        Box::new(FakeAuthority {
            process: self.process.clone(),
            native_load: self.native_load.clone(),
            worker: self.worker.clone(),
            worker_native_load: self.worker_native_load.clone(),
            model_mapping: self.model_mapping.clone(),
            process_error: false,
            native_error: false,
            worker_reobserve_error: false,
        })
    }

    fn native_request(&self) -> NativeLoadObservationRequest<'_> {
        NativeLoadObservationRequest {
            package: &self.package,
            expected_package_id: &self.package_id,
            retained_package_members: &[],
            expected_external_components: &[],
            limits: NativeLoadObservationLimits::default(),
        }
    }

    fn worker_request(&self) -> ManagedGenerationWorkerObservationRequest<'_> {
        ManagedGenerationWorkerObservationRequest {
            package: &self.package,
            expected_package_id: &self.package_id,
            retained_worker: &self.retained_worker,
            retained_model_weight: &self.retained_weight,
            profile: ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
            limits: ManagedGenerationWorkerLimits::default(),
        }
    }

    fn worker_native_request(&self) -> ManagedGenerationWorkerNativeLoadRequest<'_> {
        ManagedGenerationWorkerNativeLoadRequest {
            package: &self.package,
            expected_package_id: &self.package_id,
            retained_package_code: &[],
            expected_external_components: &[],
        }
    }

    fn structured_request(&self) -> StructuredCompletionRequest {
        let schema_json = "{\"type\":\"object\"}".to_owned();
        StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: self.worker.model_artifact_id().clone(),
            artifact_digest: self.worker.model_artifact_id().digest().clone(),
            input: "managed judge input".to_owned(),
            output: OutputContract {
                schema_digest: Digest::sha256(schema_json.as_bytes()),
                schema_json,
            },
            source_byte_count: 19,
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

    fn receipt(
        cursor: u32,
        request: &StructuredCompletionRequest,
    ) -> OllamaResidentSessionExecutionReceipt {
        let first = 8 + usize::try_from(cursor).expect("cursor") * 9;
        let last = first + 8;
        let running = running_model();
        let preflight = preflight(&running);
        Self::receipt_from(
            request,
            &preflight,
            &running,
            r#"{"candidates":[{"text":"ok"}]}"#,
            [first, last, first + 4, last],
        )
    }

    fn receipt_from(
        request: &StructuredCompletionRequest,
        preflight: &OllamaPreflight,
        running: &OllamaRunningModel,
        output: &str,
        ordinals: [usize; 4],
    ) -> OllamaResidentSessionExecutionReceipt {
        let response = StructuredCompletionResponse::complete(
            request,
            runtime_identity(),
            request.artifact_id.clone(),
            request.artifact_digest.clone(),
            output.to_owned(),
            UsageObservation {
                input_tokens: Some(11),
                output_tokens: Some(3),
                generation_micros: Some(1_250),
            },
        )
        .expect("response");
        let execution =
            OllamaSessionExecutionReceipt::for_test(preflight, &response, ordinals[0], ordinals[1])
                .expect("execution receipt");
        OllamaResidentSessionExecutionReceipt::for_test(
            execution,
            running,
            ordinals[2],
            ordinals[3],
        )
    }
}
