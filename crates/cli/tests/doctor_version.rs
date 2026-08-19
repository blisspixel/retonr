use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
}

#[test]
fn version_reports_local_identity_without_a_data_directory() {
    binary()
        .args(["version"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"version\""))
        .stdout(predicate::str::contains("\"product\": \"retonr\""))
        .stdout(predicate::str::contains("\"product_version\": \"0.1.0\""))
        .stdout(predicate::str::contains("\"local_only\": true"))
        .stdout(predicate::str::contains("\"cli_schema_version\": 1"))
        .stdout(predicate::str::contains("store_schema_version"));

    binary()
        .args(["--format", "text", "version"])
        .assert()
        .success()
        .stdout(predicate::str::contains("product: retonr"))
        .stdout(predicate::str::contains("local_only: true"));
}

#[test]
fn doctor_without_data_dir_is_local_and_does_not_request_a_repository() {
    binary()
        .args(["doctor"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"doctor\""))
        .stdout(predicate::str::contains("\"network\": \"denied\""))
        .stdout(predicate::str::contains("\"local_only\": true"))
        .stdout(predicate::str::contains("\"status\": \"not_requested\""));
}

#[test]
fn help_lists_completions_and_man() {
    binary()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("completions"))
        .stdout(predicate::str::contains("man"));
}

#[test]
fn completions_json_names_the_shell_and_script() {
    binary()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"completions\""))
        .stdout(predicate::str::contains("\"shell\": \"bash\""))
        .stdout(predicate::str::contains("\"script\":"));

    binary()
        .args(["--format", "text", "completions", "powershell"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("retonr"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn completions_reject_an_unknown_shell() {
    binary()
        .args(["completions", "cmd"])
        .assert()
        .failure()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"command\": \"cli\""))
        .stderr(predicate::str::contains("\"code\": \"invalid_invocation\""));
}

#[test]
fn man_writes_a_section_one_page() {
    binary()
        .args(["man"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains("\"command\": \"man\""))
        .stdout(predicate::str::contains("\"name\": \"retonr\""))
        .stdout(predicate::str::contains("\"section\": \"1\""))
        .stdout(predicate::str::contains(".TH"));

    binary()
        .args(["--format", "text", "man"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .stdout(predicate::str::contains(".TH"))
        .stdout(predicate::str::contains("retonr"))
        .stdout(predicate::str::contains("schema_version").not());
}

#[test]
fn doctor_reports_a_missing_repository_without_creating_one() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("missing-repository");
    binary()
        .arg("--data-dir")
        .arg(&data)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"not_initialized\""));
    assert!(!data.exists(), "doctor must not create a data directory");
}
