use super::*;
use crate::{ArtifactSetRelativePath, EffectivePackageEvidenceV2};

struct ReceiptSetFixture {
    suite: GenerationSuiteManifestV1,
    repetition: GenerationRepetitionRecordV1,
    system: GenerationSystemRecordV1,
    policy: CandidateSelectionPolicyV1,
    plan: GenerationQualificationPlanV1,
    planned: PlannedCandidateAttemptV1,
    precursor: CandidateGenerationAttemptPrecursorV1,
    effective_package: EffectivePackageEvidenceV2,
    managed: ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: CandidateGenerationCleanupRecordV1,
    bundle: CandidateGenerationEvidenceBundleManifestV1,
    readback: CandidateGenerationEvidenceBundleReadbackV1,
    receipt: CandidateGenerationReceiptV1,
    attempt: CandidateGenerationAttemptRecordV1,
}

impl ReceiptSetFixture {
    fn relations(&self) -> CandidateGenerationReceiptSetV1Relations<'_> {
        CandidateGenerationReceiptSetV1Relations {
            qualification_plan: &self.plan,
            suite: &self.suite,
            repetition: &self.repetition,
            generation_system: &self.system,
            selection_policy: &self.policy,
            planned_attempts: std::slice::from_ref(&self.planned),
            attempt_records: std::slice::from_ref(&self.attempt),
            receipts: std::slice::from_ref(&self.receipt),
        }
    }
}

fn local_path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn local_entry<T: serde::Serialize>(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    record: &T,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    let bytes = serde_json::to_vec(record).expect("record JSON");
    CandidateGenerationEvidenceBundleEntryV1::new(
        local_path(relative_path),
        role,
        ArtifactId::from_digest(Digest::sha256(&bytes)),
        u64::try_from(bytes.len()).expect("byte count"),
    )
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture assembles the complete retained receipt chain explicitly"
)]
fn receipt_set_fixture() -> ReceiptSetFixture {
    let cluster =
        GenerationClusterRecordV1::new("single-case", digest("cluster policy")).expect("cluster");
    let case = case(&cluster, "single-case", "source");
    let suite =
        GenerationSuiteManifestV1::new(digest("single protocol"), std::slice::from_ref(&case))
            .expect("suite");
    let repetition =
        GenerationRepetitionRecordV1::new(&suite, 0, digest("repeat")).expect("repetition");
    let support =
        super::super::super::super::super::generation_system::test_support::fixture(false);
    let system = support.record();
    let policy = CandidateSelectionPolicyV1::new(&suite, &[1]).expect("policy");
    let planned = attempt_record(&suite, &case, &cluster, &repetition, &system, 0);
    let plan = GenerationQualificationPlanV1::new(
        &suite,
        std::slice::from_ref(&repetition),
        std::slice::from_ref(&system),
        std::slice::from_ref(&planned),
        GenerationQualificationPlanV1Input {
            limits: limits(1),
            selection_policy_digest: policy.selection_policy_id().digest().clone(),
            failure_policy_digest: digest("failure policy"),
        },
    )
    .expect("plan");
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        &plan,
        &planned,
        &system,
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: 7,
            model_installation_generation: 11,
            structured_request_binding_id:
                StructuredCompletionRequestBindingId::from_derived_digest(digest("wire request")),
        },
    )
    .expect("precursor");
    let managed = ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: &planned,
            generation_system: &system,
            effective_package_evidence_v2: &support.effective_package,
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
    let response_bytes = b"response";
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(response_bytes)),
        u64::try_from(response_bytes.len()).expect("response size"),
    )
    .expect("response artifact");
    let candidate_bytes = [
        b"first candidate".as_slice(),
        b"second candidate".as_slice(),
    ];
    let candidates = candidate_bytes
        .iter()
        .enumerate()
        .map(|(ordinal, bytes)| {
            CandidateArtifactEntryV1::new(
                &precursor,
                &planned,
                &case,
                u8::try_from(ordinal).expect("ordinal"),
                local_path(&format!("candidates/{ordinal:03}.txt")),
                bytes,
            )
            .expect("candidate")
        })
        .collect::<Vec<_>>();
    let mut entries = vec![
        local_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            &planned,
        ),
        local_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            &precursor,
        ),
        local_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            &managed,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            local_path("records/response.json"),
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            ArtifactId::from_digest(Digest::sha256(response_bytes)),
            u64::try_from(response_bytes.len()).expect("response size"),
        ),
        local_entry(
            "records/cleanup.json",
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            &cleanup,
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
    let bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &plan,
            planned_attempt: &planned,
            precursor: &precursor,
            managed_evidence: &managed,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
        entries,
        candidates,
    )
    .expect("bundle");
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(&bundle).expect("readback");
    let receipt = CandidateGenerationReceiptV1::new(
        CandidateGenerationReceiptV1Relations {
            qualification_plan: &plan,
            suite: &suite,
            case: &case,
            cluster: &cluster,
            repetition: &repetition,
            planned_attempt: &planned,
            precursor: &precursor,
            generation_system: &system,
            managed_evidence: &managed,
            cleanup: &cleanup,
            bundle: &bundle,
            readback: &readback,
        },
        CandidateGenerationUsageObservationV1::new(Some(3), Some(5), Some(8)),
    )
    .expect("receipt");
    let attempt = CandidateGenerationAttemptRecordV1::completed(&planned, &precursor, &receipt)
        .expect("completed attempt");
    ReceiptSetFixture {
        suite,
        repetition,
        system,
        policy,
        plan,
        planned,
        precursor,
        effective_package: support.effective_package,
        managed,
        cleanup,
        bundle,
        readback,
        receipt,
        attempt,
    }
}

#[test]
fn selection_policy_and_receipt_set_round_trip_with_stable_identity() {
    let fixture = receipt_set_fixture();
    fixture
        .policy
        .validate_against_plan(&fixture.plan, std::slice::from_ref(&fixture.planned))
        .expect("policy plan closure");
    assert_eq!(fixture.policy.schema_version(), 1);
    assert_eq!(
        fixture.policy.suite_manifest_id(),
        fixture.suite.suite_manifest_id(),
    );
    assert_eq!(fixture.policy.entry_count(), 1);
    let policy_debug = format!("{:?}", fixture.policy);
    assert!(policy_debug.contains(fixture.policy.selection_policy_id().digest().as_str()));
    assert!(!policy_debug.contains(fixture.suite.case_ids()[0].digest().as_str()));
    let policy_bytes = serde_json::to_vec(&fixture.policy).expect("policy JSON");
    assert_eq!(
        CandidateSelectionPolicyV1::from_json_bytes(&policy_bytes, &fixture.suite)
            .expect("policy decode"),
        fixture.policy,
    );
    let set = CandidateGenerationReceiptSetV1::new(fixture.relations()).expect("receipt set");
    assert_eq!(set.schema_version(), 1);
    assert_eq!(
        set.qualification_plan_id(),
        fixture.plan.qualification_plan_id(),
    );
    assert_eq!(set.suite_manifest_id(), fixture.suite.suite_manifest_id());
    assert_eq!(set.repetition_id(), fixture.repetition.repetition_id());
    assert_eq!(
        set.generation_system_id(),
        fixture.system.generation_system_id(),
    );
    assert_eq!(
        set.selection_policy_id(),
        fixture.policy.selection_policy_id(),
    );
    assert_eq!(set.entry_count(), 1);
    assert_eq!(set.entries()[0].selected_ordinal(), 1);
    assert_eq!(
        set.entries()[0].selected_candidate_evidence_id(),
        fixture.receipt.candidate_entries()[1].candidate_evidence_id(),
    );
    let set_bytes = serde_json::to_vec(&set).expect("set JSON");
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(&set_bytes, fixture.relations())
            .expect("set decode"),
        set,
    );
    assert_eq!(
        CANDIDATE_SELECTION_POLICY_ID_DOMAIN,
        b"retonr:candidate-selection-policy:v1\0",
    );
    assert_eq!(
        CANDIDATE_GENERATION_RECEIPT_SET_ID_DOMAIN,
        b"retonr:candidate-generation-receipt-set:v1\0",
    );
    assert_eq!(
        policy_bytes,
        br#"{"schema_version":1,"suite_manifest_id":"cb8e7606b70c52e5eecdbfed2ecdbf0a43fb8e407d81990d67c807e6d6cb3b18","entries":[{"case_id":"e70e87009e01e2633bcdfb4bfc8bf2d5c90d11a3bc7be64241a1daa40d470ccc","selected_ordinal":1}],"entry_count":1}"#,
    );
    assert_eq!(
        fixture.policy.selection_policy_id().digest().as_str(),
        "a8fba500aedc6aeab73a94e881dd19130ef21f8041c42bc6fa4f869111bf289e",
    );
    assert_eq!(
        set.receipt_set_id().digest().as_str(),
        "d5a5b7aec00490415ed8cc7e129e8e4cee36c7bab069d1e2a15dd545b4d0370b",
    );
}

#[test]
fn policy_parser_rejects_untrusted_encodings_and_relationship_substitution() {
    let fixture = receipt_set_fixture();
    assert_eq!(
        CandidateSelectionPolicyV1::new(&fixture.suite, &[]),
        Err(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch),
    );
    let encoded = serde_json::to_vec(&fixture.policy).expect("policy JSON");
    let mut trailing = encoded.clone();
    trailing.push(b' ');
    let cases = [
        (
            vec![b' '; MAX_CANDIDATE_SELECTION_POLICY_JSON_BYTES + 1],
            GenerationQualificationContractError::EncodedRecordTooLarge,
        ),
        (
            b"{".to_vec(),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2"),
            GenerationQualificationContractError::UnsupportedSchema(2),
        ),
        (
            replace_once(
                &encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"unknown\":true",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(
                &encoded,
                "\"schema_version\":1",
                "\"schema_version\":1,\"schema_version\":1",
            ),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            replace_once(&encoded, ",\"entry_count\":1", ""),
            GenerationQualificationContractError::InvalidEncoding,
        ),
        (
            serde_json::to_vec(
                &serde_json::from_slice::<serde_json::Value>(&encoded).expect("policy value"),
            )
            .expect("reordered policy"),
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
        (
            trailing,
            GenerationQualificationContractError::NonCanonicalEncoding,
        ),
    ];
    for (bytes, error) in cases {
        assert_eq!(
            CandidateSelectionPolicyV1::from_json_bytes(&bytes, &fixture.suite),
            Err(error),
        );
    }
    let changed = replace_once(&encoded, "\"entry_count\":1", "\"entry_count\":0");
    assert_eq!(
        CandidateSelectionPolicyV1::from_json_bytes(&changed, &fixture.suite),
        Err(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch),
    );
    let out_of_range = CandidateSelectionPolicyV1::new(&fixture.suite, &[2]).expect("policy");
    let changed_plan = GenerationQualificationPlanV1::new(
        &fixture.suite,
        std::slice::from_ref(&fixture.repetition),
        std::slice::from_ref(&fixture.system),
        std::slice::from_ref(&fixture.planned),
        GenerationQualificationPlanV1Input {
            limits: limits(1),
            selection_policy_digest: out_of_range.selection_policy_id().digest().clone(),
            failure_policy_digest: digest("failure policy"),
        },
    )
    .expect("plan");
    assert_eq!(
        out_of_range.validate_against_plan(&changed_plan, std::slice::from_ref(&fixture.planned),),
        Err(GenerationQualificationContractError::SelectionPolicyRelationshipMismatch),
    );
}

#[test]
fn receipt_set_rejects_failed_reordered_and_substituted_closures() {
    let fixture = receipt_set_fixture();
    let set = CandidateGenerationReceiptSetV1::new(fixture.relations()).expect("receipt set");
    let encoded = serde_json::to_vec(&set).expect("set JSON");
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES + 1],
            fixture.relations(),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge),
    );
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(b"{", fixture.relations()),
        Err(GenerationQualificationContractError::InvalidEncoding),
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":9");
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(&future, fixture.relations()),
        Err(GenerationQualificationContractError::UnsupportedSchema(9)),
    );
    for invalid in [
        replace_once(
            &encoded,
            "\"schema_version\":1",
            "\"schema_version\":1,\"unknown\":true",
        ),
        replace_once(
            &encoded,
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
        replace_once(&encoded, ",\"entry_count\":1", ""),
    ] {
        assert_eq!(
            CandidateGenerationReceiptSetV1::from_json_bytes(&invalid, fixture.relations()),
            Err(GenerationQualificationContractError::InvalidEncoding),
        );
    }
    let reordered = serde_json::to_vec(
        &serde_json::from_slice::<serde_json::Value>(&encoded).expect("receipt-set value"),
    )
    .expect("reordered receipt set");
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(&reordered, fixture.relations()),
        Err(GenerationQualificationContractError::NonCanonicalEncoding),
    );
    let selected = set.entries()[0].selected_candidate_evidence_id();
    let substituted = replace_once(
        &encoded,
        selected.digest().as_str(),
        digest("substituted candidate").as_str(),
    );
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(&substituted, fixture.relations()),
        Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch),
    );
    let failed = CandidateGenerationAttemptRecordV1::failed(
        &fixture.planned,
        Some(&fixture.precursor),
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            failure_category: CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
            traffic_observed: true,
            output_observed: false,
            cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        },
    )
    .expect("failed attempt");
    assert_eq!(
        CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
            attempt_records: std::slice::from_ref(&failed),
            ..fixture.relations()
        }),
        Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch),
    );
    let mut trailing = encoded;
    trailing.push(b'\n');
    assert_eq!(
        CandidateGenerationReceiptSetV1::from_json_bytes(&trailing, fixture.relations()),
        Err(GenerationQualificationContractError::NonCanonicalEncoding),
    );
}

#[test]
fn receipt_set_debug_redacts_evidence_and_retains_supporting_records() {
    let fixture = receipt_set_fixture();
    let set = CandidateGenerationReceiptSetV1::new(fixture.relations()).expect("receipt set");
    let debug = format!("{set:?}");
    assert!(debug.contains(set.receipt_set_id().digest().as_str()));
    for hidden in [
        fixture.receipt.receipt_id().digest().as_str(),
        fixture.receipt.candidate_entries()[1]
            .candidate_evidence_id()
            .digest()
            .as_str(),
        fixture.bundle.evidence_bundle_id().digest().as_str(),
    ] {
        assert!(!debug.contains(hidden));
    }
    assert_eq!(
        &fixture.effective_package.effective_package_evidence_v2_id(),
        fixture.managed.effective_package_evidence_v2_id(),
    );
    assert_eq!(fixture.cleanup.cleanup_id(), fixture.receipt.cleanup_id());
    assert_eq!(
        fixture.readback.readback_id(),
        fixture.receipt.readback_id()
    );
}
