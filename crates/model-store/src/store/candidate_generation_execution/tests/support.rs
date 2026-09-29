use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, CandidateArtifactEntryV1,
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationAttemptPrecursorV1Input,
    CandidateGenerationCleanupRecordV1, CandidateGenerationCleanupRecordV1Input,
    CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationEvidenceBundleRoleV1,
    CandidateGenerationPackageRevalidationStatusV1, CandidateGenerationProcessCleanupStatusV1,
    CandidateGenerationReceiptV1, CandidateGenerationReceiptV1Relations,
    CandidateGenerationUsageObservationV1, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, OllamaRetainedSessionResponseId,
    StructuredResponseArtifactV1Input,
};
use rewrite_types::Digest;

use super::super::*;
use crate::store::candidate_generation_attempt_precursor::CandidateGenerationAttemptPrecursorCheckpointV1Input;
use crate::store::generation_qualification_preregistration::{
    GenerationQualificationPreregistrationReadInput, GenerationQualificationPreregistrationV1Input,
    tests::support::{self as prereg_support, Fixture},
};
use crate::{
    ArtifactStateStore, CandidateGenerationEvidenceStorageRootId,
    CandidateGenerationEvidenceStorageV1Limits, WriteDisposition,
};

pub(super) fn preregistration_input(
    fixture: &Fixture,
) -> GenerationQualificationPreregistrationV1Input<'_> {
    GenerationQualificationPreregistrationV1Input {
        operation_policy: &fixture.policy,
        operation_policy_relations: fixture.relations(),
        operation_policy_input: &fixture.policy_input,
        request_projection: &fixture.projection,
        request_projection_entry_inputs: &fixture.entry_inputs,
    }
}

pub(crate) fn read_input(fixture: &Fixture) -> GenerationQualificationPreregistrationReadInput<'_> {
    GenerationQualificationPreregistrationReadInput {
        operation_policy_id: fixture.policy.operation_policy_id(),
        request_projection_id: fixture.projection.request_projection_id(),
        operation_policy_relations: fixture.relations(),
        operation_policy_input: &fixture.policy_input,
        request_projection_entry_inputs: &fixture.entry_inputs,
    }
}

pub(crate) fn prepared_store(path: &std::path::Path, fixture: &Fixture) -> ArtifactStateStore {
    let mut store = ArtifactStateStore::open(path).expect("open store");
    prereg_support::persist_plan_foundation(&mut store, fixture);
    store
        .transact_generation_qualification_preregistration(
            preregistration_input(fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("persist preregistration");
    store
}

pub(super) fn precursor(fixture: &Fixture) -> CandidateGenerationAttemptPrecursorV1 {
    CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        &fixture.attempts[0],
        &fixture.systems[0],
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: 1,
            model_installation_generation: 1,
            structured_request_binding_id: fixture.entry_inputs[0]
                .structured_completion_request_binding_id
                .clone(),
        },
    )
    .expect("candidate precursor")
}

pub(crate) fn checkpoint_precursor(
    store: &mut ArtifactStateStore,
    fixture: &Fixture,
    precursor: &CandidateGenerationAttemptPrecursorV1,
) {
    let ((), disposition) = store
        .transact_candidate_generation_attempt_precursor_checkpoint_v1(
            CandidateGenerationAttemptPrecursorCheckpointV1Input {
                precursor,
                preregistration: read_input(fixture),
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("checkpoint precursor");
    assert_eq!(disposition.precursor, WriteDisposition::Inserted);
}

pub(crate) struct CompletedFixture {
    /// Checkpointed precursor required by the completed attempt.
    pub(crate) precursor: CandidateGenerationAttemptPrecursorV1,
    /// Managed-evidence facts consumed while rederiving the attempt ledger.
    pub(crate) managed_input: ManagedOllamaCandidateGenerationEvidenceV2Input,
    pub(super) managed: ManagedOllamaCandidateGenerationEvidenceV2,
    pub(super) cleanup: CandidateGenerationCleanupRecordV1,
    pub(super) bundle: CandidateGenerationEvidenceBundleManifestV1,
    pub(super) storage: CandidateGenerationEvidenceBundleStorageV1,
    pub(super) readback: CandidateGenerationEvidenceBundleReadbackV1,
    pub(crate) receipt: CandidateGenerationReceiptV1,
    /// Exact completed terminal record.
    pub(crate) attempt: rewrite_model::CandidateGenerationAttemptRecordV1,
}

#[expect(
    clippy::too_many_lines,
    reason = "complete successful execution fixture"
)]
pub(crate) fn completed(fixture: &Fixture) -> CompletedFixture {
    let planned = &fixture.attempts[0];
    let precursor = precursor(fixture);
    let managed_input = ManagedOllamaCandidateGenerationEvidenceV2Input {
        bracket_observation_v1_id:
            ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest("bracket")),
        effective_runtime_state_join_id:
            ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest("runtime join")),
        response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest("response")),
    };
    let managed = ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: planned,
            generation_system: &fixture.systems[0],
            effective_package_evidence_v2: &fixture.system.effective_package,
        },
        managed_input.clone(),
    )
    .expect("managed evidence");
    let cleanup = CandidateGenerationCleanupRecordV1::new(
        &precursor,
        &managed,
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Succeeded,
            runtime_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            model_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
        },
    )
    .expect("cleanup");
    let response = b"structured response";
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(response)),
        u64::try_from(response.len()).expect("response length"),
    )
    .expect("response artifact");
    let candidate_bytes = b"candidate";
    let candidate = CandidateArtifactEntryV1::new(
        &precursor,
        planned,
        &fixture.cases[0],
        0,
        path("candidates/000.txt"),
        candidate_bytes,
    )
    .expect("candidate");
    let mut entries = vec![
        record_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            planned,
        ),
        record_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            &precursor,
        ),
        record_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            &managed,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            path("records/response.json"),
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            response_artifact.artifact_id().clone(),
            u64::try_from(response.len()).expect("response length"),
        ),
        record_entry(
            "records/cleanup.json",
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            &cleanup,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            candidate.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::Candidate,
            candidate.artifact_id().clone(),
            candidate.byte_count(),
        ),
    ];
    entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &fixture.plan,
            planned_attempt: planned,
            precursor: &precursor,
            managed_evidence: &managed,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
        entries,
        vec![candidate],
    )
    .expect("bundle");
    let storage = CandidateGenerationEvidenceBundleStorageV1::new(
        CandidateGenerationEvidenceStorageRootId::new(digest("storage root").as_str().to_owned())
            .expect("storage root"),
        fixture.plan.qualification_plan_id().clone(),
        planned.planned_attempt_id().clone(),
        bundle.evidence_bundle_id().clone(),
        CandidateGenerationEvidenceStorageV1Limits::new(256, 64, 1_048_576)
            .expect("storage limits"),
    )
    .expect("storage registration");
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(&bundle).expect("readback");
    let receipt = CandidateGenerationReceiptV1::new(
        CandidateGenerationReceiptV1Relations {
            qualification_plan: &fixture.plan,
            suite: &fixture.suite,
            case: &fixture.cases[0],
            cluster: &fixture.clusters[0],
            repetition: &fixture.repetitions[0],
            planned_attempt: planned,
            precursor: &precursor,
            generation_system: &fixture.systems[0],
            managed_evidence: &managed,
            cleanup: &cleanup,
            bundle: &bundle,
            readback: &readback,
        },
        CandidateGenerationUsageObservationV1::new(Some(2), Some(3), Some(5)),
    )
    .expect("receipt");
    let attempt =
        rewrite_model::CandidateGenerationAttemptRecordV1::completed(planned, &precursor, &receipt)
            .expect("completed attempt");
    CompletedFixture {
        precursor,
        managed_input,
        managed,
        cleanup,
        bundle,
        storage,
        readback,
        receipt,
        attempt,
    }
}

pub(crate) fn completed_input<'a>(
    fixture: &'a Fixture,
    completed: &'a CompletedFixture,
) -> CandidateGenerationExecutionV1Input<'a> {
    CandidateGenerationExecutionV1Input::Completed {
        preregistration: read_input(fixture),
        precursor: &completed.precursor,
        managed_evidence: &completed.managed,
        managed_evidence_input: &completed.managed_input,
        cleanup: &completed.cleanup,
        bundle: &completed.bundle,
        storage: &completed.storage,
        readback: &completed.readback,
        receipt: &completed.receipt,
        attempt: &completed.attempt,
    }
}

pub(super) fn insert_managed_prefix(store: &ArtifactStateStore, completed: &CompletedFixture) {
    let value = &completed.managed;
    let canonical_json = serde_json::to_vec(value).expect("managed JSON");
    store
        .connection()
        .execute(
            "INSERT INTO managed_candidate_generation_evidence (
                managed_candidate_generation_evidence_id,
                candidate_generation_attempt_precursor_id, bracket_observation_v1_id,
                effective_package_evidence_v2_id, effective_runtime_state_id,
                effective_runtime_state_join_id, generation_request_binding_id,
                structured_request_binding_id, response_id, canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                value.managed_evidence_v2_id().digest().as_str(),
                value.precursor_id().digest().as_str(),
                value.bracket_observation_v1_id().digest().as_str(),
                value.effective_package_evidence_v2_id().digest().as_str(),
                value.effective_runtime_state_id().digest().as_str(),
                value.effective_runtime_state_join_id().digest().as_str(),
                value.generation_request_binding_id().digest().as_str(),
                value.structured_request_binding_id().digest().as_str(),
                value.response_id().digest().as_str(),
                canonical_json,
            ],
        )
        .expect("insert managed prefix");
}

fn record_entry<T: serde::Serialize>(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    record: &T,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    let bytes = serde_json::to_vec(record).expect("record JSON");
    CandidateGenerationEvidenceBundleEntryV1::new(
        path(relative_path),
        role,
        ArtifactId::from_digest(Digest::sha256(&bytes)),
        u64::try_from(bytes.len()).expect("record length"),
    )
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}
