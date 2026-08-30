use std::{fs::File, os::unix::fs::PermissionsExt as _, time::Instant};

use tempfile::tempdir;

use super::*;

#[test]
fn aggregate_sparse_bytes_are_rejected_before_file_hashing() {
    let root = tempdir().expect("temporary output");
    let source = RuntimeSourceBuildOutputSource::new(root.path()).expect("output source");
    let cancellation = CancellationToken::new();
    let mut pinned =
        PinnedRuntimeSourceBuildOutput::open(&source, &cancellation).expect("pin output");
    let sparse_bytes = MAXIMUM_CONTROLLED_BUILD_OUTPUT_BYTES / 2 + 1;
    for name in ["first.bin", "second.bin"] {
        File::create(root.path().join(name))
            .expect("create sparse member")
            .set_len(sparse_bytes)
            .expect("size sparse member");
    }
    let expected = ControlledBuildOutputTree::compile(vec![
        ControlledBuildOutputTreeEntry::regular_file("first.bin", 1, Digest::sha256(b"x"), 0o600)
            .expect("expected member"),
    ])
    .expect("expected tree");
    let started = Instant::now();
    assert!(matches!(
        pinned.seal(&expected, &cancellation),
        Err(RuntimeSourceBuildExecutionError::OutputByteLimitExceeded)
    ));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

#[test]
fn special_file_mode_is_rejected_before_output_sealing() {
    let root = tempdir().expect("temporary output");
    let source = RuntimeSourceBuildOutputSource::new(root.path()).expect("output source");
    let cancellation = CancellationToken::new();
    let mut pinned =
        PinnedRuntimeSourceBuildOutput::open(&source, &cancellation).expect("pin output");
    let path = root.path().join("artifact");
    fs::write(&path, b"artifact").expect("write output member");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o4_755)).expect("set set-user-ID mode");
    let expected = ControlledBuildOutputTree::compile(vec![
        ControlledBuildOutputTreeEntry::regular_file(
            "artifact",
            8,
            Digest::sha256(b"artifact"),
            0o755,
        )
        .expect("expected member"),
    ])
    .expect("expected tree");
    assert!(matches!(
        pinned.seal(&expected, &cancellation),
        Err(RuntimeSourceBuildExecutionError::UnsafeOutput)
    ));
}

#[test]
fn special_directory_mode_is_rejected_before_output_sealing() {
    let root = tempdir().expect("temporary output");
    let source = RuntimeSourceBuildOutputSource::new(root.path()).expect("output source");
    let cancellation = CancellationToken::new();
    let mut pinned =
        PinnedRuntimeSourceBuildOutput::open(&source, &cancellation).expect("pin output");
    let path = root.path().join("directory");
    fs::create_dir(&path).expect("create output directory");
    fs::write(path.join("artifact"), b"artifact").expect("write nested output member");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o1_755)).expect("set sticky mode");
    let expected = ControlledBuildOutputTree::compile(vec![
        ControlledBuildOutputTreeEntry::directory("directory", 0o755).expect("expected directory"),
        ControlledBuildOutputTreeEntry::regular_file(
            "directory/artifact",
            8,
            Digest::sha256(b"artifact"),
            0o644,
        )
        .expect("expected nested member"),
    ])
    .expect("expected tree");
    assert!(matches!(
        pinned.seal(&expected, &cancellation),
        Err(RuntimeSourceBuildExecutionError::UnsafeOutput)
    ));
}
