use rewrite_model::CandidateGenerationAttemptOutcomeV1;
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::support::BundleFixture;
use crate::{
    CandidateGenerationEvidenceBundleDestination, CandidateGenerationEvidenceBundleLimits,
    CandidateGenerationEvidenceBundlePublicationPlan, CandidateGenerationEvidenceBundlePublisher,
    CandidateGenerationReceiptCompilationError, CandidateGenerationReceiptCompilationInput,
    CandidateGenerationReceiptCompiler,
};

#[test]
fn compiler_reloads_the_bundle_and_derives_usage_and_completed_attempt() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary receipt parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("candidate-bundle"),
    )
    .expect("portable destination");
    let publication = CandidateGenerationEvidenceBundlePublisher::publish(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &fixture.plan_input(),
            CandidateGenerationEvidenceBundleLimits::default(),
            &CancellationToken::new(),
        )
        .expect("compile publication plan"),
        &destination,
        &CancellationToken::new(),
    )
    .expect("publish exact bundle");
    let compilation = CandidateGenerationReceiptCompiler::compile(
        input(
            &fixture,
            publication.into_lease(),
            &fixture.structured_request,
        ),
        &CancellationToken::new(),
    )
    .expect("compile completed receipt");

    let usage = compilation.receipt().usage_observation();
    assert_eq!(usage.input_tokens(), Some(11));
    assert_eq!(usage.output_tokens(), Some(3));
    assert_eq!(usage.generation_micros(), Some(1_250));
    assert_eq!(compilation.candidate_count(), 1);
    assert_eq!(
        compilation
            .candidate_bytes(0, &CancellationToken::new())
            .expect("read verified candidate"),
        b"ok",
    );
    assert!(compilation.candidate_entry(1).is_none());
    assert!(matches!(
        compilation.attempt_record().outcome(),
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id,
            precursor_id,
            receipt_id,
        } if planned_attempt_id == fixture.planned_attempt.planned_attempt_id()
            && precursor_id == fixture.precursor.precursor_id()
            && receipt_id == compilation.receipt().receipt_id()
    ));
    compilation
        .revalidate(&CancellationToken::new())
        .expect("completed receipt retains the exact tree");
    let debug = format!("{compilation:?}");
    assert!(debug.contains(compilation.receipt().receipt_id().digest().as_str()));
    assert!(!debug.contains(fixture.structured_response.output_json()));
}

#[test]
fn compiler_rejects_request_substitution_and_cancellation() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary receipt parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("candidate-bundle"),
    )
    .expect("portable destination");
    let publication = CandidateGenerationEvidenceBundlePublisher::publish(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &fixture.plan_input(),
            CandidateGenerationEvidenceBundleLimits::default(),
            &CancellationToken::new(),
        )
        .expect("compile publication plan"),
        &destination,
        &CancellationToken::new(),
    )
    .expect("publish exact bundle");
    let mut wrong_request = fixture.structured_request.clone();
    wrong_request.input.push('!');
    assert!(matches!(
        CandidateGenerationReceiptCompiler::compile(
            input(&fixture, publication.into_lease(), &wrong_request),
            &CancellationToken::new(),
        ),
        Err(CandidateGenerationReceiptCompilationError::ResponseArtifact(_))
    ));

    let second = BundleFixture::new();
    let second_destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("cancelled-bundle"),
    )
    .expect("portable destination");
    let second_publication = CandidateGenerationEvidenceBundlePublisher::publish(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &second.plan_input(),
            CandidateGenerationEvidenceBundleLimits::default(),
            &CancellationToken::new(),
        )
        .expect("compile publication plan"),
        &second_destination,
        &CancellationToken::new(),
    )
    .expect("publish exact bundle");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        CandidateGenerationReceiptCompiler::compile(
            input(
                &second,
                second_publication.into_lease(),
                &second.structured_request,
            ),
            &cancellation,
        ),
        Err(CandidateGenerationReceiptCompilationError::Bundle(_))
    ));
}

#[test]
fn empty_candidate_remains_readable_and_receiptable() {
    let fixture = BundleFixture::with_candidate("");
    let directory = tempdir().expect("temporary empty-candidate parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("empty-candidate-bundle"),
    )
    .expect("portable destination");
    let publication = CandidateGenerationEvidenceBundlePublisher::publish(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &fixture.plan_input(),
            CandidateGenerationEvidenceBundleLimits::default(),
            &CancellationToken::new(),
        )
        .expect("compile empty-candidate publication plan"),
        &destination,
        &CancellationToken::new(),
    )
    .expect("publish empty-candidate evidence bundle");
    assert_eq!(
        publication
            .lease()
            .member_bytes(&fixture.candidate_path, 0, &CancellationToken::new())
            .expect("read exact empty candidate"),
        Vec::<u8>::new(),
    );
    let compilation = CandidateGenerationReceiptCompiler::compile(
        input(
            &fixture,
            publication.into_lease(),
            &fixture.structured_request,
        ),
        &CancellationToken::new(),
    )
    .expect("compile empty candidate receipt material");
    assert_eq!(
        compilation
            .candidate_bytes(0, &CancellationToken::new())
            .expect("read receipted empty candidate"),
        Vec::<u8>::new(),
    );
}

fn input<'a>(
    fixture: &'a BundleFixture,
    readback_lease: crate::CandidateGenerationEvidenceBundleReadbackLease,
    structured_request: &'a rewrite_inference::StructuredCompletionRequest,
) -> CandidateGenerationReceiptCompilationInput<'a> {
    CandidateGenerationReceiptCompilationInput {
        qualification_plan: &fixture.qualification_plan,
        suite: &fixture.suite,
        case: &fixture.case_manifest,
        cluster: &fixture.cluster,
        repetition: &fixture.repetition,
        planned_attempt: &fixture.planned_attempt,
        generation_system: &fixture.generation_system,
        effective_package: &fixture.effective_package,
        precursor: &fixture.precursor,
        managed_evidence: &fixture.managed_evidence,
        cleanup: &fixture.cleanup,
        structured_request,
        readback_lease,
    }
}
