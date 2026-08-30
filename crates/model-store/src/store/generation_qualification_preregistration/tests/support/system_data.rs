use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, EffectivePackageMemberEvidenceV2,
    EffectivePackageMemberUseV2, EmbeddedModelComponent, EmbeddedModelComponentPurpose,
    PackageSource, PackageSourceKind,
};
use rewrite_types::Digest;

pub(super) fn member_evidence(
    path_value: &str,
    artifact_label: &str,
    bytes: u64,
    member_use: EffectivePackageMemberUseV2,
) -> EffectivePackageMemberEvidenceV2 {
    EffectivePackageMemberEvidenceV2::new(
        path(path_value),
        artifact(artifact_label),
        bytes,
        member_use,
    )
    .expect("member evidence")
}

pub(super) fn embedded_component(
    purpose: EmbeddedModelComponentPurpose,
    selector: &str,
    label: &str,
) -> EmbeddedModelComponent {
    EmbeddedModelComponent::new(
        path("model/model.gguf"),
        purpose,
        "gguf-metadata",
        1,
        selector,
        digest(label),
    )
    .expect("embedded component")
}

pub(super) fn source(label: &str) -> PackageSource {
    PackageSource::new(
        PackageSourceKind::LocalArchive,
        format!("{label}-archive"),
        "sha256-fixture",
        digest(&format!("{label} provenance")),
    )
    .expect("source")
}

pub(super) fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("path")
}

fn artifact(label: &str) -> ArtifactId {
    ArtifactId::from_digest(digest(label))
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
