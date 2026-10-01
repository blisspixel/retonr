use super::super::TuiArgs;
use super::*;

fn request(source: &Path, candidate: Option<&Path>) -> ReviewRequest {
    ReviewRequest {
        args: TuiArgs {
            source: source.into(),
            candidate: candidate.map(Into::into),
            protected_terms: Vec::new(),
            plain: true,
            recursive: true,
            document: None,
        },
        index: None,
    }
}

#[test]
fn nested_catalog_is_sorted_exactly_paired_and_navigation_reloads_current_bytes() {
    let source = tempfile::tempdir().expect("source");
    let candidate = tempfile::tempdir().expect("candidate");
    fs::create_dir(source.path().join("nested")).expect("nested");
    fs::create_dir(candidate.path().join("nested")).expect("nested");
    fs::write(source.path().join("a.txt"), b"Hello world.").expect("first");
    fs::write(
        source.path().join("nested/b.txt"),
        b"Moreover, hello world.",
    )
    .expect("second");
    fs::write(candidate.path().join("nested/b.txt"), b"Hello world.").expect("candidate");
    fs::write(candidate.path().join("A.txt"), b"Wrong case.").expect("unmatched");
    let mut request = request(source.path(), Some(candidate.path()));
    let first = load(&request, &CancellationToken::new()).expect("folder");
    let navigation = first.directory.expect("directory navigation");
    assert_eq!(navigation.total, 2);
    assert_eq!(navigation.selected_relative.as_deref(), Some("a.txt"));
    assert_eq!(navigation.unmatched_candidates, 1);
    assert!(first.candidate.is_none());
    assert!(
        first
            .findings
            .iter()
            .any(|row| row.contains("check unavailable"))
    );
    request.index = Some(1);
    let second = load(&request, &CancellationToken::new()).expect("next");
    assert!(second.status.contains("Candidate check:"));
    assert_eq!(second.candidate.as_deref(), Some("Hello world."));
    fs::write(source.path().join("nested/b.txt"), b"Reloaded source.").expect("change");
    let reloaded = load(&request, &CancellationToken::new()).expect("fresh discovery");
    assert_eq!(reloaded.source, "Reloaded source.");
    assert_eq!(
        fs::read(candidate.path().join("nested/b.txt")).expect("unchanged"),
        b"Hello world."
    );
}

#[test]
fn exact_document_selection_missing_and_overlapping_roots_are_closed() {
    let root = tempfile::tempdir().expect("root");
    fs::write(root.path().join("first.txt"), b"text").expect("first");
    let mut selected = request(root.path(), None);
    selected.args.document = Some("first.txt".into());
    assert_eq!(
        load(&selected, &CancellationToken::new())
            .expect("selected")
            .source,
        "text"
    );
    for path in ["First.txt", "../first.txt", "/first.txt"] {
        selected.args.document = Some(path.into());
        assert!(load(&selected, &CancellationToken::new()).is_err());
    }
    let overlap = request(root.path(), Some(root.path()));
    assert!(load(&overlap, &CancellationToken::new()).is_err());
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        serde_json::to_value(
            load(&request(Path::new("missing"), None), &cancellation)
                .err()
                .expect("cancelled")
                .body
        )
        .expect("body")["code"],
        "operation_cancelled"
    );
}

#[test]
fn skipped_unsupported_and_empty_directories_are_disclosed_without_false_checks() {
    let root = tempfile::tempdir().expect("root");
    fs::write(root.path().join(".hidden.txt"), b"hidden").expect("hidden");
    fs::write(root.path().join("wide.txt"), b"\xff\xfeh\0i\0").expect("UTF16");
    let snapshot = load(&request(root.path(), None), &CancellationToken::new())
        .expect("no eligible documents");
    assert!(snapshot.status.contains("No supported source documents"));
    assert_eq!(snapshot.directory.expect("navigation").source_skipped, 2);
    assert!(
        snapshot
            .findings
            .iter()
            .any(|line| line.contains("encoding"))
    );
    assert!(snapshot.findings.iter().any(|line| line.contains("hidden")));
}

#[test]
fn changed_discovery_digest_and_hardlinks_never_supply_review_bytes() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("input.txt");
    fs::write(&path, b"initial").expect("initial");
    let discovered = discover_cancellable(
        root.path(),
        true,
        CommandName::Tui,
        &CancellationToken::new(),
    )
    .expect("catalog");
    fs::write(&path, b"changed").expect("changed");
    assert!(
        read(
            root.path(),
            &discovered.documents[0],
            &CancellationToken::new()
        )
        .is_err()
    );
    fs::write(&path, b"initial").expect("restore bytes");
    fs::hard_link(&path, root.path().join("alias.txt")).expect("alias");
    assert!(
        read(
            root.path(),
            &discovered.documents[0],
            &CancellationToken::new()
        )
        .is_err()
    );
}

#[test]
fn hostile_paths_are_sanitized_in_catalog_and_selected_preview() {
    let root = tempfile::tempdir().expect("root");
    let name = "draft\u{202e}.txt";
    fs::write(root.path().join(name), "hello\u{1b}[2J").expect("hostile");
    let snapshot =
        load(&request(root.path(), None), &CancellationToken::new()).expect("safe preview");
    assert!(!crate::render::contains_terminal_effect(&snapshot.source));
    assert!(
        snapshot
            .findings
            .iter()
            .all(|row| !crate::render::contains_terminal_effect(row))
    );
    assert!(snapshot.source_label.contains("\\u{202e}"));
    assert_eq!(
        snapshot
            .directory
            .expect("navigation")
            .selected_relative
            .as_deref(),
        Some(name)
    );
}

#[test]
fn discovery_entry_ceiling_refuses_before_any_selected_review() {
    let root = tempfile::tempdir().expect("root");
    for index in 0..4097 {
        fs::write(root.path().join(format!("{index:04}.txt")), b"").expect("bounded fixture");
    }
    let failure = load(&request(root.path(), None), &CancellationToken::new())
        .err()
        .expect("entry ceiling");
    assert_eq!(
        serde_json::to_value(failure.body).expect("body")["code"],
        "resource_limit_exceeded"
    );
    assert_eq!(
        fs::read_dir(root.path())
            .expect("unchanged entries")
            .count(),
        4097
    );
}

#[test]
fn oversized_inputs_refuse_and_invalid_candidate_encoding_is_disclosed() {
    let source = tempfile::tempdir().expect("source");
    let candidate = tempfile::tempdir().expect("candidate");
    fs::write(source.path().join("a.txt"), b"Plain text.").expect("source");
    let large = fs::File::create(source.path().join("large.txt")).expect("large fixture");
    large
        .set_len((MAX_CANDIDATE_CHECK_BYTES + 1) as u64)
        .expect("sparse fixture length");
    fs::write(candidate.path().join("a.txt"), b"\xff\xfeh\0i\0").expect("unsupported candidate");
    let failure = load(
        &request(source.path(), Some(candidate.path())),
        &CancellationToken::new(),
    )
    .err()
    .expect("oversized discovery refuses instead of partial review");
    assert_eq!(
        serde_json::to_value(failure.body).expect("body")["code"],
        "resource_limit_exceeded"
    );
    drop(large);
    fs::remove_file(source.path().join("large.txt")).expect("remove oversized fixture");
    let snapshot = load(
        &request(source.path(), Some(candidate.path())),
        &CancellationToken::new(),
    )
    .expect("remaining supported source");
    assert!(snapshot.candidate.is_none());
    let navigation = snapshot.directory.expect("navigation");
    assert_eq!(navigation.total, 1);
    assert_eq!(navigation.source_skipped, 0);
    assert_eq!(navigation.candidate_skipped, 1);
    assert!(
        snapshot
            .findings
            .iter()
            .any(|row| row.contains("check unavailable"))
    );
}
