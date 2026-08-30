use std::{cell::Cell, fs};

use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::support::{BundleFixture, CANDIDATE_BYTES};
use crate::{
    CandidateGenerationEvidenceBundleDestination, CandidateGenerationEvidenceBundleError,
    CandidateGenerationEvidenceBundleLimits, CandidateGenerationEvidenceBundlePublicationPlan,
    CandidateGenerationEvidenceBundlePublisher, CandidateGenerationEvidenceBundleSource,
    CandidateGenerationEvidenceBundleVerifier,
};

#[test]
fn publication_is_no_replace_and_returns_a_fresh_reacquirable_lease() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary publication parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("candidate-bundle"),
    )
    .expect("portable destination");
    let publication = CandidateGenerationEvidenceBundlePublisher::publish(
        compiled(&fixture),
        &destination,
        &CancellationToken::new(),
    )
    .expect("publish exact bundle");

    assert_eq!(publication.lease().manifest(), &fixture.manifest);
    assert_eq!(
        publication
            .lease()
            .member_bytes(
                &fixture.candidate_path,
                u64::try_from(CANDIDATE_BYTES.len()).expect("candidate length"),
                &CancellationToken::new(),
            )
            .expect("read retained candidate"),
        CANDIDATE_BYTES,
    );
    let source =
        CandidateGenerationEvidenceBundleSource::new(destination.path()).expect("published source");
    let reacquired = CandidateGenerationEvidenceBundleVerifier::reacquire(
        &source,
        &fixture.manifest,
        publication.readback(),
        CandidateGenerationEvidenceBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("fresh complete readback");
    assert!(publication.lease().same_bundle(&reacquired));
    publication
        .lease()
        .revalidate(&CancellationToken::new())
        .expect("original lease remains exact");
    reacquired
        .revalidate(&CancellationToken::new())
        .expect("fresh lease remains exact");
}

#[test]
fn occupied_destination_is_preserved_and_never_replaced() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary publication parent");
    let occupied = directory.path().join("candidate-bundle");
    fs::create_dir(&occupied).expect("create occupied destination");
    fs::write(occupied.join("winner.txt"), b"winner").expect("write winner");
    let destination =
        CandidateGenerationEvidenceBundleDestination::new(&occupied).expect("portable destination");

    assert!(matches!(
        CandidateGenerationEvidenceBundlePublisher::publish(
            compiled(&fixture),
            &destination,
            &CancellationToken::new(),
        ),
        Err(CandidateGenerationEvidenceBundleError::DestinationExists)
    ));
    assert_eq!(
        fs::read(occupied.join("winner.txt")).expect("read preserved winner"),
        b"winner",
    );
    assert_eq!(
        fs::read_dir(directory.path())
            .expect("enumerate publication parent")
            .count(),
        1,
        "failed publication leaves no staging root",
    );
}

#[test]
fn fresh_readback_rejects_digest_drift_and_extra_tree_entries() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary publication parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("candidate-bundle"),
    )
    .expect("portable destination");
    let publication = CandidateGenerationEvidenceBundlePublisher::publish(
        compiled(&fixture),
        &destination,
        &CancellationToken::new(),
    )
    .expect("publish exact bundle");
    let readback = publication.readback().clone();
    drop(publication);
    let source =
        CandidateGenerationEvidenceBundleSource::new(destination.path()).expect("published source");
    let candidate = destination.path().join("candidates/000.txt");
    fs::write(&candidate, b"NO").expect("mutate candidate after releasing lease");
    assert_reacquire_fails(
        &source,
        &fixture,
        &readback,
        &CandidateGenerationEvidenceBundleError::Changed,
    );

    fs::write(candidate, CANDIDATE_BYTES).expect("restore exact candidate");
    fs::write(destination.path().join("extra.txt"), b"extra").expect("add extra tree entry");
    assert_reacquire_fails(
        &source,
        &fixture,
        &readback,
        &CandidateGenerationEvidenceBundleError::TreeMismatch,
    );
}

#[test]
fn exact_returned_member_bytes_reject_a_read_race() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary read-race parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("candidate-bundle"),
    )
    .expect("portable destination");
    let publication = CandidateGenerationEvidenceBundlePublisher::publish(
        compiled(&fixture),
        &destination,
        &CancellationToken::new(),
    )
    .expect("publish exact bundle");
    let candidate_path = destination.path().join("candidates/000.txt");
    let mutated = Cell::new(false);
    let result = publication.lease().member_bytes_with_read_hooks(
        &fixture.candidate_path,
        u64::try_from(CANDIDATE_BYTES.len()).expect("candidate length"),
        &CancellationToken::new(),
        || {
            mutated.set(fs::write(&candidate_path, b"NO").is_ok());
        },
        || {
            if mutated.get() {
                fs::write(&candidate_path, CANDIDATE_BYTES)
                    .expect("restore candidate after intercepted read");
            }
        },
    );

    if mutated.get() {
        assert!(matches!(
            result,
            Err(CandidateGenerationEvidenceBundleError::Changed)
        ));
    } else {
        assert_eq!(
            result.expect("sealed platform blocks concurrent mutation"),
            CANDIDATE_BYTES,
        );
    }
}

#[test]
fn cancellation_before_publication_creates_nothing() {
    let fixture = BundleFixture::new();
    let directory = tempdir().expect("temporary publication parent");
    let destination = CandidateGenerationEvidenceBundleDestination::new(
        directory.path().join("candidate-bundle"),
    )
    .expect("portable destination");
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    assert!(matches!(
        CandidateGenerationEvidenceBundlePublisher::publish(
            compiled(&fixture),
            &destination,
            &cancellation,
        ),
        Err(CandidateGenerationEvidenceBundleError::Cancelled)
    ));
    assert!(!destination.path().exists());
    assert_eq!(
        fs::read_dir(directory.path())
            .expect("enumerate empty parent")
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

fn assert_reacquire_fails(
    source: &CandidateGenerationEvidenceBundleSource,
    fixture: &BundleFixture,
    readback: &rewrite_model::CandidateGenerationEvidenceBundleReadbackV1,
    expected: &CandidateGenerationEvidenceBundleError,
) {
    let observed = CandidateGenerationEvidenceBundleVerifier::reacquire(
        source,
        &fixture.manifest,
        readback,
        CandidateGenerationEvidenceBundleLimits::default(),
        &CancellationToken::new(),
    );
    let Err(error) = observed else {
        panic!("drifted bundle must not be reacquired");
    };
    assert_eq!(
        std::mem::discriminant(&error),
        std::mem::discriminant(expected)
    );
}
