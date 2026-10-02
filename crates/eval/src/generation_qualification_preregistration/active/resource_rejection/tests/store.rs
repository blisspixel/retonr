//! Real immediate transactions, full parent corruption, replay and immutable reads.

use super::*;
use rewrite_model_store::{
    ArtifactStateStore, GenerationQualificationTerminalEvidenceV1TransactionError,
};

pub(super) fn exercise(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    phase: &mut VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
    repository: &mut GenerationQualificationPreregistrationRepository,
    database: &std::path::Path,
    cancellation: &CancellationToken,
) {
    let ledger = active.attempt_ledger.as_ref().expect("retained ledger");
    let foundation = active.prepared.plan_foundation().clone();
    let context = JudgeSettlementContext {
        ledger,
        subject: &active.subject,
        subject_matches: ledger.matches_active_subject(&active.subject),
        selection_policy: foundation.candidate_selection_policy(),
        candidate_count: active.next_candidate_attempt,
        executed_count: active.next_judge_repetition,
        executed_joins: &active.executed_judge_joins,
        settled_count: active.next_judge_settlement,
        suite: foundation.suite(),
        cases: foundation.cases(),
        contracts: foundation.deterministic_case_contracts(),
    };
    let peak = u32::from(active.peak_live_authorities());
    active.prepared.with_validated_view(cancellation, |prepared| {
        phase.with_settlement_joins(cancellation, |closure, repeatability_manifest, results, manifest, joins| {
            publication::with_input(&context, &prepared, closure, publication::ResourceClosure {repeatability_manifest, resource_results: results, manifest}, joins, repository, cancellation, |_, input| {
                evidence::with_evidence(&context, &prepared, input, peak, |input| {
                    let mut store = ArtifactStateStore::open(database).expect("independent store");
                    let connection = rusqlite::Connection::open(database).expect("test-only SQL corruption handle");
                    let changes = connection.total_changes();
                    assert_eq!(store.generation_resource_rejection_v1(input).expect("read-only absent closeout"), None);
                    assert_eq!(connection.total_changes(), changes);
                    let mut gates = 0;
                    assert!(matches!(store.transact_generation_resource_rejection_v1(input, || {gates += 1; if gates == 3 {Err(())} else {Ok(())}}), Err(GenerationQualificationTerminalEvidenceV1TransactionError::Gate(()))));
                    assert_eq!(gates, 3);
                    assert_eq!(store.generation_resource_rejection_v1(input).expect("complete rollback"), None);
                    assert_eq!(store.transact_generation_resource_rejection_v1(input, || Ok::<_, ()>(())).expect("atomic negative terminal cohort"), WriteDisposition::Inserted);
                    let mut cold = ArtifactStateStore::open(database).expect("cold reopened store");
                    assert_eq!(cold.generation_resource_rejection_v1(input).expect("exact cold closure"), Some(input.record.clone()));
                    assert_eq!(cold.transact_generation_resource_rejection_v1(input, || Ok::<_, ()>(())).expect("same frozen terminal facts replay"), WriteDisposition::AlreadyPresent);
                    for table in ["generation_qualification_operation_receipts", "generation_qualification_records", "generation_human_adjudication_evidence_manifests", "generation_qualification_platform_evidence", "generation_qualification_license_evidence", "generation_resource_attempt_result_records", "generation_resource_evidence_manifests", "generation_repeatability_evidence_manifests", "generation_attempt_ledger_manifests", "candidate_generation_attempt_records", "generation_repeatability_terminal_result_records", "candidate_judge_response_aggregates", "candidate_deterministic_evaluation_records", "candidate_generation_receipt_sets"] {
                        let (row_id, bytes): (i64, Vec<u8>) = connection.query_row(&format!("SELECT rowid, canonical_json FROM {table} ORDER BY rowid LIMIT 1"), [], |row| Ok((row.get(0)?, row.get(1)?))).expect("original parent bytes");
                        connection.execute(&format!("UPDATE {table} SET canonical_json = ?1 WHERE rowid = ?2"), rusqlite::params![b"{}".as_slice(), row_id]).expect("corrupt real canonical row");
                        assert!(cold.generation_resource_rejection_v1(input).is_err(), "{table}");
                        assert!(cold.transact_generation_resource_rejection_v1(input, || Ok::<_, ()>(())).is_err(), "{table}");
                        connection.execute(&format!("UPDATE {table} SET canonical_json = ?1 WHERE rowid = ?2"), rusqlite::params![&bytes, row_id]).expect("restore exact bytes");
                        assert_eq!(cold.generation_resource_rejection_v1(input).expect("restored full closure"), Some(input.record.clone()), "{table}");
                    }
                    connection.execute("UPDATE generation_qualification_records SET status = 'qualified'", []).expect("indexed status substitution");
                    assert!(cold.generation_resource_rejection_v1(input).is_err());
                    connection.execute("UPDATE generation_qualification_records SET status = 'rejected'", []).expect("restore derived status");
                    connection.execute("UPDATE generation_human_adjudication_evidence_manifests SET status = 'passed', evidence_item_count = 1", []).expect("indexed human substitution");
                    assert!(cold.generation_resource_rejection_v1(input).is_err());
                    connection.execute("UPDATE generation_human_adjudication_evidence_manifests SET status = 'skipped', evidence_item_count = 0", []).expect("restore skipped human");
                    let original: Vec<u8> = connection.query_row("SELECT canonical_json FROM generation_qualification_records", [], |row| row.get(0)).expect("record bytes");
                    assert!(connection.execute("UPDATE generation_qualification_records SET canonical_json = ?1", [vec![b'x'; 20_000]]).is_err(), "real schema refuses oversized canonical records");
                    assert_eq!(cold.generation_resource_rejection_v1(input).expect("oversize mutation refused without damaging existing record"), Some(input.record.clone()));
                    assert!(connection.execute("UPDATE generation_qualification_records SET canonical_json = ?1", ["\u{e9}".repeat(12_000)]).is_err(), "STRICT schema refuses malformed multibyte TEXT");
                    assert_eq!(cold.generation_resource_rejection_v1(input).expect("TEXT mutation refused without damaging existing record"), Some(input.record.clone()));
                    connection.execute("UPDATE generation_qualification_records SET canonical_json = ?1", [original]).expect("restore bounded record");
                    connection.execute("DELETE FROM generation_qualification_records", []).expect("partial suffix");
                    assert!(cold.generation_resource_rejection_v1(input).is_err());
                    assert!(cold.transact_generation_resource_rejection_v1(input, || Ok::<_, ()>(())).is_err(), "partial suffix cannot repair itself");
                    connection.execute_batch("DELETE FROM generation_qualification_operation_receipts; DELETE FROM generation_human_adjudication_evidence_manifests; DELETE FROM generation_qualification_license_evidence; DELETE FROM generation_qualification_platform_evidence;").expect("reset only synthetic terminal suffix");
                    assert_eq!(cold.generation_resource_rejection_v1(input).expect("absent suffix remains inert"), None);
                    Ok(())
                })
            })
        }).expect("exact retained resource projection")
    }).expect("Prepared closure and real store regressions");
}
