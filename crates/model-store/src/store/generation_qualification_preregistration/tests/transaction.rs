use std::cell::Cell;

use rusqlite::params;
use tempfile::tempdir;

use super::{input, support};
use crate::{
    ArtifactStateStore, GenerationQualificationPreregistrationTransactionError, StoreError,
};

#[test]
fn rejections_preserve_a_preexisting_policy_and_rollback_the_projection() {
    for reject_callback in [true, false] {
        let directory = tempdir().expect("temporary directory");
        let path = directory
            .path()
            .join(format!("preexisting-{reject_callback}.db"));
        let fixture = support::fixture();
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        support::persist_plan_foundation(&mut store, &fixture);
        let policy_json = serde_json::to_vec(&fixture.policy).expect("policy JSON");
        store
            .connection()
            .execute(
                "INSERT INTO generation_qualification_operation_policies VALUES
                     (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    fixture.policy.operation_policy_id().digest().as_str(),
                    fixture
                        .policy
                        .generation_qualification_plan_id()
                        .digest()
                        .as_str(),
                    fixture.policy.suite_manifest_id().digest().as_str(),
                    fixture
                        .policy
                        .target_generation_system_id()
                        .digest()
                        .as_str(),
                    fixture
                        .policy
                        .baseline_generation_system_id()
                        .digest()
                        .as_str(),
                    &policy_json,
                ],
            )
            .expect("seed exact policy");
        let gates = Cell::new(0);
        let result = store.transact_generation_qualification_preregistration(
            input(&fixture),
            || {
                gates.set(gates.get() + 1);
                if !reject_callback && gates.get() == 3 {
                    Err("final gate")
                } else {
                    Ok(())
                }
            },
            |_| {
                if reject_callback {
                    Err("callback")
                } else {
                    Ok(())
                }
            },
        );
        assert!(matches!(
            result,
            Err(
                GenerationQualificationPreregistrationTransactionError::Validation("callback")
                    | GenerationQualificationPreregistrationTransactionError::Gate("final gate")
            )
        ));
        let (stored_policy, projection_count): (Vec<u8>, i64) = store
            .connection()
            .query_row(
                "SELECT p.canonical_json,
                        (SELECT COUNT(*) FROM generation_qualification_request_projections)
                 FROM generation_qualification_operation_policies p
                 WHERE p.operation_policy_id = ?1",
                [fixture.policy.operation_policy_id().digest().as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(StoreError::from)
            .expect("read preserved policy");
        assert_eq!(stored_policy, policy_json);
        assert_eq!(projection_count, 0);
    }
}
