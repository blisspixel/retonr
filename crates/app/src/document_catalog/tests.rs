use super::*;
use tempfile::tempdir;

#[test]
fn counterpart_distinguishes_exact_missing_skipped_unsupported_and_metadata() {
    let root = tempdir().expect("catalog fixture contract");
    for name in ["Draft.txt", "marked.txt"] {
        fs::write(root.path().join(name), b"draft").expect("catalog fixture contract");
    }
    fs::write(root.path().join("unsupported.txt"), [0xff, 0xfe, 0])
        .expect("catalog fixture contract");
    fs::write(root.path().join("marked.txt.xmp"), b"metadata").expect("catalog fixture contract");
    fs::create_dir(root.path().join("nested")).expect("catalog fixture contract");
    let catalog = DocumentCatalogService::discover(
        root.path(),
        false,
        DocumentCatalogLimits::default(),
        &CancellationToken::new(),
    )
    .expect("catalog fixture contract");
    let relative =
        |name: &str| RelativeDocumentPath::new(name.to_owned()).expect("catalog fixture contract");
    assert!(matches!(
        catalog.counterpart(&relative("Draft.txt")),
        CatalogCounterpart::Matched(_)
    ));
    assert!(matches!(
        catalog.counterpart(&relative("draft.txt")),
        CatalogCounterpart::Missing
    ));
    assert!(matches!(
        catalog.counterpart(&relative("nested")),
        CatalogCounterpart::Skipped(_)
    ));
    assert!(matches!(
        catalog.counterpart(&relative("unsupported.txt")),
        CatalogCounterpart::Unsupported(_)
    ));
    assert!(matches!(
        catalog.counterpart(&relative("marked.txt")),
        CatalogCounterpart::DecisionRequired(_)
    ));
}

#[test]
fn vanished_entry_is_explicitly_unreadable_and_never_inventoried() {
    let root = tempdir().expect("catalog fixture contract");
    let path = root.path().join("gone.txt");
    fs::write(&path, b"draft").expect("catalog fixture contract");
    let entry = fs::read_dir(root.path())
        .expect("catalog fixture contract")
        .next()
        .expect("catalog fixture contract")
        .expect("catalog fixture contract");
    fs::remove_file(path).expect("catalog fixture contract");
    let Class::Skipped(entry) = classify_entry(&entry, "") else {
        panic!("vanished entry was accepted");
    };
    assert_eq!(entry.relative_path.as_deref(), Some("gone.txt"));
    assert_eq!(entry.reason, CatalogSkippedReason::Unreadable);
}

#[test]
fn enumeration_enforces_budget_and_cancellation_before_retaining_more_entries() {
    let root = tempdir().expect("catalog fixture contract");
    for name in ["a", "b", "c"] {
        fs::write(root.path().join(name), b"draft").expect("catalog fixture contract");
    }
    let token = CancellationToken::new();
    assert_eq!(
        read_sorted_entries(root.path(), 3, &token)
            .expect("catalog fixture contract")
            .len(),
        3
    );
    for budget in [0, 2] {
        assert!(matches!(
            read_sorted_entries(root.path(), budget, &token),
            Err(DocumentCatalogError::ResourceLimitExceeded)
        ));
    }
    token.cancel();
    assert!(matches!(
        read_sorted_entries(root.path(), 3, &token),
        Err(DocumentCatalogError::Cancelled)
    ));
}

#[test]
fn relative_component_and_skipped_reason_contracts_are_explicit() {
    for malformed in ["", ".", "..", "a/b", "a\\b", "a\0b"] {
        assert!(!portable_component(malformed));
    }
    assert!(portable_component("chapter:notes"));
    assert_eq!(join_relative("", "a.txt"), "a.txt");
    assert_eq!(join_relative("nested", "a.txt"), "nested/a.txt");
    assert!(is_ignored("TARGET") && is_ignored("Node_Modules"));
    assert!(!is_ignored("src"));
    for (reason, expected) in [
        (CatalogSkippedReason::Directory, "directory"),
        (CatalogSkippedReason::DepthLimit, "depth_limit"),
        (CatalogSkippedReason::MalformedName, "malformed_name"),
        (CatalogSkippedReason::Hidden, "hidden"),
        (CatalogSkippedReason::Ignored, "ignored"),
        (CatalogSkippedReason::Unreadable, "unreadable"),
        (CatalogSkippedReason::Symlink, "symlink"),
        (CatalogSkippedReason::NonRegular, "non_regular"),
    ] {
        assert_eq!(reason.as_str(), expected);
    }
    let private = DocumentCatalogError::Input(std::io::Error::other("private source text"));
    assert_eq!(format!("{private:?}"), "DocumentCatalogError::Input");
    assert!(!private.to_string().contains("private source text"));
}

#[test]
fn discovery_is_sorted_bounded_and_counts_unsupported_encoding_bytes() {
    let root = tempdir().expect("catalog fixture contract");
    fs::write(root.path().join("b.txt"), [0xff, 0xfe, 0]).expect("catalog fixture contract");
    fs::write(root.path().join("a.txt"), b"abc").expect("catalog fixture contract");
    let token = CancellationToken::new();
    let limits = DocumentCatalogLimits {
        entries: 2,
        depth: 0,
        bytes: 6,
    };
    let catalog = DocumentCatalogService::discover(root.path(), false, limits, &token)
        .expect("catalog fixture contract");
    assert_eq!(
        catalog
            .documents
            .iter()
            .map(|entry| entry.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["a.txt", "b.txt"]
    );
    assert_eq!(
        catalog.documents[0].observation.inventory.digest,
        crate::inspect_plain_text(b"abc")
            .expect("catalog fixture contract")
            .digest
    );
    assert!(matches!(
        DocumentCatalogService::discover(
            root.path(),
            false,
            DocumentCatalogLimits { bytes: 5, ..limits },
            &token
        ),
        Err(DocumentCatalogError::ResourceLimitExceeded)
    ));
    assert!(matches!(
        DocumentCatalogService::discover(
            root.path(),
            false,
            DocumentCatalogLimits {
                entries: 1,
                ..limits
            },
            &token
        ),
        Err(DocumentCatalogError::ResourceLimitExceeded)
    ));
    token.cancel();
    assert!(matches!(
        DocumentCatalogService::discover(root.path(), false, limits, &token),
        Err(DocumentCatalogError::Cancelled)
    ));
}

#[test]
fn zero_byte_budget_accepts_empty_files_and_refuses_data() {
    let root = tempdir().expect("catalog fixture contract");
    fs::write(root.path().join("empty.txt"), []).expect("catalog fixture contract");
    let token = CancellationToken::new();
    let limits = DocumentCatalogLimits {
        bytes: 0,
        ..DocumentCatalogLimits::default()
    };
    assert_eq!(
        DocumentCatalogService::discover(root.path(), false, limits, &token)
            .expect("catalog fixture contract")
            .documents
            .len(),
        1
    );
    fs::write(root.path().join("nonempty.txt"), b"x").expect("catalog fixture contract");
    assert!(matches!(
        DocumentCatalogService::discover(root.path(), false, limits, &token),
        Err(DocumentCatalogError::ResourceLimitExceeded)
    ));
}

#[test]
fn recursion_and_skips_are_explicit_and_depth_is_enforced() {
    let root = tempdir().expect("catalog fixture contract");
    fs::create_dir_all(root.path().join("nested/deep")).expect("catalog fixture contract");
    fs::create_dir(root.path().join("node_modules")).expect("catalog fixture contract");
    fs::write(root.path().join("nested/a.txt"), b"draft").expect("catalog fixture contract");
    fs::write(root.path().join("nested/deep/b.txt"), b"draft").expect("catalog fixture contract");
    fs::write(root.path().join(".hidden"), b"secret").expect("catalog fixture contract");
    let token = CancellationToken::new();
    let shallow = DocumentCatalogService::discover(
        root.path(),
        false,
        DocumentCatalogLimits::default(),
        &token,
    )
    .expect("catalog fixture contract");
    assert!(shallow.documents.is_empty());
    assert!(
        shallow
            .skipped
            .iter()
            .any(|entry| entry.reason == CatalogSkippedReason::Directory)
    );
    let recursive = DocumentCatalogService::discover(
        root.path(),
        true,
        DocumentCatalogLimits {
            depth: 1,
            ..DocumentCatalogLimits::default()
        },
        &token,
    )
    .expect("catalog fixture contract");
    assert_eq!(recursive.documents.len(), 1);
    assert_eq!(
        recursive.documents[0].relative_path.as_str(),
        "nested/a.txt"
    );
    for reason in [
        CatalogSkippedReason::DepthLimit,
        CatalogSkippedReason::Ignored,
        CatalogSkippedReason::Hidden,
    ] {
        assert!(recursive.skipped.iter().any(|entry| entry.reason == reason));
    }
}

#[test]
fn caller_cannot_enlarge_hard_ceilings_and_invalid_roots_are_refused() {
    let root = tempdir().expect("catalog fixture contract");
    let token = CancellationToken::new();
    for limits in [
        DocumentCatalogLimits {
            entries: usize::MAX,
            ..DocumentCatalogLimits::default()
        },
        DocumentCatalogLimits {
            depth: usize::MAX,
            ..DocumentCatalogLimits::default()
        },
        DocumentCatalogLimits {
            bytes: usize::MAX,
            ..DocumentCatalogLimits::default()
        },
    ] {
        assert!(matches!(
            DocumentCatalogService::discover(root.path(), true, limits, &token),
            Err(DocumentCatalogError::ResourceLimitExceeded)
        ));
    }
    fs::write(root.path().join("file"), b"draft").expect("catalog fixture contract");
    assert!(matches!(
        DocumentCatalogService::discover(
            &root.path().join("file"),
            false,
            DocumentCatalogLimits::default(),
            &token
        ),
        Err(DocumentCatalogError::InvalidPath)
    ));
    token.cancel();
    for path in [root.path().join("file"), root.path().join("missing")] {
        assert!(matches!(
            DocumentCatalogService::discover(
                &path,
                false,
                DocumentCatalogLimits {
                    entries: usize::MAX,
                    ..DocumentCatalogLimits::default()
                },
                &token,
            ),
            Err(DocumentCatalogError::Cancelled)
        ));
    }
}

#[cfg(unix)]
#[test]
fn links_and_native_byte_names_never_become_documents() {
    use std::os::unix::{ffi::OsStringExt as _, fs::symlink};
    let root = tempdir().expect("catalog fixture contract");
    let outside = tempdir().expect("catalog fixture contract");
    fs::write(root.path().join("draft.txt"), b"draft").expect("catalog fixture contract");
    symlink("draft.txt", root.path().join("alias")).expect("catalog fixture contract");
    symlink(outside.path(), root.path().join("foreign")).expect("catalog fixture contract");
    let invalid = root.path().join(std::ffi::OsString::from_vec(vec![0xff]));
    match fs::write(&invalid, b"private") {
        Ok(()) => {}
        Err(error) => assert!(
            cfg!(target_os = "macos")
                && error.raw_os_error() == Some(rustix::io::Errno::ILSEQ.raw_os_error())
        ),
    }
    let catalog = DocumentCatalogService::discover(
        root.path(),
        true,
        DocumentCatalogLimits::default(),
        &CancellationToken::new(),
    )
    .expect("catalog fixture contract");
    assert_eq!(catalog.documents.len(), 1);
    assert_eq!(
        catalog
            .skipped
            .iter()
            .filter(|entry| entry.reason == CatalogSkippedReason::Symlink)
            .count(),
        2
    );
    if invalid.exists() {
        assert!(
            catalog
                .skipped
                .iter()
                .any(|entry| entry.relative_path.is_none()
                    && entry.reason == CatalogSkippedReason::MalformedName)
        );
    }
}
