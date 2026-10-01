use super::*;
use crate::contract::{CommandName, ErrorBody, ErrorCategory, ErrorCode};
use crate::failure::RunFailure;
use std::{fs, io};
use tempfile::tempdir;

#[test]
fn shared_reader_keeps_cli_input_error_envelopes() {
    let root = tempdir().expect("root");
    let path = root.path().join("private-draft.txt");
    fs::write(&path, b"draft").expect("source");
    // Mapping remains independent of the private marker's implementation.
    let error = io::Error::from(io::ErrorKind::Interrupted);
    assert!(!is_changed(&error));
    assert_eq!(
        RunFailure::input_read(CommandName::Check, &error).body,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::InputUnreadable,
            false
        )
    );
    assert_eq!(
        RunFailure::concurrent_modification(CommandName::Check).body,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::ConcurrentModification,
            true
        )
    );
    assert_eq!(
        read_regular_bounded(&path, 5).expect("shared read"),
        b"draft"
    );
}
