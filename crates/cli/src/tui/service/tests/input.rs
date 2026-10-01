use super::*;

#[test]
fn refuses_stdin_directory_missing_files_and_protection_without_candidate() {
    let fixture = Fixture::new(b"Hello world.\n", b"Hello world!\n");
    for source in [
        PathBuf::from("-"),
        fixture.directory.path().to_owned(),
        fixture.directory.path().join("absent"),
    ] {
        let mut request = fixture.request(false);
        request.source = source;
        assert!(load(&request, &CancellationToken::new()).is_err());
    }
    let mut request = fixture.request(true);
    request.candidate = Some(PathBuf::from("-"));
    assert_eq!(
        load(&request, &CancellationToken::new())
            .expect_err("no stdin")
            .body,
        expected_body(ErrorCode::InvalidInvocation)
    );
    request = fixture.request(false);
    request.protected_terms.push("Acme".to_owned());
    assert_eq!(
        load(&request, &CancellationToken::new())
            .expect_err("candidate required")
            .body,
        expected_body(ErrorCode::InvalidInvocation)
    );
}

#[test]
fn refuses_file_byte_budget_and_both_unsupported_encoding_roles() {
    let fixture = Fixture::new(b"Hello world.\n", b"Hello world!\n");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(&fixture.source)
        .expect("limit file");
    file.set_len(u64::try_from(MAX_CANDIDATE_CHECK_BYTES + 1).expect("bound"))
        .expect("oversized fixture");
    drop(file);
    assert_eq!(
        load(&fixture.request(false), &CancellationToken::new())
            .expect_err("file budget")
            .body,
        expected_body(ErrorCode::ResourceLimitExceeded)
    );
    fs::write(&fixture.source, b"Hello world.\n").expect("source reset");
    for bytes in [
        b"\xff\xfeh\0".as_slice(),
        b"\xfe\xff\0h".as_slice(),
        b"\xffprivate".as_slice(),
    ] {
        fs::write(&fixture.candidate, bytes).expect("candidate encoding");
        assert_eq!(
            load(&fixture.request(true), &CancellationToken::new())
                .expect_err("candidate encoding rejected")
                .body,
            expected_body(ErrorCode::InputUnreadable)
        );
        fs::write(&fixture.source, bytes).expect("source encoding");
        assert_eq!(
            load(&fixture.request(false), &CancellationToken::new())
                .expect_err("source encoding rejected")
                .body,
            expected_body(ErrorCode::InputUnreadable)
        );
        fs::write(&fixture.source, b"Hello world.\n").expect("source restored");
    }
}

#[test]
fn shared_carrier_and_sidecar_policy_requires_explicit_decision_for_both_roles() {
    let fixture = Fixture::new(b"Hello world.\n", b"Hello world!\n");
    for path in [&fixture.source, &fixture.candidate] {
        let sidecar = PathBuf::from(format!("{}.c2pa", path.display()));
        fs::write(&sidecar, b"metadata is not validated by review").expect("sidecar");
        assert_eq!(
            load(&fixture.request(true), &CancellationToken::new())
                .expect_err("sidecar decision")
                .body,
            expected_body(ErrorCode::Unsupported)
        );
        fs::remove_file(&sidecar).expect("remove exact fixture sidecar");
        let original = fs::read(path).expect("original role");
        fs::write(path, "\u{feff}\u{fe0f}Hello world.\n".as_bytes()).expect("possible carrier");
        assert_eq!(
            load(&fixture.request(true), &CancellationToken::new())
                .expect_err("carrier decision")
                .body,
            expected_body(ErrorCode::Unsupported)
        );
        fs::write(path, original).expect("restore role bytes");
    }
}

#[test]
fn ordinary_utf8_bom_preserves_source_digest_and_uses_exact_app_policy() {
    let source = "\u{feff}Hello world\r\n";
    let candidate = "\u{feff}Hello, world!\r\n";
    let fixture = Fixture::new(source.as_bytes(), candidate.as_bytes());
    let snapshot =
        load(&fixture.request(true), &CancellationToken::new()).expect("ordinary UTF8 BOM");
    assert!(snapshot.inspection.utf8_bom);
    let reference = CandidateCheckService::check(CandidateCheckRequest::new(
        source.as_bytes().to_vec(),
        candidate.to_owned(),
        Vec::new(),
    ))
    .expect("CLI equivalent");
    assert_eq!(snapshot.check.as_ref(), Some(&reference.record));
    assert_eq!(
        fs::read(&fixture.source).expect("exact BOM and CRLF retained"),
        source.as_bytes()
    );
}

#[test]
fn hostile_paths_are_escaped_in_success_and_redacted_on_failure() {
    let mut fixture = Fixture::new(b"Hello world.\n", b"unused");
    let renamed = fixture.directory.path().join("draft-\u{202e}private.txt");
    fs::rename(&fixture.source, &renamed).expect("hostile label fixture");
    fixture.source = renamed;
    let presentation = load(&fixture.request(false), &CancellationToken::new())
        .expect("hostile filename review")
        .into_presentation();
    assert!(!presentation.source_label.contains('\u{202e}'));
    assert!(presentation.source_label.contains("\\u{202e}"));
    let mut request = fixture.request(false);
    request.source = fixture.directory.path().join("missing-\u{202e}private.txt");
    let failure = load(&request, &CancellationToken::new()).expect_err("missing hostile path");
    assert!(!format!("{failure:?} {}", failure.message).contains("private.txt"));
    assert!(!format!("{failure:?}").contains('\u{202e}'));
}

#[test]
fn protected_term_budget_uses_the_app_failure_contract_and_preserves_files() {
    let fixture = Fixture::new(b"Keep Acme 42 exactly.\n", b"Keep Acme 42 exactly!\n");
    let mut request = fixture.request(true);
    request.protected_terms = (0..33).map(|index| format!("protected-{index}")).collect();
    let failure = load(&request, &CancellationToken::new()).expect_err("app protection budget");
    let reference = CandidateCheckService::check(CandidateCheckRequest::new(
        fs::read(&fixture.source).expect("source"),
        fs::read_to_string(&fixture.candidate).expect("candidate"),
        request.protected_terms,
    ))
    .expect_err("same app budget");
    assert_eq!(
        failure.body,
        RunFailure::app(CommandName::Tui, &reference).body
    );
    assert_eq!(failure.body, expected_body(ErrorCode::InvalidInvocation));
    assert_eq!(
        fs::read(&fixture.source).expect("source retained"),
        b"Keep Acme 42 exactly.\n"
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_selected_source_and_candidate_sidecars_require_a_decision() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt as _;
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../fixtures/cli/non-utf8-sidecar.json"
    ))
    .expect("regression fixture");
    let name = OsString::from_vec(
        serde_json::from_value(data["filename_bytes"].clone()).expect("filename bytes"),
    );
    for candidate in [false, true] {
        let mut fixture = Fixture::new(b"Hello world.\n", b"Hello world!\n");
        let path = fixture.directory.path().join(&name);
        let selected = if candidate {
            &mut fixture.candidate
        } else {
            &mut fixture.source
        };
        fs::rename(&*selected, &path).expect("non UTF8 selected file");
        *selected = path;
        load(&fixture.request(true), &CancellationToken::new())
            .expect("ordinary path remains supported");
        for suffix in [".c2pa", ".xmp"] {
            let mut adjacent = name.clone();
            adjacent.push(suffix);
            let sidecar = fixture.directory.path().join(adjacent);
            fs::write(&sidecar, data["sidecar"].as_str().expect("sidecar")).expect("sidecar");
            assert_eq!(
                load(&fixture.request(true), &CancellationToken::new())
                    .expect_err("explicit decision")
                    .body,
                expected_body(ErrorCode::Unsupported)
            );
            fs::remove_file(sidecar).expect("remove fixture sidecar");
        }
    }
}
