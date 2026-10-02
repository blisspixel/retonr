//! Synthetic authority fixtures exercise real consuming rejection and SQL boundaries.

use super::*;
use crate::VerifiedCandidateJudgeJoin;
use rewrite_model::{
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationStatusV1,
};
use rewrite_model_store::WriteDisposition;

mod store;

pub(crate) fn exercise(
    mut active: ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    repository: &mut GenerationQualificationPreregistrationRepository,
    join: VerifiedCandidateJudgeJoin<'_, '_, '_, '_>,
    database: &std::path::Path,
    failure: &str,
    cancellation: &CancellationToken,
    control: &crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl,
) {
    // This private fixture substitutes the successful consumed managed call;
    // seed only its existing test lifecycle observation, never production approval.
    if failure != "rejection_no_acquisition" {
        active.lifecycle.mark_authority_acquired_for_test();
    }
    let (mut complete, _) = super::super::repeatability_settlement::tests::complete_fixture(
        &active,
        join,
        cancellation,
    );
    active
        .persist_complete_passed_repeatability(repository, &mut complete, cancellation)
        .expect("complete durable Passed parent");
    let mut phase = VerifiedGenerationQualificationResourcePhase::synthetic_settlement_fixture(
        complete,
        cancellation,
    );
    active
        .persist_generation_resource_phase(repository, &mut phase, cancellation)
        .expect("complete durable resource parent");
    let connection = rusqlite::Connection::open(database).expect("fixture connection");
    if failure == "rejection_ok" {
        store::exercise(&mut active, &mut phase, repository, database, cancellation);
    }
    configure_failure(
        &mut active,
        &connection,
        database,
        failure,
        cancellation,
        control,
    );
    let finalizer_calls_before = control.revalidation_calls();
    let result = active.reject_failed_generation_resources(repository, phase, cancellation);
    assert!(
        control.revalidation_calls() > finalizer_calls_before,
        "independent join finalization runs for every primary outcome"
    );
    if failure == "rejection_ok" {
        let (receipt, record, disposition) = result.expect("real consuming resource rejection");
        assert_eq!(
            receipt.terminal_status(),
            GenerationQualificationOperationTerminalStatusV1::Completed
        );
        assert_eq!(
            receipt.finalization_status(),
            GenerationQualificationOperationFinalizationStatusV1::Passed
        );
        assert_eq!(record.status(), GenerationQualificationStatusV1::Rejected);
        assert_eq!(disposition, WriteDisposition::Inserted);
    } else {
        assert!(result.is_err(), "{failure}");
        if failure == "rejection_initial_finalization" {
            assert_eq!(result, Err(Error::MandatoryFinalization));
        }
        if failure == "rejection_postcommit_cancel" {
            assert_eq!(result, Err(Error::Cancelled));
        }
        if failure == "rejection_postcommit_finalization" {
            assert_eq!(result, Err(Error::PrimaryAndFinalization));
        }
        if failure == "rejection_deadline" {
            assert!(matches!(result, Err(Error::DeadlineExceeded)));
        }
    }
    let committed = matches!(
        failure,
        "rejection_ok" | "rejection_postcommit_cancel" | "rejection_postcommit_finalization"
    );
    for table in [
        "generation_qualification_platform_evidence",
        "generation_qualification_license_evidence",
        "generation_human_adjudication_evidence_manifests",
        "generation_qualification_operation_receipts",
        "generation_qualification_records",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("suffix row count");
        assert_eq!(count, i64::from(committed), "{failure}: {table}");
    }
    assert_eq!(
        connection
            .query_row(
                "SELECT status FROM generation_repeatability_evidence_manifests",
                [],
                |row| row.get::<_, String>(0)
            )
            .expect("durable complete parent preserved"),
        "passed"
    );
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
        "rejection_foreign" => active.subject = crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject::new(),
        "rejection_unsettled" => active.next_judge_settlement = 0,
        "rejection_substituted_execution" => active.executed_judge_joins.clear(),
        "rejection_missing_ledger" => active.attempt_ledger = None,
        "rejection_initial_finalization" => control.fail_on_call(control.revalidation_calls() + 1),
        "rejection_wrong_execution" => active.executed_judge_joins[0] = serde_json::from_str(&format!("\"{}\"", rewrite_types::Digest::sha256(b"wrong consumed resource rejection join"))).expect("foreign inert join identity"),
        "rejection_missing_resource" => { connection.execute("DELETE FROM generation_resource_evidence_manifests", []).expect("remove parent"); },
        "rejection_corrupt_parent" => { connection.execute("UPDATE candidate_judge_response_aggregates SET canonical_json = ?1", [b"{}".as_slice()]).expect("corrupt canonical parent"); },
        "rejection_abort" => connection.execute_batch("CREATE TRIGGER abort_rejection BEFORE INSERT ON generation_qualification_records BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").expect("abort closeout"),
        "rejection_tamper" => connection.execute_batch("CREATE TRIGGER tamper_rejection AFTER INSERT ON generation_qualification_records BEGIN UPDATE generation_human_adjudication_evidence_manifests SET canonical_json = X'7b7d'; END;").expect("tamper closeout"),
        "rejection_cancel" => cancellation.cancel(),
        "rejection_deadline" => { active.prepared.expire_deadline_for_test(); cancellation.cancel(); },
        _ => {}
    }
    if matches!(
        failure,
        "rejection_postcommit_cancel" | "rejection_postcommit_finalization"
    ) {
        let database = database.to_owned();
        let token = cancellation.clone();
        let cancel = failure == "rejection_postcommit_cancel";
        control.fail_when(move || {
            let count: i64 = rusqlite::Connection::open(&database)
                .expect("postcommit database")
                .query_row(
                    "SELECT count(*) FROM generation_qualification_records",
                    [],
                    |row| row.get(0),
                )
                .expect("record count");
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
