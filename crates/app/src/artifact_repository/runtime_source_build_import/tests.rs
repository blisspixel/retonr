use std::{
    cell::Cell,
    fs::{self, File},
    path::{Path, PathBuf},
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama::{OllamaCloudDisableFeaturePolicy, OllamaManagedGenerationAdmissionPolicy};
use rewrite_ollama_package::{
    ADMITTED_RUNTIME_FAMILY, RUNTIME_LAYOUT_SCHEMA_VERSION, ReconstructedRuntimePackage,
    RuntimeLayoutLimits, reconstruct_runtime_package_with_limits,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use tempfile::{TempDir, tempdir};

use super::{PrimaryRuntimeImportSource, *};
use crate::{
    ArtifactRepositoryErrorKind, ArtifactSetImportDisposition, ArtifactSetImportLimits,
    PackageManifestWriteDisposition, RuntimeSourceBuildEvidenceBundleError,
};

struct TestPrimarySource {
    _root: TempDir,
    member_root: PathBuf,
    substitute: PathBuf,
    reconstructed: ReconstructedRuntimePackage,
    identical: bool,
    substitute_first_member: bool,
    fail_revalidation_at: Cell<Option<usize>>,
    revalidation_count: Cell<usize>,
}

impl TestPrimarySource {
    fn fail_once_at(&self, call: usize) {
        self.fail_revalidation_at.set(Some(call));
        self.revalidation_count.set(0);
    }
}

impl PrimaryRuntimeImportSource for TestPrimarySource {
    fn revalidate(&self, cancellation: &CancellationToken) -> Result<(), OllamaRuntimeImportError> {
        if cancellation.is_cancelled() {
            return Err(OllamaRuntimeImportError::Cancelled);
        }
        let call = self.revalidation_count.get().saturating_add(1);
        self.revalidation_count.set(call);
        if self.fail_revalidation_at.get() == Some(call) {
            self.fail_revalidation_at.set(None);
            return Err(OllamaRuntimeImportError::SourceBuildEvidence(
                RuntimeSourceBuildEvidenceBundleError::Changed,
            ));
        }
        Ok(())
    }

    fn attempts_are_byte_identical(&self) -> bool {
        self.identical
    }

    fn primary(&self) -> &ReconstructedRuntimePackage {
        &self.reconstructed
    }

    fn open_primary_member(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<File, OllamaRuntimeImportError> {
        if cancellation.is_cancelled() {
            return Err(OllamaRuntimeImportError::Cancelled);
        }
        let selected = if self.substitute_first_member
            && path
                == self
                    .reconstructed
                    .artifact_set()
                    .members()
                    .first()
                    .expect("fixture member")
                    .relative_path()
        {
            self.substitute.clone()
        } else {
            self.member_root.join(path.as_str())
        };
        File::open(selected).map_err(OllamaRuntimeImportError::SourceIo)
    }
}

#[test]
fn primary_import_is_exact_idempotent_and_policy_neutral() {
    let fixture = write_source(true, false);
    let root = tempdir().expect("temporary repository parent");
    let data = root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");

    let first = repository
        .import_primary_runtime_source(&fixture, import_limits(), &CancellationToken::new())
        .expect("import primary runtime");
    assert_eq!(
        first.artifact_set_disposition,
        ArtifactSetImportDisposition::Imported
    );
    assert_eq!(
        first.runtime_package_disposition,
        PackageManifestWriteDisposition::Inserted
    );
    assert_eq!(
        first.evidence.artifact_set(),
        fixture.reconstructed.artifact_set()
    );
    assert_eq!(
        first.evidence.runtime_package(),
        fixture.reconstructed.runtime_package()
    );

    let set_root = managed_set_root(&data, &first);
    assert_exact_managed_members(&set_root, first.evidence.artifact_set());
    assert!(!set_root.join("runtime-layout.json").exists());
    assert!(!set_root.join("extra-build-evidence.json").exists());

    let second = repository
        .import_primary_runtime_source(&fixture, import_limits(), &CancellationToken::new())
        .expect("repeat exact primary import");
    assert_eq!(
        second.artifact_set_disposition,
        ArtifactSetImportDisposition::AlreadyPresent
    );
    assert_eq!(
        second.runtime_package_disposition,
        PackageManifestWriteDisposition::AlreadyPresent
    );
    assert_eq!(second.artifact_set_key, first.artifact_set_key);
    assert_eq!(second.evidence, first.evidence);
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/.set-staging"))
            .expect("read staging")
            .count(),
        0
    );
    assert_eq!(OllamaCloudDisableFeaturePolicy::reviewed_runtime_count(), 0);
    assert_eq!(
        OllamaManagedGenerationAdmissionPolicy::reviewed_runtime_count(),
        0
    );
}

#[test]
fn nonidentical_cancellation_and_limits_fail_before_repository_mutation() {
    let root = tempdir().expect("temporary repository parent");

    let nonidentical = write_source(false, false);
    let data = root.path().join("nonidentical");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let error = repository
        .import_primary_runtime_source(&nonidentical, import_limits(), &CancellationToken::new())
        .expect_err("different attempts cannot import");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Conflict);
    assert!(!data.exists());

    let cancelled = write_source(true, false);
    let data = root.path().join("cancelled");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = repository
        .import_primary_runtime_source(&cancelled, import_limits(), &cancellation)
        .expect_err("cancelled import cannot mutate");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::Cancelled);
    assert!(!data.exists());

    let limited = write_source(true, false);
    let data = root.path().join("limited");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let limits = ArtifactSetImportLimits {
        maximum_members: 1,
        ..import_limits()
    };
    let error = repository
        .import_primary_runtime_source(&limited, limits, &CancellationToken::new())
        .expect_err("member limit cannot mutate");
    assert_eq!(error.kind(), ArtifactRepositoryErrorKind::ResourceLimit);
    assert!(!data.exists());
}

#[test]
fn substituted_member_fails_digest_verification_and_cleans_staging() {
    let fixture = write_source(true, true);
    let root = tempdir().expect("temporary repository parent");
    let data = root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let error = repository
        .import_primary_runtime_source(&fixture, import_limits(), &CancellationToken::new())
        .expect_err("substituted retained member must fail");
    assert_eq!(
        error.kind(),
        ArtifactRepositoryErrorKind::ConcurrentModification
    );
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/.set-staging"))
            .expect("read staging")
            .count(),
        0
    );
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/sets"))
            .expect("read managed sets")
            .count(),
        0
    );
}

#[test]
fn source_drift_before_publication_cleans_staging() {
    let fixture = write_source(true, false);
    fixture.fail_once_at(3);
    let root = tempdir().expect("temporary repository parent");
    let data = root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let error = repository
        .import_primary_runtime_source(&fixture, import_limits(), &CancellationToken::new())
        .expect_err("prepublication evidence drift must fail");
    assert_eq!(
        error.kind(),
        ArtifactRepositoryErrorKind::ConcurrentModification
    );
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/.set-staging"))
            .expect("read staging")
            .count(),
        0
    );
    assert_eq!(
        fs::read_dir(data.join("artifact-storage/sets"))
            .expect("read managed sets")
            .count(),
        0
    );
}

#[test]
fn source_drift_after_readback_returns_failure_but_leaves_only_inert_exact_state() {
    let fixture = write_source(true, false);
    fixture.fail_once_at(4);
    let root = tempdir().expect("temporary repository parent");
    let data = root.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("repository");
    let error = repository
        .import_primary_runtime_source(&fixture, import_limits(), &CancellationToken::new())
        .expect_err("post-readback evidence drift must fail");
    assert_eq!(
        error.kind(),
        ArtifactRepositoryErrorKind::ConcurrentModification
    );

    let recovered = repository
        .import_primary_runtime_source(&fixture, import_limits(), &CancellationToken::new())
        .expect("exact inert state is idempotently recoverable");
    assert_eq!(
        recovered.artifact_set_disposition,
        ArtifactSetImportDisposition::AlreadyPresent
    );
    assert_eq!(
        recovered.runtime_package_disposition,
        PackageManifestWriteDisposition::AlreadyPresent
    );
    assert_exact_managed_members(
        &managed_set_root(&data, &recovered),
        recovered.evidence.artifact_set(),
    );
}

fn write_source(identical: bool, substitute_first_member: bool) -> TestPrimarySource {
    let root = tempdir().expect("temporary source root");
    let member_root = root.path().join("primary");
    fs::create_dir(&member_root).expect("create primary root");
    let mut declared = Vec::new();
    let mut observed = Vec::new();
    for (path, roles, policy, bytes) in fixture_members() {
        let destination = member_root.join(path);
        fs::create_dir_all(destination.parent().expect("member parent"))
            .expect("create member parent");
        fs::write(&destination, bytes).expect("write member");
        observed.push(path);
        declared.push(json!({
            "byte_size": bytes.len(),
            "digest": Digest::sha256(bytes),
            "load_policy": policy,
            "relative_path": path,
            "roles": roles
        }));
    }
    fs::write(
        member_root.join("extra-build-evidence.json"),
        b"not a package member",
    )
    .expect("write excluded build evidence");
    let layout = fixture_layout(&declared, &observed);
    let reconstructed = reconstruct_runtime_package_with_limits(
        &layout,
        &RuntimeLayoutLimits::default(),
        |path| {
            File::open(member_root.join(path.as_str()))
                .map_err(|_| rewrite_ollama_package::MemberOpenError)
        },
        || false,
    )
    .expect("reconstruct primary package");
    let first = reconstructed
        .artifact_set()
        .members()
        .first()
        .expect("fixture member");
    let original =
        fs::read(member_root.join(first.relative_path().as_str())).expect("read first member");
    let substitute = root.path().join("substitute");
    let changed = original
        .into_iter()
        .map(|byte| byte ^ 1)
        .collect::<Vec<_>>();
    fs::write(&substitute, changed).expect("write same-size substitute");
    TestPrimarySource {
        _root: root,
        member_root,
        substitute,
        reconstructed,
        identical,
        substitute_first_member,
        fail_revalidation_at: Cell::new(None),
        revalidation_count: Cell::new(0),
    }
}

fn fixture_members() -> [(&'static str, serde_json::Value, &'static str, &'static [u8]); 6] {
    [
        (
            "bin/ollama",
            json!(["entrypoint"]),
            "required_at_ready",
            b"ollama-entrypoint\n".as_slice(),
        ),
        (
            "helper/retonr-isolation",
            json!(["helper_executable"]),
            "must_not_be_code_loaded",
            b"isolation-helper\n".as_slice(),
        ),
        (
            "legal/license.txt",
            json!(["license_text"]),
            "must_not_be_code_loaded",
            b"Fixture runtime license\n".as_slice(),
        ),
        (
            "lib/ollama/libggml-cpu.so",
            json!(["native_dependency"]),
            "backend_conditional",
            b"ggml-cpu\n".as_slice(),
        ),
        (
            "lib/ollama/llama-server",
            json!(["worker_executable"]),
            "backend_conditional",
            b"llama-server\n".as_slice(),
        ),
        (
            "provenance/source.txt",
            json!(["provenance_record"]),
            "must_not_be_code_loaded",
            b"reviewed-source\n".as_slice(),
        ),
    ]
}

fn fixture_layout(declared: &[serde_json::Value], observed: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "build_revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
        "members": declared,
        "observed_tree": observed,
        "reported_version": "0.32.15",
        "runtime_family": ADMITTED_RUNTIME_FAMILY,
        "schema_version": RUNTIME_LAYOUT_SCHEMA_VERSION,
        "source": {
            "kind": "repository_revision",
            "locator": "https://github.com/ollama/ollama",
            "provenance_digest": Digest::sha256(b"runtime-source"),
            "revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
            "schema_version": 1
        },
        "target": {
            "abi": "linux_gnu_libc",
            "architecture": "x86_64",
            "operating_system": "linux"
        },
        "transformation": {
            "evidence_digest": Digest::sha256(b"untransformed"),
            "kind": "untransformed"
        }
    }))
    .expect("encode runtime layout")
}

const fn import_limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 16,
        maximum_member_bytes: 1024 * 1024,
        maximum_total_bytes: 2 * 1024 * 1024,
        maximum_tree_entries: 32,
        maximum_storage_entries: 8,
        maximum_staging_entries: 8,
    }
}

fn managed_set_root(data: &Path, result: &OllamaRuntimeImportResult) -> PathBuf {
    data.join("artifact-storage/sets").join(format!(
        "set-v1-{}",
        result.artifact_set_key.artifact_set_id().digest().as_str()
    ))
}

fn assert_exact_managed_members(root: &Path, manifest: &ArtifactSetManifest) {
    let mut actual = Vec::new();
    collect_files(root, root, &mut actual);
    actual.sort_unstable();
    let expected = manifest
        .members()
        .iter()
        .map(|member| member.relative_path().as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<String>) {
    let mut entries = fs::read_dir(directory)
        .expect("read managed directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect managed directory");
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        if entry.file_type().expect("entry type").is_dir() {
            collect_files(root, &entry.path(), output);
        } else {
            output.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .expect("managed relative path")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}
