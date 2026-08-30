use std::io::Cursor;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, EmbeddedModelComponent,
    ModelPackageManifest, ModelPackageMember, ModelPackageMemberRole, PackageSource,
    PackageSourceKind, PackageTransformation,
};
use rewrite_types::Digest;

use super::{
    OllamaLocalArchiveFoundationError, verify_ollama_local_archive_foundation,
    verify_ollama_local_archive_foundation_bindings,
};
use crate::{BlobOpenError, ReconstructionError, ReconstructionLimits};

mod fixture;

use fixture::{
    Fixture, artifact_path, fixture, open_blob, rebuild_package, transformed_artifact_set,
    transformed_embedded, transformed_members,
};

#[test]
fn exact_local_archive_derives_inert_foundation_evidence() {
    let fixture = fixture(true);
    let evidence = verify(&fixture).expect("verify exact local archive");
    let license = &evidence.license_members()[0];
    let template = fixture
        .package
        .members()
        .iter()
        .find(|member| member.roles() == [ModelPackageMemberRole::PromptTemplate])
        .expect("template member");

    assert_eq!(
        evidence.artifact_set_id(),
        &fixture.artifact_set.artifact_set_id()
    );
    assert_eq!(
        evidence.model_package_manifest_id(),
        &fixture.package.model_package_manifest_id()
    );
    assert_eq!(
        evidence.package_source_id(),
        &fixture.package.source().package_source_id()
    );
    assert_eq!(
        evidence.provenance_manifest().relative_path().as_str(),
        "provenance/ollama-manifest-v2.json"
    );
    assert_eq!(
        evidence.provenance_manifest().artifact_id().digest(),
        &Digest::sha256(&fixture.raw_manifest)
    );
    assert_eq!(
        evidence.provenance_manifest().byte_size(),
        fixture.raw_manifest.len() as u64
    );
    assert_eq!(evidence.license_members().len(), 1);
    assert_eq!(license.relative_path().as_str(), "legal/license.txt");
    assert_eq!(license.artifact_id(), template.artifact_id());
    assert_eq!(license.byte_size(), template.byte_size());
    assert_eq!(evidence.descriptor_mapping_digest().as_str().len(), 64);
    assert_eq!(evidence.logical_binding_digest().as_str().len(), 64);
    assert_eq!(evidence, verify(&fixture).expect("deterministic evidence"));
    assert_eq!(
        evidence,
        verify_ollama_local_archive_foundation_bindings(
            &fixture.raw_manifest,
            &fixture.artifact_set,
            &fixture.package,
            &ReconstructionLimits::default(),
        )
        .expect("lightweight post-rehash binding verification")
    );
}

#[test]
fn source_kind_and_exact_raw_manifest_source_binding_are_required() {
    let fixture = fixture(false);
    let unsupported_source = PackageSource::new(
        PackageSourceKind::UpstreamRelease,
        fixture.package.source().locator(),
        fixture.package.source().revision(),
        fixture.package.source().provenance_digest().clone(),
    )
    .expect("valid alternate source");
    let unsupported = rebuild_package(
        &fixture,
        unsupported_source,
        fixture.package.transformation().clone(),
        &fixture.artifact_set,
        fixture.package.members().to_vec(),
        fixture.package.embedded_components().to_vec(),
    );
    assert_eq!(
        verify_package(&fixture, &fixture.artifact_set, &unsupported),
        Err(OllamaLocalArchiveFoundationError::UnsupportedSource)
    );

    let changed_source = PackageSource::new(
        PackageSourceKind::LocalArchive,
        fixture.package.source().locator(),
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        Digest::sha256(b"substituted provenance"),
    )
    .expect("valid substituted local source");
    let changed = rebuild_package(
        &fixture,
        changed_source,
        fixture.package.transformation().clone(),
        &fixture.artifact_set,
        fixture.package.members().to_vec(),
        fixture.package.embedded_components().to_vec(),
    );
    assert_eq!(
        verify_package(&fixture, &fixture.artifact_set, &changed),
        Err(OllamaLocalArchiveFoundationError::SourceBindingMismatch)
    );
}

#[test]
fn untransformed_logical_binding_is_recomputed() {
    let fixture = fixture(false);
    let substituted = rebuild_package(
        &fixture,
        fixture.package.source().clone(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"substituted binding"),
        },
        &fixture.artifact_set,
        fixture.package.members().to_vec(),
        fixture.package.embedded_components().to_vec(),
    );
    assert_eq!(
        verify_package(&fixture, &fixture.artifact_set, &substituted),
        Err(OllamaLocalArchiveFoundationError::TransformationBindingMismatch)
    );

    let transformed_set = transformed_artifact_set(&fixture);
    let transformed = rebuild_package(
        &fixture,
        fixture.package.source().clone(),
        PackageTransformation::Transformed {
            source_artifact_set_id: fixture.artifact_set.artifact_set_id(),
            tool_evidence_digest: Digest::sha256(b"tool"),
            parameters_digest: Digest::sha256(b"parameters"),
            log_digest: Digest::sha256(b"log"),
        },
        &transformed_set,
        transformed_members(&fixture),
        transformed_embedded(&fixture),
    );
    assert_eq!(
        verify_package(&fixture, &transformed_set, &transformed),
        Err(OllamaLocalArchiveFoundationError::TransformationBindingMismatch)
    );
}

#[test]
fn descriptor_substitution_and_duplicate_logical_license_path_fail_closed() {
    let fixture = fixture(true);
    let license = fixture
        .package
        .members()
        .iter()
        .find(|member| member.roles() == [ModelPackageMemberRole::LicenseText])
        .expect("license member");
    let mut artifact_members = fixture.artifact_set.members().to_vec();
    artifact_members.push(ArtifactSetMember::new(
        license.artifact_id().clone(),
        license.byte_size(),
        artifact_path("legal/license2.txt"),
    ));
    artifact_members.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let artifact_set = ArtifactSetManifest::new(artifact_members).expect("valid duplicate bytes");
    let mut members = fixture.package.members().to_vec();
    members.push(ModelPackageMember::new(
        license.artifact_id().clone(),
        license.byte_size(),
        artifact_path("legal/license2.txt"),
        vec![ModelPackageMemberRole::LicenseText],
    ));
    members.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let package = rebuild_package(
        &fixture,
        fixture.package.source().clone(),
        fixture.package.transformation().clone(),
        &artifact_set,
        members,
        fixture.package.embedded_components().to_vec(),
    );
    assert_eq!(
        verify_package(&fixture, &artifact_set, &package),
        Err(OllamaLocalArchiveFoundationError::DescriptorRelationshipMismatch)
    );

    let mut substituted_members = fixture.artifact_set.members().to_vec();
    substituted_members[0] = ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(b"substituted config")),
        substituted_members[0].byte_size(),
        substituted_members[0].relative_path().clone(),
    );
    let substituted_set = ArtifactSetManifest::new(substituted_members).expect("substituted set");
    let substituted_package_members = fixture
        .package
        .members()
        .iter()
        .zip(substituted_set.members())
        .map(|(member, artifact)| {
            ModelPackageMember::new(
                artifact.artifact_id().clone(),
                artifact.byte_size(),
                artifact.relative_path().clone(),
                member.roles().to_vec(),
            )
        })
        .collect();
    let substituted_package = rebuild_package(
        &fixture,
        fixture.package.source().clone(),
        fixture.package.transformation().clone(),
        &substituted_set,
        substituted_package_members,
        fixture.package.embedded_components().to_vec(),
    );
    assert_eq!(
        verify_package(&fixture, &substituted_set, &substituted_package),
        Err(OllamaLocalArchiveFoundationError::DescriptorRelationshipMismatch)
    );
}

#[test]
fn reconstructed_embedded_package_substitution_is_rejected() {
    let fixture = fixture(false);
    let embedded = fixture
        .package
        .embedded_components()
        .iter()
        .enumerate()
        .map(|(index, component)| {
            EmbeddedModelComponent::new(
                component.container_path().clone(),
                component.purpose(),
                component.extraction_contract_id(),
                component.extraction_contract_schema_version(),
                component.selector(),
                if index == 0 {
                    Digest::sha256(b"substituted embedded value")
                } else {
                    component.value_digest().clone()
                },
            )
            .expect("valid embedded substitution")
        })
        .collect();
    let package = rebuild_package(
        &fixture,
        fixture.package.source().clone(),
        fixture.package.transformation().clone(),
        &fixture.artifact_set,
        fixture.package.members().to_vec(),
        embedded,
    );
    assert_eq!(
        verify_package(&fixture, &fixture.artifact_set, &package),
        Err(OllamaLocalArchiveFoundationError::ModelPackageMismatch)
    );
}

#[test]
fn strict_manifest_errors_and_changed_valid_bytes_are_rejected_before_blob_use() {
    let fixture = fixture(false);
    let duplicate = br#"{"schemaVersion":2,"schemaVersion":2}"#;
    let malformed = br#"{"schemaVersion":"#;
    let trailing = [fixture.raw_manifest.as_slice(), b" false"].concat();
    for input in [
        duplicate.as_slice(),
        malformed.as_slice(),
        trailing.as_slice(),
    ] {
        let opened = std::cell::Cell::new(false);
        let result = verify_ollama_local_archive_foundation::<Cursor<Vec<u8>>, _, _>(
            input,
            &fixture.artifact_set,
            &fixture.package,
            &ReconstructionLimits::default(),
            |_digest| {
                opened.set(true);
                Err(BlobOpenError)
            },
            || false,
        );
        assert!(matches!(
            result,
            Err(OllamaLocalArchiveFoundationError::Reconstruction(
                ReconstructionError::InvalidManifest | ReconstructionError::UnsupportedManifest
            ))
        ));
        assert!(!opened.get());
    }

    let mut oversized = fixture.raw_manifest.clone();
    oversized.resize(ReconstructionLimits::default().manifest_bytes + 1, b' ');
    assert!(matches!(
        verify_raw(&fixture, &oversized),
        Err(OllamaLocalArchiveFoundationError::Reconstruction(
            ReconstructionError::ManifestTooLarge
        ))
    ));

    let mut noncanonical = Vec::with_capacity(fixture.raw_manifest.len() + 2);
    noncanonical.push(b' ');
    noncanonical.extend_from_slice(&fixture.raw_manifest);
    noncanonical.push(b' ');
    assert_eq!(
        verify_raw(&fixture, &noncanonical),
        Err(OllamaLocalArchiveFoundationError::SourceBindingMismatch)
    );
}

#[test]
fn blob_failure_and_cancellation_remain_reconstruction_failures() {
    let fixture = fixture(false);
    let unavailable = verify_ollama_local_archive_foundation::<Cursor<Vec<u8>>, _, _>(
        &fixture.raw_manifest,
        &fixture.artifact_set,
        &fixture.package,
        &ReconstructionLimits::default(),
        |_digest| Err(BlobOpenError),
        || false,
    );
    assert_eq!(
        unavailable,
        Err(OllamaLocalArchiveFoundationError::Reconstruction(
            ReconstructionError::BlobUnavailable
        ))
    );
    let cancelled = verify_ollama_local_archive_foundation(
        &fixture.raw_manifest,
        &fixture.artifact_set,
        &fixture.package,
        &ReconstructionLimits::default(),
        |digest| open_blob(&fixture.blobs, digest),
        || true,
    );
    assert_eq!(
        cancelled,
        Err(OllamaLocalArchiveFoundationError::Reconstruction(
            ReconstructionError::Cancelled
        ))
    );
}

fn verify(
    fixture: &Fixture,
) -> Result<super::OllamaLocalArchiveFoundationEvidence, OllamaLocalArchiveFoundationError> {
    verify_package(fixture, &fixture.artifact_set, &fixture.package)
}

fn verify_package(
    fixture: &Fixture,
    artifact_set: &ArtifactSetManifest,
    package: &ModelPackageManifest,
) -> Result<super::OllamaLocalArchiveFoundationEvidence, OllamaLocalArchiveFoundationError> {
    verify_ollama_local_archive_foundation(
        &fixture.raw_manifest,
        artifact_set,
        package,
        &ReconstructionLimits::default(),
        |digest| open_blob(&fixture.blobs, digest),
        || false,
    )
}

fn verify_raw(
    fixture: &Fixture,
    raw_manifest: &[u8],
) -> Result<super::OllamaLocalArchiveFoundationEvidence, OllamaLocalArchiveFoundationError> {
    verify_ollama_local_archive_foundation(
        raw_manifest,
        &fixture.artifact_set,
        &fixture.package,
        &ReconstructionLimits::default(),
        |digest| open_blob(&fixture.blobs, digest),
        || false,
    )
}
