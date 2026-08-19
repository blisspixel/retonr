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
