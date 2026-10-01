//! Current full parent bytes and indexed scalars must match the retained snapshot.

use super::*;

pub(super) fn exercise(
    store: &mut ArtifactStateStore,
    input: GenerationRepeatabilityPhaseV1Input<'_>,
    connection: &rusqlite::Connection,
) {
    let parent = &input.expected_parents[0];
    let judge = &parent.judge_execution;
    let records = [
        ("candidate_judge_plans", serde_json::to_vec(judge.plan())),
        (
            "candidate_judge_schedules",
            serde_json::to_vec(judge.schedule()),
        ),
        (
            "candidate_judge_request_aggregates",
            serde_json::to_vec(judge.request_aggregate()),
        ),
        (
            "candidate_judge_response_aggregates",
            serde_json::to_vec(judge.response_aggregate()),
        ),
        (
            "candidate_judge_observation_batches",
            serde_json::to_vec(judge.observation_batch()),
        ),
        (
            "managed_local_judge_receipts",
            serde_json::to_vec(judge.managed_receipt()),
        ),
        (
            "candidate_judge_join_records",
            serde_json::to_vec(judge.join()),
        ),
        (
            "candidate_deterministic_evaluation_records",
            serde_json::to_vec(&parent.deterministic_evaluation),
        ),
    ];
    for (table, original) in records {
        let sql = format!("UPDATE {table} SET canonical_json = ?1");
        connection
            .execute(&sql, [b"{}".as_slice()])
            .expect("corrupt exact typed parent");
        assert!(
            store.generation_repeatability_phase_v1(input).is_err(),
            "{table}"
        );
        assert!(
            store
                .transact_generation_repeatability_phase_v1(input, || Ok::<_, ()>(()))
                .is_err(),
            "{table}"
        );
        connection
            .execute(&sql, [original.expect("canonical retained parent")])
            .expect("restore exact fixture bytes");
    }
    for receipt in [&parent.target_receipt_set, &parent.baseline_receipt_set] {
        let sql = "UPDATE candidate_generation_receipt_sets SET canonical_json = ?1 WHERE candidate_generation_receipt_set_id = ?2";
        connection
            .execute(
                sql,
                rusqlite::params![b"{}".as_slice(), receipt.receipt_set_id().digest().as_str()],
            )
            .expect("corrupt receipt parent");
        assert!(store.generation_repeatability_phase_v1(input).is_err());
        assert!(
            store
                .transact_generation_repeatability_phase_v1(input, || Ok::<_, ()>(()))
                .is_err()
        );
        connection
            .execute(
                sql,
                rusqlite::params![
                    serde_json::to_vec(receipt).expect("canonical receipt"),
                    receipt.receipt_set_id().digest().as_str()
                ],
            )
            .expect("restore exact receipt");
    }
    connection
        .execute(
            "UPDATE candidate_judge_plans SET maximum_source_bytes = maximum_source_bytes + 1",
            [],
        )
        .expect("corrupt valid indexed scalar");
    assert!(store.generation_repeatability_phase_v1(input).is_err());
    assert!(
        store
            .transact_generation_repeatability_phase_v1(input, || Ok::<_, ()>(()))
            .is_err()
    );
    connection
        .execute(
            "UPDATE candidate_judge_plans SET maximum_source_bytes = maximum_source_bytes - 1",
            [],
        )
        .expect("restore indexed scalar");
    assert_eq!(
        store
            .generation_repeatability_phase_v1(input)
            .expect("all parents restored"),
        Some(input.manifest.clone())
    );
}
