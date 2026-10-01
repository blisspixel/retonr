use super::*;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt as _;

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
    fs::write(&path, bytes).expect("document");
    assert_eq!(
        inspect_file(&path, CommandName::Inspect)
            .expect("plain file")
            .derivative(),
        "not_required"
    );
    for suffix in SIDECAR_SUFFIXES {
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
