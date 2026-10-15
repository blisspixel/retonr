use super::{
    Connection, PreparedCohort, StoreError, StoreResult, WriteDisposition, insert_human,
    insert_license, insert_platform, insert_receipt, require_uniform,
};
use crate::GenerationHumanAdjudicationPolicyDenialV1Input;

pub(in crate::store::generation_qualification_terminal_evidence) fn insert_closeout_suffix(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
    record: &rewrite_model::GenerationQualificationRecordV1,
    human_policy_denial: Option<&GenerationHumanAdjudicationPolicyDenialV1Input<'_>>,
) -> StoreResult<WriteDisposition> {
    let elapsed = i64::try_from(prepared.records.receipt.elapsed_nanoseconds())
        .map_err(|_| StoreError::CorruptRecord)?;
    let mut written = Vec::new();
    if let Some(denial) = human_policy_denial {
        let disposition = crate::store::phase_policy_denial::write_human_denial_on_connection(
            connection,
            denial.record,
            denial.relations,
        )?;
        written.push(disposition);
    }
    written.push(insert_platform(connection, prepared)?);
    written.push(insert_license(connection, prepared)?);
    written.push(insert_human(connection, prepared)?);
    written.push(insert_receipt(connection, prepared, elapsed)?);
    written.push(
        crate::store::generation_qualification_record::write_on_connection(connection, record)?,
    );
    require_uniform(&written)
}
