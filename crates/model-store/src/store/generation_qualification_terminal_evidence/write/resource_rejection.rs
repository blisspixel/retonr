use super::{
    Connection, PreparedCohort, StoreError, StoreResult, WriteDisposition, insert_human,
    insert_license, insert_platform, insert_receipt, require_uniform,
};

pub(in crate::store::generation_qualification_terminal_evidence) fn insert_suffix(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
    record: &rewrite_model::GenerationQualificationRecordV1,
) -> StoreResult<WriteDisposition> {
    let elapsed = i64::try_from(prepared.records.receipt.elapsed_nanoseconds())
        .map_err(|_| StoreError::CorruptRecord)?;
    let written = [
        insert_platform(connection, prepared)?,
        insert_license(connection, prepared)?,
        insert_human(connection, prepared)?,
        insert_receipt(connection, prepared, elapsed)?,
        crate::store::generation_qualification_record::write_on_connection(connection, record)?,
    ];
    require_uniform(&written)
}
