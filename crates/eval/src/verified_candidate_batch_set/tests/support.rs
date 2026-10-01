use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use rewrite_inference::GenerationCandidate;
use rewrite_model::{
    ArtifactId, CandidateArtifactEntryV1, CandidateGenerationAttemptPrecursorV1,
    CandidateGenerationAttemptPrecursorV1Input, CandidateGenerationAttemptRecordV1,
    CandidateGenerationCleanupRecordV1, CandidateGenerationCleanupRecordV1Input,
    CandidateGenerationEvidenceBundleEntryV1, CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations,
    CandidateGenerationEvidenceBundleReadbackV1, CandidateGenerationEvidenceBundleRoleV1,
    CandidateGenerationPackageRevalidationStatusV1, CandidateGenerationProcessCleanupStatusV1,
    CandidateGenerationReceiptV1, CandidateGenerationReceiptV1Relations,
    CandidateGenerationUsageObservationV1, CandidateOutputCeilingsV1, CandidateSelectionPolicyV1,
    GenerationCaseManifestV1, GenerationCaseManifestV1Input, GenerationClusterRecordV1,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPlanLimitsV1,
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationRepetitionRecordV1, GenerationRequestBindingId,
    GenerationResourceAttemptResultRecordV1, GenerationSuiteManifestV1, GenerationSystemRecordV1,
    MAX_GENERATION_CANDIDATES_PER_COMPLETION, MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES, MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
    MAX_GENERATION_RETAINED_INPUT_BYTES, ManagedOllamaCandidateGenerationEvidenceV2,
    ManagedOllamaCandidateGenerationEvidenceV2Input,
    ManagedOllamaCandidateGenerationEvidenceV2Relations, ManagedOllamaEffectiveRuntimeStateJoinId,
    ManagedOllamaGenerationBracketObservationV1Id, OllamaRetainedSessionResponseId,
    PlannedCandidateAttemptV1, PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
    StructuredCompletionRequestBindingId, StructuredResponseArtifactV1Input,
};
use rewrite_types::CancellationToken;

use super::super::{RetainedCandidateBatch, VerifiedCandidateBatchSetInput};

#[path = "support/system.rs"]
mod system;

#[path = "support/resource.rs"]
mod resource;

use system::{digest, fixture as system_fixture};
pub(crate) use system::{judge_system, judge_system_with_model};

#[path = "support/paired.rs"]
mod paired;
pub(super) use paired::paired_scenario;

#[path = "support/control.rs"]
mod control;
pub(crate) use control::{OfflineBatchError, OfflineBatchFailureControl};

#[path = "support/portable.rs"]
mod portable;
use portable::path;

#[path = "support/portable_attempts.rs"]
mod portable_attempts;

#[path = "support/prepared.rs"]
mod prepared;
pub(crate) use prepared::{prepared_scenario, prepared_scenario_with_suffix};

#[path = "support/publication.rs"]
mod publication;

/// Private non-authoritative adapter over exact portable records and copied candidates.
pub(crate) struct OfflineBatch {
    receipt: CandidateGenerationReceiptV1,
    attempt_record: CandidateGenerationAttemptRecordV1,
    resource_result: Option<GenerationResourceAttemptResultRecordV1>,
    candidates: Vec<GenerationCandidate>,
    label: usize,
    revalidation_calls: Rc<Cell<usize>>,
    fail_on_call: Rc<Cell<Option<usize>>>,
    log: Rc<RefCell<Vec<usize>>>,
    publication: publication::PortablePublication,
}

impl RetainedCandidateBatch for OfflineBatch {
    type Error = OfflineBatchError;

    fn receipt(&self) -> &CandidateGenerationReceiptV1 {
        &self.receipt
    }

    fn attempt_record(&self) -> &CandidateGenerationAttemptRecordV1 {
        &self.attempt_record
    }

    fn resource_result(&self) -> Option<&rewrite_model::GenerationResourceAttemptResultRecordV1> {
        self.resource_result.as_ref()
    }

    fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    fn candidate(
        &self,
        ordinal: u8,
        _cancellation: &CancellationToken,
    ) -> Result<Option<&GenerationCandidate>, Self::Error> {
        Ok(self
            .candidates
            .get(usize::from(ordinal))
            .filter(|candidate| candidate.ordinal == ordinal))
    }

    fn revalidate(&self, _cancellation: &CancellationToken) -> Result<(), Self::Error> {
        let call = self.revalidation_calls.get() + 1;
        self.revalidation_calls.set(call);
        self.log.borrow_mut().push(self.label);
        if self.fail_on_call.get() == Some(call) {
            Err(OfflineBatchError::ForcedRevalidation)
        } else {
            Ok(())
        }
    }
}

pub(crate) struct Scenario {
    pub(crate) input: VerifiedCandidateBatchSetInput,
    pub(crate) batches: Vec<OfflineBatch>,
    pub(super) log: Rc<RefCell<Vec<usize>>>,
}

pub(crate) fn scenario(suffix: &str) -> Scenario {
    let cluster = GenerationClusterRecordV1::new(
        format!("batch-set-{suffix}"),
        digest(&format!("cluster policy {suffix}")),
    )
    .expect("cluster");
    let cases = [
        generation_case(&cluster, suffix, 0),
        generation_case(&cluster, suffix, 1),
    ];
    let suite = GenerationSuiteManifestV1::new(digest(&format!("protocol {suffix}")), &cases)
        .expect("suite");
    let repetition =
        GenerationRepetitionRecordV1::new(&suite, 0, digest(&format!("repetition {suffix}")))
            .expect("repetition");
    let system_fixture = system_fixture();
    let generation_system = system_fixture.record();
    let selection_policy = CandidateSelectionPolicyV1::new(&suite, &[1, 1]).expect("policy");
    let planned_attempts = cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            planned_attempt(
                &suite,
                case,
                &cluster,
                &repetition,
                &generation_system,
                index,
                suffix,
            )
        })
        .collect::<Vec<_>>();
    let qualification_plan = GenerationQualificationPlanV1::new(
        &suite,
        std::slice::from_ref(&repetition),
        std::slice::from_ref(&generation_system),
        &planned_attempts,
        GenerationQualificationPlanV1Input {
            limits: limits(2),
            selection_policy_digest: selection_policy.selection_policy_id().digest().clone(),
            failure_policy_digest: digest(&format!("failure policy {suffix}")),
        },
    )
    .expect("plan");
    let log = Rc::new(RefCell::new(Vec::new()));
    let batches = planned_attempts
        .iter()
        .zip(cases.iter())
        .enumerate()
        .map(|(index, (planned, case))| {
            offline_batch(
                &qualification_plan,
                &suite,
                case,
                &cluster,
                &repetition,
                planned,
                &generation_system,
                &system_fixture.effective_package,
                index,
                Rc::clone(&log),
                suffix,
                None,
                None,
            )
        })
        .collect();
    Scenario {
        input: VerifiedCandidateBatchSetInput {
            qualification_plan,
            suite,
            repetition,
            generation_system,
            selection_policy,
            planned_attempts,
        },
        batches,
        log,
    }
}

fn generation_case(
    cluster: &GenerationClusterRecordV1,
    suffix: &str,
    index: usize,
) -> GenerationCaseManifestV1 {
    let source = format!("source {suffix} {index}");
    let source_digest = digest(&source);
    GenerationCaseManifestV1::new(
        cluster,
        GenerationCaseManifestV1Input {
            case_key: format!("case-{suffix}-{index}"),
            source_artifact_id: ArtifactId::from_digest(source_digest.clone()),
            source_digest,
            source_byte_count: source.len() as u64,
            case_contract_digest: digest(&format!("case contract {suffix} {index}")),
            language_digest: digest("language"),
            mode_digest: digest("mode"),
            format_digest: digest("format"),
        },
    )
    .expect("case")
}

fn planned_attempt(
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    system: &GenerationSystemRecordV1,
    index: usize,
    suffix: &str,
) -> PlannedCandidateAttemptV1 {
    let qualification_closure = suffix.contains("qualification-closure");
    let candidate_count = if qualification_closure { 1 } else { 2 };
    let aggregate_bytes = if qualification_closure { 1_024 } else { 2_048 };
    PlannedCandidateAttemptV1::new(
        PlannedCandidateAttemptV1Relations {
            suite,
            case,
            cluster,
            repetition,
            generation_system: system,
        },
        PlannedCandidateAttemptV1Input {
            attempt_ordinal: u32::try_from(index).expect("attempt ordinal"),
            declared_seed: u64::try_from(index).expect("seed"),
            grounded_request_digest: digest(&format!("grounded {suffix} {index}")),
            generation_request_binding_id: GenerationRequestBindingId::from_derived_digest(digest(
                &format!("request {suffix} {index}"),
            )),
            candidate_output_contract_digest: digest("candidate output contract"),
            output_ceilings: CandidateOutputCeilingsV1::new(
                candidate_count,
                1_024,
                aggregate_bytes,
            )
            .expect("ceilings"),
        },
    )
    .expect("planned attempt")
}

fn limits(maximum_predeclared_attempts: u32) -> GenerationQualificationPlanLimitsV1 {
    GenerationQualificationPlanLimitsV1::new(
        MAX_GENERATION_CANDIDATES_PER_COMPLETION,
        maximum_predeclared_attempts,
        MAX_GENERATION_RETAINED_INPUT_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_ENTRIES,
        MAX_GENERATION_EVIDENCE_RELATIVE_PATH_BYTES,
        MAX_GENERATION_EVIDENCE_BUNDLE_BYTES,
    )
    .expect("limits")
}

#[expect(
    clippy::too_many_arguments,
    reason = "the test fixture keeps every exact receipt relationship explicit"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the private fixture assembles one complete exact portable receipt chain"
)]
fn offline_batch(
    plan: &GenerationQualificationPlanV1,
    suite: &GenerationSuiteManifestV1,
    case: &GenerationCaseManifestV1,
    cluster: &GenerationClusterRecordV1,
    repetition: &GenerationRepetitionRecordV1,
    planned: &PlannedCandidateAttemptV1,
    system: &GenerationSystemRecordV1,
    effective_package: &rewrite_model::EffectivePackageEvidenceV2,
    index: usize,
    log: Rc<RefCell<Vec<usize>>>,
    suffix: &str,
    operation_policy: Option<&GenerationQualificationOperationPolicyV1>,
    structured_request: Option<&StructuredCompletionRequestBindingId>,
) -> OfflineBatch {
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        plan,
        planned,
        system,
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: 7 + index as u64,
            model_installation_generation: 11 + index as u64,
            structured_request_binding_id: structured_request.cloned().unwrap_or_else(|| {
                StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                    "structured {suffix} {index}"
                )))
            }),
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
                    &format!("bracket {suffix} {index}"),
                )),
            effective_runtime_state_join_id:
                ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest(&format!(
                    "join {suffix} {index}"
                ))),
            response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest(&format!(
                "response {suffix} {index}"
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
    let selected_text = paired::selected_candidate_text(case.case_key(), suffix, index);
    let candidate_text = if suffix.contains("qualification-closure") {
        vec![selected_text]
    } else {
        vec![format!("first candidate {suffix} {index}"), selected_text]
    };
    let candidate_entries = candidate_text
        .iter()
        .enumerate()
        .map(|(ordinal, text)| {
            CandidateArtifactEntryV1::new(
                &precursor,
                planned,
                case,
                u8::try_from(ordinal).expect("candidate ordinal"),
                path(&format!("candidates/{ordinal:03}.txt")),
                text.as_bytes(),
            )
            .expect("candidate entry")
        })
        .collect::<Vec<_>>();
    let response_bytes = format!("response {suffix} {index}").into_bytes();
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(rewrite_types::Digest::sha256(&response_bytes)),
        response_bytes.len() as u64,
    )
    .expect("response artifact");
    let entries = bundle_entries(
        planned,
        &precursor,
        &managed,
        &cleanup,
        &response_bytes,
        &candidate_entries,
    );
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
        candidate_entries,
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
        CandidateGenerationUsageObservationV1::new(Some(3), Some(5), Some(8)),
    )
    .expect("receipt");
    let attempt_record =
        CandidateGenerationAttemptRecordV1::completed(planned, &precursor, &receipt)
            .expect("attempt record");
    let resource_result = operation_policy.map(|operation_policy| {
        resource::qualification_resource_result(
            plan,
            suite,
            case,
            repetition,
            planned,
            system,
            operation_policy,
            &attempt_record,
            &receipt,
        )
    });
    let publication = publication::PortablePublication {
        precursor,
        managed,
        cleanup,
        bundle,
        readback,
    };
    OfflineBatch {
        publication,
        receipt,
        attempt_record,
        resource_result,
        candidates: candidate_text
            .into_iter()
            .enumerate()
            .map(|(ordinal, text)| GenerationCandidate {
                ordinal: u8::try_from(ordinal).expect("candidate ordinal"),
                text,
            })
            .collect(),
        label: index,
        revalidation_calls: Rc::new(Cell::new(0)),
        fail_on_call: Rc::new(Cell::new(None)),
        log,
    }
}

fn bundle_entry<T: serde::Serialize>(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    record: &T,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    let bytes = serde_json::to_vec(record).expect("record JSON");
    CandidateGenerationEvidenceBundleEntryV1::new(
        path(relative_path),
        role,
        ArtifactId::from_digest(rewrite_types::Digest::sha256(&bytes)),
        bytes.len() as u64,
    )
}

fn bundle_entries(
    planned: &PlannedCandidateAttemptV1,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    managed: &ManagedOllamaCandidateGenerationEvidenceV2,
    cleanup: &CandidateGenerationCleanupRecordV1,
    response_bytes: &[u8],
    candidates: &[CandidateArtifactEntryV1],
) -> Vec<CandidateGenerationEvidenceBundleEntryV1> {
    let mut entries = vec![
        bundle_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            planned,
        ),
        bundle_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            precursor,
        ),
        bundle_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            managed,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            path("records/response.json"),
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            ArtifactId::from_digest(rewrite_types::Digest::sha256(response_bytes)),
            response_bytes.len() as u64,
        ),
        bundle_entry(
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
