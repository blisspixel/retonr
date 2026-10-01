use super::*;
#[cfg(unix)]
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt as _;

#[cfg(unix)]
#[test]
fn non_utf8_selected_paths_detect_each_exact_adjacent_sidecar() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/cli/non-utf8-sidecar.json"
    ))
    .expect("regression fixture");
    let name = OsString::from_vec(
        serde_json::from_value(fixture["filename_bytes"].clone()).expect("filename bytes"),
    );
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join(&name);
    let bytes = fixture["document"].as_str().expect("document").as_bytes();
    if let Err(error) = fs::write(&path, bytes) {
        // APFS rejects this raw filename before the application can inspect it.
        assert!(
            unsupported_filename_on_macos(&error),
            "unexpected fixture creation failure: {error}"
        );
        assert_eq!(
            error.raw_os_error(),
            Some(rustix::io::Errno::ILSEQ.raw_os_error())
        );
        let failure = inspect_file(&path, CommandName::Inspect)
            .err()
            .expect("unrepresentable selected path must be refused");
        assert_eq!(
            failure.body,
            crate::contract::ErrorBody::new(
                crate::contract::ErrorCategory::Operational,
                crate::contract::ErrorCode::InputUnreadable,
                false,
            )
        );
        assert_eq!(
            fs::read_dir(root.path())
                .expect("fixture directory")
                .count(),
            0
        );
        return;
    }
    assert_eq!(
        inspect_file(&path, CommandName::Inspect)
            .expect("plain file")
            .derivative(),
        "not_required"
    );
    for suffix in [".c2pa", ".xmp"] {
        let mut sidecar_name = name.clone();
        sidecar_name.push(suffix);
        let sidecar = root.path().join(sidecar_name);
        fs::write(&sidecar, fixture["sidecar"].as_str().expect("sidecar"))
            .expect("adjacent sidecar");
        let report = inspect_file(&path, CommandName::Inspect).expect("inventory");
        assert_eq!(report.sidecars.status, "complete");
        assert_eq!(report.sidecars.present.len(), 1);
        assert!(report.sidecars.present[0].ends_with(suffix));
        assert_eq!(report.derivative(), "explicit_decision_required");
        assert_eq!(fs::read(&path).expect("unchanged document"), bytes);
        fs::remove_file(sidecar).expect("remove sidecar");
    }
}

#[cfg(unix)]
fn unsupported_filename_on_macos(error: &std::io::Error) -> bool {
    cfg!(target_os = "macos")
        && error.raw_os_error() == Some(rustix::io::Errno::ILSEQ.raw_os_error())
}

#[test]
fn incomplete_sidecar_scan_is_visible_and_requires_a_derivative_decision() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/cli/sidecar-scan-incomplete.json"
    ))
    .expect("fixture");
    let root = tempfile::tempdir().expect("root");
    let name = "a".repeat(
        usize::try_from(fixture["filename_bytes"].as_u64().expect("length")).expect("length"),
    );
    let path = root.path().join(name);
    let bytes = fixture["document"].as_str().expect("document").as_bytes();
    fs::write(&path, bytes).expect("maximum legal filename");
    let report = inspect_file(&path, CommandName::Inspect).expect("honest inventory");
    assert_eq!(
        report.sidecars.status,
        fixture["expected_scan"].as_str().expect("status")
    );
    assert!(report.sidecars.present.is_empty());
    assert_eq!(
        report.derivative(),
        fixture["expected_derivative"]
            .as_str()
            .expect("disposition")
    );
    assert!(
        crate::inspect_source::requires_derivative_decision(&path, bytes, CommandName::Tui)
            .expect("shared gate")
    );
    assert_eq!(fs::read(&path).expect("unchanged source"), bytes);
    assert_eq!(fs::read_dir(root.path()).expect("no outputs").count(), 1);
}

#[test]
fn stdin_metadata_policy_and_io_error_mapping_remain_presentation_owned() {
    let (report, count) = inventory_report(Path::new("-"), CommandName::Inspect, b"Hello\n")
        .expect("stream inventory");
    assert_eq!(count, 6);
    assert_eq!(report.sidecars.status, "not_applicable");
    assert_eq!(report.derivative(), "not_required");
    assert_eq!(
        intake_failure(CommandName::Inspect, &DocumentIntakeError::Changed).body,
        crate::contract::ErrorBody::new(
            crate::contract::ErrorCategory::Operational,
            crate::contract::ErrorCode::ConcurrentModification,
            true
        )
    );
    assert_eq!(
        intake_failure(CommandName::Inspect, &DocumentIntakeError::Cancelled).body,
        crate::contract::ErrorBody::new(
            crate::contract::ErrorCategory::Cancelled,
            crate::contract::ErrorCode::OperationCancelled,
            false
        )
    );
}
