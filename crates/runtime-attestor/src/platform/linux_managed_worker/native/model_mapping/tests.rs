use super::mapping_evidence;
use crate::platform::linux_managed_worker::process::{ObjectIdentity, ObjectKey};
use rewrite_model::ArtifactId;
use rewrite_types::Digest;

fn identity(device: u64, inode: u64, bytes: u64) -> ObjectIdentity {
    ObjectIdentity {
        key: ObjectKey { device, inode },
        bytes,
    }
}

#[test]
fn evidence_admits_byte_identical_private_materialization_with_distinct_inode() {
    let source = identity(1, 10, 18_000_000_000);
    let mapped = identity(2, 20, source.bytes);
    assert_ne!(source.key, mapped.key);
    let artifact = ArtifactId::from_digest(Digest::sha256(b"model"));
    let evidence = mapping_evidence(
        "/tmp/retonr-managed-runtime-input-v1/blobs/sha256-model",
        &artifact,
        mapped,
        &[(1, 2)],
    )
    .expect("distinct private object evidence");
    assert_eq!(evidence.model_artifact_id(), &artifact);
    assert_eq!(evidence.mapping_region_count(), 1);
}

#[test]
fn evidence_commitment_changes_for_private_object_or_mapping_drift() {
    let artifact = ArtifactId::from_digest(Digest::sha256(b"model"));
    let expected = mapping_evidence("/private/model", &artifact, identity(2, 20, 4), &[(1, 2)])
        .expect("baseline");
    for drifted in [
        mapping_evidence("/private/model", &artifact, identity(2, 21, 4), &[(1, 2)]),
        mapping_evidence("/private/model", &artifact, identity(2, 20, 5), &[(1, 2)]),
        mapping_evidence("/private/model", &artifact, identity(2, 20, 4), &[(2, 3)]),
    ] {
        assert_ne!(drifted.expect("drift evidence"), expected);
    }
}
