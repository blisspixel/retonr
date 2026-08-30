use std::io::Write as _;

use flate2::{Compression, write::GzEncoder};
use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;

use super::super::MemberMeasurement;

pub(super) fn measurement(name: &str, bytes: &[u8]) -> MemberMeasurement {
    MemberMeasurement {
        bytes: u64::try_from(bytes.len()).expect("fixture length fits u64"),
        digest: Digest::sha256(bytes),
        path: ArtifactSetRelativePath::new(format!("{name}.tar")).expect("fixture path is valid"),
    }
}

pub(super) fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).expect("fixture gzip writes");
    encoder.finish().expect("fixture gzip finishes")
}
