use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn accepts_utf8_bom_on_both_source_and_candidate_files() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, b"\xEF\xBB\xBFHello world\n").expect("write source with BOM");
    fs::write(&candidate, b"\xEF\xBB\xBFHello, world!\n").expect("write candidate with BOM");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"rewritten\""));
}

#[test]
fn reports_rewritten_candidate_as_json() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema_version\": 2"))
        .stdout(predicate::str::contains("\"status\": \"rewritten\""))
        .stdout(predicate::str::contains("\"assessments\""))
        .stdout(predicate::str::contains("\"generation\"").not());
}

#[test]
fn abstention_can_be_used_as_a_ci_failure() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Version 2\n").expect("write source fixture");
    fs::write(&candidate, "Version 3\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .arg("--fail-on-abstain")
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"status\": \"abstained\""))
        .stdout(predicate::str::contains(
            "\"reason\": \"protected_value_changed\"",
        ));
}

#[test]
fn short_format_and_output_flags_match_the_long_forms() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let output = directory.path().join("accepted.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check", "-f", "text", "-o"])
        .arg(&output)
        .arg(&source)
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("accepted    candidate passed"));
    assert_eq!(
        fs::read(&output).expect("accepted bytes"),
        b"Hello, world!\n"
    );
}

#[test]
fn global_format_is_accepted_before_or_after_the_command() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    for arguments in [
        vec!["--format", "text", "check"],
        vec!["check", "--format", "text"],
    ] {
        Command::cargo_bin("retonr")
            .expect("compiled CLI")
            .args(arguments)
            .arg(&source)
            .arg(&candidate)
            .assert()
            .success()
            .stdout(predicate::str::contains("nothing written (use -o)"));
    }
}

#[test]
fn text_report_never_contains_raw_document_content() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "private phrase\n").expect("write source fixture");
    fs::write(&candidate, "private phrase.\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .args(["--format", "text"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing written (use -o)"))
        .stdout(predicate::str::contains("private phrase").not());
}

#[test]
fn directory_source_is_rejected_as_unreadable() {
    let directory = tempdir().expect("temporary directory");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&candidate, "Hello\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(directory.path())
        .arg(candidate)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"category\": \"usage\""))
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""));
}

#[test]
fn missing_source_is_an_unreadable_input_not_a_retryable_failure() {
    let directory = tempdir().expect("temporary directory");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&candidate, "Hello\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(directory.path().join("missing.txt"))
        .arg(candidate)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("\"category\": \"operational\""))
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""))
        .stderr(predicate::str::contains("\"input_role\": \"source\""))
        .stderr(predicate::str::contains("\"retryable\": false"));
}

#[test]
fn oversized_candidate_is_a_resource_limit() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "short\n").expect("write source fixture");
    fs::write(&candidate, "a".repeat(16 * 1024 * 1024 + 1)).expect("write oversized candidate");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .code(4)
        .stderr(predicate::str::contains("\"category\": \"compatibility\""))
        .stderr(predicate::str::contains(
            "\"code\": \"resource_limit_exceeded\"",
        ))
        .stderr(predicate::str::contains("\"retryable\": false"));
}

#[test]
fn invalid_utf8_source_is_usage() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, b"a\xFF").expect("write invalid source");
    fs::write(&candidate, "a\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"category\": \"usage\""))
        .stderr(predicate::str::contains("\"code\": \"input_unreadable\""));
}
/// The checked-in fixtures back the documented reproduction and the retained
/// screenshots, so their bytes are a contract. Line-ending normalization on
/// checkout silently changed these digests once; `.gitattributes` now pins
/// `fixtures/**` with `-text` and this test fails if that protection regresses.
#[test]
fn checked_in_cli_fixtures_keep_their_documented_bytes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repository root")
        .to_path_buf();

    let cases: [(&str, &[u8]); 2] = [
        ("fixtures/cli/source.txt", b"Hello world\n"),
        ("fixtures/cli/candidate.txt", b"Hello, world!\n"),
    ];

    for (relative, expected) in cases {
        let actual = fs::read(root.join(relative)).expect("read checked-in fixture");
        assert_eq!(
            actual, expected,
            "{relative} lost its exact committed bytes; check .gitattributes"
        );
    }
}

#[test]
fn diff_dry_run_and_trace_are_safe_and_non_replacing() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let output = directory.path().join("accepted.txt");
    let trace = directory.path().join("trace.json");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(&candidate)
        .args(["--diff", "--dry-run", "--format", "text"])
        .arg("--output")
        .arg(&output)
        .arg("--trace")
        .arg(&trace)
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing written"))
        .stderr(predicate::str::contains("diff: changed"))
        .stderr(predicate::str::contains("replace"))
        .stderr(predicate::str::contains("Hello, world!"));

    assert!(!output.exists(), "dry-run must not create --output");
    let trace_text = fs::read_to_string(&trace).expect("read trace");
    assert!(trace_text.contains("\"command\": \"check\""));
    assert!(trace_text.contains("\"status\": \"rewritten\""));
    assert!(!trace_text.contains("Hello world"));

    fs::write(&output, b"keep existing").expect("write existing destination");
    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(&candidate)
        .args(["--dry-run", "--output"])
        .arg(&output)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(
        fs::read(&output).expect("read destination"),
        b"keep existing"
    );
}

#[test]
fn diff_neutralizes_controls_and_trace_refuses_to_replace() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let trace = directory.path().join("trace.json");
    fs::write(&source, "safe line\n").expect("write source fixture");
    fs::write(&candidate, "safe line.\n").expect("write candidate fixture");
    fs::write(&trace, b"existing").expect("write existing trace");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(&candidate)
        .arg("--diff")
        .assert()
        .success()
        .stderr(predicate::str::contains("safe line."));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .arg("--trace")
        .arg(&trace)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));
    assert_eq!(fs::read(trace).expect("read existing trace"), b"existing");
}

#[test]
fn existing_trace_is_refused_before_in_place_mutation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let trace = directory.path().join("trace.json");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");
    fs::write(&trace, b"existing").expect("write existing trace");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(&candidate)
        .args(["--in-place", "--trace"])
        .arg(&trace)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("\"code\": \"output_exists\""));

    assert_eq!(fs::read(&source).expect("read source"), b"Hello world\n");
    assert_eq!(fs::read(&trace).expect("read trace"), b"existing");
    assert!(!directory.path().join("source.txt.retonr-backup").exists());
    assert!(!directory.path().join("source.txt.retonr-staging").exists());
}

#[test]
fn trace_cannot_share_a_primary_output_path() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let output = directory.path().join("accepted.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(&candidate)
        .arg("--output")
        .arg(&output)
        .arg("--trace")
        .arg(&output)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));

    assert!(!output.exists(), "a conflicting path must not be created");
}

#[test]
fn layout_max_chars_abstains_with_character_budget_exceeded() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Short headline\n").expect("write source fixture");
    fs::write(&candidate, "A considerably longer candidate headline\n")
        .expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .args(["--max-chars", "20", "--fail-on-abstain"])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"status\": \"abstained\""))
        .stdout(predicate::str::contains(
            "\"reason\": \"character_budget_exceeded\"",
        ));
}

#[test]
fn layout_max_expansion_pct_abstains_when_expansion_exceeds_ceiling() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Source text with thirty-five chars\n").expect("write source fixture");
    fs::write(
        &candidate,
        "Source text with significantly expanded candidate copy\n",
    )
    .expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .args(["--max-expansion-pct", "8", "--fail-on-abstain"])
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"status\": \"abstained\""))
        .stdout(predicate::str::contains(
            "\"reason\": \"character_budget_exceeded\"",
        ));
}

#[test]
fn edit_level_argument_is_accepted_by_cli() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("write source fixture");
    fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(source)
        .arg(candidate)
        .args(["--edit-level", "voice-pass"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"rewritten\""))
        .stdout(predicate::str::contains(
            "\"semantic_evaluator\": \"literal-token-sequence-v1\"",
        ))
        .stdout(predicate::str::contains(
            "\"edit_level_effective\": \"touch-up\"",
        ))
        .stdout(predicate::str::contains(
            "\"edit_level_requested\": \"voice-pass\"",
        ));
}

#[test]
fn edit_level_reports_effective_touch_up_for_every_requested_level() {
    let levels = ["touch-up", "voice-pass", "rewrite", "reconstruct"];
    for level in levels {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source.txt");
        let candidate = directory.path().join("candidate.txt");
        fs::write(&source, "Hello world\n").expect("write source fixture");
        fs::write(&candidate, "Hello, world!\n").expect("write candidate fixture");
        let mut command = Command::cargo_bin("retonr").expect("compiled CLI");
        command
            .args(["--format", "text", "check"])
            .arg(&source)
            .arg(&candidate)
            .args(["--edit-level", level]);
        let assertion = command
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "semantic_evaluator: literal-token-sequence-v1",
            ))
            .stdout(predicate::str::contains("edit_level_effective: touch-up"))
            .stdout(predicate::str::contains(format!(
                "edit_level_requested: {level}"
            )));
        if level == "touch-up" {
            assertion.stdout(predicate::str::contains("note:").not());
        } else {
            assertion.stdout(predicate::str::contains(format!(
                "note: edit level {level} requested; the current evaluator accepts touch-up changes only (punctuation and capitalization)"
            )));
        }
    }
}

#[test]
fn missing_input_names_the_unreadable_side_and_path() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello\n").expect("write source");

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "text", "check"])
        .arg(directory.path().join("missing.txt"))
        .arg(&candidate)
        .assert()
        .code(1)
        .stderr(predicate::str::contains("error: source could not be read:"))
        .stderr(predicate::str::contains("missing.txt"));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "text", "check"])
        .arg(&source)
        .arg(directory.path().join("absent.txt"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error: candidate could not be read:",
        ))
        .stderr(predicate::str::contains("absent.txt"));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(directory.path().join("absent.txt"))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("\"input_role\": \"candidate\""))
        .stderr(predicate::str::contains("\"diagnostic\":"))
        .stderr(predicate::str::contains("absent.txt"));
}

#[test]
fn semantic_abstention_locates_the_changed_word_without_echoing_it() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("orig.txt");
    let candidate = directory.path().join("cand.txt");
    fs::write(
        &source,
        "The quarterly budget rose by 4.5% to $12,400, and the team shipped 3 releases before March 1.\nRevenue was -2% year over year, which is better than the -7% we forecast.\n",
    )
    .expect("write source");
    fs::write(
        &candidate,
        "The quarterly budget rose by 4.5% to $12,400, and the team shipped 3 releases before March 1.\nRevenue was -2% year over year, better than the -7% we forecast.\n",
    )
    .expect("write candidate");

    let text = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "text", "check", "--diff"])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("run check");
    let stdout = String::from_utf8(text.stdout).expect("stdout");
    let stderr = String::from_utf8(text.stderr).expect("stderr");
    assert!(text.status.success(), "abstention exits 0");
    assert!(stdout.contains("abstained   original kept unchanged"));
    assert!(stdout.contains("reason: semantic_uncertain"));
    assert!(stdout.contains("line 2 col 33: word_changed altered"));
    assert!(stderr.contains("no diff: the original was kept"));
    assert!(!stdout.contains("which"));
    assert!(!stdout.contains("better"));
    assert!(!stderr.contains("which"));
    assert!(!stderr.contains("better"));
}

fn case_edit(changes: usize) -> (String, String) {
    let mut source = String::new();
    let mut candidate = String::new();
    for index in 0..100 {
        if index > 0 {
            source.push(' ');
            candidate.push(' ');
        }
        source.push_str("word");
        if index < changes {
            candidate.push_str("Word");
        } else {
            candidate.push_str("word");
        }
    }
    (source, candidate)
}

#[test]
fn change_share_fixtures_honor_the_optional_budget_and_keep_style_out_of_acceptance() {
    let (source_text, two) = case_edit(2);
    let (_, ten) = case_edit(10);
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let small = directory.path().join("two.txt");
    let large = directory.path().join("ten.txt");
    let sample = directory.path().join("sample.txt");
    fs::write(&source, &source_text).expect("write source");
    fs::write(&small, &two).expect("write two percent candidate");
    fs::write(&large, ten).expect("write ten percent candidate");
    fs::write(&sample, "qxsampletokenqx truly groundbreaking").expect("write sample");

    let accepted = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args([
            "--format",
            "json",
            "check",
            "--max-change-pct",
            "2",
            "--style-sample",
        ])
        .arg(&sample)
        .arg(&source)
        .arg(&small)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let accepted = String::from_utf8(accepted).expect("utf-8");
    assert!(accepted.contains("\"status\": \"rewritten\""));
    assert!(accepted.contains("\"changed_token_pct\": 2"));
    assert!(accepted.contains("\"changed_char_pct\": 2"));
    assert!(accepted.contains("style-feature-distance-v1"));
    assert!(accepted.contains(
        "This is a style-feature distance, not an authorship verdict. It has no acceptance or ranking authority."
    ));
    assert!(!accepted.contains("qxsampletokenqx"));
    assert!(!accepted.contains("groundbreaking"));

    Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args([
            "--format",
            "json",
            "check",
            "--max-change-pct",
            "2",
            "--fail-on-abstain",
        ])
        .arg(&source)
        .arg(&large)
        .assert()
        .code(3)
        .stdout(predicate::str::contains("\"status\": \"abstained\""))
        .stdout(predicate::str::contains(
            "\"reason\": \"change_budget_exceeded\"",
        ))
        .stdout(predicate::str::contains("\"changed_token_pct\": 10"))
        .stdout(predicate::str::contains("\"changed_char_pct\": 10"));

    let identical = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check"])
        .arg(&source)
        .arg(&source)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let identical = String::from_utf8(identical).expect("utf-8");
    assert!(identical.contains("\"changed_token_pct\": 0"));
    assert!(identical.contains("\"changed_char_pct\": 0"));
    assert!(!identical.contains("style_distance"));

    let unchanged = directory.path().join("same.txt");
    let substituted = directory.path().join("substituted.txt");
    fs::write(&unchanged, "word word\n").expect("write lexical source");
    fs::write(&substituted, "word bird\n").expect("write lexical candidate");
    let lexical = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check", "--style-sample"])
        .arg(&substituted)
        .arg(&unchanged)
        .arg(&substituted)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let lexical = String::from_utf8(lexical).expect("utf-8");
    assert!(lexical.contains("\"status\": \"abstained\""));
    assert!(lexical.contains("\"reason\": \"semantic_uncertain\""));
    assert!(lexical.contains("style-feature-distance-v1"));
    assert!(!lexical.contains("bird"));
}

#[test]
fn abstention_reports_span_location_without_protected_text() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    let marker = "zzMARKER9f3k";
    fs::write(&source, format!("Keep {marker} stable.\n")).expect("write source");
    fs::write(&candidate, "Keep other stable.\n").expect("write candidate");

    let text = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "text", "check", "--protect", marker])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("run check");
    let stdout = String::from_utf8(text.stdout).expect("stdout");
    assert!(text.status.success());
    assert!(stdout.contains("line 1 col 6: declared_term"));
    assert!(!stdout.contains(marker));

    let json = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["--format", "json", "check", "--protect", marker])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("run json check");
    let body = String::from_utf8(json.stdout).expect("json stdout");
    assert!(json.status.success());
    assert!(body.contains("\"kind\": \"declared_term\""));
    assert!(!body.contains(marker));
}

#[test]
fn text_is_the_default_and_no_color_wins_over_always() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.txt");
    let candidate = directory.path().join("candidate.txt");
    fs::write(&source, "Hello world\n").expect("source");
    fs::write(&candidate, "Hello, world!\n").expect("candidate");

    let implicit = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .args(["check"])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("default check");
    let implicit_text = String::from_utf8(implicit.stdout).expect("text");
    assert!(implicit.status.success());
    assert!(implicit_text.contains("nothing written (use -o)"));
    assert!(implicit_text.contains("kept") || implicit_text.contains("passed"));
    assert!(!implicit_text.contains("schema_version"));

    let painted = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .env_remove("NO_COLOR")
        .env("TERM", "xterm-256color")
        .args(["--color", "always", "--format", "text", "check"])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("colored check");
    let painted_text = String::from_utf8(painted.stdout).expect("text");
    assert!(painted.status.success());
    assert!(
        painted_text.contains("\u{1b}[32maccepted\u{1b}[0m"),
        "{painted_text}"
    );

    let plain = Command::cargo_bin("retonr")
        .expect("compiled CLI")
        .env("NO_COLOR", "1")
        .env("TERM", "xterm-256color")
        .args(["--color", "always", "--format", "text", "check"])
        .arg(&source)
        .arg(&candidate)
        .output()
        .expect("plain check");
    let plain_text = String::from_utf8(plain.stdout).expect("text");
    assert!(plain.status.success());
    assert!(!plain_text.contains('\u{1b}'));
    assert!(plain_text.contains("accepted"));
}
