use std::{
    fs,
    io::{Read as _, Seek as _, SeekFrom},
    path::Path,
};

use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    EmbeddedModelComponent, EmbeddedModelComponentPurpose, ModelPackageManifest,
    ModelPackageMember, ModelPackageMemberRole, ModelWeightLayout, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::{CancellationToken, Digest};
use tempfile::TempDir;

use crate::{
    ArtifactRepository, ArtifactSetImportLimits, OfflineArtifactSetImportRequest,
    RuntimeArtifactSetLease, RuntimeArtifactSetLeaseLimits,
};

use super::{
    ModelPackageLease, PackageAttestationError, PackageAttestationScope, PackageAttestationService,
    RuntimePackageLease, RuntimePackageLeaseLimits,
    verification::{VerificationObserver, VerificationStage},
};

mod adversarial;
#[cfg(unix)]
#[path = "tests/retained_entrypoint.rs"]
mod retained_entrypoint;
#[path = "tests/retained_members.rs"]
mod retained_members;

pub(super) const RUNTIME_FILES: [(&str, &[u8]); 6] = [
    ("bin/helper", b"helper-v1"),
    ("bin/runtime", b"runtime-v1"),
    ("config/build.json", b"{\"build\":1}"),
    ("legal/license.txt", b"license"),
    ("legal/provenance.txt", b"provenance"),
    ("lib/backend.so", b"backend-v1"),
];

const MODEL_FILES: [(&str, &[u8]); 3] = [
    ("legal/license.txt", b"model-license"),
    ("legal/provenance.txt", b"model-provenance"),
    ("model/model.gguf", b"model-weights"),
];

pub(super) const SET_LIMITS: RuntimeArtifactSetLeaseLimits = RuntimeArtifactSetLeaseLimits {
    maximum_members: 16,
    maximum_member_bytes: 1024,
    maximum_total_bytes: 4096,
    maximum_tree_entries: 32,
    maximum_storage_entries: 16,
};

const IMPORT_LIMITS: ArtifactSetImportLimits = ArtifactSetImportLimits {
    maximum_members: 16,
    maximum_member_bytes: 1024,
    maximum_total_bytes: 4096,
    maximum_tree_entries: 32,
    maximum_storage_entries: 16,
    maximum_staging_entries: 16,
};

pub(super) const PACKAGE_LIMITS: RuntimePackageLeaseLimits = RuntimePackageLeaseLimits {
    maximum_code_members: 8,
    maximum_code_member_bytes: 1024,
    maximum_code_bytes: 4096,
};

pub(super) fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid fixture path")
}

fn artifact(bytes: &[u8]) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(bytes))
}

pub(super) fn artifact_set(files: &[(&str, &[u8])]) -> ArtifactSetManifest {
    ArtifactSetManifest::new(
        files
            .iter()
            .map(|(relative_path, bytes)| {
                ArtifactSetMember::new(
                    artifact(bytes),
                    u64::try_from(bytes.len()).expect("fixture byte size"),
                    path(relative_path),
                )
            })
            .collect(),
    )
    .expect("valid fixture artifact set")
}

fn source() -> PackageSource {
    PackageSource::new(
        PackageSourceKind::LocalArchive,
        "local-fixture",
        "sha256-fixture",
        Digest::sha256(b"fixture provenance"),
    )
    .expect("valid fixture source")
}

pub(super) fn runtime_package(set: &ArtifactSetManifest) -> RuntimePackageManifest {
    let roles = [
        (
            RuntimePackageMemberRole::HelperExecutable,
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            RuntimePackageMemberRole::Entrypoint,
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        (
            RuntimePackageMemberRole::BuildConfiguration,
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            RuntimePackageMemberRole::LicenseText,
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            RuntimePackageMemberRole::ProvenanceRecord,
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            RuntimePackageMemberRole::NativeDependency,
            RuntimePackageLoadPolicy::BackendConditional,
        ),
    ];
    let members = set
        .members()
        .iter()
        .zip(roles)
        .map(|(member, (role, policy))| {
            RuntimePackageMember::new(
                member.artifact_id().clone(),
                member.byte_size(),
                member.relative_path().clone(),
                vec![role],
                policy,
            )
        })
        .collect();
    RuntimePackageManifest::new(
        set,
        "fixture-runtime",
        "1.0.0",
        Some("revision-1".to_owned()),
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("valid runtime target"),
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"untransformed runtime"),
        },
        members,
    )
    .expect("valid runtime package")
}

fn model_package(set: &ArtifactSetManifest) -> ModelPackageManifest {
    let roles = [
        ModelPackageMemberRole::LicenseText,
        ModelPackageMemberRole::ProvenanceRecord,
        ModelPackageMemberRole::ModelWeights,
    ];
    let members = set
        .members()
        .iter()
        .zip(roles)
        .map(|(member, role)| {
            ModelPackageMember::new(
                member.artifact_id().clone(),
                member.byte_size(),
                member.relative_path().clone(),
                vec![role],
            )
        })
        .collect();
    let embedded = [
        (
            EmbeddedModelComponentPurpose::ModelConfiguration,
            "general.architecture",
        ),
        (
            EmbeddedModelComponentPurpose::GenerationConfiguration,
            "generation.defaults",
        ),
        (EmbeddedModelComponentPurpose::Tokenizer, "tokenizer.ggml"),
        (
            EmbeddedModelComponentPurpose::PromptTemplate,
            "tokenizer.chat-template",
        ),
    ]
    .into_iter()
    .map(|(purpose, selector)| {
        EmbeddedModelComponent::new(
            path("model/model.gguf"),
            purpose,
            "gguf-metadata",
            1,
            selector,
            Digest::sha256(selector.as_bytes()),
        )
        .expect("valid embedded component")
    })
    .collect();
    ModelPackageManifest::new(
        set,
        "gguf",
        1,
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"untransformed model"),
        },
        members,
        ModelWeightLayout::Single {
            member: path("model/model.gguf"),
        },
        embedded,
    )
    .expect("valid model package")
}

fn write_source(root: &Path, files: &[(&str, &[u8])]) {
    for (relative_path, bytes) in files {
        let target = root.join(relative_path);
        fs::create_dir_all(target.parent().expect("member parent")).expect("source directory");
        fs::write(target, bytes).expect("source member");
    }
}

pub(super) fn import_set(
    repository: &ArtifactRepository,
    directory: &Path,
    label: &str,
    files: &[(&str, &[u8])],
) -> ArtifactSetManifest {
    let source_root = directory.join(format!("source-{label}"));
    write_source(&source_root, files);
    let manifest = artifact_set(files);
    repository
        .import_set(
            &OfflineArtifactSetImportRequest {
                source_root,
                manifest: manifest.clone(),
            },
            IMPORT_LIMITS,
            &CancellationToken::new(),
        )
        .expect("import fixture set");
    manifest
}

pub(super) fn runtime_fixture() -> (
    TempDir,
    ArtifactRepository,
    ArtifactSetManifest,
    RuntimePackageManifest,
) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let repository = ArtifactRepository::new(directory.path().join("data")).expect("repository");
    let set = import_set(&repository, directory.path(), "runtime", &RUNTIME_FILES);
    let package = runtime_package(&set);
    (directory, repository, set, package)
}

pub(super) fn lease_set(
    repository: &ArtifactRepository,
    id: &ArtifactSetId,
) -> RuntimeArtifactSetLease {
    repository
        .lease_set(id, SET_LIMITS, &CancellationToken::new())
        .expect("lease fixture set")
}

pub(super) fn set_root(directory: &Path, id: &ArtifactSetId) -> std::path::PathBuf {
    directory
        .join("data")
        .join("artifact-storage")
        .join("sets")
        .join(format!("set-v1-{}", id.digest().as_str()))
}

fn attest_runtime(
    repository: &ArtifactRepository,
    package: &RuntimePackageManifest,
) -> RuntimePackageLease {
    PackageAttestationService::attest_runtime(
        lease_set(repository, package.artifact_set_id()),
        package,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
    )
    .expect("attest runtime fixture")
}

#[test]
fn runtime_evidence_is_typed_redacted_and_revalidatable() {
    let (_directory, repository, set, package) = runtime_fixture();
    let mut lease = attest_runtime(&repository, &package);
    let evidence = lease.evidence();
    assert_eq!(evidence.schema_version(), 1);
    assert_eq!(
        evidence.scope(),
        PackageAttestationScope::StaticManagedBytes
    );
    assert_eq!(evidence.artifact_set_id(), &set.artifact_set_id());
    assert_eq!(
        evidence.runtime_package_manifest_id(),
        &package.runtime_package_manifest_id()
    );
    assert_eq!(
        evidence.entrypoint_artifact_id(),
        package.entrypoint().artifact_id()
    );
    assert_eq!(evidence.code_member_count(), 3);
    assert_eq!(
        evidence.code_byte_size(),
        u64::try_from(b"helper-v1".len() + b"runtime-v1".len() + b"backend-v1".len())
            .expect("fixture size")
    );
    assert!(!format!("{lease:?}").contains("bin/"));
    lease
        .revalidate(&CancellationToken::new())
        .expect("stable retained package revalidates");
    let mut entrypoint = lease
        .clone_entrypoint_for_launch(&CancellationToken::new())
        .expect("clone retained entrypoint");
    entrypoint
        .seek(SeekFrom::Start(0))
        .expect("rewind retained entrypoint");
    let mut bytes = Vec::new();
    entrypoint
        .read_to_end(&mut bytes)
        .expect("read retained entrypoint");
    assert_eq!(bytes, b"runtime-v1");
}

#[test]
fn model_evidence_pins_and_revalidates_the_exact_set() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let repository = ArtifactRepository::new(directory.path().join("data")).expect("repository");
    let set = import_set(&repository, directory.path(), "model", &MODEL_FILES);
    let package = model_package(&set);
    let lease: ModelPackageLease = PackageAttestationService::attest_model(
        lease_set(&repository, &set.artifact_set_id()),
        &package,
        &CancellationToken::new(),
    )
    .expect("attest model package");
    assert_eq!(lease.evidence().artifact_set_id(), &set.artifact_set_id());
    assert_eq!(
        lease.evidence().model_package_manifest_id(),
        &package.model_package_manifest_id()
    );
    assert_eq!(lease.evidence().member_count(), 3);
    assert_eq!(lease.evidence().byte_size(), set.total_byte_size());
    lease
        .revalidate(&CancellationToken::new())
        .expect("stable model package revalidates");
}

#[test]
fn exact_runtime_and_model_relationship_mismatches_fail_closed() {
    let (directory, repository, runtime_set, _runtime) = runtime_fixture();
    let changed_files: [(&str, &[u8]); 6] = [
        ("bin/helper", b"helper-v2"),
        RUNTIME_FILES[1],
        RUNTIME_FILES[2],
        RUNTIME_FILES[3],
        RUNTIME_FILES[4],
        RUNTIME_FILES[5],
    ];
    let other_set = artifact_set(&changed_files);
    let other_runtime = runtime_package(&other_set);
    let error = PackageAttestationService::attest_runtime(
        lease_set(&repository, &runtime_set.artifact_set_id()),
        &other_runtime,
        PACKAGE_LIMITS,
        &CancellationToken::new(),
    )
    .expect_err("runtime relationship mismatch");
    assert!(matches!(
        error,
        PackageAttestationError::RuntimeRelationship(_)
    ));

    let model_set = import_set(&repository, directory.path(), "model", &MODEL_FILES);
    let model = model_package(&model_set);
    let error = PackageAttestationService::attest_model(
        lease_set(&repository, &runtime_set.artifact_set_id()),
        &model,
        &CancellationToken::new(),
    )
    .expect_err("model relationship mismatch");
    assert!(matches!(
        error,
        PackageAttestationError::ModelRelationship(_)
    ));
}

#[test]
fn limits_and_cancellation_are_enforced_before_granting_evidence() {
    let (_directory, repository, _set, package) = runtime_fixture();
    let cases = [
        (
            RuntimePackageLeaseLimits {
                maximum_code_members: 0,
                ..PACKAGE_LIMITS
            },
            "invalid",
        ),
        (
            RuntimePackageLeaseLimits {
                maximum_code_members: 2,
                ..PACKAGE_LIMITS
            },
            "count",
        ),
        (
            RuntimePackageLeaseLimits {
                maximum_code_member_bytes: 9,
                ..PACKAGE_LIMITS
            },
            "member",
        ),
        (
            RuntimePackageLeaseLimits {
                maximum_code_bytes: 10,
                ..PACKAGE_LIMITS
            },
            "total",
        ),
    ];
    for (limits, expected) in cases {
        let error = PackageAttestationService::attest_runtime(
            lease_set(&repository, package.artifact_set_id()),
            &package,
            limits,
            &CancellationToken::new(),
        )
        .expect_err("limit must fail closed");
        assert!(
            matches!(
                (&error, expected),
                (PackageAttestationError::InvalidLimits, "invalid")
                    | (PackageAttestationError::TooManyCodeMembers { .. }, "count")
                    | (PackageAttestationError::CodeMemberTooLarge { .. }, "member")
                    | (PackageAttestationError::CodeBytesTooLarge { .. }, "total")
            ),
            "unexpected error {error:?}"
        );
    }

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = PackageAttestationService::attest_runtime(
        lease_set(&repository, package.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &cancellation,
    )
    .expect_err("cancelled attestation");
    assert!(matches!(error, PackageAttestationError::Cancelled));
}

#[test]
fn cancellation_between_member_hashes_fails_closed() {
    let (_directory, repository, _set, package) = runtime_fixture();
    let cancellation = CancellationToken::new();
    let observed_cancellation = cancellation.clone();
    let mut callback = |stage| {
        if stage == VerificationStage::AfterMemberHash(0) {
            observed_cancellation.cancel();
        }
    };
    let mut observer = VerificationObserver::new(&mut callback);
    let error = PackageAttestationService::attest_runtime_with_observer(
        lease_set(&repository, package.artifact_set_id()),
        &package,
        PACKAGE_LIMITS,
        &cancellation,
        &mut observer,
    )
    .expect_err("mid-verification cancellation");
    assert!(matches!(error, PackageAttestationError::Cancelled));
}
