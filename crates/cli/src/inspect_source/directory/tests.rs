use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn discovery_counts_all_encodings_and_refuses_the_next_file_before_inspection() {
    let root = tempdir().expect("temporary directory");
    fs::write(root.path().join("a.txt"), "abc").expect("UTF-8 source");
    fs::write(root.path().join("b.txt"), [0xff, 0xfe, 0x00]).expect("unsupported encoding");
    let cancellation = rewrite_types::CancellationToken::new();
    let (documents, _) = walk_cancellable(
        root.path(),
        false,
        10,
        0,
        6,
        CommandName::Inspect,
        &cancellation,
    )
    .expect("exact byte budget");
    assert_eq!(documents.len(), 2);
    let failure = walk_cancellable(
        root.path(),
        false,
        10,
        0,
        5,
        CommandName::Inspect,
        &cancellation,
    )
    .err()
    .expect("second file exceeds remaining discovery budget");
    assert_eq!(
        failure.body,
        ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false
        )
    );
    assert_eq!(failure.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
}

#[test]
fn zero_discovery_budget_accepts_empty_files_and_refuses_nonempty_files() {
    let root = tempdir().expect("temporary directory");
    fs::write(root.path().join("a.txt"), []).expect("empty source");
    let cancellation = rewrite_types::CancellationToken::new();
    assert!(
        walk_cancellable(
            root.path(),
            false,
            10,
            0,
            0,
            CommandName::Lint,
            &cancellation
        )
        .is_ok()
    );
    fs::write(root.path().join("b.txt"), "x").expect("nonempty source");
    assert!(
        walk_cancellable(
            root.path(),
            false,
            10,
            0,
            0,
            CommandName::Lint,
            &cancellation
        )
        .is_err()
    );
}

#[test]
fn enumeration_enforces_remaining_budget_before_inspection() {
    let root = tempdir().expect("temporary directory");
    for name in ["a", "b", "c"] {
        fs::write(root.path().join(name), "draft").expect("write entry");
    }
    let cancellation = rewrite_types::CancellationToken::new();
    assert_eq!(
        read_sorted_entries(root.path(), 3, CommandName::Lint, &cancellation)
            .expect("exact budget")
            .len(),
        3
    );
    assert!(read_sorted_entries(root.path(), 2, CommandName::Lint, &cancellation).is_err());
    assert!(read_sorted_entries(root.path(), 0, CommandName::Lint, &cancellation).is_err());
    cancellation.cancel();
    let Err(failure) = read_sorted_entries(root.path(), 3, CommandName::Lint, &cancellation) else {
        panic!("cancelled enumeration")
    };
    assert_eq!(
        failure.exit_code,
        ExitCode::from(crate::contract::EXIT_CANCELLED)
    );
}

#[test]
fn recursive_walk_lists_nested_files_and_skips_ignored_names() {
    let root = tempdir().expect("temporary directory");
    let path = root.path();
    fs::write(path.join("a.txt"), "alpha\n").expect("write a");
    fs::create_dir(path.join("nested")).expect("create nested");
    fs::write(path.join("nested").join("inner.txt"), "inner\n").expect("write nested");
    fs::create_dir(path.join("nested").join(".cache")).expect("create hidden");
    fs::write(path.join("nested").join(".cache").join("x.txt"), "x\n").expect("write hidden");
    fs::create_dir(path.join("node_modules")).expect("create node_modules");
    fs::write(path.join("node_modules").join("pkg.js"), "pkg\n").expect("write ignored");

    let (documents, skipped) = walk_bounded(
        path,
        true,
        MAXIMUM_DIRECTORY_ENTRIES,
        MAXIMUM_DIRECTORY_DEPTH,
        CommandName::Inspect,
    )
    .expect("walk");
    let document_paths: Vec<&str> = documents
        .iter()
        .map(|document| document.relative_path.as_str())
        .collect();
    assert_eq!(document_paths, vec!["a.txt", "nested/inner.txt"]);
    assert!(skipped.iter().any(
        |entry| entry.relative_path.as_deref() == Some("node_modules") && entry.reason == "ignored"
    ));
    assert!(skipped.iter().any(
        |entry| entry.relative_path.as_deref() == Some("nested/.cache") && entry.reason == "hidden"
    ));
    assert!(!document_paths.iter().any(|name| name.contains("pkg.js")));
    assert!(!document_paths.iter().any(|name| name.contains('\\')));
}

#[test]
fn recursive_walk_skips_directories_past_the_depth_limit() {
    let root = tempdir().expect("temporary directory");
    let path = root.path();
    let nested = path.join("d1").join("d2");
    fs::create_dir_all(&nested).expect("create nested");
    fs::write(nested.join("leaf.txt"), "leaf\n").expect("write leaf");

    let (documents, skipped) = walk_bounded(
        path,
        true,
        MAXIMUM_DIRECTORY_ENTRIES,
        1,
        CommandName::Inspect,
    )
    .expect("walk");
    assert!(documents.is_empty());
    assert!(
        skipped
            .iter()
            .any(|entry| entry.relative_path.as_deref() == Some("d1/d2")
                && entry.reason == "depth_limit")
    );
}

#[test]
fn recursive_walk_refuses_when_the_entry_limit_is_exceeded() {
    let root = tempdir().expect("temporary directory");
    let path = root.path();
    fs::write(path.join("a.txt"), "a\n").expect("write a");
    fs::write(path.join("b.txt"), "b\n").expect("write b");
    fs::write(path.join("c.txt"), "c\n").expect("write c");

    let Err(failure) = walk_bounded(path, true, 2, MAXIMUM_DIRECTORY_DEPTH, CommandName::Inspect)
    else {
        panic!("entry limit should refuse");
    };
    assert_eq!(failure.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
    assert_eq!(
        failure.body,
        ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false
        )
    );
}

#[test]
fn non_recursive_walk_skips_child_directories() {
    let root = tempdir().expect("temporary directory");
    let path = root.path();
    fs::write(path.join("a.txt"), "a\n").expect("write a");
    fs::create_dir(path.join("nested")).expect("create nested");
    fs::write(path.join("nested").join("inner.txt"), "inner\n").expect("write nested");

    let (documents, skipped) = walk_bounded(
        path,
        false,
        MAXIMUM_DIRECTORY_ENTRIES,
        MAXIMUM_DIRECTORY_DEPTH,
        CommandName::Inspect,
    )
    .expect("walk");
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0].relative_path, "a.txt");
    assert!(skipped.iter().any(
        |entry| entry.relative_path.as_deref() == Some("nested") && entry.reason == "directory"
    ));
}

#[test]
fn portable_relative_paths_reject_separators_and_join_with_slash() {
    assert!(!portable_component("a/b"));
    assert!(!portable_component("a\\b"));
    assert!(!portable_component(""));
    assert_eq!(join_relative("", "a.txt"), "a.txt");
    assert_eq!(join_relative("nested", "inner.txt"), "nested/inner.txt");
    assert!(is_ignored("TARGET"));
    assert!(is_ignored("Node_Modules"));
    assert!(!is_ignored("src"));
}
