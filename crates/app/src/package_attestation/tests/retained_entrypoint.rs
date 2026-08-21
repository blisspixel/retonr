use std::io::{Read as _, Seek as _, SeekFrom};

use rewrite_types::CancellationToken;

use super::{attest_runtime, runtime_fixture, set_root};

#[test]
fn retained_entrypoint_clone_survives_path_replacement_and_package_recheck_fails() {
    let (directory, repository, set, package) = runtime_fixture();
    let mut lease = attest_runtime(&repository, &package);
    let mut entrypoint = lease
        .clone_entrypoint_for_launch(&CancellationToken::new())
        .expect("clone retained entrypoint");
    let root = set_root(directory.path(), &set.artifact_set_id());
    let current = root.join("bin/runtime");
    let moved = root.join("bin/runtime.replaced");
    std::fs::rename(&current, &moved).expect("move named entrypoint");
    std::fs::write(&current, b"substitute!").expect("write replacement entrypoint");

    entrypoint
        .seek(SeekFrom::Start(0))
        .expect("rewind retained entrypoint");
    let mut bytes = Vec::new();
    entrypoint
        .read_to_end(&mut bytes)
        .expect("read retained entrypoint");
    assert_eq!(bytes, b"runtime-v1");
    assert!(lease.revalidate(&CancellationToken::new()).is_err());
}
