use rewrite_model::GenerationQualificationSelectionV1;
use rusqlite::Connection;

use crate::{StoreError, StoreResult};

pub(super) struct Cited<'a> {
    pub(super) id: &'a str,
    pub(super) qualification: &'a str,
}

impl<'a> Cited<'a> {
    pub(super) fn from_record(record: &'a GenerationQualificationSelectionV1) -> Self {
        Self {
            id: record
                .generation_qualification_selection_id()
                .digest()
                .as_str(),
            qualification: record.generation_qualification_id().digest().as_str(),
        }
    }
}

pub(super) fn require(connection: &Connection, cited: &Cited<'_>) -> StoreResult<()> {
    let present: i64 = connection.query_row(
        "SELECT count(*) FROM generation_qualification_records
         WHERE generation_qualification_id = ?1",
        [cited.qualification],
        |row| row.get(0),
    )?;
    if present == 1 {
        Ok(())
    } else {
        Err(StoreError::MissingRecord)
    }
}
