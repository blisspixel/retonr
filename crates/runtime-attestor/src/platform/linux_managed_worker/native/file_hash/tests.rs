use std::{
    io::{Seek as _, SeekFrom, Write as _},
    time::Instant,
};

use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};

use super::hash_file;
use crate::ManagedGenerationWorkerLimits;

#[test]
fn positioned_hashing_preserves_cursor_and_detects_exact_bytes() {
    let mut file = tempfile::tempfile().expect("temporary file");
    file.write_all(b"native").expect("write fixture");
    file.seek(SeekFrom::Start(3)).expect("position fixture");
    let mut budget = 6;
    let observed = hash_file(
        &file,
        6,
        &mut budget,
        ManagedGenerationWorkerLimits::default(),
        &CancellationToken::new(),
        Instant::now(),
    )
    .expect("positioned hash");
    assert_eq!(observed, ArtifactId::from_digest(Digest::sha256(b"native")));
    assert_eq!(file.stream_position().expect("cursor"), 3);
    assert_eq!(budget, 0);
}
