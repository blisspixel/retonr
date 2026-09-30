use rewrite_model::CandidateDeterministicEvaluationRecordV1;
use rusqlite::{Connection, OptionalExtension as _};

use crate::{StoreError, StoreResult};

pub(super) struct Cited<'a> {
    pub(super) id: &'a str,
    pub(super) schema_version: i64,
    pub(super) receipt_a: &'a str,
    pub(super) receipt_b: &'a str,
    pub(super) suite_id: &'a str,
    pub(super) repetition_id: &'a str,
    pub(super) system_a: &'a str,
    pub(super) system_b: &'a str,
    pub(super) total: u32,
}

impl<'a> Cited<'a> {
    pub(super) fn from_record(record: &'a CandidateDeterministicEvaluationRecordV1) -> Self {
        Self {
            id: record.deterministic_evaluation_id().digest().as_str(),
            schema_version: i64::from(record.schema_version()),
            receipt_a: record.candidate_a_receipt_set_id().digest().as_str(),
            receipt_b: record.candidate_b_receipt_set_id().digest().as_str(),
            suite_id: record.suite_manifest_id().digest().as_str(),
            repetition_id: record.repetition_id().digest().as_str(),
            system_a: record.candidate_a_generation_system_id().digest().as_str(),
            system_b: record.candidate_b_generation_system_id().digest().as_str(),
            total: record.total(),
        }
    }
}

pub(super) fn require(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    if cited.receipt_a == cited.receipt_b {
        return Err(StoreError::CorruptRecord);
    }
    let leading = receipt_parent(connection, cited.receipt_a)?;
    let trailing = receipt_parent(connection, cited.receipt_b)?;
    if parents_agree(cited, &leading, &trailing) {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn parents_agree(cited: &Cited<'_>, leading: &ReceiptParent, trailing: &ReceiptParent) -> bool {
    cited.schema_version == leading.schema_version
        && cited.schema_version == trailing.schema_version
        && cited.suite_id == leading.suite_id
        && cited.suite_id == trailing.suite_id
        && cited.repetition_id == leading.repetition_id
        && cited.repetition_id == trailing.repetition_id
        && cited.system_a == leading.system_id
        && cited.system_b == trailing.system_id
        && leading.system_id != trailing.system_id
        && leading.plan_id == trailing.plan_id
        && leading.policy_id == trailing.policy_id
        && entry_counts_match(cited.total, leading.entry_count, trailing.entry_count)
}

fn entry_counts_match(total: u32, leading: i64, trailing: i64) -> bool {
    leading == trailing
        && u32::try_from(leading)
            .ok()
            .and_then(|count| count.checked_mul(2))
            == Some(total)
}

struct ReceiptParent {
    schema_version: i64,
    plan_id: String,
    suite_id: String,
    repetition_id: String,
    system_id: String,
    policy_id: String,
    entry_count: i64,
}

fn receipt_parent(connection: &Connection, id: &str) -> StoreResult<ReceiptParent> {
    connection
        .query_row(
            "SELECT schema_version, generation_qualification_plan_id,
                generation_suite_manifest_id, generation_repetition_id, generation_system_id,
                candidate_selection_policy_id, entry_count
             FROM candidate_generation_receipt_sets
             WHERE candidate_generation_receipt_set_id = ?1",
            [id],
            |row| {
                Ok(ReceiptParent {
                    schema_version: row.get(0)?,
                    plan_id: row.get(1)?,
                    suite_id: row.get(2)?,
                    repetition_id: row.get(3)?,
                    system_id: row.get(4)?,
                    policy_id: row.get(5)?,
                    entry_count: row.get(6)?,
                })
            },
        )
        .optional()?
        .ok_or(StoreError::MissingRecord)
}
