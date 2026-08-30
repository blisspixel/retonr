use super::super::*;
use crate::generation_qualification::operation_contracts::operation_policy_test_support;
use crate::generation_qualification::*;
use crate::{ArtifactId, ArtifactSetRelativePath};
use rewrite_types::Digest;

pub(super) struct ResourceFixture {
    pub(super) base: operation_policy_test_support::Fixture,
    pub(super) receipt: CandidateGenerationReceiptV1,
    pub(super) alternate_receipt: CandidateGenerationReceiptV1,
    pub(super) attempt_record: CandidateGenerationAttemptRecordV1,
}

struct ResourceUpstream {
    precursor: CandidateGenerationAttemptPrecursorV1,
    managed_evidence: ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: CandidateGenerationCleanupRecordV1,
    bundle: CandidateGenerationEvidenceBundleManifestV1,
    readback: CandidateGenerationEvidenceBundleReadbackV1,
}

impl ResourceFixture {
    pub(super) fn scope(&self) -> GenerationQualificationPhaseScopeV1<'_> {
        GenerationQualificationPhaseScopeV1 {
            generation_system: &self.base.systems[0],
            qualification_plan: &self.base.plan,
            suite: &self.base.suite,
        }
    }

    pub(super) fn relations(&self) -> GenerationResourceAttemptResultRecordV1Relations<'_> {
        GenerationResourceAttemptResultRecordV1Relations {
            scope: self.scope(),
            operation_policy: &self.base.policy,
            case: &self.base.cases[0],
            repetition: &self.base.repetitions[0],
            planned_attempt: &self.base.attempts[0],
            attempt_record: &self.attempt_record,
            candidate_generation_receipt: &self.receipt,
        }
    }
}

pub(super) fn input() -> GenerationResourceAttemptResultRecordV1Input {
    GenerationResourceAttemptResultRecordV1Input {
        prompt_token_count: 11,
        generated_token_count: 17,
        total_duration_nanoseconds: 30_000_000,
        load_duration_nanoseconds: 1_000_000,
        prompt_evaluation_duration_nanoseconds: 2_000_000,
        evaluation_duration_nanoseconds: 23_000_999,
        attempt_elapsed_nanoseconds: 40_000_000,
        first_response_elapsed_nanoseconds: 5_000_000,
        cleanup_elapsed_nanoseconds: 4_000_000,
        worker_high_water_resident_bytes: 8_192,
        runtime_installed_payload_bytes: 100,
        model_installed_payload_bytes: 900,
        installed_footprint_bytes: 1_000,
        exceeded_limits: vec![
            GenerationResourceExceededLimitV1::FirstResponse,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
        ],
    }
}

pub(super) fn fixture() -> ResourceFixture {
    fixture_with_usage(CandidateGenerationUsageObservationV1::new(
        Some(11),
        Some(17),
        Some(23_000),
    ))
}

pub(super) fn zero_fixture() -> ResourceFixture {
    fixture_with_usage(CandidateGenerationUsageObservationV1::new(
        Some(0),
        Some(0),
        Some(0),
    ))
}

pub(super) fn fixture_with_usage(usage: CandidateGenerationUsageObservationV1) -> ResourceFixture {
    let base = operation_policy_test_support::fixture();
    let upstream = build_upstream(&base);
    let receipt = CandidateGenerationReceiptV1::new(receipt_relations(&base, &upstream), usage)
        .expect("receipt");
    let alternate_receipt = CandidateGenerationReceiptV1::new(
        receipt_relations(&base, &upstream),
        CandidateGenerationUsageObservationV1::new(Some(101), Some(102), Some(103)),
    )
    .expect("alternate receipt");
    let attempt_record = CandidateGenerationAttemptRecordV1::completed(
        &base.attempts[0],
        &upstream.precursor,
        &receipt,
    )
    .expect("attempt record");
    ResourceFixture {
        base,
        receipt,
        alternate_receipt,
        attempt_record,
    }
}

fn build_upstream(base: &operation_policy_test_support::Fixture) -> ResourceUpstream {
    let planned = &base.attempts[0];
    let system = &base.systems[0];
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        &base.plan,
        planned,
        system,
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: 7,
            model_installation_generation: 11,
            structured_request_binding_id:
                StructuredCompletionRequestBindingId::from_derived_digest(digest("structured")),
        },
    )
    .expect("precursor");
    let managed_evidence = ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: planned,
            generation_system: system,
            effective_package_evidence_v2: &base.system_fixture.effective_package,
        },
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id:
                ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(
                    "bracket",
                )),
            effective_runtime_state_join_id:
                ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest("live join")),
            response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest("response")),
        },
    )
    .expect("managed evidence");
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
    .expect("cleanup");
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(b"response")),
        8,
    )
    .expect("response artifact");
    let candidate = CandidateArtifactEntryV1::new(
        &precursor,
        planned,
        &base.cases[0],
        0,
        path("candidates/000.txt"),
        b"candidate",
    )
    .expect("candidate");
    let candidates = vec![candidate];
    let bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &base.plan,
            planned_attempt: planned,
            precursor: &precursor,
            managed_evidence: &managed_evidence,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
        bundle_entries(
            planned,
            &precursor,
            &managed_evidence,
            &cleanup,
            &candidates,
        ),
        candidates,
    )
    .expect("bundle");
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(&bundle).expect("readback");
    ResourceUpstream {
        precursor,
        managed_evidence,
        cleanup,
        bundle,
        readback,
    }
}

fn receipt_relations<'a>(
    base: &'a operation_policy_test_support::Fixture,
    upstream: &'a ResourceUpstream,
) -> CandidateGenerationReceiptV1Relations<'a> {
    CandidateGenerationReceiptV1Relations {
        qualification_plan: &base.plan,
        suite: &base.suite,
        case: &base.cases[0],
        cluster: &base.cluster,
        repetition: &base.repetitions[0],
        planned_attempt: &base.attempts[0],
        precursor: &upstream.precursor,
        generation_system: &base.systems[0],
        managed_evidence: &upstream.managed_evidence,
        cleanup: &upstream.cleanup,
        bundle: &upstream.bundle,
        readback: &upstream.readback,
    }
}

fn bundle_entries(
    planned: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    managed: &ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: &CandidateGenerationCleanupRecordV1,
    candidates: &[CandidateArtifactEntryV1],
) -> Vec<CandidateGenerationEvidenceBundleEntryV1> {
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
        entry(
            "records/response.json",
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            b"response",
        ),
        record_entry(
            "records/cleanup.json",
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            cleanup,
        ),
    ];
    entries.extend(candidates.iter().map(|candidate| {
        CandidateGenerationEvidenceBundleEntryV1::new(
            candidate.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::Candidate,
            candidate.artifact_id().clone(),
            candidate.byte_count(),
        )
    }));
    entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    entries
}

fn record_entry<T: serde::Serialize>(
    path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    value: &T,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    entry(path, role, &serde_json::to_vec(value).expect("record JSON"))
}

fn entry(
    path_value: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    bytes: &[u8],
) -> CandidateGenerationEvidenceBundleEntryV1 {
    CandidateGenerationEvidenceBundleEntryV1::new(
        path(path_value),
        role,
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("byte size"),
    )
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}
