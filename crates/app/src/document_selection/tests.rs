use super::*;

#[test]
fn relative_paths_preserve_case_and_existing_native_component_policy() {
    for path in [
        "draft.txt",
        "Notes/Draft.txt",
        "a:b.txt",
        "name-\u{202e}.txt",
    ] {
        assert_eq!(
            RelativeDocumentPath::new(path).expect("relative").as_str(),
            path
        );
    }
    for path in [
        "",
        "/draft.txt",
        "draft/",
        "draft//file",
        ".",
        "..",
        "a/../b",
        "a/./b",
        "a\\b",
        "a\0b",
    ] {
        assert!(
            RelativeDocumentPath::new(path).is_err(),
            "invalid relative path"
        );
    }
}

#[test]
fn selections_and_path_errors_have_content_free_debug_output() {
    let selection = DocumentSelection::explicit("private-draft.txt");
    assert_eq!(format!("{selection:?}"), "DocumentSelection::Explicit");
    let relative = RelativeDocumentPath::new("private-draft.txt").expect("relative");
    assert!(!format!("{relative:?}").contains("private"));
    let catalog = DocumentSelection::catalog("private-root", relative, Digest::sha256(b"secret"));
    assert_eq!(format!("{catalog:?}"), "DocumentSelection::Catalog");
    assert!(!DocumentSelectionError.to_string().contains("private"));
}
