//! A malformed independent schema verifies the exact production reader byte cap.

use super::*;

#[test]
fn malformed_multibyte_text_is_byte_capped_before_type_rejection() {
    let connection = Connection::open_in_memory().expect("independent corrupt-file fixture");
    // The real STRICT schema refuses TEXT before insertion. This separate fixture
    // represents an externally malformed database without weakening that schema.
    connection
        .execute_batch(
            "CREATE TABLE generation_qualification_records (
        generation_qualification_id TEXT, target_generation_system_id TEXT,
        baseline_generation_system_id TEXT, operation_policy_id TEXT,
        request_projection_id TEXT, generation_qualification_platform_evidence_id TEXT,
        generation_qualification_license_evidence_id TEXT,
        generation_qualification_operation_receipt_id TEXT, status TEXT,
        canonical_json BLOB);",
        )
        .expect("malformed independent table");
    let text = "\u{e9}".repeat(12_000);
    connection.execute("INSERT INTO generation_qualification_records VALUES ('id', 'target', 'baseline', 'policy', 'projection', 'platform', 'license', 'receipt', 'rejected', ?1)", [&text]).expect("external malformed TEXT row");
    let sql = format!("SELECT {SELECT_COLUMNS} FROM generation_qualification_records");
    let (kind, characters, bytes): (String, i64, Vec<u8>) = connection
        .query_row(&sql, [], |row| {
            Ok((row.get(9)?, row.get(10)?, row.get(11)?))
        })
        .expect("actual bounded production projection");
    assert_eq!(kind, "text");
    assert_eq!(characters, 12_000);
    assert_eq!(text.len(), 24_000);
    assert_eq!(
        bytes.len(),
        MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES + 1
    );
    assert!(matches!(
        one_row(&connection, &sql, []),
        Err(StoreError::CorruptRecord)
    ));
}
