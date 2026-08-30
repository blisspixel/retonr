use rewrite_model::ArtifactId;
use rewrite_types::Digest;

use super::model_path;

#[test]
fn private_model_path_is_content_derived_under_the_fixed_root() {
    let artifact = ArtifactId::from_digest(Digest::sha256(b"weight"));
    let path = model_path(&artifact);
    assert_eq!(
        path,
        format!(
            "/tmp/retonr-managed-runtime-input-v1/blobs/sha256-{}",
            artifact.digest().as_str()
        )
    );
    assert!(!path.contains("weight"));
}
