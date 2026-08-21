mod support;

use std::io::Cursor;

use rewrite_model::{
    EmbeddedModelComponentPurpose, ModelPackageMemberRole, PackageSourceKind, PackageTransformation,
};
use rewrite_ollama_package::{
    BlobOpenError, ReconstructionError, ReconstructionLimits, reconstruct_model_package,
    reconstruct_model_package_with_limits,
};
use rewrite_types::Digest;

use support::{MetadataValue, PackageFixture, package_fixture};

fn replace_blob(fixture: &mut PackageFixture, layer_index: Option<usize>, bytes: Vec<u8>) {
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fixture.manifest).expect("fixture manifest JSON");
    let descriptor = if let Some(index) = layer_index {
        &mut manifest["layers"][index]
    } else {
        &mut manifest["config"]
    };
    let old = descriptor["digest"]
        .as_str()
        .expect("fixture descriptor digest")
        .strip_prefix("sha256:")
        .expect("fixture SHA-256 prefix")
        .to_owned();
    fixture.blobs.remove(&old);
    let digest = Digest::sha256(&bytes);
    descriptor["digest"] = serde_json::json!(format!("sha256:{}", digest.as_str()));
    descriptor["size"] = serde_json::json!(bytes.len());
    fixture.blobs.insert(digest.as_str().to_owned(), bytes);
    fixture.manifest = serde_json::to_vec(&manifest).expect("mutated manifest serializes");
}

#[test]
fn exact_fixture_reconstructs_golden_dual_prompt_package() {
    assert!(matches!(
        MetadataValue::Raw {
            kind: 0,
            bytes: Vec::new()
        },
        MetadataValue::Raw { .. }
    ));
    let fixture = package_fixture();
    let result = reconstruct_model_package(
        &fixture.manifest,
        "registry.ollama.ai/library/qwen3",
        |digest| {
            fixture
                .blobs
                .get(digest.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(BlobOpenError)
        },
        || false,
    )
    .expect("fixture reconstructs");
    assert_eq!(result.artifact_set().members().len(), 6);
    assert_eq!(result.model_package().members().len(), 6);
    assert_eq!(
        result.model_package().format_contract_id(),
        "ollama-manifest-v2"
    );
    assert_eq!(
        result.model_package().source().kind(),
        PackageSourceKind::LocalArchive
    );
    let PackageTransformation::Untransformed { evidence_digest } =
        result.model_package().transformation()
    else {
        panic!("fixture package must be untransformed");
    };
    assert_eq!(
        evidence_digest.as_str(),
        "129f014f6c09ffd26be26df81b995e1c1400c2d2ddead62360ef34b451d99543"
    );
    assert_eq!(
        result.model_package().source().provenance_digest(),
        result.plan().raw_manifest_digest()
    );
    let prompt_files = result
        .model_package()
        .members()
        .iter()
        .filter(|member| {
            member
                .roles()
                .contains(&ModelPackageMemberRole::PromptTemplate)
        })
        .count();
    let prompt_embedded = result
        .model_package()
        .embedded_components()
        .iter()
        .filter(|component| component.purpose() == EmbeddedModelComponentPurpose::PromptTemplate)
        .count();
    assert_eq!((prompt_files, prompt_embedded), (1, 1));
    assert!(result.rootfs_comparison().same_cardinality());
    assert_eq!(
        result.rootfs_comparison().matches_by_position(),
        &[true, false, true, true]
    );
    assert!(!result.rootfs_comparison().all_match());
    assert_eq!(
        result.artifact_set().artifact_set_id().digest().as_str(),
        "65d45aa96d0263beb1ce70dfe9c708973df1a117f5822efd3fe68d4958974d46"
    );
    assert_eq!(
        result
            .model_package()
            .model_package_manifest_id()
            .digest()
            .as_str(),
        "318f454b24dd5f21c83a654419d713d8e18f756e87e34469bfdf4408054653d6"
    );
}

#[test]
fn missing_short_long_and_changed_blobs_fail_closed() {
    let fixture = package_fixture();
    let model_digest = rewrite_ollama_package::parse_manifest_v2(
        &fixture.manifest,
        &ReconstructionLimits::default(),
    )
    .expect("plan")
    .model()
    .digest()
    .as_str()
    .to_owned();
    for mode in 0..4 {
        let result = reconstruct_model_package(
            &fixture.manifest,
            "registry.ollama.ai/library/qwen3",
            |digest| {
                let mut bytes = fixture
                    .blobs
                    .get(digest.as_str())
                    .cloned()
                    .ok_or(BlobOpenError)?;
                if digest.as_str() == model_digest {
                    match mode {
                        0 => return Err(BlobOpenError),
                        1 => {
                            bytes.pop();
                        }
                        2 => bytes.push(0),
                        3 => {
                            *bytes.last_mut().expect("model is nonempty") ^= 1;
                        }
                        _ => unreachable!(),
                    }
                }
                Ok(Cursor::new(bytes))
            },
            || false,
        );
        assert!(result.is_err());
    }
}

#[test]
fn configuration_parameters_text_and_locator_are_validated() {
    for (layer_index, replacement, expected) in [
        (
            None,
            b"{}".to_vec(),
            ReconstructionError::UnsupportedConfiguration,
        ),
        (Some(3), b"[".to_vec(), ReconstructionError::InvalidJson),
        (
            Some(3),
            br#"{"a":1,"a":2}"#.to_vec(),
            ReconstructionError::InvalidJson,
        ),
        (Some(1), vec![0xff], ReconstructionError::InvalidTextLayer),
        (Some(2), vec![0xff], ReconstructionError::InvalidTextLayer),
    ] {
        let mut fixture = package_fixture();
        replace_blob(&mut fixture, layer_index, replacement);
        let result = reconstruct_model_package(
            &fixture.manifest,
            "registry.ollama.ai/library/qwen3",
            |digest| {
                fixture
                    .blobs
                    .get(digest.as_str())
                    .cloned()
                    .map(Cursor::new)
                    .ok_or(BlobOpenError)
            },
            || false,
        );
        assert_eq!(result, Err(expected));
    }
    let fixture = package_fixture();
    assert_eq!(
        reconstruct_model_package(
            &fixture.manifest,
            "invalid@locator",
            |digest| fixture
                .blobs
                .get(digest.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(BlobOpenError),
            || false
        ),
        Err(ReconstructionError::ModelContract)
    );
}

#[test]
fn parameters_accept_any_unambiguous_json_value() {
    for bytes in [b"[]".as_slice(), b"null".as_slice(), b"42".as_slice()] {
        let mut fixture = package_fixture();
        replace_blob(&mut fixture, Some(3), bytes.to_vec());
        assert!(
            reconstruct_model_package(
                &fixture.manifest,
                "registry.ollama.ai/library/qwen3",
                |digest| fixture
                    .blobs
                    .get(digest.as_str())
                    .cloned()
                    .map(Cursor::new)
                    .ok_or(BlobOpenError),
                || false
            )
            .is_ok()
        );
    }
}

#[test]
fn rootfs_cardinality_drift_is_reported_without_becoming_blob_authority() {
    let mut fixture = package_fixture();
    let plan = rewrite_ollama_package::parse_manifest_v2(
        &fixture.manifest,
        &ReconstructionLimits::default(),
    )
    .expect("fixture plan");
    let config_bytes = fixture
        .blobs
        .get(plan.config().digest().as_str())
        .expect("fixture config");
    let mut config: serde_json::Value =
        serde_json::from_slice(config_bytes).expect("fixture config JSON");
    config["rootfs"]["diff_ids"]
        .as_array_mut()
        .expect("fixture diff IDs")
        .pop();
    replace_blob(
        &mut fixture,
        None,
        serde_json::to_vec(&config).expect("changed config serializes"),
    );
    let result = reconstruct_model_package(
        &fixture.manifest,
        "registry.ollama.ai/library/qwen3",
        |digest| {
            fixture
                .blobs
                .get(digest.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(BlobOpenError)
        },
        || false,
    )
    .expect("informational rootfs mismatch remains reconstructable");
    assert!(!result.rootfs_comparison().same_cardinality());
    assert_eq!(
        result.rootfs_comparison().matches_by_position(),
        &[true, false, true, false]
    );
}

#[test]
fn explicit_limits_and_cancellation_apply_before_unbounded_work() {
    let fixture = package_fixture();
    let limits = ReconstructionLimits {
        model_bytes: 1,
        ..ReconstructionLimits::default()
    };
    assert_eq!(
        reconstruct_model_package_with_limits(
            &fixture.manifest,
            "registry.ollama.ai/library/qwen3",
            &limits,
            |_digest| -> Result<Cursor<Vec<u8>>, BlobOpenError> { Err(BlobOpenError) },
            || false
        ),
        Err(ReconstructionError::InvalidDescriptor)
    );
    assert_eq!(
        reconstruct_model_package(
            &fixture.manifest,
            "registry.ollama.ai/library/qwen3",
            |_digest| -> Result<Cursor<Vec<u8>>, BlobOpenError> { Err(BlobOpenError) },
            || true
        ),
        Err(ReconstructionError::Cancelled)
    );
    let mut checks = 0_u32;
    assert_eq!(
        reconstruct_model_package(
            &fixture.manifest,
            "registry.ollama.ai/library/qwen3",
            |digest| fixture
                .blobs
                .get(digest.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(BlobOpenError),
            || {
                checks += 1;
                checks > 3
            }
        ),
        Err(ReconstructionError::Cancelled)
    );
}
