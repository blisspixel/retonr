use std::{fs, path::PathBuf};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend,
    EffectiveRuntimeState, EffectiveRuntimeStateInput, ExecutionPlacement, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeBuildIdentity, RuntimeBuildMode, RuntimeOperatingSystem, RuntimePackageLoadPolicy,
    RuntimePackageManifest, RuntimePackageMember, RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_ollama_package::{OllamaLocalArchiveMemberBinding, ReconstructionLimits};
use rewrite_runtime_attestor::{
    CompiledFrozenExternalNativeComponentSet, ExternalNativeComponentReview,
    ExternalNativeComponentReviewDisposition, NativeLoadDiscovery,
    VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::{IsolationPolicy, PreparedIsolation};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;

use crate::{
    ArtifactSetImportLimits, ModelLicenseControlCompiler, ModelLicenseControlVerifier,
    ModelLicensePermission, OfflineArtifactSetImportRequest, OllamaModelReference,
    PackageAttestationService, ProductionModelLicenseApprovalPolicy, RuntimeArtifactSetLeaseLimits,
    RuntimePackageLease, RuntimePackageLeaseLimits, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath, VerifiedManagedOllamaLaunchPlan,
    VerifiedManagedOllamaModelPackageLease,
};

#[cfg_attr(
    test,
    expect(
        clippy::duplicate_mod,
        reason = "reuse the exact managed-model storage fixture at the new trust boundary"
    )
)]
#[expect(
    dead_code,
    reason = "the shared fixture contains generation-advance support unused by this boundary"
)]
#[path = "../../../model_license_control/fixture.rs"]
mod model_fixture;

const SET_LIMITS: RuntimeArtifactSetLeaseLimits = RuntimeArtifactSetLeaseLimits {
    maximum_members: 8,
    maximum_member_bytes: 16 * 1_024,
    maximum_total_bytes: 32 * 1_024,
    maximum_tree_entries: 16,
    maximum_storage_entries: 16,
};
const IMPORT_LIMITS: ArtifactSetImportLimits = ArtifactSetImportLimits {
    maximum_members: 8,
    maximum_member_bytes: 16 * 1_024,
    maximum_total_bytes: 32 * 1_024,
    maximum_tree_entries: 16,
    maximum_storage_entries: 16,
    maximum_staging_entries: 8,
};

pub(crate) struct Fixture {
    storage: model_fixture::Fixture,
    pub(crate) model_lease: VerifiedManagedOllamaModelPackageLease,
    pub(crate) runtime: RuntimeFixture,
}

pub(crate) struct RuntimeFixture {
    pub(crate) runtime_set: ArtifactSetManifest,
    pub(crate) runtime_manifest: RuntimePackageManifest,
    pub(crate) runtime_package: RuntimePackageLease,
    pub(crate) runtime_build: RuntimeBuildIdentity,
    pub(crate) runtime_state: EffectiveRuntimeState,
    pub(crate) frozen: VerifiedFrozenExternalNativeComponentSet,
    pub(crate) admitted: VerifiedAdmittedRuntime,
    pub(crate) path: VerifiedManagedGenerationPath,
    pub(crate) prepared_isolation: PreparedIsolation,
    runtime_root: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        Self::new_model_variant(false)
    }

    pub(crate) fn new_model_variant(shared_license_and_template: bool) -> Self {
        Self::new_model_variant_with_target(
            shared_license_and_template,
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxGnuLibc,
            )
            .expect("reviewed candidate precursor target"),
        )
    }

    pub(crate) fn new_with_runtime_target(target: RuntimeTarget) -> Self {
        Self::new_model_variant_with_target(false, target)
    }

    fn new_model_variant_with_target(
        shared_license_and_template: bool,
        target: RuntimeTarget,
    ) -> Self {
        let storage = model_fixture::Fixture::new(shared_license_and_template);
        let (set, runtime_manifest, files) = runtime_package(target);
        let source = storage.directory.path().join("candidate-runtime-source");
        write_source(&source, &files);
        let key = storage
            .repository
            .import_set(
                &OfflineArtifactSetImportRequest {
                    source_root: source,
                    manifest: set.clone(),
                },
                IMPORT_LIMITS,
                &CancellationToken::new(),
            )
            .expect("import runtime set")
            .key;
        let runtime_package = PackageAttestationService::attest_runtime(
            storage
                .repository
                .lease_set(key.artifact_set_id(), SET_LIMITS, &CancellationToken::new())
                .expect("lease runtime set"),
            &runtime_manifest,
            RuntimePackageLeaseLimits {
                maximum_code_members: 8,
                maximum_code_member_bytes: 16 * 1_024,
                maximum_code_bytes: 32 * 1_024,
            },
            &CancellationToken::new(),
        )
        .expect("attest runtime");
        let runtime_root = storage
            .directory
            .path()
            .join("data/artifact-storage/sets")
            .join(format!(
                "set-v1-{}",
                key.artifact_set_id().digest().as_str()
            ));
        let runtime_build = RuntimeBuildIdentity::new_from_package_manifest(
            RuntimeBuildMode::ManagedProcess,
            &runtime_manifest,
        )
        .expect("runtime build");
        let isolation_policy = IsolationPolicy::new(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(5),
            32,
            32,
            4_096,
            256,
            64,
        )
        .expect("isolation policy");
        let prepared_isolation = PreparedIsolation::test_support_from_policy(isolation_policy);
        let runtime_state = EffectiveRuntimeState::new(
            &runtime_build,
            EffectiveRuntimeStateInput {
                provider_snapshot_contract: "candidate-precursor-snapshot".to_owned(),
                provider_snapshot_schema_version: 1,
                provider_snapshot_digest: digest("snapshot"),
                launch_policy_digest: digest("launch policy"),
                loaded_components_digest: digest("loaded components"),
                effective_configuration_digest: digest("effective configuration"),
                platform_digest: digest("platform"),
                execution_class_digest: digest("runtime execution class"),
                isolation_policy_digest: prepared_isolation.policy_digest(),
                effective_context_tokens: 8_192,
                compute_backend: ComputeBackend::NativeCpu,
                placement: ExecutionPlacement::CpuOnly,
            },
        )
        .expect("runtime state");
        let frozen = frozen(&runtime_manifest);
        let admitted = VerifiedAdmittedRuntime::exact_candidate_precursor_test_fixture(
            &runtime_manifest,
            frozen.frozen_set_id().clone(),
        );
        let path = VerifiedManagedGenerationPath::exact_candidate_precursor_test_fixture(
            &admitted,
            &runtime_manifest,
        );
        let model_lease = storage.verify();
        Self {
            storage,
            model_lease,
            runtime: RuntimeFixture {
                runtime_set: set,
                runtime_manifest,
                runtime_package,
                runtime_build,
                runtime_state,
                frozen,
                admitted,
                path,
                prepared_isolation,
                runtime_root,
            },
        }
    }

    pub(crate) fn add_runtime_member(&self) {
        fs::write(
            self.runtime.runtime_root.join("unexpected.bin"),
            b"unexpected",
        )
        .expect("drift runtime tree");
    }

    pub(crate) fn add_model_member(&self) {
        self.storage.add_unexpected_member();
    }

    pub(crate) fn runtime_drift_path(&self) -> PathBuf {
        self.runtime.runtime_root.join("unexpected.bin")
    }

    pub(crate) fn model_drift_path(&self) -> PathBuf {
        self.storage
            .directory
            .path()
            .join("data/artifact-storage/sets")
            .join(format!(
                "set-v1-{}",
                self.storage.set.artifact_set_id().digest().as_str()
            ))
            .join("unexpected-member.bin")
    }
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn runtime_package(
    target: RuntimeTarget,
) -> (
    ArtifactSetManifest,
    RuntimePackageManifest,
    Vec<(String, Vec<u8>)>,
) {
    let specifications = [
        (
            "bin/ollama",
            b"entrypoint".to_vec(),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        (
            "legal/license.txt",
            b"license".to_vec(),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            "lib/ollama/worker",
            b"worker".to_vec(),
            vec![RuntimePackageMemberRole::WorkerExecutable],
            RuntimePackageLoadPolicy::BackendConditional,
        ),
        (
            "provenance/source.json",
            b"provenance".to_vec(),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ];
    let set = ArtifactSetManifest::new(
        specifications
            .iter()
            .map(|(relative, bytes, ..)| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(Digest::sha256(bytes)),
                    bytes.len() as u64,
                    path(relative),
                )
            })
            .collect(),
    )
    .expect("runtime set");
    let members = specifications
        .iter()
        .zip(set.members())
        .map(|((_, _, roles, policy), member)| {
            RuntimePackageMember::new(
                member.artifact_id().clone(),
                member.byte_size(),
                member.relative_path().clone(),
                roles.clone(),
                *policy,
            )
        })
        .collect();
    let package = RuntimePackageManifest::new(
        &set,
        "ollama",
        "0.32.15",
        Some("candidate-precursor-runtime".to_owned()),
        target,
        PackageSource::new(
            PackageSourceKind::RepositoryRevision,
            "https://example.invalid/ollama",
            "candidate-precursor-runtime",
            digest("runtime provenance"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: digest("runtime transformation"),
        },
        members,
    )
    .expect("runtime package");
    let files = specifications
        .into_iter()
        .map(|(relative, bytes, ..)| (relative.to_owned(), bytes))
        .collect();
    (set, package, files)
}

fn write_source(root: &std::path::Path, files: &[(String, Vec<u8>)]) {
    for (relative, bytes) in files {
        let target = root.join(relative);
        fs::create_dir_all(target.parent().expect("source parent")).expect("source directory");
        fs::write(target, bytes).expect("source bytes");
    }
}

fn frozen(package: &RuntimePackageManifest) -> VerifiedFrozenExternalNativeComponentSet {
    frozen_with_label(package, "managed process")
}

pub(crate) fn frozen_with_label(
    package: &RuntimePackageManifest,
    process_label: &str,
) -> VerifiedFrozenExternalNativeComponentSet {
    let discovery_bytes = serde_json::to_vec(&serde_json::json!({
        "authority":"none",
        "evidence_class":"linux_proc_map_files",
        "external_components":[],
        "observation_contract_id":"candidate-precursor-test",
        "observation_contract_schema_version":1,
        "process_evidence_digest":digest(process_label),
        "runtime_package_manifest_id":package.runtime_package_manifest_id(),
        "schema_version":1,
        "status":"proposed"
    }))
    .expect("discovery bytes");
    let discovery = NativeLoadDiscovery::from_json_bytes(
        &discovery_bytes,
        &package.runtime_package_manifest_id(),
    )
    .expect("discovery");
    let reviewer = b"candidate precursor reviewer evidence";
    let review = ExternalNativeComponentReview::compile(
        &discovery,
        ExternalNativeComponentReviewDisposition::Approved,
        reviewer,
    )
    .expect("review");
    let compiled = CompiledFrozenExternalNativeComponentSet::compile(&discovery, &review)
        .expect("compiled frozen set");
    VerifiedFrozenExternalNativeComponentSet::verify(
        compiled.canonical_json_bytes(),
        discovery.canonical_json_bytes(),
        review.canonical_json_bytes(),
        reviewer,
        &package.runtime_package_manifest_id(),
    )
    .expect("verified frozen set")
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ReviewDecision {
    Approved,
}

#[derive(Serialize)]
struct Review<'a> {
    decision: ReviewDecision,
    foundation: Foundation<'a>,
    license_members: Vec<Member<'a>>,
    permission: ModelLicensePermission,
    procedure_id: &'static str,
    procedure_version: u32,
    schema_version: u32,
    source: Source<'a>,
}

#[derive(Serialize)]
struct Foundation<'a> {
    artifact_set_id: &'a Digest,
    descriptor_mapping_digest: &'a Digest,
    #[serde(rename = "foundation_id")]
    identity_digest: &'a Digest,
    logical_binding_digest: &'a Digest,
    model_package_manifest_id: &'a Digest,
    package_source_id: &'a Digest,
    provenance_manifest: Member<'a>,
}

#[derive(Serialize)]
struct Source<'a> {
    kind: PackageSourceKind,
    locator: &'a str,
    provenance_digest: &'a Digest,
    revision: &'a str,
}

#[derive(Serialize)]
struct Member<'a> {
    artifact_id: &'a ArtifactId,
    byte_size: u64,
    relative_path: &'a str,
}

fn member(value: &OllamaLocalArchiveMemberBinding) -> Member<'_> {
    Member {
        artifact_id: value.artifact_id(),
        byte_size: value.byte_size(),
        relative_path: value.relative_path().as_str(),
    }
}

pub(crate) fn launch<'a>(
    lease: &'a VerifiedManagedOllamaModelPackageLease,
    tag: &str,
) -> VerifiedManagedOllamaLaunchPlan<'a> {
    let view = lease.private_view();
    let foundation = view.foundation_evidence();
    let source = view.model_package_manifest().source();
    let review = serde_json::to_vec(&Review {
        decision: ReviewDecision::Approved,
        foundation: Foundation {
            artifact_set_id: foundation.artifact_set_id().digest(),
            descriptor_mapping_digest: foundation.descriptor_mapping_digest(),
            identity_digest: lease.foundation_id().digest(),
            logical_binding_digest: foundation.logical_binding_digest(),
            model_package_manifest_id: foundation.model_package_manifest_id().digest(),
            package_source_id: foundation.package_source_id().digest(),
            provenance_manifest: member(foundation.provenance_manifest()),
        },
        license_members: foundation.license_members().iter().map(member).collect(),
        permission: ModelLicensePermission::LocalGeneration,
        procedure_id: crate::MODEL_LICENSE_REVIEW_PROCEDURE_ID,
        procedure_version: crate::MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
        schema_version: crate::MODEL_LICENSE_REVIEW_SCHEMA_VERSION,
        source: Source {
            kind: source.kind(),
            locator: source.locator(),
            provenance_digest: source.provenance_digest(),
            revision: source.revision(),
        },
    })
    .expect("review bytes");
    let compiled = ModelLicenseControlCompiler::compile(
        lease,
        ModelLicensePermission::LocalGeneration,
        &review,
        &CancellationToken::new(),
    )
    .expect("compile license control");
    let approval = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        compiled.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );
    let license = ModelLicenseControlVerifier::verify(
        compiled.canonical_bytes(),
        lease,
        ModelLicensePermission::LocalGeneration,
        &approval,
        &CancellationToken::new(),
    )
    .expect("verify license control");
    let reference = OllamaModelReference::new("registry.ollama.ai", "library", "fixture", tag)
        .expect("model reference");
    let input = PackageAttestationService::prepare_verified_managed_ollama_inputs(
        lease,
        &reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("managed model input");
    PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(lease, input, license)
        .expect("managed launch plan")
}
