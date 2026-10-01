//! Real complete phase transactions over the synthetic fixture's durable parents.

use super::*;
use rewrite_model_store::{
    ArtifactStateStore, GenerationAttemptLedgerV1Input, GenerationRepeatabilityPhaseV1Input,
    GenerationRepeatabilityPhaseV1TransactionError,
};

pub(super) fn exercise(
    active: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    complete: &mut VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>,
    database: &std::path::Path,
    cancellation: &CancellationToken,
) {
    complete.with_settlement_joins(cancellation, |closure, manifest, joins| {
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
        assert_eq!(store.generation_repeatability_phase_v1(input).expect("read-only phase absence"), None);
        let missing_parents = GenerationRepeatabilityPhaseV1Input { expected_parents: &[], ..input };
        assert!(store.transact_generation_repeatability_phase_v1(missing_parents, || Ok::<_, ()>(())).is_err());
        let incomplete = GenerationRepeatabilityPhaseV1Input { ordered_results: &[], ..input };
        assert!(store.transact_generation_repeatability_phase_v1(incomplete, || Ok::<_, ()>(())).is_err());
        let mut gates = 0;
        assert!(matches!(store.transact_generation_repeatability_phase_v1(input, || {
            gates += 1;
            if gates == 3 { Err(()) } else { Ok(()) }
        }), Err(GenerationRepeatabilityPhaseV1TransactionError::Gate(()))));
        assert_eq!(gates, 3);
        assert_eq!(store.generation_repeatability_phase_v1(input).expect("phase rolled back"), None);
        assert_eq!(store.transact_generation_repeatability_phase_v1(input, || Ok::<_, ()>(())).expect("complete phase transaction"), WriteDisposition::Inserted);
        assert_eq!(store.generation_repeatability_phase_v1(input).expect("canonical phase readback"), Some(manifest.clone()));
        assert_eq!(store.transact_generation_repeatability_phase_v1(input, || Ok::<_, ()>(())).expect("exact immutable phase replay"), WriteDisposition::AlreadyPresent);
        let connection = rusqlite::Connection::open(database).expect("fixture state");
        corruption::exercise(&mut store, input, &connection);
        connection.execute("UPDATE generation_repeatability_terminal_result_records SET canonical_json = ?1", [b"{}".as_slice()]).expect("corrupt exact durable parent");
        assert!(store.generation_repeatability_phase_v1(input).is_err());
        assert!(store.transact_generation_repeatability_phase_v1(input, || Ok::<_, ()>(())).is_err());
        assert_eq!(super::row_count(&connection, "generation_repeatability_evidence_manifests"), 1);
        Ok::<_, ()>(())
    }).expect("complete private authority projection").expect("store fixture checks");
}

mod corruption;
