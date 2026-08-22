mod adversarial;
mod support;

use std::fs;

use rewrite_model::{RuntimePackageLoadPolicy, RuntimePackageMemberRole};
use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::ReviewedOllamaRuntimeSource;
use crate::{ArtifactRepository, ArtifactSetImportDisposition, PackageManifestWriteDisposition};

use support::{import_limits, write_runtime_fixture};

#[test]
fn imports_exact_reviewed_runtime_and_reads_it_back() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let result = repository
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect("exact reviewed runtime imports");
    assert_imported_runtime(&data, &result);
    let second = repository
        .import_reviewed_ollama_runtime(
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
        second.runtime_package_disposition,
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

fn assert_imported_runtime(data: &std::path::Path, result: &crate::OllamaRuntimeImportResult) {
    assert_eq!(
        result.artifact_set_disposition,
        ArtifactSetImportDisposition::Imported
    );
    assert_eq!(
        result.runtime_package_disposition,
        PackageManifestWriteDisposition::Inserted
    );
    assert_eq!(result.evidence.artifact_set().members().len(), 5);
    assert_eq!(
        result.artifact_set_key.artifact_set_id(),
        &result.evidence.artifact_set().artifact_set_id()
    );
    assert_eq!(
        result
            .evidence
            .runtime_package()
            .entrypoint()
            .relative_path()
            .as_str(),
        "bin/ollama"
    );
    let helper = result
        .evidence
        .runtime_package()
        .members()
        .iter()
        .find(|member| member.relative_path().as_str() == "helper/retonr-isolation")
        .expect("isolation helper");
    assert_eq!(
        helper.roles(),
        &[RuntimePackageMemberRole::HelperExecutable]
    );
    assert_eq!(
        helper.load_policy(),
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded
    );
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
            "bin/ollama",
            "helper/retonr-isolation",
            "legal/license.txt",
            "lib/ollama/libggml-cpu.so",
            "provenance/source.txt",
        ]
    );
    assert!(paths.iter().all(|path| set_root.join(path).is_file()));
    let store = ArtifactStateStore::open_existing_read_only(&data.join("artifact-state.sqlite3"))
        .expect("read repository state");
    assert_eq!(
        store
            .runtime_package_manifest(&result.runtime_package_manifest_id())
            .expect("read runtime package"),
        Some(result.evidence.runtime_package().clone())
    );
    assert!(
        store
            .artifact_inventory(1)
            .expect("legacy artifact inventory")
            .is_empty()
    );
}

#[test]
fn source_constructor_requires_absolute_paths() {
    let root = tempdir().expect("temporary root");
    let layout = root.path().join("review/runtime-layout.json");
    fs::create_dir(root.path().join("review")).expect("create layout parent");
    let members = root.path().join("package");
    fs::write(&layout, b"{}").expect("write layout");
    fs::create_dir(&members).expect("create members");
    let source = ReviewedOllamaRuntimeSource::new(&layout, &members).expect("absolute source");
    assert!(source.layout_path().is_absolute());
    assert!(source.member_root().is_absolute());
    let nested_layout = members.join("runtime-layout.json");
    fs::write(&nested_layout, b"{}").expect("write nested layout");
    assert!(
        ReviewedOllamaRuntimeSource::new(&nested_layout, &members).is_err(),
        "layout inside the member tree is unsafe"
    );
}

#[test]
fn limits_and_invalid_layout_fail_before_repository_initialization() {
    let fixture_root = tempdir().expect("temporary fixture root");
    let fixture = write_runtime_fixture(fixture_root.path());
    let data = fixture_root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");

    let mut limits = import_limits();
    limits.reconstruction.layout_bytes = 1;
    let limited = repository
        .import_reviewed_ollama_runtime(&fixture.selection, limits, &CancellationToken::new())
        .expect_err("layout ceiling must be enforced");
    assert_eq!(
        limited.kind(),
        crate::ArtifactRepositoryErrorKind::ResourceLimit
    );
    assert!(!data.exists());

    let missing_source = ReviewedOllamaRuntimeSource::new(
        fixture_root.path().join("missing-layout.json"),
        fixture_root.path().join("missing-members"),
    )
    .expect("missing source selection");
    let mut relaxed_limits = import_limits();
    relaxed_limits.reconstruction.layout_bytes = usize::MAX;
    let relaxed = repository
        .import_reviewed_ollama_runtime(&missing_source, relaxed_limits, &CancellationToken::new())
        .expect_err("relaxed layout ceiling must fail before source access");
    assert_eq!(
        relaxed.kind(),
        crate::ArtifactRepositoryErrorKind::ResourceLimit
    );
    assert!(!data.exists());

    let invalid = crate::OllamaRuntimeImportLimits {
        artifact_set: crate::ArtifactSetImportLimits {
            maximum_members: 0,
            ..import_limits().artifact_set
        },
        ..import_limits()
    };
    let invalid = repository
        .import_reviewed_ollama_runtime(&fixture.selection, invalid, &CancellationToken::new())
        .expect_err("set limits must be validated before repository mutation");
    assert_eq!(
        invalid.kind(),
        crate::ArtifactRepositoryErrorKind::InvalidInput
    );
    assert!(!data.exists());

    fs::write(&fixture.layout_path, b"{").expect("truncate layout");
    let malformed = repository
        .import_reviewed_ollama_runtime(
            &fixture.selection,
            import_limits(),
            &CancellationToken::new(),
        )
        .expect_err("malformed layout cannot import");
    assert_eq!(
        malformed.kind(),
        crate::ArtifactRepositoryErrorKind::Conflict
    );
    assert!(!data.exists());
}
