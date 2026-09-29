use std::fs;
use std::time::{Duration, Instant};

use rewrite_model_store::{
    CandidateGenerationAttemptAdmissionClassV1, CandidateGenerationAttemptAdmissionV1,
};
use rewrite_types::{CancellationToken, Digest};
use serde::de::DeserializeOwned;

use super::CandidateActivationAdmissionError;
use crate::CandidateGenerationEvidenceRepository;

#[test]
fn pristine_plan_is_the_only_admission_success() {
    let (repository, data, temporary) = open_root("pristine");
    let plan_id = id("plan");
    let attempt = admission(
        &id("attempt"),
        CandidateGenerationAttemptAdmissionClassV1::NotStarted,
        None,
    );
    repository
        .reconcile_candidate_activation_admission(
            &plan_id,
            &[attempt],
            Instant::now() + Duration::from_secs(30),
            &CancellationToken::new(),
        )
        .expect("pristine admission");
    assert!(
        data.join("generation-evidence")
            .join(".staging")
            .read_dir()
            .expect("staging directory")
            .next()
            .is_none()
    );
    drop(repository);
    drop(temporary);
}

#[test]
fn unexpected_staging_refuses_and_keeps_the_leftover() {
    let (repository, data, temporary) = open_root("refusals");
    let plan_id = id("plan");
    let attempt_id = id("attempt");
    fs::write(
        data.join("generation-evidence")
            .join(".staging")
            .join("leftover"),
        b"x",
    )
    .expect("stage leftover");
    assert_eq!(
        refuse(&repository, &plan_id, &attempt_id, None),
        CandidateActivationAdmissionError::UnexpectedStaging
    );
    assert!(
        data.join("generation-evidence")
            .join(".staging")
            .join("leftover")
            .is_file()
    );
    drop(repository);
    drop(temporary);
}

#[test]
fn checkpoint_only_metadata_refuses_activation() {
    let (repository, _data, temporary) = open_root("checkpoint");
    let plan_id = id("plan");
    let attempt_id = id("attempt");
    let checkpoint = admission(
        &attempt_id,
        CandidateGenerationAttemptAdmissionClassV1::CheckpointOnly,
        None,
    );
    assert_eq!(
        repository
            .reconcile_candidate_activation_admission(&plan_id, &[checkpoint], deadline(), &token())
            .expect_err("checkpoint refusal"),
        CandidateActivationAdmissionError::CheckpointOnly
    );
    drop(repository);
    drop(temporary);
}

#[test]
fn publication_orphan_refuses_and_keeps_the_directory() {
    let (repository, data, temporary) = open_root("orphan");
    let plan_id = id("plan");
    let attempt_id = id("attempt");
    let bundle_id: rewrite_model::CandidateGenerationEvidenceBundleId = id("bundle");
    fs::create_dir_all(bundle_path(&data, &plan_id, &attempt_id, &bundle_id))
        .expect("publication orphan");
    assert_eq!(
        refuse(&repository, &plan_id, &attempt_id, None),
        CandidateActivationAdmissionError::PublicationOrphan
    );
    assert!(bundle_path(&data, &plan_id, &attempt_id, &bundle_id).is_dir());
    drop(repository);
    drop(temporary);
}

#[test]
fn completed_metadata_refuses_until_its_bundle_directory_matches() {
    let (repository, data, temporary) = open_root("completed");
    let plan_id = id("plan");
    let attempt_id = id("attempt");
    let bundle_id: rewrite_model::CandidateGenerationEvidenceBundleId = id("bundle");
    assert_eq!(
        refuse(&repository, &plan_id, &attempt_id, Some(bundle_id.clone())),
        CandidateActivationAdmissionError::StorageMismatch
    );
    fs::create_dir_all(bundle_path(&data, &plan_id, &attempt_id, &bundle_id))
        .expect("matching bundle");
    let completed = admission(
        &attempt_id,
        CandidateGenerationAttemptAdmissionClassV1::TerminalCompleted,
        Some(bundle_id.clone()),
    );
    assert_eq!(
        repository
            .reconcile_candidate_activation_admission(&plan_id, &[completed], deadline(), &token())
            .expect_err("completed refusal"),
        CandidateActivationAdmissionError::TerminalCompleted
    );
    assert!(bundle_path(&data, &plan_id, &attempt_id, &bundle_id).is_dir());
    drop(repository);
    drop(temporary);
}

fn refuse(
    repository: &CandidateGenerationEvidenceRepository,
    plan_id: &rewrite_model::GenerationQualificationPlanId,
    attempt_id: &rewrite_model::PlannedCandidateAttemptId,
    bundle_id: Option<rewrite_model::CandidateGenerationEvidenceBundleId>,
) -> CandidateActivationAdmissionError {
    let class = if bundle_id.is_some() {
        CandidateGenerationAttemptAdmissionClassV1::TerminalCompleted
    } else {
        CandidateGenerationAttemptAdmissionClassV1::NotStarted
    };
    repository
        .reconcile_candidate_activation_admission(
            plan_id,
            &[admission(attempt_id, class, bundle_id)],
            deadline(),
            &token(),
        )
        .expect_err("admission refusal")
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

fn token() -> CancellationToken {
    CancellationToken::new()
}

fn bundle_path(
    data: &std::path::Path,
    plan_id: &rewrite_model::GenerationQualificationPlanId,
    attempt_id: &rewrite_model::PlannedCandidateAttemptId,
    bundle_id: &rewrite_model::CandidateGenerationEvidenceBundleId,
) -> std::path::PathBuf {
    data.join("generation-evidence")
        .join("bundles")
        .join("v1")
        .join(plan_id.digest().as_str())
        .join(attempt_id.digest().as_str())
        .join(bundle_id.digest().as_str())
}

fn open_root(
    label: &str,
) -> (
    CandidateGenerationEvidenceRepository,
    std::path::PathBuf,
    tempfile::TempDir,
) {
    let temporary = tempfile::tempdir().expect("temporary root");
    let data = temporary.path().join(label);
    fs::create_dir(&data).expect("app data");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("evidence root");
    (repository, data, temporary)
}

fn admission(
    attempt_id: &rewrite_model::PlannedCandidateAttemptId,
    class: CandidateGenerationAttemptAdmissionClassV1,
    bundle_id: Option<rewrite_model::CandidateGenerationEvidenceBundleId>,
) -> CandidateGenerationAttemptAdmissionV1 {
    CandidateGenerationAttemptAdmissionV1::from_observation(attempt_id.clone(), class, bundle_id)
        .expect("admission observation")
}

fn id<T: DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(
        Digest::sha256(label.as_bytes()).as_str().to_owned(),
    ))
    .expect("typed digest")
}
