//! Synthetic authority fixtures exercise real consuming qualification closeout.

use super::*;
use crate::VerifiedCandidateJudgeJoin;
use rewrite_model::{
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseScopeV1,
    GenerationQualificationStatusV1,
};
use rewrite_model_store::WriteDisposition;

mod store;

#[expect(
    clippy::too_many_lines,
    reason = "exhaustive closeout execution matrix"
)]
pub(crate) fn exercise(
    mut active: ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    repository: &mut GenerationQualificationPreregistrationRepository,
    join: VerifiedCandidateJudgeJoin<'_, '_, '_, '_>,
    database: &std::path::Path,
    failure: &str,
    cancellation: &CancellationToken,
    control: &crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl,
) {
    if failure != "closeout_no_acquisition" {
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
    if failure == "closeout_ok" {
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

    let foundation = active.prepared.plan_foundation().clone();
    let target_id = active
        .prepared
        .operation_policy()
        .target_generation_system_id()
        .clone();
    let target = foundation
        .generation_systems()
        .iter()
        .find(|s| s.generation_system_id() == &target_id)
        .expect("target generation system");
    let scope = GenerationQualificationPhaseScopeV1 {
        generation_system: target,
        qualification_plan: foundation.plan(),
        suite: foundation.suite(),
    };
    let human_policy_digest = active
        .prepared
        .operation_policy()
        .human_adjudication_policy_digest();

    let human_status = if failure == "closeout_rejected_review" {
        GenerationQualificationPhaseStatusV1::Failed
    } else {
        GenerationQualificationPhaseStatusV1::Passed
    };
    let human_digest = [Digest::sha256(b"human adjudication review evidence")];
    let human_manifest = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope,
            phase_policy_digest: human_policy_digest,
            evidence_record_digests: &human_digest,
            status: human_status,
        },
    )
    .expect("human adjudication manifest");

    let human_adjudication = ActiveGenerationHumanAdjudicationOutcome::Reviewed {
        human_manifest: &human_manifest,
        human_evidence_digests: &human_digest,
    };

    let platform_evidence = active.prepared.platform_evidence().clone();
    let license_evidence = active.prepared.license_evidence().clone();

    let result = active.close_generation_qualification(
        repository,
        &mut phase,
        &human_adjudication,
        cancellation,
    );
    assert!(
        control.revalidation_calls() > finalizer_calls_before,
        "independent join finalization runs for every primary outcome"
    );
    if failure == "closeout_ok" || failure == "closeout_rejected_review" {
        let (receipt, record, disposition) = result.expect("real consuming qualification closeout");
        assert_eq!(
            receipt.terminal_status(),
            GenerationQualificationOperationTerminalStatusV1::Completed
        );
        assert_eq!(
            receipt.finalization_status(),
            GenerationQualificationOperationFinalizationStatusV1::Passed
        );
        let expected_status = if failure == "closeout_ok" {
            GenerationQualificationStatusV1::Qualified
        } else {
            GenerationQualificationStatusV1::Rejected
        };
        assert_eq!(record.status(), expected_status);
        assert_eq!(disposition, WriteDisposition::Inserted);

        if failure == "closeout_ok" {
            let input = crate::VerifiedGenerationQualificationCompilerInput {
                receipt: receipt.clone(),
                record: record.clone(),
                human_manifest: human_manifest.clone(),
                platform_evidence: platform_evidence.clone(),
                license_evidence: license_evidence.clone(),
            };
            let mut verified = crate::VerifiedGenerationQualificationCompiler::compile(
                repository,
                phase,
                input,
                cancellation,
            )
            .expect("verified generation qualification compilation");

            assert_eq!(verified.qualification_record(), &record);
            assert_eq!(verified.operation_receipt(), &receipt);
            assert_eq!(verified.human_manifest(), &human_manifest);
            assert_eq!(verified.platform_evidence(), &platform_evidence);
            assert_eq!(verified.license_evidence(), &license_evidence);
            assert_eq!(
                verified.target_generation_system().generation_system_id(),
                &target_id
            );
            assert_eq!(
                verified.qualification_plan().qualification_plan_id(),
                foundation.plan().qualification_plan_id()
            );
            assert_eq!(
                verified.suite_manifest().suite_manifest_id(),
                foundation.suite().suite_manifest_id()
            );

            verified
                .revalidate(cancellation)
                .expect("revalidation passes");
            verified
                .revalidate_for_mandatory_settlement_finalization()
                .expect("mandatory finalization revalidation passes");

            let debug_repr = format!("{verified:?}");
            assert!(debug_repr.contains("VerifiedGenerationQualification"));

            let cancelled = CancellationToken::new();
            cancelled.cancel();
            assert_eq!(
                verified.revalidate(&cancelled),
                Err(crate::VerifiedGenerationQualificationError::Cancelled)
            );
        } else {
            let input = crate::VerifiedGenerationQualificationCompilerInput {
                receipt,
                record,
                human_manifest,
                platform_evidence,
                license_evidence,
            };
            assert_eq!(
                crate::VerifiedGenerationQualificationCompiler::compile(
                    repository,
                    phase,
                    input,
                    cancellation,
                )
                .err(),
                Some(crate::VerifiedGenerationQualificationError::NotQualified)
            );
        }
    } else {
        assert!(result.is_err(), "{failure}");
        if failure == "closeout_initial_finalization" {
            assert_eq!(
                result,
                Err(ActiveGenerationQualificationCloseoutError::MandatoryFinalization)
            );
        }
        if failure == "closeout_postcommit_cancel" {
            assert_eq!(
                result,
                Err(ActiveGenerationQualificationCloseoutError::Cancelled)
            );
        }
        if failure == "closeout_postcommit_finalization" {
            assert_eq!(
                result,
                Err(ActiveGenerationQualificationCloseoutError::PrimaryAndFinalization)
            );
        }
        if failure == "closeout_deadline" {
            assert!(matches!(
                result,
                Err(ActiveGenerationQualificationCloseoutError::DeadlineExceeded)
            ));
        }
    }
    let committed = matches!(
        failure,
        "closeout_ok"
            | "closeout_rejected_review"
            | "closeout_postcommit_cancel"
            | "closeout_postcommit_finalization"
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
        "closeout_foreign" => {
            active.subject =
                crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject::new();
        }
        "closeout_unsettled" => active.next_judge_settlement = 0,
        "closeout_substituted_execution" => active.executed_judge_joins.clear(),
        "closeout_missing_ledger" => active.attempt_ledger = None,
        "closeout_initial_finalization" => {
            control.fail_on_call(control.revalidation_calls() + 1);
        }
        "closeout_wrong_execution" => {
            active.executed_judge_joins[0] = serde_json::from_str(&format!(
                "\"{}\"",
                rewrite_types::Digest::sha256(b"wrong consumed qualification closeout join")
            ))
            .expect("foreign inert join identity");
        }
        "closeout_missing_resource" => {
            connection
                .execute("DELETE FROM generation_resource_evidence_manifests", [])
                .expect("remove parent");
        }
        "closeout_corrupt_parent" => {
            connection
                .execute(
                    "UPDATE candidate_judge_response_aggregates SET canonical_json = ?1",
                    [b"{}".as_slice()],
                )
                .expect("corrupt canonical parent");
        }
        "closeout_abort" => connection
            .execute_batch(
                "CREATE TRIGGER abort_closeout BEFORE INSERT ON generation_qualification_records BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;",
            )
            .expect("abort closeout"),
        "closeout_tamper" => connection
            .execute_batch(
                "CREATE TRIGGER tamper_closeout AFTER INSERT ON generation_qualification_records BEGIN UPDATE generation_human_adjudication_evidence_manifests SET canonical_json = X'7b7d'; END;",
            )
            .expect("tamper closeout"),
        "closeout_cancel" => cancellation.cancel(),
        "closeout_deadline" => {
            active.prepared.expire_deadline_for_test();
            cancellation.cancel();
        }
        _ => {}
    }
    if matches!(
        failure,
        "closeout_postcommit_cancel" | "closeout_postcommit_finalization"
    ) {
        let database = database.to_owned();
        let token = cancellation.clone();
        let cancel = failure == "closeout_postcommit_cancel";
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

#[test]
fn terminal_gate_has_deadline_and_cancellation_priority_with_finalization_failures() {
    use ActiveGenerationQualificationCloseoutError as Error;
    for primary in [Ok(7), Err(Error::OperationScope)] {
        assert_eq!(
            finish_closeout(primary, true, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineAndFinalization)
        );
        assert_eq!(
            finish_closeout(primary, true, Err(Error::Cancelled)),
            Err(Error::CancelledAndFinalization)
        );
        assert_eq!(
            finish_closeout(primary, false, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineExceeded)
        );
        assert_eq!(
            finish_closeout(primary, false, Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
    }
    assert_eq!(
        finish_closeout(Ok(7), true, Ok(())),
        Err(Error::MandatoryFinalization)
    );
    assert_eq!(
        finish_closeout::<u8>(Err(Error::Publication), true, Ok(())),
        Err(Error::PrimaryAndFinalization)
    );
    assert_eq!(finish_closeout(Ok(7), false, Ok(())), Ok(7));
    assert_eq!(
        map_closeout_preparation_error(GenerationQualificationPreparationError::Cancelled),
        Error::Cancelled
    );
    assert_eq!(
        map_closeout_preparation_error(GenerationQualificationPreparationError::DeadlineExceeded),
        Error::DeadlineExceeded
    );
    assert!(!format!("{:?} {}", Error::Publication, Error::Publication).contains("Retain Acme"));
}
