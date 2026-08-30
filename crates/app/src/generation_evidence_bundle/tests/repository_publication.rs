use std::fs;

use rewrite_model_store::CandidateGenerationEvidenceStorageV1Limits;
use rewrite_types::CancellationToken;

use super::support::{BundleFixture, CANDIDATE_BYTES};
use crate::{
    CandidateGenerationEvidenceBundleLimits, CandidateGenerationEvidenceBundlePublicationPlan,
    CandidateGenerationEvidenceRepository, CandidateGenerationEvidenceRepositoryError,
};

#[test]
fn repository_publishes_and_reacquires_without_a_path_handoff() {
    let fixture = BundleFixture::new();
    let temporary = tempfile::tempdir().expect("temporary app data parent");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize repository");
    let storage = repository
        .bundle_storage_reference(
            fixture.qualification_plan.qualification_plan_id().clone(),
            fixture.planned_attempt.planned_attempt_id().clone(),
            fixture.manifest.evidence_bundle_id().clone(),
            stored_limits(),
        )
        .expect("compile durable storage reference");
    let publication = repository
        .publish_bundle(
            storage.clone(),
            compiled(&fixture),
            &CancellationToken::new(),
        )
        .expect("repository-owned publication");
    assert_eq!(publication.storage(), &storage);
    assert_eq!(publication.lease().manifest(), &fixture.manifest);
    assert_eq!(
        publication
            .lease()
            .member_bytes(
                &fixture.candidate_path,
                u64::try_from(CANDIDATE_BYTES.len()).expect("candidate size"),
                &CancellationToken::new(),
            )
            .expect("retained candidate bytes"),
        CANDIDATE_BYTES,
    );
    let persisted_readback = publication.readback().clone();
    drop(publication);

    let reacquired = repository
        .reacquire_bundle(
            storage.clone(),
            &fixture.manifest,
            &persisted_readback,
            &CancellationToken::new(),
        )
        .expect("repository-owned reacquisition");
    assert_eq!(reacquired.storage(), &storage);
    assert_eq!(reacquired.readback(), &persisted_readback);
    reacquired
        .lease()
        .revalidate(&CancellationToken::new())
        .expect("reacquired lease remains exact");

    assert!(
        data.join("generation-evidence")
            .join(storage.relative_reference().as_str())
            .is_dir()
    );
    assert_eq!(
        fs::read_dir(data.join("generation-evidence/.staging"))
            .expect("enumerate clean staging root")
            .count(),
        0,
    );
    assert!(matches!(
        repository.publish_bundle(storage, compiled(&fixture), &CancellationToken::new()),
        Err(CandidateGenerationEvidenceRepositoryError::EvidenceBundle(
            crate::CandidateGenerationEvidenceBundleError::DestinationExists
        ))
    ));
}

#[test]
fn repository_rejects_plan_limit_substitution_and_destination_case_alias() {
    let fixture = BundleFixture::new();
    let temporary = tempfile::tempdir().expect("temporary app data parent");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize repository");
    let substituted_limits = CandidateGenerationEvidenceStorageV1Limits::new(
        u32::try_from(CandidateGenerationEvidenceBundleLimits::default().maximum_tree_entries - 1)
            .expect("tree limit"),
        u16::try_from(CandidateGenerationEvidenceBundleLimits::default().maximum_tree_depth)
            .expect("depth limit"),
        CandidateGenerationEvidenceBundleLimits::default().maximum_total_bytes,
    )
    .expect("substituted stored limits");
    let storage = repository
        .bundle_storage_reference(
            fixture.qualification_plan.qualification_plan_id().clone(),
            fixture.planned_attempt.planned_attempt_id().clone(),
            fixture.manifest.evidence_bundle_id().clone(),
            substituted_limits,
        )
        .expect("compile substituted reference");
    assert!(matches!(
        repository.publish_bundle(storage, compiled(&fixture), &CancellationToken::new()),
        Err(CandidateGenerationEvidenceRepositoryError::StorageContract(
            _
        ))
    ));

    let storage = repository
        .bundle_storage_reference(
            fixture.qualification_plan.qualification_plan_id().clone(),
            fixture.planned_attempt.planned_attempt_id().clone(),
            fixture.manifest.evidence_bundle_id().clone(),
            stored_limits(),
        )
        .expect("compile exact reference");
    let attempt_parent = data
        .join("generation-evidence/bundles/v1")
        .join(storage.qualification_plan_id().digest().as_str())
        .join(storage.planned_attempt_id().digest().as_str());
    fs::create_dir_all(&attempt_parent).expect("create canonical parents");
    fs::create_dir(
        attempt_parent.join(
            storage
                .evidence_bundle_id()
                .digest()
                .as_str()
                .to_ascii_uppercase(),
        ),
    )
    .expect("case-alias destination");
    assert!(matches!(
        repository.publish_bundle(storage, compiled(&fixture), &CancellationToken::new()),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));
}

#[cfg(unix)]
#[test]
fn repository_rejects_indirect_typed_id_parent() {
    use std::os::unix::fs::symlink;

    let fixture = BundleFixture::new();
    let temporary = tempfile::tempdir().expect("temporary app data parent");
    let data = temporary.path().join("data");
    let foreign = temporary.path().join("foreign");
    fs::create_dir(&data).expect("app data directory");
    fs::create_dir(&foreign).expect("foreign directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize repository");
    let storage = repository
        .bundle_storage_reference(
            fixture.qualification_plan.qualification_plan_id().clone(),
            fixture.planned_attempt.planned_attempt_id().clone(),
            fixture.manifest.evidence_bundle_id().clone(),
            stored_limits(),
        )
        .expect("compile exact reference");
    symlink(
        &foreign,
        data.join("generation-evidence/bundles/v1")
            .join(storage.qualification_plan_id().digest().as_str()),
    )
    .expect("indirect plan directory");

    assert!(matches!(
        repository.publish_bundle(storage, compiled(&fixture), &CancellationToken::new()),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));
    assert_eq!(
        fs::read_dir(&foreign)
            .expect("enumerate untouched foreign directory")
            .count(),
        0,
    );
}

fn compiled(fixture: &BundleFixture) -> CandidateGenerationEvidenceBundlePublicationPlan {
    CandidateGenerationEvidenceBundlePublicationPlan::compile(
        &fixture.plan_input(),
        CandidateGenerationEvidenceBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("compile publication plan")
}

fn stored_limits() -> CandidateGenerationEvidenceStorageV1Limits {
    let limits = CandidateGenerationEvidenceBundleLimits::default();
    CandidateGenerationEvidenceStorageV1Limits::new(
        u32::try_from(limits.maximum_tree_entries).expect("tree limit"),
        u16::try_from(limits.maximum_tree_depth).expect("depth limit"),
        limits.maximum_total_bytes,
    )
    .expect("stored read limits")
}
