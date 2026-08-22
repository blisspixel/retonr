mod adversarial;
mod support;

use std::fs;

use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::{
    InstalledOllamaModelSource, OllamaModelImportLimits, OllamaModelReference,
    PackageManifestWriteDisposition,
};
use crate::{ArtifactRepository, ArtifactSetImportDisposition};

use support::{import_limits, write_installed_fixture};

#[test]
fn imports_exact_six_member_package_and_reads_it_back() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let result = repository
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("exact installed model imports");

    assert_eq!(
        result.artifact_set_disposition,
        ArtifactSetImportDisposition::Imported
    );
    assert_eq!(
        result.model_package_disposition,
        PackageManifestWriteDisposition::Inserted
    );
    assert_eq!(result.evidence.artifact_set().members().len(), 6);
    assert_eq!(
        result.artifact_set_key.artifact_set_id(),
        &result.evidence.artifact_set().artifact_set_id()
    );
    assert!(!result.evidence.rootfs_comparison().all_match());

    let set_root = data.join("artifact-storage/sets").join(format!(
        "set-v1-{}",
        result.artifact_set_key.artifact_set_id().digest().as_str()
    ));
    let paths = result
        .evidence
        .artifact_set()
        .members()
        .iter()
        .map(|member| member.relative_path().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "config/ollama-config.json",
            "config/parameters.json",
            "legal/license.txt",
            "model/model.gguf",
            "prompts/template.go.tmpl",
            "provenance/ollama-manifest-v2.json",
        ]
    );
    assert!(paths.iter().all(|path| set_root.join(path).is_file()));

    let store = ArtifactStateStore::open_existing_read_only(&data.join("artifact-state.sqlite3"))
        .expect("read repository state");
    assert_eq!(
        store
            .model_package_manifest(&result.model_package_manifest_id())
            .expect("read package manifest"),
        Some(result.evidence.model_package().clone())
    );
    assert!(
        store
            .artifact_inventory(1)
            .expect("legacy artifact inventory")
            .is_empty()
    );

    let second = repository
        .import_installed_ollama_model(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("exact repeat is idempotent");
    assert_eq!(
        second.artifact_set_disposition,
        ArtifactSetImportDisposition::AlreadyPresent
    );
    assert_eq!(
        second.model_package_disposition,
        PackageManifestWriteDisposition::AlreadyPresent
    );
    assert_eq!(second.evidence, result.evidence);
    assert_eq!(second.artifact_set_key, result.artifact_set_key);
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/.set-staging"))
            .expect("read staging")
            .count(),
        0
    );
}

#[test]
fn reference_and_source_constructors_reject_path_capable_components() {
    for component in [
        "", ".", "..", "a/b", "a\\b", "Upper", "-lead", "tail-", "con",
    ] {
        assert!(
            OllamaModelReference::new(component, "library", "qwen3", "latest").is_err(),
            "registry component {component:?}"
        );
        assert!(
            OllamaModelReference::new("registry.ollama.ai", component, "qwen3", "latest").is_err(),
            "namespace component {component:?}"
        );
        assert!(
            OllamaModelReference::new("registry.ollama.ai", "library", component, "latest")
                .is_err(),
            "model component {component:?}"
        );
        assert!(
            OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", component).is_err(),
            "tag component {component:?}"
        );
    }
    let reference =
        OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "0.6b-q4_k_m")
            .expect("canonical installed reference");
    let source = InstalledOllamaModelSource::new("relative-models", reference.clone())
        .expect("absolute source selection");
    assert!(source.models_root().is_absolute());
    assert_eq!(source.reference(), &reference);
}

#[test]
fn runtime_reference_uses_exact_shortest_ollama_name() {
    let default = OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "latest")
        .expect("default reference");
    assert_eq!(default.runtime_reference(), "qwen3:latest");

    let namespace = OllamaModelReference::new("registry.ollama.ai", "example", "qwen3", "q4_k_m")
        .expect("namespace reference");
    assert_eq!(namespace.runtime_reference(), "example/qwen3:q4_k_m");

    let registry = OllamaModelReference::new("models.example.test", "example", "qwen3", "v1")
        .expect("registry reference");
    assert_eq!(
        registry.runtime_reference(),
        "models.example.test/example/qwen3:v1"
    );
}

#[test]
fn cancellation_and_parser_limits_fail_before_repository_initialization() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_installed_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let cancelled = repository
        .import_installed_ollama_model(&fixture.selection, import_limits(), &cancellation)
        .expect_err("cancelled source cannot import");
    assert_eq!(
        cancelled.kind(),
        crate::ArtifactRepositoryErrorKind::Cancelled
    );
    assert!(!data.exists());

    let mut limits = import_limits();
    limits.reconstruction.manifest_bytes = 1;
    let limited = repository
        .import_installed_ollama_model(&fixture.selection, limits, &CancellationToken::new())
        .expect_err("manifest ceiling must be enforced");
    assert_eq!(
        limited.kind(),
        crate::ArtifactRepositoryErrorKind::ResourceLimit
    );
    assert!(!data.exists());

    let missing_source = InstalledOllamaModelSource::new(
        fixture_root.path().join("missing-models"),
        OllamaModelReference::new("registry.ollama.ai", "library", "qwen3", "latest")
            .expect("valid missing-source reference"),
    )
    .expect("missing source selection");
    let mut relaxed_limits = import_limits();
    relaxed_limits.reconstruction.manifest_bytes = usize::MAX;
    let relaxed = repository
        .import_installed_ollama_model(&missing_source, relaxed_limits, &CancellationToken::new())
        .expect_err("relaxed manifest ceiling must fail before source access");
    assert_eq!(
        relaxed.kind(),
        crate::ArtifactRepositoryErrorKind::ResourceLimit
    );
    assert!(!data.exists());

    let invalid = OllamaModelImportLimits {
        artifact_set: crate::ArtifactSetImportLimits {
            maximum_members: 0,
            ..import_limits().artifact_set
        },
        ..import_limits()
    };
    let invalid = repository
        .import_installed_ollama_model(&fixture.selection, invalid, &CancellationToken::new())
        .expect_err("set limits must be validated before repository mutation");
    assert_eq!(
        invalid.kind(),
        crate::ArtifactRepositoryErrorKind::InvalidInput
    );
    assert!(!data.exists());
}
