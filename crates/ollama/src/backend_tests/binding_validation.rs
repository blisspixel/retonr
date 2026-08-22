use rewrite_model::ArtifactId;
use rewrite_types::Digest;

use super::binding;
use crate::{OllamaBackend, OllamaEndpoint, OllamaLimits, OllamaModelBinding};

#[test]
fn rejects_duplicate_runtime_inventory_identities() {
    let first = binding();
    let alias_artifact = Digest::sha256(b"alias immutable model bytes");
    let alias = OllamaModelBinding::new_with_inventory(
        "fixture:alias",
        ArtifactId::from_digest(alias_artifact.clone()),
        alias_artifact,
        first.inventory_digest().clone(),
    )
    .expect("structurally valid alias binding");
    let error = OllamaBackend::new(
        OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint"),
        vec![first, alias],
        OllamaLimits::default(),
    )
    .expect_err("one runtime inventory cannot represent two bound artifacts");

    assert_eq!(error.code, "duplicate_model_binding");
}

#[test]
fn rejects_oversized_model_binding_sets() {
    let endpoint = OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint");
    let error = OllamaBackend::new(endpoint, bindings(65), OllamaLimits::default())
        .expect_err("oversized model binding set");
    assert_eq!(error.code, "invalid_model_bindings");
}

#[test]
fn accepts_the_exact_model_binding_limit() {
    OllamaBackend::new(
        OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint"),
        bindings(64),
        OllamaLimits::default(),
    )
    .expect("exact model binding limit");
}

fn bindings(count: usize) -> Vec<OllamaModelBinding> {
    (0..count)
        .map(|index| {
            let artifact = Digest::sha256(format!("artifact-{index}").as_bytes());
            OllamaModelBinding::new_with_inventory(
                format!("fixture-{index}:latest"),
                ArtifactId::from_digest(artifact.clone()),
                artifact,
                Digest::sha256(format!("inventory-{index}").as_bytes()),
            )
            .expect("unique bounded fixture binding")
        })
        .collect()
}
