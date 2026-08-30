use std::{fs, fs::File};

use rewrite_types::Digest;

use super::*;

struct Source(Vec<(String, Digest, u64, File)>);

impl RetainedRuntimeInputSource for Source {
    fn transfer(self, sink: &mut RetainedRuntimeInputSink<'_>) -> IsolationResult<()> {
        for (alias, digest, bytes, file) in self.0 {
            sink.retain(alias, digest, bytes, file)?;
        }
        Ok(())
    }
}

fn member(alias: &str, bytes: &[u8]) -> (String, Digest, u64, File) {
    let directory = tempfile::tempdir().expect("temporary input root");
    let path = directory.path().join("member");
    fs::write(&path, bytes).expect("write input");
    let file = File::open(path).expect("open read-only input");
    let digest = Digest::sha256(bytes);
    (alias.to_owned(), digest, bytes.len() as u64, file)
}

#[test]
fn opaque_tree_retains_ordered_members_and_redacts_debug() {
    let cancellation = CancellationToken::new();
    let tree = RetainedRuntimeInputTree::from_source(
        Source(vec![
            member("config/ollama-config.json", b"config"),
            member("model/model.gguf", b"weights"),
        ]),
        &cancellation,
    )
    .expect("valid tree");
    assert_eq!(tree.member_count(), 2);
    assert_eq!(tree.total_bytes(), 13);
    let debug = format!("{tree:?}");
    assert!(!debug.contains("ollama-config"));
    assert!(!debug.contains("weights"));
}

#[test]
fn source_rejects_empty_duplicate_reordered_and_noncanonical_aliases() {
    let cancellation = CancellationToken::new();
    assert!(RetainedRuntimeInputTree::from_source(Source(Vec::new()), &cancellation).is_err());
    for aliases in [
        vec!["model/a", "model/a"],
        vec!["model/z", "model/a"],
        vec!["/model/a"],
        vec!["model/../a"],
        vec!["Model/a"],
        vec!["model\\a"],
        vec!["model//a"],
    ] {
        let members = aliases
            .into_iter()
            .map(|alias| member(alias, b"x"))
            .collect();
        assert_eq!(
            RetainedRuntimeInputTree::from_source(Source(members), &cancellation)
                .expect_err("invalid alias must fail"),
            IsolationError::RuntimeInputObjectMismatch
        );
    }
}

#[test]
fn source_rejects_size_and_object_shape_drift() {
    let cancellation = CancellationToken::new();
    let (alias, digest, _bytes, file) = member("model/model.gguf", b"weights");
    assert_eq!(
        RetainedRuntimeInputTree::from_source(
            Source(vec![(alias, digest, 1, file)]),
            &cancellation
        )
        .expect_err("wrong size must fail"),
        IsolationError::RuntimeInputObjectMismatch
    );
    #[cfg(target_os = "linux")]
    {
        let directory = tempfile::tempdir().expect("temporary directory");
        let directory_file = File::open(directory.path()).expect("open directory");
        assert_eq!(
            RetainedRuntimeInputTree::from_source(
                Source(vec![(
                    "model/model.gguf".to_owned(),
                    Digest::sha256(b"x"),
                    1,
                    directory_file,
                )]),
                &cancellation,
            )
            .expect_err("directory must fail"),
            IsolationError::RuntimeInputObjectMismatch
        );
    }
}

#[test]
fn source_rejects_wrong_expected_digest() {
    let cancellation = CancellationToken::new();
    let (alias, _digest, bytes, file) = member("model/model.gguf", b"weights");
    assert_eq!(
        RetainedRuntimeInputTree::from_source(
            Source(vec![(alias, Digest::sha256(b"different"), bytes, file)]),
            &cancellation,
        )
        .expect_err("wrong digest must fail"),
        IsolationError::RuntimeInputObjectMismatch
    );
}

#[cfg(any(unix, target_os = "windows"))]
#[test]
fn multi_chunk_hash_observes_cancellation_between_positioned_reads() {
    let bytes = vec![0x5a; 128 * 1024];
    let (_alias, _digest, expected_bytes, file) = member("model/model.gguf", &bytes);
    let cancellation = CancellationToken::new();
    let result =
        runtime_input_file_digest_inner(&file, expected_bytes, Some(&cancellation), |offset| {
            if offset == 64 * 1024 {
                cancellation.cancel();
            }
        });
    assert_eq!(result, Err(IsolationError::Cancelled));
}

#[cfg(any(unix, target_os = "windows"))]
#[test]
fn final_hash_chunk_observes_cancellation_before_acceptance() {
    let bytes = vec![0x5a; 64 * 1024];
    let (_alias, _digest, expected_bytes, file) = member("model/model.gguf", &bytes);
    let cancellation = CancellationToken::new();
    let result =
        runtime_input_file_digest_inner(&file, expected_bytes, Some(&cancellation), |offset| {
            if offset == expected_bytes {
                cancellation.cancel();
            }
        });
    assert_eq!(result, Err(IsolationError::Cancelled));
}

#[test]
fn layout_digest_changes_with_alias_content_size_and_object() {
    let cancellation = CancellationToken::new();
    let first =
        RetainedRuntimeInputTree::from_source(Source(vec![member("model/a", b"x")]), &cancellation)
            .expect("first");
    let second =
        RetainedRuntimeInputTree::from_source(Source(vec![member("model/b", b"x")]), &cancellation)
            .expect("second");
    let third = RetainedRuntimeInputTree::from_source(
        Source(vec![member("model/a", b"xx")]),
        &cancellation,
    )
    .expect("third");
    assert_ne!(first.redacted_digest(), second.redacted_digest());
    assert_ne!(first.redacted_digest(), third.redacted_digest());
}
