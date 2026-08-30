use rewrite_model::{ActivationDecision, ActivationId};
use rusqlite::{Transaction, params};

use super::role_key;
use crate::record::encode_record;
use crate::{StoreError, StoreResult};

pub(super) fn require_absent_activation(
    transaction: &Transaction<'_>,
    activation_id: &ActivationId,
) -> StoreResult<()> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM activation_decisions WHERE activation_id = ?1
         )",
        [activation_id.digest().as_str()],
        |row| row.get(0),
    )?;
    if exists {
        Err(StoreError::ImmutableConflict)
    } else {
        Ok(())
    }
}

pub(super) fn insert_decision(
    transaction: &Transaction<'_>,
    decision: &ActivationDecision,
) -> StoreResult<()> {
    let encoded = encode_record(decision)?;
    transaction.execute(
        "INSERT INTO activation_decisions (activation_id, role, record_json)
         VALUES (?1, ?2, ?3)",
        params![
            decision.activation_id.digest().as_str(),
            role_key(decision.role),
            encoded
        ],
    )?;
    Ok(())
}
