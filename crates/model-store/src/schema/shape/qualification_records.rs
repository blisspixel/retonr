use rusqlite::Connection;

use super::{
    SchemaObject, create_schema_one, create_schema_two, migrate_schema_eight,
    migrate_schema_eleven, migrate_schema_fifteen, migrate_schema_five, migrate_schema_four,
    migrate_schema_fourteen, migrate_schema_nine, migrate_schema_one, migrate_schema_seven,
    migrate_schema_six, migrate_schema_ten, migrate_schema_thirteen, migrate_schema_three,
    migrate_schema_twelve, migrate_schema_two, schema_objects,
};
use crate::{StoreError, StoreResult};

pub(crate) fn validate_schema_sixteen(connection: &Connection) -> StoreResult<()> {
    let actual = schema_objects(connection)?;
    if actual == canonical_schema_sixteen_objects()?
        || actual == canonical_migrated_schema_sixteen_objects()?
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn canonical_schema_sixteen_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_two(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    migrate_schema_four(&connection)?;
    migrate_schema_five(&connection)?;
    migrate_schema_six(&connection)?;
    migrate_schema_seven(&connection)?;
    migrate_schema_eight(&connection)?;
    migrate_schema_nine(&connection)?;
    migrate_schema_ten(&connection)?;
    migrate_schema_eleven(&connection)?;
    migrate_schema_twelve(&connection)?;
    migrate_schema_thirteen(&connection)?;
    migrate_schema_fourteen(&connection)?;
    migrate_schema_fifteen(&connection)?;
    schema_objects(&connection)
}

fn canonical_migrated_schema_sixteen_objects() -> StoreResult<Vec<SchemaObject>> {
    let connection = Connection::open_in_memory()?;
    create_schema_one(&connection)?;
    migrate_schema_one(&connection)?;
    migrate_schema_two(&connection)?;
    migrate_schema_three(&connection)?;
    migrate_schema_four(&connection)?;
    migrate_schema_five(&connection)?;
    migrate_schema_six(&connection)?;
    migrate_schema_seven(&connection)?;
    migrate_schema_eight(&connection)?;
    migrate_schema_nine(&connection)?;
    migrate_schema_ten(&connection)?;
    migrate_schema_eleven(&connection)?;
    migrate_schema_twelve(&connection)?;
    migrate_schema_thirteen(&connection)?;
    migrate_schema_fourteen(&connection)?;
    migrate_schema_fifteen(&connection)?;
    schema_objects(&connection)
}
