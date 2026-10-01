//! Private synthetic authorities exercise real resource persistence contracts.

use super::*;
use crate::VerifiedCandidateJudgeJoin;
use rewrite_model::GenerationQualificationPhaseStatusV1;
use rewrite_model_store::WriteDisposition;

pub(crate) fn exercise(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    repository: &mut GenerationQualificationPreregistrationRepository,
    join: VerifiedCandidateJudgeJoin<'_, '_, '_, '_>,
    database: &std::path::Path,
    failure: &str,
    cancellation: &CancellationToken,
    control: &crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl,
) {
    let (mut complete, _) =
        super::super::repeatability_settlement::tests::complete_fixture(active, join, cancellation);
    active
        .persist_complete_passed_repeatability(repository, &mut complete, cancellation)
        .expect("real durable complete phase parent");
    let mut phase = VerifiedGenerationQualificationResourcePhase::synthetic_settlement_fixture(
        complete,
        cancellation,
    );
    let expected_manifest = phase.resource_manifest().clone();
    let connection = rusqlite::Connection::open(database).expect("fixture database");
    configure_failure(
        active,
        &connection,
        database,
        failure,
        cancellation,
        control,
    );
    let result = active.persist_generation_resource_phase(repository, &mut phase, cancellation);
    if matches!(failure, "resource_passed" | "resource_failed") {
        let (manifest, disposition) = result.expect("actual resource phase publication");
        assert_eq!(manifest, expected_manifest);
        assert_eq!(
            manifest.status(),
            if failure == "resource_failed" {
                GenerationQualificationPhaseStatusV1::Failed
            } else {
                GenerationQualificationPhaseStatusV1::Passed
            }
        );
        assert_eq!(disposition.manifest, WriteDisposition::Inserted);
        assert_eq!(
            disposition.ordered_results,
            vec![WriteDisposition::Inserted]
        );
        let mut reopened = GenerationQualificationPreregistrationRepository::open(database)
            .expect("cold repository");
        let (replayed, disposition) = active
            .persist_generation_resource_phase(&mut reopened, &mut phase, cancellation)
            .expect("same live authority cold immutable replay");
        assert_eq!(replayed, manifest);
        assert_eq!(disposition.manifest, WriteDisposition::AlreadyPresent);
        assert_eq!(
            disposition.ordered_results,
            vec![WriteDisposition::AlreadyPresent]
        );
        store::exercise(active, &mut phase, database, cancellation);
        // Actual production revalidation refuses these synthetic portable policy
        // facts even after their exact inert cohort is already durable.
        phase.refuse_synthetic_settlement_fixture();
        assert!(phase.revalidate(cancellation).is_err());
        assert_eq!(
            active.persist_generation_resource_phase(repository, &mut phase, cancellation),
            Err(ActiveGenerationQualificationResourceSettlementError::PrimaryAndFinalization)
        );
    } else {
        assert!(result.is_err(), "{failure}");
        assert_eq!(active.terminal, failure != "resource_unsettled");
        if failure == "resource_postcommit_cancel" {
            assert_eq!(
                result,
                Err(ActiveGenerationQualificationResourceSettlementError::Cancelled)
            );
        }
        if failure == "resource_deadline" {
            assert!(matches!(
                result,
                Err(ActiveGenerationQualificationResourceSettlementError::DeadlineExceeded)
            ));
        }
        let committed = matches!(
            failure,
            "resource_postcommit_finalization" | "resource_postcommit_cancel"
        );
        for table in [
            "generation_resource_attempt_result_records",
            "generation_resource_evidence_manifests",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .expect("resource rows");
            assert_eq!(count, i64::from(committed), "{failure}: {table}");
        }
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM generation_repeatability_evidence_manifests",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .expect("parent retained"),
            i64::from(failure != "resource_missing_repeatability")
        );
    }
}

fn configure_failure(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    connection: &rusqlite::Connection,
    database: &std::path::Path,
    failure: &str,
    cancellation: &CancellationToken,
    control: &crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl,
) {
    match failure {
        "resource_abort" => connection.execute_batch("CREATE TRIGGER abort_resource BEFORE INSERT ON generation_resource_evidence_manifests BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").expect("abort trigger"),
        "resource_tamper" => connection.execute_batch("CREATE TRIGGER tamper_resource AFTER INSERT ON generation_resource_evidence_manifests BEGIN UPDATE generation_resource_attempt_result_records SET canonical_json = X'7b7d'; END;").expect("tamper trigger"),
        "resource_corrupt_parent" => { connection.execute("UPDATE candidate_judge_response_aggregates SET canonical_json = ?1", [b"{}".as_slice()]).expect("corrupt full parent"); },
        "resource_missing_repeatability" => { connection.execute("DELETE FROM generation_repeatability_evidence_manifests", []).expect("missing complete durable parent"); },
        "resource_foreign" => active.subject = crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject::new(),
        "resource_unsettled" => active.next_judge_settlement = 0,
        "resource_substituted_execution" => active.executed_judge_joins.clear(),
        "resource_cancel" => cancellation.cancel(),
        "resource_deadline" => { active.prepared.expire_deadline_for_test(); cancellation.cancel(); },
        _ => {}
    }
    if matches!(
        failure,
        "resource_postcommit_finalization" | "resource_postcommit_cancel"
    ) {
        let database = database.to_owned();
        let token = cancellation.clone();
        let cancel = failure == "resource_postcommit_cancel";
        control.fail_when(move || {
            let count = rusqlite::Connection::open(&database)
                .expect("finalizer database")
                .query_row(
                    "SELECT count(*) FROM generation_resource_evidence_manifests",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .expect("phase count");
            if count == 0 {
                return false;
            }
            if cancel {
                token.cancel();
                false
            } else {
                true
            }
        });
    }
}

mod store;

#[test]
fn final_gate_has_deadline_priority_after_every_primary_and_finalizer() {
    use ActiveGenerationQualificationResourceSettlementError as Error;
    for primary in [Ok(7), Err(Error::OperationScope)] {
        assert_eq!(
            finish(primary, true, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineAndFinalization)
        );
        assert_eq!(
            finish(primary, true, Err(Error::Cancelled)),
            Err(Error::CancelledAndFinalization)
        );
        assert_eq!(
            finish(primary, false, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineExceeded)
        );
        assert_eq!(
            finish(primary, false, Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
    }
    assert_eq!(
        finish(Ok(7), true, Ok(())),
        Err(Error::MandatoryFinalization)
    );
    assert_eq!(
        finish::<u8>(Err(Error::Publication), true, Ok(())),
        Err(Error::PrimaryAndFinalization)
    );
}
