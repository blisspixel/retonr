use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::tempdir;

fn binary() -> Command {
    Command::cargo_bin("retonr").expect("built binary")
}

fn write_source(directory: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
    let source = directory.join(name);
    fs::write(&source, body).expect("write source");
    source
}

#[test]
fn default_cleanup_writes_a_new_file_and_leaves_the_source() {
    let directory = tempdir().expect("temporary directory");
    let body = "See https://example.com/guide?id=7&utm_source=chatgpt.com for details.\n";
    let source = write_source(directory.path(), "draft.txt", body);
    let output = directory.path().join("cleaned.txt");

    let report = binary()
        .args(["--format", "json", "links", "clean"])
        .arg(&source)
        .arg("--output")
        .arg(&output)
        .output()
        .expect("run links clean");
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let cleaned = fs::read_to_string(&output).expect("read cleaned");
    assert_eq!(cleaned, "See https://example.com/guide?id=7 for details.\n");
    assert_eq!(fs::read_to_string(&source).expect("source"), body);
    let text = String::from_utf8(report.stdout).expect("report");
    assert!(!text.contains("https://"));
    assert!(!text.contains("chatgpt.com"));
    let value: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value["command"], "links.clean");
    assert_eq!(value["result"]["wrote_output"], true);
    assert_eq!(value["result"]["removals"][0]["name"], "utm_source");
    assert_eq!(
        value["result"]["removals"][0]["rule_id"],
        "utm_source_chatgpt_com"
    );
    assert!(value["result"]["removals"][0].get("reason").is_none());

    binary()
        .args(["links", "clean"])
        .arg(&output)
        .arg("--output")
        .arg(directory.path().join("again.txt"))
        .assert()
        .success()
        .stdout(predicate::str::contains("removed 0 parameters"));
}

#[test]
fn dry_run_and_an_existing_destination_do_not_write() {
    let directory = tempdir().expect("temporary directory");
    let body = "https://example.com/guide?utm_source=chatgpt.com\n";
    let source = write_source(directory.path(), "draft.txt", body);
    let output = directory.path().join("cleaned.txt");
    fs::write(&output, "keep\n").expect("existing output");

    let preview = directory.path().join("preview.txt");
    binary()
        .args(["links", "clean"])
        .arg(&source)
        .args(["--dry-run", "--output"])
        .arg(&preview)
        .assert()
        .success()
        .stdout(predicate::str::contains("would remove 1 parameter"))
        .stdout(predicate::str::contains("https://").not());
    assert!(!preview.exists(), "dry-run must not create --output");
    assert_eq!(fs::read_to_string(&output).expect("output"), "keep\n");
    assert_eq!(fs::read_to_string(&source).expect("source"), body);

    binary()
        .args(["links", "clean"])
        .arg(&source)
        .args(["--output"])
        .arg(&output)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "output destination already exists",
        ));
    assert_eq!(fs::read_to_string(&source).expect("source"), body);
    assert_eq!(fs::read_to_string(&output).expect("output"), "keep\n");
}

#[test]
fn optional_groups_and_signed_urls_follow_the_selection() {
    let directory = tempdir().expect("temporary directory");
    let gemini = "https://example.com/guide?utm_source=gemini&id=7\n";
    let source = write_source(directory.path(), "gemini.txt", gemini);
    binary()
        .args(["links", "clean"])
        .arg(&source)
        .arg("--output")
        .arg(directory.path().join("default.txt"))
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(directory.path().join("default.txt")).expect("default"),
        gemini
    );
    binary()
        .args(["links", "clean", "--campaign"])
        .arg(&source)
        .arg("--output")
        .arg(directory.path().join("campaign.txt"))
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(directory.path().join("campaign.txt")).expect("campaign"),
        "https://example.com/guide?id=7\n"
    );

    let mail = "https://example.com/?mc_cid=campaign-fixture&mc_eid=recipient-fixture&id=7\n";
    let mail_source = write_source(directory.path(), "mail.txt", mail);
    let tracked = directory.path().join("tracked.txt");
    let report = binary()
        .args(["--format", "json", "links", "clean", "--tracking"])
        .arg(&mail_source)
        .arg("--output")
        .arg(&tracked)
        .output()
        .expect("tracking");
    assert!(report.status.success());
    assert_eq!(
        fs::read_to_string(&tracked).expect("tracked"),
        "https://example.com/?id=7\n"
    );
    let text = String::from_utf8(report.stdout).expect("report");
    assert!(!text.contains("recipient-fixture"));
    assert!(!text.contains("campaign-fixture"));

    let sized = "https://example.com/guide?source=chatgpt.com&sz=32&id=7\n";
    let sized_source = write_source(directory.path(), "sized.txt", sized);
    binary()
        .args(["links", "clean", "--sz-32"])
        .arg(&sized_source)
        .arg("--output")
        .arg(directory.path().join("sized-out.txt"))
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(directory.path().join("sized-out.txt")).expect("sz"),
        "https://example.com/guide?source=chatgpt.com&id=7\n"
    );

    let secret = "super-secret-token";
    let signed =
        format!("https://example.com/guide?id=7&utm_source=chatgpt.com&X-Amz-Signature={secret}\n");
    let signed_source = write_source(directory.path(), "signed.txt", &signed);
    let signed_out = directory.path().join("signed-out.txt");
    let signed_report = binary()
        .args(["--format", "text", "links", "clean"])
        .arg(&signed_source)
        .arg("--output")
        .arg(&signed_out)
        .output()
        .expect("signed");
    assert!(signed_report.status.success());
    assert_eq!(fs::read_to_string(&signed_out).expect("signed out"), signed);
    let signed_text = String::from_utf8(signed_report.stdout).expect("signed report");
    assert!(signed_text.contains("reason signed_query"));
    assert!(!signed_text.contains(secret));
    assert!(!signed_text.contains("https://"));
}

#[test]
fn inspect_names_parameters_without_urls_and_check_stays_strict() {
    let directory = tempdir().expect("temporary directory");
    let body = "See https://example.com/guide?id=7&utm_source=chatgpt.com for details.\n";
    let source = write_source(directory.path(), "draft.txt", body);
    let inspected = binary()
        .args(["--format", "json", "inspect"])
        .arg(&source)
        .output()
        .expect("inspect");
    assert!(inspected.status.success());
    let text = String::from_utf8(inspected.stdout).expect("inspect json");
    assert!(!text.contains("https://"));
    assert!(!text.contains("chatgpt.com"));
    let value: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value["result"]["schema_version"], 3);
    assert_eq!(value["result"]["http_links"]["http_url_count"], 1);
    assert_eq!(
        value["result"]["http_links"]["parameters"][0]["rule_id"],
        "utm_source_chatgpt_com"
    );
    assert_eq!(
        value["result"]["http_links"]["parameters"][0]["name"],
        "utm_source"
    );
    assert_eq!(fs::read_to_string(&source).expect("source"), body);

    let candidate = directory.path().join("candidate.txt");
    fs::write(
        &candidate,
        "See https://example.com/guide?id=7 for details.\n",
    )
    .expect("candidate");
    binary()
        .args(["--format", "text", "check"])
        .arg(&source)
        .arg(&candidate)
        .assert()
        .success()
        .stdout(predicate::str::contains("reason: protected_value_changed"));
    assert_eq!(fs::read_to_string(&source).expect("source"), body);

    for edited in [
        "See https://example.com/guide?utm_source=chatgpt.com&id=7 for details.\n",
        "See https://example.org/guide?id=7&utm_source=chatgpt.com for details.\n",
        "See https://example.com/guide?id=7&utm_source=chatgpt%2Ecom for details.\n",
    ] {
        fs::write(&candidate, edited).expect("candidate");
        binary()
            .args(["check"])
            .arg(&source)
            .arg(&candidate)
            .assert()
            .success()
            .stdout(predicate::str::contains("reason: protected_value_changed"));
    }
}

#[test]
fn scope_flags_and_refused_inputs_stay_local() {
    let directory = tempdir().expect("temporary directory");
    let body = "https://example.com/a?utm_source=chatgpt.com https://other.test/a?utm_source=chatgpt.com\n";
    let source = write_source(directory.path(), "draft.txt", body);
    let scoped = directory.path().join("scoped.txt");
    binary()
        .args(["links", "clean", "--host", "example.com"])
        .arg(&source)
        .args(["--output"])
        .arg(&scoped)
        .assert()
        .success()
        .stdout(predicate::str::contains("removed 1 parameter"))
        .stdout(predicate::str::contains("https://").not());
    assert_eq!(
        fs::read_to_string(&scoped).expect("scoped"),
        "https://example.com/a https://other.test/a?utm_source=chatgpt.com\n"
    );
    assert_eq!(fs::read_to_string(&source).expect("source"), body);

    let streamed = binary()
        .args(["links", "clean", "--output", "-"])
        .arg(&source)
        .output()
        .expect("stream");
    assert!(streamed.status.success());
    let document = String::from_utf8(streamed.stdout).expect("document");
    assert_eq!(document, "https://example.com/a https://other.test/a\n");
    assert!(!document.contains("catalog_version"));
    let report = String::from_utf8(streamed.stderr).expect("report");
    assert!(report.contains("removed 2 parameters"));
    assert!(!report.contains("https://"));

    binary()
        .args(["links", "clean", "--host", ""])
        .arg(&source)
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "a host or path prefix must not be empty",
        ));
    binary()
        .args(["links", "clean"])
        .arg(directory.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("links clean reads one document"));
    assert_eq!(fs::read_to_string(&source).expect("source"), body);
}
