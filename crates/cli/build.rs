fn main() {
    println!("cargo:rerun-if-env-changed=RETONR_COMMIT");
    println!("cargo:rerun-if-env-changed=RETONR_SNAPSHOT_TAG");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    let commit = captured("RETONR_COMMIT", &["rev-parse", "HEAD"], "unknown");
    let snapshot = captured(
        "RETONR_SNAPSHOT_TAG",
        &["describe", "--tags", "--exact-match"],
        "none",
    );
    println!("cargo:rustc-env=RETONR_COMMIT={commit}");
    println!("cargo:rustc-env=RETONR_SNAPSHOT_TAG={snapshot}");
}

fn captured(env_key: &str, args: &[&str], fallback: &str) -> String {
    if let Ok(value) = std::env::var(env_key) {
        let sanitized = sanitize(&value);
        if !sanitized.is_empty() {
            return sanitized;
        }
    }
    let Ok(output) = std::process::Command::new("git").args(args).output() else {
        return fallback.to_owned();
    };
    if !output.status.success() {
        return fallback.to_owned();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let sanitized = sanitize(text.trim());
    if sanitized.is_empty() {
        fallback.to_owned()
    } else {
        sanitized
    }
}

fn sanitize(value: &str) -> String {
    let mut out = String::new();
    for character in value.chars() {
        if !character.is_ascii_graphic() || character == '"' || character == '\'' {
            continue;
        }
        let next = character.len_utf8();
        if out.len() + next > 80 {
            break;
        }
        out.push(character);
    }
    out
}
