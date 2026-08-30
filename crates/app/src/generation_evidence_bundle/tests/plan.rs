use super::support::{BundleFixture, CANDIDATE_BYTES};
use crate::{
    CandidateGenerationEvidenceBundleAuxiliaryArtifact, CandidateGenerationEvidenceBundleError,
    CandidateGenerationEvidenceBundleLimits, CandidateGenerationEvidenceBundlePublicationPlan,
};
use rewrite_model::{
    ArtifactId, CandidateGenerationEvidenceBundleEntryV1,
    CandidateGenerationEvidenceBundleManifestV1,
    CandidateGenerationEvidenceBundleManifestV1Relations, CandidateGenerationEvidenceBundleRoleV1,
    StructuredResponseArtifactV1Input,
};
use rewrite_types::{CancellationToken, Digest};

#[test]
fn exact_plan_derives_candidate_and_every_manifest_member() {
    let fixture = BundleFixture::new();
    let plan = CandidateGenerationEvidenceBundlePublicationPlan::compile(
        &fixture.plan_input(),
        CandidateGenerationEvidenceBundleLimits::default(),
        &rewrite_types::CancellationToken::new(),
    )
    .expect("compile exact evidence bundle plan");

    assert_eq!(plan.manifest(), &fixture.manifest);
    assert_eq!(
        fixture.planned_attempt.cluster_id(),
        fixture.cluster.cluster_id(),
    );
    assert_eq!(
        fixture.planned_attempt.suite_manifest_id(),
        fixture.suite.suite_manifest_id(),
    );
    assert_eq!(
        fixture.planned_attempt.repetition_id(),
        fixture.repetition.repetition_id(),
    );
    assert_eq!(plan.files.len(), fixture.manifest.entries().len());
    assert_eq!(
        plan.files
            .iter()
            .find(|file| file.path == fixture.candidate_path)
            .expect("derived candidate file")
            .bytes,
        CANDIDATE_BYTES,
    );
    assert_eq!(
        plan.exact_tree_entries,
        plan.files.len() + 3,
        "records and candidates directories plus the reserved manifest",
    );
    let debug = format!("{plan:?}");
    assert!(
        debug.contains(fixture.manifest.evidence_bundle_id().digest().as_str()),
        "debug names only the bundle identity and counts",
    );
    assert!(!debug.contains(fixture.candidate_path.as_str()));
}

#[test]
fn plan_rejects_candidate_request_and_limit_substitution() {
    let fixture = BundleFixture::new();
    let changed_manifest = fixture.manifest_for_candidate(b"caller-selected");
    let mut changed_candidate = fixture.plan_input();
    changed_candidate.manifest = &changed_manifest;
    assert!(matches!(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &changed_candidate,
            CandidateGenerationEvidenceBundleLimits::default(),
            &rewrite_types::CancellationToken::new(),
        ),
        Err(CandidateGenerationEvidenceBundleError::PlanMismatch)
    ));

    let mut changed_request_value = fixture.structured_request.clone();
    changed_request_value.input.push('!');
    let mut changed_request = fixture.plan_input();
    changed_request.structured_request = &changed_request_value;
    assert!(matches!(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &changed_request,
            CandidateGenerationEvidenceBundleLimits::default(),
            &rewrite_types::CancellationToken::new(),
        ),
        Err(CandidateGenerationEvidenceBundleError::PlanMismatch)
    ));

    let limits = CandidateGenerationEvidenceBundleLimits {
        maximum_total_bytes: 1,
        ..CandidateGenerationEvidenceBundleLimits::default()
    };
    assert!(matches!(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &fixture.plan_input(),
            limits,
            &rewrite_types::CancellationToken::new(),
        ),
        Err(CandidateGenerationEvidenceBundleError::LimitExceeded)
    ));
}

#[test]
fn plan_observes_cancellation_before_compilation_work() {
    let fixture = BundleFixture::new();
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &fixture.plan_input(),
            CandidateGenerationEvidenceBundleLimits::default(),
            &cancellation,
        ),
        Err(CandidateGenerationEvidenceBundleError::Cancelled)
    ));
}

#[test]
fn auxiliary_bytes_are_bounded_and_matched_before_cloning() {
    let fixture = BundleFixture::new();
    let declared_bytes = b"x";
    let auxiliary_path = super::support::path("observations/worker.json");
    let auxiliary_entry = CandidateGenerationEvidenceBundleEntryV1::new(
        auxiliary_path.clone(),
        CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
        ArtifactId::from_digest(Digest::sha256(declared_bytes)),
        1,
    );
    let mut entries = fixture.manifest.entries().to_vec();
    entries.push(auxiliary_entry);
    entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let response_input = StructuredResponseArtifactV1Input::new(
        fixture.structured_response.content_artifact_id().clone(),
        fixture.structured_response.byte_size(),
    )
    .expect("compile response input");
    let manifest = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &fixture.qualification_plan,
            planned_attempt: &fixture.planned_attempt,
            precursor: &fixture.precursor,
            managed_evidence: &fixture.managed_evidence,
            cleanup: &fixture.cleanup,
            structured_response_artifact: &response_input,
        },
        entries,
        fixture.manifest.candidate_artifacts().to_vec(),
    )
    .expect("compile manifest with auxiliary observation");
    let oversized = vec![b'x'; 64 * 1_024];
    let auxiliary = [CandidateGenerationEvidenceBundleAuxiliaryArtifact {
        relative_path: &auxiliary_path,
        role: CandidateGenerationEvidenceBundleRoleV1::AuxiliaryObservation,
        bytes: &oversized,
    }];
    let mut input = fixture.plan_input();
    input.auxiliary_artifacts = &auxiliary;
    input.manifest = &manifest;

    assert!(matches!(
        CandidateGenerationEvidenceBundlePublicationPlan::compile(
            &input,
            CandidateGenerationEvidenceBundleLimits::default(),
            &CancellationToken::new(),
        ),
        Err(CandidateGenerationEvidenceBundleError::PlanMismatch)
    ));
}
