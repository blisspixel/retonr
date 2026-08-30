use super::super::*;
use crate::{ArtifactSetRelativePath, EffectivePackageEvidenceV2};

pub(in crate::generation_qualification::tests) struct PairFixture {
    pub(in crate::generation_qualification::tests) cluster: GenerationClusterRecordV1,
    pub(in crate::generation_qualification::tests) case: GenerationCaseManifestV1,
    pub(in crate::generation_qualification::tests) suite: GenerationSuiteManifestV1,
    pub(in crate::generation_qualification::tests) repetition: GenerationRepetitionRecordV1,
    pub(in crate::generation_qualification::tests) systems: Vec<GenerationSystemRecordV1>,
    pub(in crate::generation_qualification::tests) planned: Vec<PlannedCandidateAttemptV1>,
    pub(in crate::generation_qualification::tests) plan: GenerationQualificationPlanV1,
    pub(in crate::generation_qualification::tests) policy: CandidateSelectionPolicyV1,
    pub(in crate::generation_qualification::tests) candidate_a: CandidateGenerationReceiptSetV1,
    pub(in crate::generation_qualification::tests) candidate_b: CandidateGenerationReceiptSetV1,
    pub(in crate::generation_qualification::tests) completed_attempts:
        [CandidateGenerationAttemptRecordV1; 2],
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
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

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the fixture makes the complete retained receipt relationship explicit"
)]
fn completed_set(
    plan: &GenerationQualificationPlanV1,
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    systems: &[GenerationSystemRecordV1],
    planned_attempts: &[PlannedCandidateAttemptV1],
    selection_policy: &CandidateSelectionPolicyV1,
    effective_package: &EffectivePackageEvidenceV2,
    index: usize,
) -> (
    CandidateGenerationReceiptSetV1,
    CandidateGenerationAttemptRecordV1,
) {
    let system = &systems[index];
    let planned = &planned_attempts[index];
    let generation = u64::try_from(index).expect("generation index") + 1;
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        plan,
        planned,
        system,
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: generation,
            model_installation_generation: generation + 10,
            structured_request_binding_id:
                StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                    "pair request {index}"
                ))),
        },
    )
    .expect("precursor");
    let managed = ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: planned,
            generation_system: system,
            effective_package_evidence_v2: effective_package,
        },
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id:
                ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(
                    &format!("pair bracket {index}"),
                )),
            effective_runtime_state_join_id:
                ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest(&format!(
                    "pair live join {index}"
                ))),
            response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest(&format!(
                "pair response {index}"
            ))),
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
    let response = format!("response {index}").into_bytes();
    let response_byte_count = u64::try_from(response.len()).expect("response length");
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(&response)),
        response_byte_count,
    )
    .expect("response artifact");
    let candidate_bytes = [
        format!("candidate {index} zero").into_bytes(),
        format!("candidate {index} one").into_bytes(),
    ];
    let candidates = candidate_bytes
        .iter()
        .enumerate()
        .map(|(ordinal, bytes)| {
            CandidateArtifactEntryV1::new(
                &precursor,
                planned,
                case,
                u8::try_from(ordinal).expect("candidate ordinal"),
                path(&format!("candidates/{ordinal:03}.txt")),
                bytes,
            )
            .expect("candidate artifact")
        })
        .collect::<Vec<_>>();
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
            response_byte_count,
        ),
        record_entry(
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
            qualification_plan: plan,
            planned_attempt: planned,
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
            qualification_plan: plan,
            suite,
            case,
            cluster,
            repetition,
            planned_attempt: planned,
            precursor: &precursor,
            generation_system: system,
            managed_evidence: &managed,
            cleanup: &cleanup,
            bundle: &bundle,
            readback: &readback,
        },
        CandidateGenerationUsageObservationV1::new(Some(2), Some(3), Some(5)),
    )
    .expect("receipt");
    let attempt = CandidateGenerationAttemptRecordV1::completed(planned, &precursor, &receipt)
        .expect("completed attempt");
    let receipt_set =
        CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
            qualification_plan: plan,
            suite,
            repetition,
            generation_system: system,
            selection_policy,
            planned_attempts,
            attempt_records: std::slice::from_ref(&attempt),
            receipts: std::slice::from_ref(&receipt),
        })
        .expect("receipt set");
    (receipt_set, attempt)
}

pub(in crate::generation_qualification::tests) fn pair_fixture(
    failure_policy: &str,
) -> PairFixture {
    let cluster =
        GenerationClusterRecordV1::new("pair-case", digest("pair cluster")).expect("cluster");
    let case = case(&cluster, "pair-case", "pair source");
    let suite =
        GenerationSuiteManifestV1::new(digest("pair protocol"), std::slice::from_ref(&case))
            .expect("suite");
    let repetition = GenerationRepetitionRecordV1::new(&suite, 0, digest("pair repetition"))
        .expect("repetition");
    let support = super::super::super::generation_system::test_support::fixture(false);
    let mut systems = ["candidate a", "candidate b"]
        .map(|label| {
            let mut input = support.input();
            input.strategy_digest = digest(label);
            GenerationSystemRecordV1::new(support.relations(), input).expect("system")
        })
        .to_vec();
    systems.sort_by(|left, right| {
        left.generation_system_id()
            .digest()
            .as_str()
            .cmp(right.generation_system_id().digest().as_str())
    });
    let policy = CandidateSelectionPolicyV1::new(&suite, &[1]).expect("selection policy");
    let planned = systems
        .iter()
        .enumerate()
        .map(|(index, system)| attempt_record(&suite, &case, &cluster, &repetition, system, index))
        .collect::<Vec<_>>();
    let plan = GenerationQualificationPlanV1::new(
        &suite,
        std::slice::from_ref(&repetition),
        &systems,
        &planned,
        GenerationQualificationPlanV1Input {
            limits: limits(2),
            selection_policy_digest: policy.selection_policy_id().digest().clone(),
            failure_policy_digest: digest(failure_policy),
        },
    )
    .expect("plan");
    let (candidate_a, first_attempt) = completed_set(
        &plan,
        &suite,
        &case,
        &cluster,
        &repetition,
        &systems,
        &planned,
        &policy,
        &support.effective_package,
        0,
    );
    let (candidate_b, second_attempt) = completed_set(
        &plan,
        &suite,
        &case,
        &cluster,
        &repetition,
        &systems,
        &planned,
        &policy,
        &support.effective_package,
        1,
    );
    PairFixture {
        cluster,
        case,
        suite,
        repetition,
        systems,
        planned,
        plan,
        policy,
        candidate_a,
        candidate_b,
        completed_attempts: [first_attempt, second_attempt],
    }
}

pub(in crate::generation_qualification::tests) fn mixed_repetition_pair_fixture() -> PairFixture {
    let seed = pair_fixture("mixed repetition failure policy");
    let PairFixture {
        cluster,
        case,
        suite,
        systems,
        policy,
        repetition,
        ..
    } = seed;
    let repetitions = vec![
        repetition,
        GenerationRepetitionRecordV1::new(&suite, 1, digest("pair repetition 1"))
            .expect("second repetition"),
    ];
    let planned = repetitions
        .iter()
        .flat_map(|repetition| systems.iter().map(move |system| (repetition, system)))
        .enumerate()
        .map(|(index, (repetition, system))| {
            attempt_record(&suite, &case, &cluster, repetition, system, index)
        })
        .collect::<Vec<_>>();
    let plan = GenerationQualificationPlanV1::new(
        &suite,
        &repetitions,
        &systems,
        &planned,
        GenerationQualificationPlanV1Input {
            limits: limits(4),
            selection_policy_digest: policy.selection_policy_id().digest().clone(),
            failure_policy_digest: digest("mixed repetition failure policy"),
        },
    )
    .expect("mixed repetition plan");
    let support = super::super::super::generation_system::test_support::fixture(false);
    let (candidate_a, first_attempt) = completed_set(
        &plan,
        &suite,
        &case,
        &cluster,
        &repetitions[0],
        &systems,
        &planned,
        &policy,
        &support.effective_package,
        0,
    );
    let (candidate_b, second_attempt) = completed_set(
        &plan,
        &suite,
        &case,
        &cluster,
        &repetitions[0],
        &systems,
        &planned,
        &policy,
        &support.effective_package,
        1,
    );
    PairFixture {
        cluster,
        case,
        suite,
        repetition: repetitions[0].clone(),
        systems,
        planned,
        plan,
        policy,
        candidate_a,
        candidate_b,
        completed_attempts: [first_attempt, second_attempt],
    }
}

pub(in crate::generation_qualification::tests) fn coverage(
    acceptable: u32,
    rewritten: u32,
) -> CandidateDeterministicTransformationCoverageV1 {
    CandidateDeterministicTransformationCoverageV1::new(acceptable, rewritten).expect("coverage")
}

pub(in crate::generation_qualification::tests) fn summary(
    passed: u32,
    coverage: CandidateDeterministicTransformationCoverageV1,
) -> CandidateDeterministicReportSummaryV1 {
    CandidateDeterministicReportSummaryV1::new(1, passed, coverage).expect("summary")
}

pub(in crate::generation_qualification::tests) fn report_relationship(
    passed_b: u32,
) -> CandidateDeterministicReportRelationshipV1 {
    CandidateDeterministicReportRelationshipV1::new(
        br#"{"schema_version":1,"total":1,"passed":1}"#,
        br#"{"schema_version":1,"total":1,"passed":0}"#,
        summary(1, coverage(1, 1)),
        summary(passed_b, coverage(0, 0)),
    )
    .expect("report relationship")
}

pub(in crate::generation_qualification::tests) fn input<'a>(
    case_material: &'a Digest,
    suite_pair: &'a Digest,
    reports: &'a CandidateDeterministicReportRelationshipV1,
) -> CandidateDeterministicEvaluationRecordV1Input<'a> {
    CandidateDeterministicEvaluationRecordV1Input {
        case_material_set_digest: case_material,
        suite_pair_digest: suite_pair,
        report_relationship: reports,
    }
}

pub(super) fn replace_once(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let value = String::from_utf8(bytes.to_vec()).expect("UTF-8 JSON");
    assert_eq!(value.matches(from).count(), 1, "unique replacement source");
    value.replacen(from, to, 1).into_bytes()
}
