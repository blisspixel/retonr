use rusqlite::Connection;

use crate::{StoreError, StoreResult};

use super::{
    STORE_SCHEMA_VERSION, migrate_schema_eight, migrate_schema_eleven, migrate_schema_five,
    migrate_schema_four, migrate_schema_nine, migrate_schema_one, migrate_schema_seven,
    migrate_schema_six, migrate_schema_ten, migrate_schema_thirteen, migrate_schema_three,
    migrate_schema_twelve, migrate_schema_two, validate_schema_eight, validate_schema_eleven,
    validate_schema_five, validate_schema_four, validate_schema_nine, validate_schema_one,
    validate_schema_seven, validate_schema_shape, validate_schema_six, validate_schema_ten,
    validate_schema_thirteen, validate_schema_three, validate_schema_twelve, validate_schema_two,
};

pub(super) fn migrate_existing_transaction(
    connection: &Connection,
    expected_version: i64,
) -> StoreResult<()> {
    let observed: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if observed != expected_version {
        return Err(StoreError::CorruptRecord);
    }
    crate::integrity::validate_database_integrity(connection)?;
    migrate_supported_schema(connection, observed)?;
    validate_schema_shape(connection)?;
    crate::integrity::validate_database_integrity(connection)
}

#[expect(
    clippy::too_many_lines,
    reason = "explicit validated migration routing"
)]
fn migrate_supported_schema(connection: &Connection, version: i64) -> StoreResult<()> {
    match version {
        0 => {
            return Err(StoreError::MigrationRequired {
                found: version,
                current: STORE_SCHEMA_VERSION,
            });
        }
        1 => {
            validate_schema_one(connection)?;
            migrate_schema_one(connection)?;
            validate_schema_two(connection)?;
            migrate_schema_two(connection)?;
            validate_schema_three(connection)?;
            migrate_schema_three(connection)?;
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
            validate_schema_six(connection)?;
            migrate_schema_six(connection)?;
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        2 => {
            validate_schema_two(connection)?;
            migrate_schema_two(connection)?;
            validate_schema_three(connection)?;
            migrate_schema_three(connection)?;
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
            validate_schema_six(connection)?;
            migrate_schema_six(connection)?;
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        3 => {
            validate_schema_three(connection)?;
            migrate_schema_three(connection)?;
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
            validate_schema_six(connection)?;
            migrate_schema_six(connection)?;
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        4 => {
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
            validate_schema_six(connection)?;
            migrate_schema_six(connection)?;
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        5 => {
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
            validate_schema_six(connection)?;
            migrate_schema_six(connection)?;
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        6 => {
            validate_schema_six(connection)?;
            migrate_schema_six(connection)?;
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        7 => {
            validate_schema_seven(connection)?;
            migrate_schema_seven(connection)?;
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        8 => {
            validate_schema_eight(connection)?;
            migrate_schema_eight(connection)?;
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        9 => {
            validate_schema_nine(connection)?;
            migrate_schema_nine(connection)?;
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        10 => {
            validate_schema_ten(connection)?;
            migrate_schema_ten(connection)?;
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        11 => {
            validate_schema_eleven(connection)?;
            migrate_schema_eleven(connection)?;
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        12 => {
            validate_schema_twelve(connection)?;
            migrate_schema_twelve(connection)?;
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        13 => {
            validate_schema_thirteen(connection)?;
            migrate_schema_thirteen(connection)?;
        }
        STORE_SCHEMA_VERSION => validate_schema_shape(connection)?,
        value if !(0..=STORE_SCHEMA_VERSION).contains(&value) => {
            return Err(StoreError::UnsupportedSchema(value));
        }
        value => {
            return Err(StoreError::MigrationRequired {
                found: value,
                current: STORE_SCHEMA_VERSION,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_rejects_version_mismatch_zero_and_future_without_mutation() {
        let connection = Connection::open_in_memory().expect("open memory database");
        assert!(matches!(
            migrate_existing_transaction(&connection, 1),
            Err(StoreError::CorruptRecord)
        ));
        assert!(matches!(
            migrate_supported_schema(&connection, 0),
            Err(StoreError::MigrationRequired {
                found: 0,
                current: STORE_SCHEMA_VERSION
            })
        ));
        assert!(matches!(
            migrate_supported_schema(&connection, STORE_SCHEMA_VERSION + 1),
            Err(StoreError::UnsupportedSchema(value)) if value == STORE_SCHEMA_VERSION + 1
        ));
    }
}
