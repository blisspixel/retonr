//! Real complete phase transactions over the synthetic fixture's durable parents.

use super::*;
use rewrite_model_store::{
    ArtifactStateStore, GenerationAttemptLedgerV1Input, GenerationRepeatabilityPhaseV1Input,
};
use rewrite_model_store::{
    GenerationResourcePhaseV1Input, GenerationResourcePhaseV1TransactionError,
};

pub(super) fn exercise(
    active: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    phase: &mut VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
    database: &std::path::Path,
    cancellation: &CancellationToken,
) {
    phase.with_settlement_joins(cancellation, |closure, manifest, resource_results, resource_manifest, joins| {
        let mut repository_for_fixture = GenerationQualificationPreregistrationRepository::open(database).expect("fixture parent repository");
        let mut store = ArtifactStateStore::open(database).expect("cold phase store");
        let mut managed = Vec::new();
        let mut expected_parents = Vec::new();
        for join in joins {
            join.with_settlement_view(cancellation, |view| {
                let (judge_execution, _) = repository_for_fixture.persist_judge_execution(view.input, active.prepared.operation_deadline(), cancellation).expect("exact live judge parent readback");
                expected_parents.push(rewrite_model_store::GenerationRepeatabilityPhaseParentV1 {
                    target_receipt_set: view.input.candidate_a_receipt_set.clone(),
                    baseline_receipt_set: view.input.candidate_b_receipt_set.clone(),
                    deterministic_evaluation: view.input.deterministic_evaluation.clone(), judge_execution,
                });
                managed.extend(view.target.managed_evidence_inputs(cancellation).expect("retained managed facts"));
                Ok::<_, ()>(())
            }).expect("retained projection").expect("projection callback");
        }
        let ordered = closure.planned_attempts.iter().filter(|planned| planned.generation_system_id() == manifest.generation_system_id()).map(|planned| {
            let index = managed.iter().position(|(id, _)| id == planned.planned_attempt_id()).expect("exact target order");
            managed.remove(index).1
        }).collect::<Vec<_>>();
        assert!(managed.is_empty());
        let input = GenerationRepeatabilityPhaseV1Input {
            ledger: &GenerationAttemptLedgerV1Input {
                preregistration: active.prepared.preregistration_read_input(),
                managed_evidence_inputs: &ordered,
                manifest: closure.attempt_ledger_manifest,
            },
            ordered_results: closure.ordered_results,
            expected_parents: &expected_parents,
            manifest,
        };
        let resource_input = GenerationResourcePhaseV1Input { repeatability: &input, ordered_results: resource_results, manifest: resource_manifest };
        assert_eq!(store.generation_resource_phase_v1(resource_input).expect("full readback").expect("durable phase").ordered_results, resource_results);
        let missing = GenerationResourcePhaseV1Input { ordered_results: &[], ..resource_input };
        assert!(store.transact_generation_resource_phase_v1(missing, || Ok::<_, ()>(())).is_err());
        let duplicate = vec![resource_results[0].clone(), resource_results[0].clone()];
        assert!(store.transact_generation_resource_phase_v1(GenerationResourcePhaseV1Input { ordered_results: &duplicate, ..resource_input }, || Ok::<_, ()>(())).is_err());
        let digests = resource_results.iter().map(|record| record.resource_attempt_result_id().digest().clone()).collect::<Vec<_>>();
        let opposite = if resource_manifest.status() == GenerationQualificationPhaseStatusV1::Passed { GenerationQualificationPhaseStatusV1::Failed } else { GenerationQualificationPhaseStatusV1::Passed };
        for status in [opposite, GenerationQualificationPhaseStatusV1::Skipped] {
            let manifest = rewrite_model::GenerationResourceEvidenceManifestV1::new(rewrite_model::GenerationResourceEvidenceManifestV1Relations { scope: closure.scope, phase_policy_digest: resource_manifest.phase_policy_digest(), evidence_record_digests: if status == GenerationQualificationPhaseStatusV1::Skipped { &[] } else { &digests }, status }).expect("inert caller status record");
            let substituted = GenerationResourcePhaseV1Input { manifest: &manifest, ..resource_input };
            assert!(store.generation_resource_phase_v1(substituted).is_err());
            assert!(store.transact_generation_resource_phase_v1(substituted, || Ok::<_, ()>(())).is_err());
        }
        let missing_parent = GenerationRepeatabilityPhaseV1Input { expected_parents: &[], ..input };
        assert!(store.transact_generation_resource_phase_v1(GenerationResourcePhaseV1Input { repeatability: &missing_parent, ..resource_input }, || Ok::<_, ()>(())).is_err());
        let connection = rusqlite::Connection::open(database).expect("fixture database");
        for (table, key) in [
            ("generation_resource_attempt_result_records", "generation_resource_attempt_result_id"),
            ("generation_resource_evidence_manifests", "generation_resource_evidence_manifest_id"),
            ("generation_repeatability_evidence_manifests", "generation_repeatability_evidence_manifest_id"),
            ("candidate_judge_response_aggregates", "candidate_judge_response_aggregate_id"),
            ("candidate_deterministic_evaluation_records", "candidate_deterministic_evaluation_id"),
            ("candidate_generation_receipt_sets", "candidate_generation_receipt_set_id"),
        ] {
            let mut statement = connection.prepare(&format!("SELECT {key}, canonical_json FROM {table}")).expect("parent snapshot query");
            let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))).expect("parent snapshots").collect::<Result<Vec<_>, _>>().expect("exact parent bytes");
            drop(statement);
            for (id, bytes) in rows {
                connection.execute(&format!("UPDATE {table} SET canonical_json = X'7b7d' WHERE {key} = ?1"), [&id]).expect("corrupt parent");
                assert!(store.generation_resource_phase_v1(resource_input).is_err(), "{table}");
                assert!(store.transact_generation_resource_phase_v1(resource_input, || Ok::<_, ()>(())).is_err(), "{table}");
                connection.execute(&format!("UPDATE {table} SET canonical_json = ?1 WHERE {key} = ?2"), rusqlite::params![bytes, id]).expect("restore exact bytes");
            }
        }
        connection.execute("UPDATE generation_resource_attempt_result_records SET schema_version = 1, resource_policy_digest = ?1", [rewrite_types::Digest::sha256(b"wrong resource policy").as_str()]).expect("indexed substitution");
        assert!(store.generation_resource_phase_v1(resource_input).is_err());
        connection.execute("UPDATE generation_resource_attempt_result_records SET resource_policy_digest = ?1", [resource_manifest.phase_policy_digest().as_str()]).expect("restore scalar");
        connection.execute_batch("DELETE FROM generation_resource_evidence_manifests; DELETE FROM generation_resource_attempt_result_records;").expect("reset only resource cohort");
        assert_eq!(store.generation_resource_phase_v1(resource_input).expect("immutable absent read"), None);
        let mut gates = 0;
        assert!(matches!(store.transact_generation_resource_phase_v1(resource_input, || { gates += 1; if gates == 3 { Err(()) } else { Ok(()) } }), Err(GenerationResourcePhaseV1TransactionError::Gate(()))));
        assert_eq!(gates, 3);
        for table in ["generation_resource_evidence_manifests", "generation_resource_attempt_result_records"] {
            assert_eq!(connection.query_row(&format!("SELECT count(*) FROM {table}"), [], |r|r.get::<_, i64>(0)).expect("rolled back rows"), 0);
        }
        // Compatibility with exact legacy per-attempt rows: complete atomically
        // without rewriting that existing inert observation.
        store.transact_generation_resource_attempt_result_v1(rewrite_model_store::GenerationResourceAttemptResultV1Input { record: &resource_results[0] }, || Ok::<_, ()>(())).expect("legacy exact result");
        let disposition = store.transact_generation_resource_phase_v1(resource_input, || Ok::<_, ()>(())).expect("complete existing result cohort");
        assert_eq!(disposition.ordered_results, vec![WriteDisposition::AlreadyPresent]);
        assert_eq!(disposition.manifest, WriteDisposition::Inserted);
        assert_eq!(store.generation_resource_phase_v1(resource_input).expect("canonical cold phase").expect("stored phase").manifest, *resource_manifest);
        Ok::<_, ()>(())
    }).expect("private live resource projection").expect("resource store checks");
}
