//! Process-level standard-input and output-policy fixtures for `retonr check`.

use std::{fs, path::Path};

use assert_cmd::Command;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
}

fn write(path: &Path, bytes: &[u8]) -> String {
    fs::write(path, bytes).expect("write fixture");
    path.to_str().expect("UTF-8 path").to_owned()
}

/// Standard input must be preserved byte for byte, so each case supplies the
/// candidate through the stream and requires an exact accepted result.
#[test]
fn standard_input_is_read_to_end_of_file_without_trimming() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let cases: [(&str, &[u8], &[u8]); 9] = [
        ("lf", b"alpha\nbeta\n", b"alpha\nbeta\n"),
        ("crlf", b"alpha\r\nbeta\r\n", b"alpha\r\nbeta\r\n"),
        (
            "mixed_lf_crlf",
            b"alpha\nbeta\r\ngamma\n",
            b"alpha\nbeta\r\ngamma\n",
        ),
        (
            "mixed_crlf_first",
            b"alpha\r\nbeta\ngamma\r\n",
            b"alpha\r\nbeta\ngamma\r\n",
        ),
        ("crlf_no_final_newline", b"alpha\r\nbeta", b"alpha\r\nbeta"),
        ("no_final_newline", b"alpha\nbeta", b"alpha\nbeta"),
        ("blank_lines", b"alpha\n\n\nbeta\n", b"alpha\n\n\nbeta\n"),
        (
            "surrounding_whitespace",
            b"  alpha  \n\tbeta\t\n",
            b"  alpha  \n\tbeta\t\n",
        ),
        (
            "byte_order_mark",
            b"\xEF\xBB\xBFalpha\n",
            b"\xEF\xBB\xBFalpha\n",
        ),
    ];
    for (name, bytes, expected) in cases {
        let source = write(&directory.path().join(format!("{name}.txt")), bytes);
        let output = directory.path().join(format!("{name}.out"));
        let assertion = binary()
            .args([
                "check",
                &source,
                "-",
                "--output",
                output.to_str().expect("UTF-8 path"),
                "--format",
                "text",
            ])
            .write_stdin(bytes.to_vec())
            .assert()
            .success();
        let report = String::from_utf8(assertion.get_output().stdout.clone()).expect("UTF-8");
        assert!(
            report.contains("status: unchanged_no_eligible_content")
                || report.contains("status: rewritten"),
            "{name}: unexpected report {report}"
        );
        assert_eq!(
            fs::read(&output).expect("accepted bytes"),
            expected,
            "{name}: standard input was not preserved exactly"
        );
    }
}

#[test]
fn the_source_may_also_be_read_from_standard_input() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let candidate = write(&directory.path().join("candidate.txt"), b"Hello, world!\n");
    binary()
        .args(["check", "-", &candidate, "--format", "text"])
        .write_stdin(b"Hello world\n".to_vec())
        .assert()
        .success()
        .stdout(predicates::str::contains("status: rewritten"));
}

#[test]
fn both_documents_cannot_share_one_standard_input_stream() {
    binary()
        .args(["check", "-", "-"])
        .write_stdin(b"text".to_vec())
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("invalid_invocation"));
}

#[test]
fn accepted_bytes_are_written_to_a_new_file_and_never_replace_an_existing_one() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let source = write(&directory.path().join("source.txt"), b"Hello world\n");
    let candidate = write(&directory.path().join("candidate.txt"), b"Hello, world!\n");
    let output = directory.path().join("accepted.txt");
    let output_argument = output.to_str().expect("UTF-8 path").to_owned();

    binary()
        .args([
            "check",
            &source,
            &candidate,
            "--output",
            &output_argument,
            "--format",
            "text",
        ])
        .assert()
        .success();
    assert_eq!(
        fs::read(&output).expect("accepted bytes"),
        b"Hello, world!\n"
    );

    binary()
        .args([
            "check",
            &source,
            &candidate,
            "--output",
            &output_argument,
            "--format",
            "text",
        ])
        .assert()
        .code(3)
        .stderr(predicates::str::contains("already exists"));
    assert_eq!(
        fs::read(&output).expect("accepted bytes"),
        b"Hello, world!\n",
        "an existing destination must never be replaced"
    );
    assert_eq!(
        fs::read(Path::new(&source)).expect("source bytes"),
        b"Hello world\n",
        "the source must never be modified"
    );
}

/// An abstention still yields a usable document: the exact original bytes.
#[test]
fn an_abstention_writes_the_exact_original_bytes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let source = write(
        &directory.path().join("source.txt"),
        b"Version 2 costs $10\n",
    );
    let candidate = write(
        &directory.path().join("candidate.txt"),
        b"Version 3 costs $20\n",
    );
    let output = directory.path().join("accepted.txt");
    binary()
        .args([
            "check",
            &source,
            &candidate,
            "--output",
            output.to_str().expect("UTF-8 path"),
            "--format",
            "text",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("status: abstained"));
    assert_eq!(
        fs::read(&output).expect("accepted bytes"),
        b"Version 2 costs $10\n"
    );
}

/// A non-terminal standard output carries exact bytes while the report moves to
/// standard error, so one stream is never both data and diagnostics.
#[test]
fn standard_output_carries_document_bytes_and_moves_the_report_to_standard_error() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let source = write(&directory.path().join("source.txt"), b"Hello world\n");
    let candidate = write(&directory.path().join("candidate.txt"), b"Hello, world!\n");
    let assertion = binary()
        .args([
            "check", &source, &candidate, "--output", "-", "--format", "text",
        ])
        .assert()
        .success();
    let output = assertion.get_output();
    assert_eq!(output.stdout, b"Hello, world!\n");
    let report = String::from_utf8(output.stderr.clone()).expect("UTF-8");
    assert!(
        report.contains("status: rewritten"),
        "report should move to standard error, got {report}"
    );
}
