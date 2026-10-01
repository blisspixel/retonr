use super::*;

fn request(source: &Path, candidate: &Path) -> CheckRequest {
    CheckRequest {
        source: source.to_path_buf(),
        candidate: candidate.to_path_buf(),
        traversal: super::super::CheckTraversal::Recursive,
        protected_terms: Vec::new(),
        fail_on_abstain: false,
        output: None,
        in_place: replace::InPlaceFlags {
            requested: false,
            backup: false,
        },
        raw_terminal: false,
        confirmed: false,
        inspection: super::super::CheckInspection {
            diff: false,
            dry_run: true,
            trace: None,
        },
        layout: None,
        edit_level: None,
    }
}

#[test]
fn cancellation_returns_no_partial_pair_report() {
    let source = tempfile::tempdir().expect("source");
    let candidate = tempfile::tempdir().expect("candidate");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let Err(failure) = build_report(&request(source.path(), candidate.path()), &cancellation)
    else {
        panic!("must cancel")
    };
    assert_eq!(
        failure.exit_code,
        ExitCode::from(crate::contract::EXIT_CANCELLED)
    );
}

#[test]
fn same_and_nested_roots_are_policy_refusals() {
    let root = tempfile::tempdir().expect("root");
    let nested = root.path().join("nested");
    fs::create_dir(&nested).expect("nested");
    for (source, candidate) in [
        (root.path(), root.path()),
        (root.path(), nested.as_path()),
        (nested.as_path(), root.path()),
    ] {
        let Err(failure) = validate_request(&request(source, candidate)) else {
            panic!("overlapping roots")
        };
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_POLICY));
    }
}

#[test]
fn coherent_read_detects_stale_digest_and_aggregate_overflow() {
    let root = tempfile::tempdir().expect("root");
    let path = root.path().join("draft.txt");
    fs::write(&path, "draft").expect("draft");
    let document = DiscoveredDocument {
        relative_path: "draft.txt".into(),
        encoding: TextEncoding::Utf8,
        digest: Digest::sha256(b"draft").as_str().into(),
        derivative: "not_required",
    };
    let mut total = MAXIMUM_TOTAL_BYTES - 5;
    assert_eq!(
        read_coherent(root.path(), &document, &mut total).expect("exact boundary"),
        b"draft"
    );
    assert!(read_coherent(root.path(), &document, &mut total).is_err());
    fs::write(&path, "other").expect("modify");
    let Err(failure) = read_coherent(root.path(), &document, &mut 0) else {
        panic!("stale digest")
    };
    assert_eq!(
        failure.body,
        ErrorBody::new(
            ErrorCategory::Operational,
            ErrorCode::ConcurrentModification,
            true
        )
    );
}
