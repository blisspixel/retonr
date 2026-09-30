use rewrite_model::{GenerationQualificationRecordV1, GenerationQualificationStatusV1};
use rusqlite::{Connection, OptionalExtension as _};

use crate::{StoreError, StoreResult};

pub(super) struct Cited<'a> {
    pub(super) id: &'a str,
    pub(super) target: &'a str,
    pub(super) baseline: &'a str,
    pub(super) policy: &'a str,
    pub(super) projection: &'a str,
    pub(super) platform: &'a str,
    pub(super) license: &'a str,
    pub(super) receipt: &'a str,
    pub(super) status: &'a str,
}

impl<'a> Cited<'a> {
    pub(super) fn from_record(record: &'a GenerationQualificationRecordV1) -> StoreResult<Self> {
        let target = record.target_generation_system_id().digest().as_str();
        let baseline = record.baseline_generation_system_id().digest().as_str();
        if target == baseline {
            return Err(StoreError::CorruptRecord);
        }
        Ok(Self {
            id: record.generation_qualification_id().digest().as_str(),
            target,
            baseline,
            policy: record.operation_policy_id().digest().as_str(),
            projection: record.request_projection_id().digest().as_str(),
            platform: record.platform_evidence_id().digest().as_str(),
            license: record.license_evidence_id().digest().as_str(),
            receipt: record.operation_receipt_id().digest().as_str(),
            status: status_label(record.status()),
        })
    }
}

pub(super) fn require(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    require_system(connection, cited.target)?;
    require_system(connection, cited.baseline)?;
    let policy = require_policy(connection, cited)?;
    require_projection(connection, cited, &policy)?;
    require_evidence(
        connection,
        "generation_qualification_platform_evidence",
        cited.platform,
        cited,
    )?;
    require_evidence(
        connection,
        "generation_qualification_license_evidence",
        cited.license,
        cited,
    )?;
    require_receipt(connection, cited, &policy)
}

fn status_label(status: GenerationQualificationStatusV1) -> &'static str {
    match status {
        GenerationQualificationStatusV1::Qualified => "qualified",
        GenerationQualificationStatusV1::Rejected => "rejected",
    }
}

fn require_system(connection: &Connection, id: &str) -> StoreResult<()> {
    let present: i64 = connection.query_row(
        "SELECT count(*) FROM generation_system_records WHERE generation_system_id = ?1",
        [id],
        |row| row.get(0),
    )?;
    if present == 1 {
        Ok(())
    } else {
        Err(StoreError::MissingRecord)
    }
}

struct PolicyRow {
    plan: String,
    suite: String,
    target: String,
    baseline: String,
}

fn require_policy(connection: &Connection, cited: &Cited<'_>) -> StoreResult<PolicyRow> {
    let Some(row) = policy_row(connection, cited.policy)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.target == cited.target && row.baseline == cited.baseline {
        Ok(row)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_projection(
    connection: &Connection,
    cited: &Cited<'_>,
    policy: &PolicyRow,
) -> StoreResult<()> {
    let Some(row) = projection_row(connection, cited.projection)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.policy == cited.policy
        && row.plan == policy.plan
        && row.suite == policy.suite
        && row.target == cited.target
        && row.baseline == cited.baseline
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_evidence(
    connection: &Connection,
    table: &str,
    id: &str,
    cited: &Cited<'_>,
) -> StoreResult<()> {
    let Some(row) = evidence_row(connection, table, id)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.policy == cited.policy
        && row.projection == cited.projection
        && row.target == cited.target
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_receipt(
    connection: &Connection,
    cited: &Cited<'_>,
    policy: &PolicyRow,
) -> StoreResult<()> {
    let Some(row) = receipt_row(connection, cited.receipt)? else {
        return Err(StoreError::MissingRecord);
    };
    if row.policy == cited.policy
        && row.projection == cited.projection
        && row.plan == policy.plan
        && row.suite == policy.suite
        && row.target == cited.target
        && row.baseline == cited.baseline
        && row.platform == cited.platform
        && row.license == cited.license
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

struct ProjectionRow {
    policy: String,
    plan: String,
    suite: String,
    target: String,
    baseline: String,
}

struct EvidenceRow {
    policy: String,
    projection: String,
    target: String,
}

struct ReceiptRow {
    policy: String,
    projection: String,
    plan: String,
    suite: String,
    target: String,
    baseline: String,
    platform: String,
    license: String,
}

fn policy_row(connection: &Connection, id: &str) -> StoreResult<Option<PolicyRow>> {
    connection
        .query_row(
            "SELECT generation_qualification_plan_id, suite_manifest_id,
                target_generation_system_id, baseline_generation_system_id
             FROM generation_qualification_operation_policies
             WHERE operation_policy_id = ?1",
            [id],
            |row| {
                Ok(PolicyRow {
                    plan: row.get(0)?,
                    suite: row.get(1)?,
                    target: row.get(2)?,
                    baseline: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn projection_row(connection: &Connection, id: &str) -> StoreResult<Option<ProjectionRow>> {
    connection
        .query_row(
            "SELECT operation_policy_id, generation_qualification_plan_id, suite_manifest_id,
                target_generation_system_id, baseline_generation_system_id
             FROM generation_qualification_request_projections
             WHERE request_projection_id = ?1",
            [id],
            |row| {
                Ok(ProjectionRow {
                    policy: row.get(0)?,
                    plan: row.get(1)?,
                    suite: row.get(2)?,
                    target: row.get(3)?,
                    baseline: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}

fn evidence_row(
    connection: &Connection,
    table: &str,
    id: &str,
) -> StoreResult<Option<EvidenceRow>> {
    let id_column = match table {
        "generation_qualification_platform_evidence" => {
            "generation_qualification_platform_evidence_id"
        }
        "generation_qualification_license_evidence" => {
            "generation_qualification_license_evidence_id"
        }
        _ => return Err(StoreError::CorruptRecord),
    };
    let sql = format!(
        "SELECT operation_policy_id, request_projection_id, target_generation_system_id
         FROM {table} WHERE {id_column} = ?1"
    );
    connection
        .query_row(&sql, [id], |row| {
            Ok(EvidenceRow {
                policy: row.get(0)?,
                projection: row.get(1)?,
                target: row.get(2)?,
            })
        })
        .optional()
        .map_err(StoreError::from)
}

fn receipt_row(connection: &Connection, id: &str) -> StoreResult<Option<ReceiptRow>> {
    connection
        .query_row(
            "SELECT operation_policy_id, request_projection_id, generation_qualification_plan_id,
                generation_suite_manifest_id, target_generation_system_id,
                baseline_generation_system_id, generation_qualification_platform_evidence_id,
                generation_qualification_license_evidence_id
             FROM generation_qualification_operation_receipts
             WHERE generation_qualification_operation_receipt_id = ?1",
            [id],
            |row| {
                Ok(ReceiptRow {
                    policy: row.get(0)?,
                    projection: row.get(1)?,
                    plan: row.get(2)?,
                    suite: row.get(3)?,
                    target: row.get(4)?,
                    baseline: row.get(5)?,
                    platform: row.get(6)?,
                    license: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
}
