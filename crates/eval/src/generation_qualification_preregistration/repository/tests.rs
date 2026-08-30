use std::{path::Path, time::Instant};

use rewrite_app::{
    SyntheticGenerationQualificationScenario, with_synthetic_generation_qualification_fixture,
};
use rewrite_model_store::WriteDisposition;
use rewrite_types::CancellationToken;
use rusqlite::{Connection, params};
use tempfile::tempdir;

use super::*;
use crate::{GenerationQualificationOperationDraft, ProjectedGenerationQualificationOperation};

#[test]
fn exact_typed_pair_is_durable_and_idempotent() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("idempotent.db");
        let mut repository = open(&path);
        let cancellation = CancellationToken::new();
        let first = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &cancellation,
            |_| Ok(()),
        )
        .expect("initial transaction");
        assert_eq!(first.operation_policy, WriteDisposition::Inserted);
        assert_eq!(first.request_projection, WriteDisposition::Inserted);
        let second = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &cancellation,
            |readback| {
                assert_eq!(
                    readback.plan_foundation().plan(),
                    foundation.plan_foundation.plan
                );
                assert_eq!(
                    readback.operation_policy(),
                    projected.stream.operation_policy()
                );
                assert_eq!(readback.request_projection(), projected.stream.projection());
                Ok(())
            },
        )
        .expect("idempotent transaction");
        assert_eq!(second.operation_policy, WriteDisposition::AlreadyPresent);
        assert_eq!(second.request_projection, WriteDisposition::AlreadyPresent);
        drop(repository);
        assert_eq!(row_counts(&path), (1, 1));
    });
}

#[test]
fn missing_runtime_artifact_set_prevents_preregistration() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("missing-runtime-artifact-set.db");
        let mut repository = open(&path);
        let callback_called = std::cell::Cell::new(false);
        let result = transact_exact(
            &mut repository,
            &projected,
            GenerationQualificationPreregistrationFoundationV1Input {
                runtime_artifact_sets: &[],
                plan_foundation: foundation.plan_foundation,
            },
            &CancellationToken::new(),
            |_| {
                callback_called.set(true);
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(GenerationQualificationPreparationError::ReadbackMismatch)
        );
        assert!(!callback_called.get());
        drop(repository);
        assert_eq!(row_counts(&path), (0, 0));
    });
}

#[test]
fn exact_partial_policy_is_completed_atomically() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("partial.db");
        initialize(&path);
        insert_exact_policy(&path, &projected);
        let mut repository = open(&path);
        let disposition = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .expect("complete exact partial pair");
        assert_eq!(
            disposition.operation_policy,
            WriteDisposition::AlreadyPresent
        );
        assert_eq!(disposition.request_projection, WriteDisposition::Inserted);
        drop(repository);
        assert_eq!(row_counts(&path), (1, 1));
    });
}

#[test]
fn immutable_collision_is_redacted_and_preserves_the_exact_pair() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("collision.db");
        let cancellation = CancellationToken::new();
        let mut repository = open(&path);
        transact_exact(
            &mut repository,
            &projected,
            foundation,
            &cancellation,
            |_| Ok(()),
        )
        .expect("initial transaction");
        drop(repository);
        let connection = Connection::open(&path).expect("open collision fixture");
        connection
            .execute(
                "UPDATE generation_qualification_operation_policies
                 SET canonical_json = X'7B7D' WHERE operation_policy_id = ?1",
                [projected
                    .stream
                    .operation_policy()
                    .operation_policy_id()
                    .digest()
                    .as_str()],
            )
            .expect("replace immutable bytes");
        drop(connection);
        let mut repository = open(&path);
        let result = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &cancellation,
            |_| Ok(()),
        );
        assert_eq!(
            result,
            Err(GenerationQualificationPreparationError::RepositoryConflict)
        );
        drop(repository);
        assert_eq!(row_counts(&path), (1, 1));
    });
}

#[test]
fn validation_rejection_rolls_back_both_staged_rows() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("validation-rollback.db");
        let mut repository = open(&path);
        let result = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &CancellationToken::new(),
            |_| Err(GenerationQualificationPreparationError::PlatformAssessmentMismatch),
        );
        assert_eq!(
            result,
            Err(GenerationQualificationPreparationError::PlatformAssessmentMismatch)
        );
        drop(repository);
        assert_eq!(row_counts(&path), (0, 0));
    });
}

#[test]
fn cancellation_and_deadline_gates_roll_back() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("gate-rollback.db");
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let mut repository = open(&path);
        let cancelled = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &cancellation,
            |_| Ok(()),
        );
        assert_eq!(
            cancelled,
            Err(GenerationQualificationPreparationError::Cancelled)
        );
        drop(repository);
        assert_eq!(row_counts(&path), (0, 0));

        let path = directory.path().join("deadline-rollback.db");
        let mut repository = open(&path);
        let expired = transact_at_deadline(
            &mut repository,
            &projected,
            foundation,
            Instant::now(),
            &CancellationToken::new(),
            |_| Ok(()),
        );
        assert_eq!(
            expired,
            Err(GenerationQualificationPreparationError::DeadlineExceeded)
        );
        drop(repository);
        assert_eq!(row_counts(&path), (0, 0));
    });
}

#[test]
fn cancellation_after_typed_readback_rolls_back_before_commit() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("precommit-cancel.db");
        let cancellation = CancellationToken::new();
        let mut repository = open(&path);
        let result = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &cancellation,
            |readback| {
                assert_eq!(
                    readback.operation_policy(),
                    projected.stream.operation_policy()
                );
                cancellation.cancel();
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(GenerationQualificationPreparationError::Cancelled)
        );
        drop(repository);
        assert_eq!(row_counts(&path), (0, 0));
    });
}

#[test]
fn oversized_cold_blob_is_rejected_before_readback() {
    with_projected(|projected, foundation| {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("oversized.db");
        let mut repository = open(&path);
        transact_exact(
            &mut repository,
            &projected,
            foundation,
            &CancellationToken::new(),
            |_| Ok(()),
        )
        .expect("initial transaction");
        drop(repository);
        let connection = Connection::open(&path).expect("open oversized fixture");
        connection
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("allow corruption fixture");
        connection
            .execute(
                "UPDATE generation_qualification_request_projections
                 SET canonical_json = zeroblob(4194305)
                 WHERE request_projection_id = ?1",
                [projected
                    .stream
                    .projection()
                    .request_projection_id()
                    .digest()
                    .as_str()],
            )
            .expect("oversize projection blob");
        drop(connection);
        let mut repository = open(&path);
        let callback_called = std::cell::Cell::new(false);
        let result = transact_exact(
            &mut repository,
            &projected,
            foundation,
            &CancellationToken::new(),
            |_| {
                callback_called.set(true);
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err(GenerationQualificationPreparationError::ReadbackMismatch)
        );
        assert!(!callback_called.get());
    });
}

#[test]
fn store_error_categories_remain_content_redacted() {
    assert_eq!(
        map_store_error(&StoreError::RecordTooLarge),
        GenerationQualificationPreparationError::RepositoryLimit
    );
    assert_eq!(
        map_store_error(&StoreError::InvalidLimit),
        GenerationQualificationPreparationError::RepositoryUnavailable
    );
    let directory = tempdir().expect("temporary directory");
    assert!(matches!(
        GenerationQualificationPreregistrationRepository::open(directory.path()),
        Err(GenerationQualificationPreregistrationOpenError::Unavailable)
    ));
}

fn with_projected<T>(
    callback: impl for<'records, 'store> FnOnce(
        ProjectedGenerationQualificationOperation<'records, 'store>,
        GenerationQualificationPreregistrationFoundationV1Input<'records>,
    ) -> T,
) -> T {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |input, _platform, _proof, _license_policy, _production_policy| {
            let cancellation = CancellationToken::new();
            let draft = GenerationQualificationOperationDraft::begin(
                input.operation_policy_relations,
                input.operation_policy_input,
                &cancellation,
            )
            .expect("draft");
            callback(
                draft
                    .project(input.case_authorities, &cancellation)
                    .expect("projection"),
                input.foundation,
            )
        },
    )
}

fn open(path: &Path) -> GenerationQualificationPreregistrationRepository {
    GenerationQualificationPreregistrationRepository::open(path).expect("open repository")
}

fn initialize(path: &Path) {
    drop(open(path));
}

fn transact_exact(
    repository: &mut GenerationQualificationPreregistrationRepository,
    projected: &ProjectedGenerationQualificationOperation<'_, '_>,
    foundation: GenerationQualificationPreregistrationFoundationV1Input<'_>,
    cancellation: &CancellationToken,
    validate: impl for<'readback> FnOnce(
        GenerationQualificationPreregistrationReadback<'readback>,
    ) -> Result<(), GenerationQualificationPreparationError>,
) -> Result<
    GenerationQualificationPreregistrationWriteDisposition,
    GenerationQualificationPreparationError,
> {
    transact_at_deadline(
        repository,
        projected,
        foundation,
        projected.stream.deadline(),
        cancellation,
        validate,
    )
}

fn transact_at_deadline(
    repository: &mut GenerationQualificationPreregistrationRepository,
    projected: &ProjectedGenerationQualificationOperation<'_, '_>,
    foundation: GenerationQualificationPreregistrationFoundationV1Input<'_>,
    deadline: Instant,
    cancellation: &CancellationToken,
    validate: impl for<'readback> FnOnce(
        GenerationQualificationPreregistrationReadback<'readback>,
    ) -> Result<(), GenerationQualificationPreparationError>,
) -> Result<
    GenerationQualificationPreregistrationWriteDisposition,
    GenerationQualificationPreparationError,
> {
    repository.persist_foundation(
        foundation,
        projected.stream.operation_policy_relations(),
        deadline,
        cancellation,
    )?;
    repository
        .transact(
            projected.stream.operation_policy(),
            projected.stream.operation_policy_relations(),
            projected.stream.operation_policy_input(),
            projected.stream.projection(),
            projected.stream.entry_inputs(),
            deadline,
            cancellation,
            validate,
        )
        .map(|((), disposition)| disposition)
}

fn insert_exact_policy(path: &Path, projected: &ProjectedGenerationQualificationOperation<'_, '_>) {
    let policy = projected.stream.operation_policy();
    let canonical_json = serde_json::to_vec(policy).expect("canonical policy JSON");
    let connection = Connection::open(path).expect("open partial fixture");
    connection
        .execute(
            "INSERT INTO generation_qualification_operation_policies (
                 operation_policy_id, generation_qualification_plan_id, suite_manifest_id,
                 target_generation_system_id, baseline_generation_system_id, canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                policy.operation_policy_id().digest().as_str(),
                policy.generation_qualification_plan_id().digest().as_str(),
                policy.suite_manifest_id().digest().as_str(),
                policy.target_generation_system_id().digest().as_str(),
                policy.baseline_generation_system_id().digest().as_str(),
                canonical_json,
            ],
        )
        .expect("insert exact partial policy");
}

fn row_counts(path: &Path) -> (i64, i64) {
    let connection = Connection::open(path).expect("open durable readback");
    let policies = connection
        .query_row(
            "SELECT COUNT(*) FROM generation_qualification_operation_policies",
            [],
            |row| row.get(0),
        )
        .expect("count policies");
    let projections = connection
        .query_row(
            "SELECT COUNT(*) FROM generation_qualification_request_projections",
            [],
            |row| row.get(0),
        )
        .expect("count projections");
    (policies, projections)
}
