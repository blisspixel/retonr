use std::collections::HashSet;

use super::*;

#[path = "receipt_attempt/attempt_adversarial.rs"]
mod attempt_adversarial;
#[path = "receipt_attempt/preflight.rs"]
mod preflight;
#[path = "receipt_attempt/receipt_core.rs"]
mod receipt_core;
#[path = "receipt_attempt/receipt_set.rs"]
mod receipt_set;

struct ReceiptFixture {
    bundle: BundleFixture,
    readback: CandidateGenerationEvidenceBundleReadbackV1,
    usage: CandidateGenerationUsageObservationV1,
    receipt: CandidateGenerationReceiptV1,
}

impl ReceiptFixture {
    fn relations(&self) -> CandidateGenerationReceiptV1Relations<'_> {
        receipt_relations(&self.bundle, &self.readback)
    }
}

fn receipt_relations<'a>(
    fixture: &'a BundleFixture,
    readback: &'a CandidateGenerationEvidenceBundleReadbackV1,
) -> CandidateGenerationReceiptV1Relations<'a> {
    CandidateGenerationReceiptV1Relations {
        qualification_plan: &fixture.managed.base.plan,
        suite: &fixture.managed.base.suite,
        case: &fixture.managed.base.cases[0],
        cluster: &fixture.managed.base.cluster,
        repetition: &fixture.managed.base.repetition,
        planned_attempt: &fixture.managed.base.attempts[0],
        precursor: &fixture.managed.precursor,
        generation_system: &fixture.managed.base.systems[0],
        managed_evidence: &fixture.managed.evidence,
        cleanup: &fixture.cleanup,
        bundle: &fixture.bundle,
        readback,
    }
}

fn receipt_fixture() -> ReceiptFixture {
    let bundle = bundle_fixture();
    let readback =
        CandidateGenerationEvidenceBundleReadbackV1::new(&bundle.bundle).expect("readback");
    let usage = CandidateGenerationUsageObservationV1::new(Some(11), Some(17), Some(23_000));
    let receipt = CandidateGenerationReceiptV1::new(receipt_relations(&bundle, &readback), usage)
        .expect("receipt");
    ReceiptFixture {
        bundle,
        readback,
        usage,
        receipt,
    }
}

#[test]
fn receipt_rejects_failed_cleanup_and_stale_readback() {
    let managed = managed_fixture(0);
    let cleanup = CandidateGenerationCleanupRecordV1::new(
        &managed.precursor,
        &managed.evidence,
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Failed,
            runtime_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            model_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
        },
    )
    .expect("failed cleanup record");
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(b"response")),
        8,
    )
    .expect("response artifact");
    let candidates = vec![
        candidate(&managed, 0, "candidates/000.txt", b"first candidate"),
        candidate(&managed, 1, "candidates/001.txt", b"second candidate"),
    ];
    let bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &managed.base.plan,
            planned_attempt: &managed.base.attempts[0],
            precursor: &managed.precursor,
            managed_evidence: &managed.evidence,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
        valid_entries(&managed, &cleanup, &candidates),
        candidates,
    )
    .expect("failure bundle");
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(&bundle).expect("readback");
    let failed = BundleFixture {
        managed,
        cleanup,
        response_artifact,
        candidates: bundle.candidate_artifacts().to_vec(),
        bundle,
    };
    assert_eq!(
        CandidateGenerationReceiptV1::new(
            receipt_relations(&failed, &readback),
            CandidateGenerationUsageObservationV1::new(None, None, None),
        ),
        Err(GenerationQualificationContractError::InvalidReceiptPostconditions),
    );

    let fixture = receipt_fixture();
    let mut changed_entries = valid_entries(
        &fixture.bundle.managed,
        &fixture.bundle.cleanup,
        &fixture.bundle.candidates,
    );
    changed_entries.push(entry(
        "z-aux/stale.bin",
        CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
        b"stale",
    ));
    let changed_bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        fixture.bundle.relations(),
        changed_entries,
        fixture.bundle.candidates.clone(),
    )
    .expect("changed bundle");
    let changed_readback = CandidateGenerationEvidenceBundleReadbackV1::new(&changed_bundle)
        .expect("changed readback");
    assert_eq!(
        CandidateGenerationReceiptV1::new(
            CandidateGenerationReceiptV1Relations {
                readback: &changed_readback,
                ..fixture.relations()
            },
            fixture.usage,
        ),
        Err(GenerationQualificationContractError::ReceiptRelationshipMismatch),
    );
}

#[test]
fn receipt_decoder_rejects_untrusted_substituted_usage_candidate_and_claims() {
    let fixture = receipt_fixture();
    let encoded = serde_json::to_vec(&fixture.receipt).expect("receipt JSON");
    assert_eq!(
        CandidateGenerationReceiptV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_GENERATION_RECEIPT_JSON_BYTES + 1],
            fixture.relations(),
            fixture.usage
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge),
    );
    assert_eq!(
        CandidateGenerationReceiptV1::from_json_bytes(b"{", fixture.relations(), fixture.usage),
        Err(GenerationQualificationContractError::InvalidEncoding),
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2");
    assert_eq!(
        CandidateGenerationReceiptV1::from_json_bytes(&future, fixture.relations(), fixture.usage),
        Err(GenerationQualificationContractError::UnsupportedSchema(2)),
    );
    let mut spaced = encoded.clone();
    spaced.push(b' ');
    assert_eq!(
        CandidateGenerationReceiptV1::from_json_bytes(&spaced, fixture.relations(), fixture.usage),
        Err(GenerationQualificationContractError::NonCanonicalEncoding),
    );
    for changed in [
        replace_once(&encoded, "\"input_tokens\":11", "\"input_tokens\":12"),
        replace_once(
            &encoded,
            "\"model_used_proven\":false",
            "\"model_used_proven\":true",
        ),
        replace_once(
            &encoded,
            fixture.receipt.candidate_entries()[0]
                .candidate_evidence_id()
                .digest()
                .as_str(),
            digest("substituted receipt candidate").as_str(),
        ),
        replace_once(
            &encoded,
            fixture.receipt.bundle_id().digest().as_str(),
            digest("substituted receipt bundle").as_str(),
        ),
    ] {
        assert_eq!(
            CandidateGenerationReceiptV1::from_json_bytes(
                &changed,
                fixture.relations(),
                fixture.usage
            ),
            Err(GenerationQualificationContractError::ReceiptRelationshipMismatch),
        );
    }
    let duplicate = replace_once(
        &encoded,
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
    );
    assert_eq!(
        CandidateGenerationReceiptV1::from_json_bytes(
            &duplicate,
            fixture.relations(),
            fixture.usage
        ),
        Err(GenerationQualificationContractError::InvalidEncoding),
    );
}

fn failure_input(
    failure_phase: CandidateGenerationAttemptFailurePhaseV1,
    failure_category: CandidateGenerationAttemptFailureCategoryV1,
    traffic_observed: bool,
    output_observed: bool,
    cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1,
) -> CandidateGenerationAttemptFailureV1Input {
    CandidateGenerationAttemptFailureV1Input {
        failure_phase,
        failure_category,
        traffic_observed,
        output_observed,
        cleanup_disposition,
    }
}

#[test]
fn completed_and_failed_attempts_round_trip_without_backward_reference() {
    let fixture = receipt_fixture();
    let completed = CandidateGenerationAttemptRecordV1::completed(
        &fixture.bundle.managed.base.attempts[0],
        &fixture.bundle.managed.precursor,
        &fixture.receipt,
    )
    .expect("completed attempt");
    assert_eq!(completed.schema_version(), 1);
    assert!(matches!(
        completed.outcome(),
        CandidateGenerationAttemptOutcomeV1::Completed { .. }
    ));
    let encoded = serde_json::to_vec(&completed).expect("completed JSON");
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &encoded,
            &fixture.bundle.managed.base.attempts[0],
            Some(&fixture.bundle.managed.precursor),
            Some(&fixture.receipt)
        )
        .expect("completed decode"),
        completed,
    );
    assert!(
        !String::from_utf8(encoded)
            .expect("UTF-8")
            .contains("attempt_record_id")
    );

    let failed = CandidateGenerationAttemptRecordV1::failed(
        &fixture.bundle.managed.base.attempts[0],
        Some(&fixture.bundle.managed.precursor),
        failure_input(
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
            true,
            false,
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        ),
    )
    .expect("failed attempt");
    let encoded = serde_json::to_vec(&failed).expect("failed JSON");
    assert_eq!(
        CandidateGenerationAttemptRecordV1::from_json_bytes(
            &encoded,
            &fixture.bundle.managed.base.attempts[0],
            Some(&fixture.bundle.managed.precursor),
            None
        )
        .expect("failed decode"),
        failed,
    );
    let text = String::from_utf8(encoded).expect("UTF-8");
    for forbidden in [
        "response_id",
        "managed_evidence_id",
        "candidate_entries",
        "bundle_id",
        "readback_id",
        "receipt_id",
    ] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn failure_closure_accepts_each_phase_and_secondary_cleanup_failure() {
    let fixture = receipt_fixture();
    let planned = &fixture.bundle.managed.base.attempts[0];
    let precursor = &fixture.bundle.managed.precursor;
    let phases = [
        CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
        CandidateGenerationAttemptFailurePhaseV1::PrecursorCompilation,
        CandidateGenerationAttemptFailurePhaseV1::Launch,
        CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation,
        CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic,
        CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
        CandidateGenerationAttemptFailurePhaseV1::FinalObservation,
        CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
        CandidateGenerationAttemptFailurePhaseV1::Cleanup,
        CandidateGenerationAttemptFailurePhaseV1::BundlePublication,
        CandidateGenerationAttemptFailurePhaseV1::BundleReadback,
        CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation,
    ];
    let mut ids = HashSet::new();
    for (index, phase) in phases.into_iter().enumerate() {
        let pre_precursor = index < 2;
        let cleanup_phase = phase == CandidateGenerationAttemptFailurePhaseV1::Cleanup;
        let value = CandidateGenerationAttemptRecordV1::failed(
            planned,
            (!pre_precursor).then_some(precursor),
            failure_input(
                phase,
                if cleanup_phase {
                    CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
                } else {
                    CandidateGenerationAttemptFailureCategoryV1::Cancelled
                },
                index >= 4,
                index >= 5,
                if pre_precursor {
                    CandidateGenerationAttemptCleanupDispositionV1::NotRequired
                } else if cleanup_phase {
                    CandidateGenerationAttemptCleanupDispositionV1::Failed
                } else {
                    CandidateGenerationAttemptCleanupDispositionV1::Succeeded
                },
            ),
        )
        .expect("valid phase closure");
        assert!(ids.insert(value.attempt_record_id().clone()));
    }
    assert_eq!(ids.len(), 12);
    CandidateGenerationAttemptRecordV1::failed(
        planned,
        Some(precursor),
        failure_input(
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
            true,
            false,
            CandidateGenerationAttemptCleanupDispositionV1::Failed,
        ),
    )
    .expect("secondary cleanup failure preserves primary failure");
}

#[test]
fn failure_closure_rejects_every_forbidden_combination() {
    let fixture = receipt_fixture();
    let planned = &fixture.bundle.managed.base.attempts[0];
    let precursor = &fixture.bundle.managed.precursor;
    let invalid = [
        (
            Some(precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
                CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
                false,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
            ),
        ),
        (
            None,
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::Launch,
                CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
                false,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            ),
        ),
        (
            Some(precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::Launch,
                CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
                true,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            ),
        ),
        (
            Some(precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::GenerationTraffic,
                CandidateGenerationAttemptFailureCategoryV1::TransportFailed,
                false,
                true,
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            ),
        ),
        (
            Some(precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::Launch,
                CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
                false,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
            ),
        ),
        (
            Some(precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::Cleanup,
                CandidateGenerationAttemptFailureCategoryV1::TransportFailed,
                true,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::Failed,
            ),
        ),
        (
            Some(precursor),
            failure_input(
                CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
                CandidateGenerationAttemptFailureCategoryV1::CleanupFailed,
                true,
                false,
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
            ),
        ),
    ];
    for (selected, input) in invalid {
        assert_eq!(
            CandidateGenerationAttemptRecordV1::failed(planned, selected, input),
            Err(GenerationQualificationContractError::InvalidAttemptFailure),
        );
    }
}

#[test]
fn receipt_and_attempt_ids_match_stable_vectors() {
    let fixture = receipt_fixture();
    let completed = CandidateGenerationAttemptRecordV1::completed(
        &fixture.bundle.managed.base.attempts[0],
        &fixture.bundle.managed.precursor,
        &fixture.receipt,
    )
    .expect("completed");
    let failed = CandidateGenerationAttemptRecordV1::failed(
        &fixture.bundle.managed.base.attempts[0],
        Some(&fixture.bundle.managed.precursor),
        failure_input(
            CandidateGenerationAttemptFailurePhaseV1::ResponseValidation,
            CandidateGenerationAttemptFailureCategoryV1::ResponseInvalid,
            true,
            false,
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        ),
    )
    .expect("failed");
    assert_eq!(
        (
            fixture.receipt.receipt_id().digest().as_str(),
            completed.attempt_record_id().digest().as_str(),
            failed.attempt_record_id().digest().as_str(),
        ),
        (
            "6dd6443a9c9ebfe7a11f268471c48bae0a0f790265f3813cfe5a3fbbd56b599e",
            "0ddd47ac599ca4aa06379dfac4ef518db5c16c354f3d7cc891b5e2c17f23de25",
            "09f45b127186995d44ddd0ead7f0bc36f38e02c1c4317d657ec287aa93082ca9"
        ),
    );
}
