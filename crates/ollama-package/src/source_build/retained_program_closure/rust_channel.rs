use toml::{Table, Value};

use super::RetainedProgramUpstreamClosureError;

const MAXIMUM_CHANNEL_BYTES: usize = 1024 * 1024;
const TARGET: &str = "x86_64-unknown-linux-musl";
const RUST_COMMIT: &str = "8bab26f4f68e0e26f0bb7960be334d5b520ea452";

struct ExpectedPackage {
    name: &'static str,
    version: &'static str,
    commit: &'static str,
    url: &'static str,
    hash: &'static str,
}

const CARGO: ExpectedPackage = ExpectedPackage {
    name: "cargo",
    version: "0.98.0 (c980f4866 2026-06-30)",
    commit: RUST_COMMIT,
    url: "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
    hash: "d0aeadfea55964a8866014efbe08bcfd6225d68529d522a6eef300d4f8d5c9d2",
};
const RUST_STD: ExpectedPackage = ExpectedPackage {
    name: "rust-std",
    version: "1.97.1 (8bab26f4f 2026-07-14)",
    commit: RUST_COMMIT,
    url: "https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
    hash: "d160dfc81d21fdc72534859fae249fecf6ab70640375f64fc4e77005a48c18d0",
};
const RUSTC: ExpectedPackage = ExpectedPackage {
    name: "rustc",
    version: "1.97.1 (8bab26f4f 2026-07-14)",
    commit: RUST_COMMIT,
    url: "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
    hash: "33a15df85ab0faf63b4c75b1113e47b41fd745a73bdc898c41034e9a9257b154",
};

pub(super) fn verify_rust_channel(bytes: &[u8]) -> Result<(), RetainedProgramUpstreamClosureError> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_CHANNEL_BYTES || bytes.contains(&0) {
        return Err(RetainedProgramUpstreamClosureError::InvalidRustChannel);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidRustChannel)?;
    let root = toml::from_str::<Table>(text)
        .map_err(|_| RetainedProgramUpstreamClosureError::InvalidRustChannel)?;
    if exact_string(&root, "manifest-version") != Some("2")
        || exact_string(&root, "date") != Some("2026-07-16")
    {
        return Err(RetainedProgramUpstreamClosureError::InvalidRustChannel);
    }
    let packages = table(&root, "pkg")?;
    verify_package(packages, &CARGO)?;
    verify_package(packages, &RUSTC)?;
    verify_package(packages, &RUST_STD)?;
    Ok(())
}

fn verify_package(
    packages: &Table,
    expected: &ExpectedPackage,
) -> Result<(), RetainedProgramUpstreamClosureError> {
    let package = table(packages, expected.name)?;
    if exact_string(package, "version") != Some(expected.version)
        || exact_string(package, "git_commit_hash") != Some(expected.commit)
    {
        return Err(RetainedProgramUpstreamClosureError::InvalidRustChannel);
    }
    let targets = table(package, "target")?;
    let target = table(targets, TARGET)?;
    if target.get("available").and_then(Value::as_bool) != Some(true)
        || exact_string(target, "url") != Some(expected.url)
        || exact_string(target, "hash") != Some(expected.hash)
    {
        return Err(RetainedProgramUpstreamClosureError::InvalidRustChannel);
    }
    Ok(())
}

fn table<'a>(
    parent: &'a Table,
    key: &str,
) -> Result<&'a Table, RetainedProgramUpstreamClosureError> {
    parent
        .get(key)
        .and_then(Value::as_table)
        .ok_or(RetainedProgramUpstreamClosureError::InvalidRustChannel)
}

fn exact_string<'a>(table: &'a Table, key: &str) -> Option<&'a str> {
    table.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{CARGO, ExpectedPackage, RUST_STD, RUSTC, TARGET, verify_rust_channel};

    fn channel() -> String {
        let mut text = "manifest-version = \"2\"\ndate = \"2026-07-16\"\n".to_owned();
        for expected in [&CARGO, &RUST_STD, &RUSTC] {
            text.push_str(&package(expected));
        }
        text
    }

    fn package(expected: &ExpectedPackage) -> String {
        format!(
            "\n[pkg.{name}]\nversion = \"{version}\"\ngit_commit_hash = \"{commit}\"\n\n[pkg.{name}.target.{TARGET}]\navailable = true\nurl = \"{url}\"\nhash = \"{hash}\"\n",
            name = expected.name,
            version = expected.version,
            commit = expected.commit,
            url = expected.url,
            hash = expected.hash,
        )
    }

    #[test]
    fn accepts_exact_required_release_targets() {
        assert!(verify_rust_channel(channel().as_bytes()).is_ok());
    }

    #[test]
    fn rejects_release_and_target_drift() {
        let canonical = channel();
        for changed in [
            canonical.replace("2026-07-16", "2026-07-17"),
            canonical.replace("1.97.1", "1.97.2"),
            canonical.replace("available = true", "available = false"),
            canonical.replace(CARGO.commit, "0000000000000000000000000000000000000000"),
            canonical.replace(CARGO.url, "https://example.invalid/cargo.tar.gz"),
            canonical.replace(CARGO.hash, RUSTC.hash),
            canonical.replace("\nurl =", "\nxz_url ="),
            canonical.replace("\nhash =", "\nxz_hash ="),
            canonical.replace(&package(&RUST_STD), ""),
        ] {
            assert!(verify_rust_channel(changed.as_bytes()).is_err());
        }
    }

    #[test]
    fn rejects_duplicate_keys_and_resource_excess() {
        let duplicate = channel().replacen(
            "manifest-version = \"2\"",
            "manifest-version = \"2\"\nmanifest-version = \"2\"",
            1,
        );
        assert!(verify_rust_channel(duplicate.as_bytes()).is_err());
        assert!(verify_rust_channel(&vec![b' '; 1024 * 1024 + 1]).is_err());
        assert!(verify_rust_channel(b"").is_err());
        assert!(verify_rust_channel(b"\0").is_err());
        assert!(verify_rust_channel(&[0xff]).is_err());
    }
}
