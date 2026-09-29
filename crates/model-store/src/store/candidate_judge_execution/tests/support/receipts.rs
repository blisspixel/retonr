use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, CandidateArtifactEntryV1,
    CandidateGenerationAttemptPrecursorV1, CandidateGenerationAttemptPrecursorV1Input,
    CandidateGenerationAttemptRecordV1, CandidateGenerationCleanupRecordV1,
    CandidateGenerationCleanupRecordV1Input, CandidateGenerationEvidenceBundleEntryV1,
    CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationEvidenceBundleRoleV1,
    CandidateGenerationPackageRevalidationStatusV1, CandidateGenerationProcessCleanupStatusV1,
    CandidateGenerationReceiptSetV1, CandidateGenerationReceiptSetV1Relations,
    CandidateGenerationReceiptV1, CandidateGenerationReceiptV1Relations,
    CandidateGenerationUsageObservationV1, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, OllamaRetainedSessionResponseId,
    StructuredResponseArtifactV1Input,
};
use rewrite_types::Digest;

use super::digest;
use crate::store::generation_qualification_preregistration::tests::support::Fixture;

struct BuiltAttempt {
    receipt: CandidateGenerationReceiptV1,
    attempt: CandidateGenerationAttemptRecordV1,
}

pub(super) fn receipt_set(fixture: &Fixture, index: usize) -> CandidateGenerationReceiptSetV1 {
    let built = build_attempt(fixture, index);
    CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        repetition: &fixture.repetitions[0],
        generation_system: &fixture.systems[index],
        selection_policy: &fixture.selection_policy,
        planned_attempts: &fixture.attempts,
        attempt_records: std::slice::from_ref(&built.attempt),
        receipts: std::slice::from_ref(&built.receipt),
    })
    .expect("receipt set")
}

fn build_attempt(fixture: &Fixture, index: usize) -> BuiltAttempt {
    let precursor = precursor(fixture, index);
    let managed = managed(fixture, &precursor, index);
    let cleanup = cleanup(&precursor, &managed);
    let closed = close_attempt(fixture, index, &precursor, &managed, &cleanup);
    BuiltAttempt {
        receipt: closed.0,
        attempt: closed.1,
    }
}

fn precursor(fixture: &Fixture, index: usize) -> CandidateGenerationAttemptPrecursorV1 {
    let generation = u64::try_from(index).expect("index") + 1;
    CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        &fixture.attempts[index],
        &fixture.systems[index],
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: generation,
            model_installation_generation: generation + 10,
            structured_request_binding_id: fixture.entry_inputs[index]
                .structured_completion_request_binding_id
                .clone(),
        },
    )
    .expect("precursor")
}

fn managed(
    fixture: &Fixture,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    index: usize,
) -> ManagedOllamaCandidateGenerationEvidenceV2 {
    let input = ManagedOllamaCandidateGenerationEvidenceV2Input {
        bracket_observation_v1_id:
            ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(&format!(
                "judge bracket {index}"
            ))),
        effective_runtime_state_join_id:
            ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest(&format!(
                "judge runtime join {index}"
            ))),
        response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest(&format!(
            "judge retained response {index}"
        ))),
    };
    ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor,
            planned_attempt: &fixture.attempts[index],
            generation_system: &fixture.systems[index],
            effective_package_evidence_v2: &fixture.system.effective_package,
        },
        input,
    )
    .expect("managed evidence")
}

fn cleanup(
    precursor: &CandidateGenerationAttemptPrecursorV1,
    managed: &ManagedOllamaCandidateGenerationEvidenceV2,
) -> CandidateGenerationCleanupRecordV1 {
    CandidateGenerationCleanupRecordV1::new(
        precursor,
        managed,
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Succeeded,
            runtime_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            model_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
        },
    )
    .expect("cleanup")
}

fn close_attempt(
    fixture: &Fixture,
    index: usize,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    managed: &ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: &CandidateGenerationCleanupRecordV1,
) -> (
    CandidateGenerationReceiptV1,
    CandidateGenerationAttemptRecordV1,
) {
    let planned = &fixture.attempts[index];
    let response = format!("response {index}").into_bytes();
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(&response)),
        u64::try_from(response.len()).expect("response length"),
    )
    .expect("response artifact");
    let candidate_bytes = format!("candidate {index}").into_bytes();
    let candidate = CandidateArtifactEntryV1::new(
        precursor,
        planned,
        &fixture.cases[0],
        0,
        path(&format!("candidates/{index:03}.txt")),
        &candidate_bytes,
    )
    .expect("candidate");
    let bundle = bundle(
        fixture,
        planned,
        precursor,
        managed,
        cleanup,
        &response_artifact,
        &candidate,
    );
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(&bundle).expect("readback");
    let receipt = CandidateGenerationReceiptV1::new(
        CandidateGenerationReceiptV1Relations {
            qualification_plan: &fixture.plan,
            suite: &fixture.suite,
            case: &fixture.cases[0],
            cluster: &fixture.clusters[0],
            repetition: &fixture.repetitions[0],
            planned_attempt: planned,
            precursor,
            generation_system: &fixture.systems[index],
            managed_evidence: managed,
            cleanup,
            bundle: &bundle,
            readback: &readback,
        },
        CandidateGenerationUsageObservationV1::new(Some(2), Some(3), Some(5)),
    )
    .expect("receipt");
    let attempt = CandidateGenerationAttemptRecordV1::completed(planned, precursor, &receipt)
        .expect("attempt");
    (receipt, attempt)
}

fn bundle(
    fixture: &Fixture,
    planned: &rewrite_model::PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    managed: &ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: &CandidateGenerationCleanupRecordV1,
    response_artifact: &StructuredResponseArtifactV1Input,
    candidate: &CandidateArtifactEntryV1,
) -> CandidateGenerationEvidenceBundleManifestV1 {
    let mut entries = vec![
        record_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            planned,
        ),
        record_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            precursor,
        ),
        record_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            managed,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            path("records/response.json"),
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            response_artifact.artifact_id().clone(),
            response_artifact.byte_size(),
        ),
        record_entry(
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
    CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &fixture.plan,
            planned_attempt: planned,
            precursor,
            managed_evidence: managed,
            cleanup,
            structured_response_artifact: response_artifact,
        },
        entries,
        vec![candidate.clone()],
    )
    .expect("bundle")
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
