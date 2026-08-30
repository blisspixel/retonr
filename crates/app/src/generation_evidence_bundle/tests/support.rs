use rewrite_inference::{
    StructuredCompletionRequest, StructuredCompletionResponse, UsageObservation,
};
use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, CandidateArtifactEntryV1,
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationCleanupRecordV1Input, CandidateGenerationEvidenceBundleEntryV1,
    CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations, CandidateGenerationEvidenceBundleRoleV1,
    CandidateGenerationPackageRevalidationStatusV1, CandidateGenerationProcessCleanupStatusV1,
    EffectivePackageEvidenceV2, GenerationCaseManifestV1, GenerationClusterRecordV1,
    GenerationQualificationPlanV1, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    GenerationSystemRecordV1, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, PlannedCandidateAttemptV1, RuntimeIdentity,
    StructuredResponseArtifactV1Input,
};
use rewrite_ollama::{
    OllamaInventoryEntry, OllamaModelDetails, OllamaPreflight, OllamaPreflightBinding,
    OllamaRunningModel, OllamaSessionExecutionReceipt, derive_ollama_retained_session_response_id,
};
use rewrite_types::{CancellationToken, Digest};

use crate::candidate_attempt_precursor::tests::support::{
    Fixture as PrecursorFixture, QualificationFixture, compilation_input, launch,
};
use crate::{CandidateAttemptPrecursorCompiler, RetainedStructuredResponseArtifactV1};

use super::super::CandidateGenerationEvidenceBundlePublicationPlanInput;

pub(super) const CANDIDATE_BYTES: &[u8] = b"ok";

pub(super) struct BundleFixture {
    pub(super) qualification_plan: GenerationQualificationPlanV1,
    pub(super) cluster: GenerationClusterRecordV1,
    pub(super) suite: GenerationSuiteManifestV1,
    pub(super) repetition: GenerationRepetitionRecordV1,
    pub(super) generation_system: GenerationSystemRecordV1,
    pub(super) effective_package: EffectivePackageEvidenceV2,
    pub(super) planned_attempt: PlannedCandidateAttemptV1,
    pub(super) case_manifest: GenerationCaseManifestV1,
    pub(super) precursor: CandidateGenerationAttemptPrecursorV1,
    pub(super) managed_evidence: ManagedOllamaCandidateGenerationEvidenceV2,
    pub(super) cleanup: CandidateGenerationCleanupRecordV1,
    pub(super) structured_response: RetainedStructuredResponseArtifactV1,
    pub(super) structured_request: StructuredCompletionRequest,
    pub(super) manifest: CandidateGenerationEvidenceBundleManifestV1,
    pub(super) candidate_path: ArtifactSetRelativePath,
}

impl BundleFixture {
    pub(super) fn new() -> Self {
        Self::with_candidate("ok")
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the fixture constructs one complete cross-crate authority chain without hidden defaults"
    )]
    pub(super) fn with_candidate(candidate_text: &str) -> Self {
        let mut precursor_fixture = PrecursorFixture::new();
        let launch_plan = launch(&precursor_fixture.model_lease, "v1");
        let qualification = QualificationFixture::new(&precursor_fixture, &launch_plan);
        let capability = CandidateAttemptPrecursorCompiler::compile(
            compilation_input(&mut precursor_fixture.runtime, &qualification, launch_plan),
            &CancellationToken::new(),
        )
        .expect("compile exact precursor fixture");
        let precursor = capability.precursor().clone();
        let structured_request = capability.structured_request().clone();
        drop(capability);

        let response = response(&structured_request, candidate_text);
        let response_id = derive_ollama_retained_session_response_id(&response);
        let structured_response = RetainedStructuredResponseArtifactV1::from_retained_response(
            &response,
            &session_receipt(&response),
        )
        .expect("compile retained response artifact");
        let managed_evidence = ManagedOllamaCandidateGenerationEvidenceV2::new(
            ManagedOllamaCandidateGenerationEvidenceV2Relations {
                precursor: &precursor,
                planned_attempt: &qualification.planned_attempt,
                generation_system: &qualification.generation_system,
                effective_package_evidence_v2: qualification.characterized_package.evidence(),
            },
            ManagedOllamaCandidateGenerationEvidenceV2Input {
                bracket_observation_v1_id:
                    ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(
                        "publisher bracket",
                    )),
                effective_runtime_state_join_id:
                    ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest(
                        "publisher effective state join",
                    )),
                response_id,
            },
        )
        .expect("compile managed evidence");
        let cleanup = CandidateGenerationCleanupRecordV1::new(
            &precursor,
            &managed_evidence,
            CandidateGenerationCleanupRecordV1Input {
                process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Succeeded,
                runtime_package_revalidation_status:
                    CandidateGenerationPackageRevalidationStatusV1::Verified,
                model_package_revalidation_status:
                    CandidateGenerationPackageRevalidationStatusV1::Verified,
            },
        )
        .expect("compile cleanup record");
        let candidate_path = path("candidates/000.txt");
        let candidate = CandidateArtifactEntryV1::new(
            &precursor,
            &qualification.planned_attempt,
            &qualification.case,
            0,
            candidate_path.clone(),
            candidate_text.as_bytes(),
        )
        .expect("compile candidate entry");
        let response_input = StructuredResponseArtifactV1Input::new(
            structured_response.content_artifact_id().clone(),
            structured_response.byte_size(),
        )
        .expect("compile structured response input");
        let entries = manifest_entries(
            &qualification.planned_attempt,
            &precursor,
            &managed_evidence,
            &structured_response,
            &cleanup,
            &candidate,
        );
        let manifest = CandidateGenerationEvidenceBundleManifestV1::new(
            CandidateGenerationEvidenceBundleManifestV1Relations {
                qualification_plan: &qualification.qualification_plan,
                planned_attempt: &qualification.planned_attempt,
                precursor: &precursor,
                managed_evidence: &managed_evidence,
                cleanup: &cleanup,
                structured_response_artifact: &response_input,
            },
            entries,
            vec![candidate],
        )
        .expect("compile bundle manifest");
        let effective_package = qualification.characterized_package.evidence().clone();

        Self {
            qualification_plan: qualification.qualification_plan,
            cluster: qualification.cluster,
            suite: qualification.suite,
            repetition: qualification.repetition,
            generation_system: qualification.generation_system,
            effective_package,
            planned_attempt: qualification.planned_attempt,
            case_manifest: qualification.case,
            precursor,
            managed_evidence,
            cleanup,
            structured_response,
            structured_request,
            manifest,
            candidate_path,
        }
    }

    pub(super) fn plan_input(&self) -> CandidateGenerationEvidenceBundlePublicationPlanInput<'_> {
        CandidateGenerationEvidenceBundlePublicationPlanInput {
            qualification_plan: &self.qualification_plan,
            planned_attempt: &self.planned_attempt,
            case_manifest: &self.case_manifest,
            precursor: &self.precursor,
            managed_evidence: &self.managed_evidence,
            cleanup: &self.cleanup,
            structured_response: &self.structured_response,
            structured_request: &self.structured_request,
            auxiliary_artifacts: &[],
            manifest: &self.manifest,
        }
    }

    pub(super) fn manifest_for_candidate(
        &self,
        bytes: &[u8],
    ) -> CandidateGenerationEvidenceBundleManifestV1 {
        let candidate = CandidateArtifactEntryV1::new(
            &self.precursor,
            &self.planned_attempt,
            &self.case_manifest,
            0,
            self.candidate_path.clone(),
            bytes,
        )
        .expect("compile alternate candidate entry");
        let response_input = StructuredResponseArtifactV1Input::new(
            self.structured_response.content_artifact_id().clone(),
            self.structured_response.byte_size(),
        )
        .expect("compile structured response input");
        CandidateGenerationEvidenceBundleManifestV1::new(
            CandidateGenerationEvidenceBundleManifestV1Relations {
                qualification_plan: &self.qualification_plan,
                planned_attempt: &self.planned_attempt,
                precursor: &self.precursor,
                managed_evidence: &self.managed_evidence,
                cleanup: &self.cleanup,
                structured_response_artifact: &response_input,
            },
            manifest_entries(
                &self.planned_attempt,
                &self.precursor,
                &self.managed_evidence,
                &self.structured_response,
                &self.cleanup,
                &candidate,
            ),
            vec![candidate],
        )
        .expect("compile alternate manifest")
    }
}

fn response(
    request: &StructuredCompletionRequest,
    candidate_text: &str,
) -> StructuredCompletionResponse {
    StructuredCompletionResponse::complete(
        request,
        runtime(),
        request.artifact_id.clone(),
        request.artifact_digest.clone(),
        serde_json::json!({"candidates": [{"text": candidate_text}]}).to_string(),
        UsageObservation {
            input_tokens: Some(11),
            output_tokens: Some(3),
            generation_micros: Some(1_250),
        },
    )
    .expect("complete response")
}

fn runtime() -> RuntimeIdentity {
    RuntimeIdentity {
        backend: "ollama_native".to_owned(),
        version: "0.32.15".to_owned(),
        digest: Some(digest("publisher runtime")),
    }
}

fn session_receipt(response: &StructuredCompletionResponse) -> OllamaSessionExecutionReceipt {
    OllamaSessionExecutionReceipt::for_test(&preflight(), response, 8, 16)
        .expect("retained response receipt")
}

fn preflight() -> OllamaPreflight {
    let running = OllamaRunningModel {
        reference: "fixture:latest".to_owned(),
        inventory_digest: digest("publisher inventory"),
        byte_size: 4_096,
        accelerator_bytes: 512,
        context_tokens: 4_096,
    };
    OllamaPreflight {
        runtime: runtime(),
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
                license_digest: digest("publisher license"),
                template_digest: digest("publisher template"),
                metadata_digest: digest("publisher metadata"),
            },
        }],
        running: vec![running],
    }
}

fn manifest_entries(
    planned: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    managed: &ManagedOllamaCandidateGenerationEvidenceV2,
    response: &RetainedStructuredResponseArtifactV1,
    cleanup: &CandidateGenerationCleanupRecordV1,
    candidate: &CandidateArtifactEntryV1,
) -> Vec<CandidateGenerationEvidenceBundleEntryV1> {
    let mut entries = vec![
        serialized_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            planned,
        ),
        serialized_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            precursor,
        ),
        serialized_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            managed,
        ),
        bytes_entry(
            "records/response.json",
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            response.canonical_json_bytes(),
        ),
        serialized_entry(
            "records/cleanup.json",
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            cleanup,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            candidate.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::Candidate,
            candidate.artifact_id().clone(),
            candidate.byte_count(),
        ),
    ];
    entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    entries
}

fn serialized_entry(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    record: &impl serde::Serialize,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    bytes_entry(
        relative_path,
        role,
        &serde_json::to_vec(record).expect("canonical fixture record"),
    )
}

fn bytes_entry(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    bytes: &[u8],
) -> CandidateGenerationEvidenceBundleEntryV1 {
    CandidateGenerationEvidenceBundleEntryV1::new(
        path(relative_path),
        role,
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture byte size"),
    )
}

pub(super) fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value.to_owned()).expect("portable fixture path")
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
