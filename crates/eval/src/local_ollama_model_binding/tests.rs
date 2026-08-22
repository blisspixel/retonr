pub(crate) mod support;

use rewrite_app::{OllamaModelImportResult, OllamaModelReference};
use rewrite_model::ModelPackageManifest;
use rewrite_ollama::OllamaInventoryEntry;
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use super::{
    LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, LocalOllamaModelBindingError,
    LocalOllamaModelBindingEvidence, LocalOllamaObservedTemplateMatch,
    bind_imported_ollama_model_to_preflight as bind_with_execution_receipt,
    validate_local_ollama_model_binding_evidence,
};
use crate::{
    LocalOllamaPreflightMode, LocalOllamaPreflightPlan, LocalOllamaPreflightReport,
    local_ollama_preflight::{
        issue_local_ollama_preflight_test_receipt, local_ollama_preflight_report,
    },
    run_local_ollama_preflight_with_receipt,
};
use support::{import_fixture, verified_preflight};

const EXPLICIT_TEMPLATE: &[u8] = b"{{ range .Messages }}{{ .Content }}{{ end }}";
const EMBEDDED_TEMPLATE: &[u8] = b"{{ .Messages }}";

fn bind_imported_ollama_model_to_preflight(
    import: &OllamaModelImportResult,
    reference: &OllamaModelReference,
    plan: &LocalOllamaPreflightPlan,
    report: &LocalOllamaPreflightReport,
) -> Result<LocalOllamaModelBindingEvidence, LocalOllamaModelBindingError> {
    let receipt = issue_local_ollama_preflight_test_receipt(plan, report)
        .expect("test preflight evidence encodes");
    bind_with_execution_receipt(import, reference, plan, report, receipt)
}

#[test]
fn binds_exact_import_inventory_and_unique_template_without_execution_claims() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (plan, report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    let evidence = bind_imported_ollama_model_to_preflight(
        &fixture.result,
        &fixture.reference,
        &plan,
        &report,
    )
    .expect("exact relationship binds");

    assert!(evidence.static_package_inventory_relationship_verified);
    assert_eq!(
        evidence.observed_template_match,
        LocalOllamaObservedTemplateMatch::ExplicitLayer
    );
    assert_eq!(evidence.inventory_digest, fixture.manifest_digest);
    assert_eq!(evidence.inventory_byte_size, fixture.inventory_size);
    assert_eq!(evidence.model_artifact_id.digest(), &fixture.model_digest);
    assert!(!evidence.model_loaded_proven);
    assert!(!evidence.model_used_proven);
    assert!(!evidence.application_handler_proven);
    assert!(!evidence.effective_identity_proven);
    assert!(!evidence.complete_model_details_reconstructed_proven);
    assert!(!evidence.qualified);
    assert!(validate_local_ollama_model_binding_evidence(&evidence));

    let mut tampered = evidence.clone();
    tampered.inventory_byte_size = tampered.inventory_byte_size.saturating_add(1);
    assert!(!validate_local_ollama_model_binding_evidence(&tampered));
}

pub(crate) fn exact_binding_fixture() -> (
    LocalOllamaPreflightPlan,
    LocalOllamaModelBindingEvidence,
    rewrite_ollama::OllamaModelBinding,
) {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (plan, report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    let evidence = bind_imported_ollama_model_to_preflight(
        &fixture.result,
        &fixture.reference,
        &plan,
        &report,
    )
    .expect("exact model binding fixture");
    let model = rewrite_ollama::OllamaModelBinding::new_with_inventory(
        fixture.reference.runtime_reference(),
        evidence.model_artifact_id.clone(),
        evidence.model_artifact_id.digest().clone(),
        evidence.inventory_digest.clone(),
    )
    .expect("distinct model and inventory identities");
    (plan, evidence, model)
}

#[tokio::test]
async fn runner_receipt_binds_exact_report_and_rejects_self_consistent_replay() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let server = MockServer::start().await;
    mount_runner_fixture(&server, &fixture).await;
    let (mut plan, _) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    plan.endpoint = server.uri();
    plan.mode = LocalOllamaPreflightMode::Observe;
    plan.models[0].expected_details = None;

    let observed = run_local_ollama_preflight_with_receipt(&plan, &CancellationToken::new())
        .await
        .expect("observe runner succeeds");
    plan.mode = LocalOllamaPreflightMode::Verify;
    plan.models[0].expected_details = Some(observed.report().observed.bindings[0].details.clone());

    let exact = run_local_ollama_preflight_with_receipt(&plan, &CancellationToken::new())
        .await
        .expect("verify runner succeeds");
    let (exact_report, exact_receipt) = exact.into_parts();
    let evidence = bind_with_execution_receipt(
        &fixture.result,
        &fixture.reference,
        &plan,
        &exact_report,
        exact_receipt,
    )
    .expect("runner-issued exact relationship binds");
    assert!(evidence.static_package_inventory_relationship_verified);

    let replay_source = run_local_ollama_preflight_with_receipt(&plan, &CancellationToken::new())
        .await
        .expect("second verify runner succeeds");
    let (replay_report, replay_receipt) = replay_source.into_parts();
    let mut synthetic_plan = plan.clone();
    synthetic_plan.plan_id = "synthetic-replay".to_owned();
    let synthetic_report = local_ollama_preflight_report(&synthetic_plan, replay_report.observed)
        .expect("synthetic plan and report are internally consistent");
    assert_eq!(
        bind_with_execution_receipt(
            &fixture.result,
            &fixture.reference,
            &synthetic_plan,
            &synthetic_report,
            replay_receipt,
        ),
        Err(LocalOllamaModelBindingError::InvalidPreflight)
    );
}

async fn mount_runner_fixture(server: &MockServer, fixture: &support::ImportedFixture) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.15"})))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [{
            "name": fixture.reference.runtime_reference(),
            "model": fixture.reference.runtime_reference(),
            "size": fixture.inventory_size,
            "digest": fixture.manifest_digest.as_str(),
        }]})))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "license": "Fixture license text\n",
            "template": std::str::from_utf8(EXPLICIT_TEMPLATE).expect("UTF-8 template"),
            "capabilities": ["completion"],
            "details": {
                "format": "gguf",
                "family": "qwen3",
                "quantization_level": "F32",
            },
            "model_info": {"fixture.context_length": 4096},
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .mount(server)
        .await;
}

#[test]
fn source_linked_qwen_inventory_size_rule_is_config_plus_layers() {
    // Ollama v0.32.15 tag b7871fc uses Manifest.Size(), which adds Config
    // and every layer. These are the exact qwen3:0.6b-q4_K_M descriptors
    // observed by the checked-in parser audit on 2026-08-21.
    let config = 490_u64;
    let layers = [522_640_096_u64, 1_723, 11_338, 120];
    let size = layers
        .into_iter()
        .try_fold(config, u64::checked_add)
        .expect("known descriptors fit");
    assert_eq!(LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, "0.32.15");
    assert_eq!(size, 522_653_767);
    assert_eq!(
        Digest::from_sha256_hex("7df6b6e09427a769808717c0a93cadc4ae99ed4eb8bf5ca557c90846becea435")
            .expect("known manifest digest")
            .as_str(),
        "7df6b6e09427a769808717c0a93cadc4ae99ed4eb8bf5ca557c90846becea435"
    );
}

#[test]
fn same_tag_with_different_manifest_fails_closed() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let other_manifest = Digest::sha256(b"different raw manifest");
    let (plan, report) = verified_preflight(
        &fixture,
        other_manifest,
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &plan,
            &report
        ),
        Err(LocalOllamaModelBindingError::InventoryMismatch)
    );
}

#[test]
fn same_model_blob_with_different_manifest_fails_closed() {
    let imported = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let changed = import_fixture(0.7, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    assert_eq!(imported.model_digest, changed.model_digest);
    assert_ne!(imported.manifest_digest, changed.manifest_digest);
    let (plan, report) = verified_preflight(
        &changed,
        changed.manifest_digest.clone(),
        changed.inventory_size,
        changed.explicit_template_digest.clone(),
    );
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &imported.result,
            &imported.reference,
            &plan,
            &report
        ),
        Err(LocalOllamaModelBindingError::InventoryMismatch)
    );
}

#[test]
fn model_blob_cannot_substitute_for_raw_manifest_provenance() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (plan, report) = verified_preflight(
        &fixture,
        fixture.model_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &plan,
            &report
        ),
        Err(LocalOllamaModelBindingError::InventoryMismatch)
    );
}

#[test]
fn wrong_package_source_provenance_member_fails_static_validation() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let artifact_set = fixture.result.evidence.artifact_set();
    let package = fixture.result.evidence.model_package();
    let mut value = serde_json::to_value(package).expect("package serializes");
    value["source"]["provenance_digest"] =
        serde_json::Value::String(fixture.model_digest.as_str().to_owned());
    value["source"]["revision"] =
        serde_json::Value::String(format!("sha256:{}", fixture.model_digest.as_str()));
    let wrong = ModelPackageManifest::from_json_bytes(
        &serde_json::to_vec(&value).expect("wrong package encodes"),
        artifact_set,
    )
    .expect("base package contract does not infer Ollama provenance role");

    assert!(matches!(
        super::validation::validate_static_package(artifact_set, &wrong, &fixture.reference),
        Err(LocalOllamaModelBindingError::InvalidImport)
    ));
}

#[test]
fn inventory_size_and_digest_drift_fail_independently() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (size_plan, size_report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size + 1,
        fixture.explicit_template_digest.clone(),
    );
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &size_plan,
            &size_report
        ),
        Err(LocalOllamaModelBindingError::InventoryMismatch)
    );

    let (digest_plan, digest_report) = verified_preflight(
        &fixture,
        Digest::sha256(b"digest drift"),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &digest_plan,
            &digest_report
        ),
        Err(LocalOllamaModelBindingError::InventoryMismatch)
    );
}

#[test]
fn two_matching_prompt_template_candidates_are_ambiguous() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EXPLICIT_TEMPLATE);
    assert_eq!(
        fixture.explicit_template_digest,
        fixture.embedded_template_digest
    );
    let (plan, report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &plan,
            &report
        ),
        Err(LocalOllamaModelBindingError::AmbiguousTemplate)
    );
}

#[test]
fn reordered_inventory_and_plan_digest_drift_fail_closed() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (plan, mut reordered) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    reordered.observed.inventory.push(OllamaInventoryEntry {
        reference: "aaa:latest".to_owned(),
        inventory_digest: Digest::sha256(b"aaa"),
        byte_size: 1,
    });
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &plan,
            &reordered
        ),
        Err(LocalOllamaModelBindingError::InvalidPreflight)
    );

    let (mut changed_plan, report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    changed_plan.plan_id = "changed".to_owned();
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &changed_plan,
            &report
        ),
        Err(LocalOllamaModelBindingError::InvalidPreflight)
    );
}

#[test]
fn wrong_reference_version_license_and_template_fail_closed() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (plan, report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    let other_reference =
        OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "other")
            .expect("other reference");
    assert_eq!(
        bind_imported_ollama_model_to_preflight(&fixture.result, &other_reference, &plan, &report),
        Err(LocalOllamaModelBindingError::ReferenceMismatch)
    );

    let mut wrong_version = report.clone();
    wrong_version.observed.runtime.version = "0.32.14".to_owned();
    assert_eq!(
        bind_imported_ollama_model_to_preflight(
            &fixture.result,
            &fixture.reference,
            &plan,
            &wrong_version
        ),
        Err(LocalOllamaModelBindingError::InvalidPreflight)
    );

    for change in [
        Digest::sha256(b"wrong license"),
        Digest::sha256(b"wrong template"),
    ] {
        let (mut changed_plan, mut changed_report) = verified_preflight(
            &fixture,
            fixture.manifest_digest.clone(),
            fixture.inventory_size,
            fixture.explicit_template_digest.clone(),
        );
        if change == Digest::sha256(b"wrong license") {
            changed_plan.models[0]
                .expected_details
                .as_mut()
                .expect("details")
                .license_digest = change.clone();
            changed_report.observed.bindings[0].details.license_digest = change;
        } else {
            changed_plan.models[0]
                .expected_details
                .as_mut()
                .expect("details")
                .template_digest = change.clone();
            changed_report.observed.bindings[0].details.template_digest = change;
        }
        changed_report = crate::local_ollama_preflight::local_ollama_preflight_report(
            &changed_plan,
            changed_report.observed,
        )
        .expect("internally coherent changed report");
        assert_eq!(
            bind_imported_ollama_model_to_preflight(
                &fixture.result,
                &fixture.reference,
                &changed_plan,
                &changed_report
            ),
            Err(LocalOllamaModelBindingError::DetailsMismatch)
        );
    }
}

#[test]
fn serialized_evidence_redacts_reference_and_package_strings() {
    let fixture = import_fixture(0.2, EXPLICIT_TEMPLATE, EMBEDDED_TEMPLATE);
    let (plan, report) = verified_preflight(
        &fixture,
        fixture.manifest_digest.clone(),
        fixture.inventory_size,
        fixture.explicit_template_digest.clone(),
    );
    let evidence = bind_imported_ollama_model_to_preflight(
        &fixture.result,
        &fixture.reference,
        &plan,
        &report,
    )
    .expect("exact relationship binds");
    let encoded = serde_json::to_string(&evidence).expect("evidence serializes");
    for secret in [
        "qwen3:latest",
        "registry.ollama.ai",
        "library",
        "Fixture license text",
        "Messages",
        MODEL_PATH_FOR_REDACTION,
    ] {
        assert!(!encoded.contains(secret), "leaked {secret:?}");
    }
}

const MODEL_PATH_FOR_REDACTION: &str = "model/model.gguf";
