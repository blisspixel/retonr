use std::{
    fs,
    io::{Read as _, Write as _},
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    time::Duration,
};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_runtime_isolation::{IsolationPolicy, PreparedIsolation};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    ArtifactRepository, ArtifactSetImportLimits, ManagedOllamaCloseError, ManagedOllamaLaunchError,
    ManagedOllamaModelAuthorityError, ModelLicenseControlError, OfflineArtifactSetImportRequest,
    PackageAttestationError, PackageAttestationService, RuntimeArtifactSetLeaseLimits,
    RuntimePackageLease, RuntimePackageLeaseLimits, VerifiedAdmittedRuntime,
    VerifiedManagedOllamaLaunchPlan,
};

use super::{
    launch_authority::{approved_license, input},
    support::{Fixture, verify},
};

const REQUIRE_LIVE: &str = "REWRITE_APP_REQUIRE_MANAGED_LAUNCH_LIVE";
const FIXTURE_BINARY: &str = "REWRITE_APP_MANAGED_LAUNCH_FIXTURE";
const ISOLATION_HELPER: &str = "REWRITE_ISOLATION_TEST_HELPER";

#[test]
#[ignore = "requires privileged Linux managed isolation"]
fn exact_verified_launch_reobserves_connects_and_closes() {
    if std::env::var_os(REQUIRE_LIVE).is_none() {
        return;
    }
    let cancellation = CancellationToken::new();
    let model_fixture = Fixture::new(false);
    let model_lease = verify(&model_fixture);
    let verified_input = input(&model_fixture, &model_lease);
    let license = approved_license(&model_lease, crate::ModelLicensePermission::LocalGeneration);
    let expected_control = license.control_id().clone();
    let authorization = PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(
        &model_lease,
        verified_input,
        license,
    )
    .expect("exact model authorities join");

    let runtime = RuntimeFixture::new(required_path(FIXTURE_BINARY));
    let mut runtime_lease = runtime.lease();
    let launch_digest = crate::managed_ollama_v0_32_15_launch_spec().redacted_digest();
    let admitted =
        VerifiedAdmittedRuntime::exact_launch_test_fixture(&runtime.package, launch_digest);
    let helper = required_path(ISOLATION_HELPER);
    let helper_bytes = fs::read(&helper).expect("read retained isolation helper fixture");
    let isolation = PreparedIsolation::prepare(
        &helper,
        &Digest::sha256(&helper_bytes),
        u64::try_from(helper_bytes.len()).expect("helper byte size"),
        IsolationPolicy::new(
            Duration::from_secs(10),
            Duration::from_secs(5),
            8,
            16,
            4_096,
            256,
            64,
        )
        .expect("closed live isolation policy"),
        &cancellation,
    )
    .expect("prepare complete native isolation");

    let managed = PackageAttestationService::launch_managed_ollama_v0_32_15(
        &runtime.package,
        &mut runtime_lease,
        &admitted,
        &isolation,
        authorization,
        &cancellation,
    )
    .expect("launch exact verified managed Ollama fixture");
    let initial = managed.initial_evidence();
    assert!(!initial.runtime_inputs().is_empty());
    assert_eq!(managed.input_evidence().member_count(), 6);
    assert_eq!(
        managed.model_target().artifact_id(),
        managed.input_evidence().model_artifact_id()
    );
    assert_eq!(managed.model_license_control_id(), &expected_control);
    assert_eq!(
        managed.input_bound_launch_spec_digest(),
        &initial
            .runtime_inputs()
            .input_bound_launch_digest(managed.plain_launch_spec_digest())
    );
    assert_eq!(
        managed.isolation_policy_digest(),
        &isolation.policy_digest()
    );
    assert_eq!(
        managed.plain_launch_spec_digest(),
        admitted.startup_launch_spec_digest()
    );
    assert_eq!(
        managed.retained_model_weight().artifact_id(),
        managed.input_evidence().model_artifact_id()
    );
    assert!(managed.binds_exact_model_package(&model_lease));
    assert!(managed.binds_exact_runtime_package(&runtime_lease));
    let live_subject = managed.live_subject_token();
    assert!(managed.binds_live_subject(&live_subject));
    let runtime_subject = managed.runtime_package_identity_token();
    assert!(runtime_lease.binds_identity_token(&runtime_subject));
    assert_eq!(
        managed.reobserve(&cancellation).expect("stable launch"),
        initial
    );
    managed
        .revalidate_model_package(&cancellation)
        .expect("stable model authority");

    let channel = managed
        .connect_loopback(&cancellation)
        .expect("connect exact namespace-local Ollama endpoint");
    let (mut stream, diagnostics, _capture) = channel.into_parts();
    stream.write_all(b"PING").expect("send fixture request");
    let mut response = [0_u8; 4];
    stream
        .read_exact(&mut response)
        .expect("read fixture response");
    assert_eq!(&response, b"PONG");
    drop(diagnostics);
    drop(stream);
    assert_eq!(
        managed
            .reobserve(&cancellation)
            .expect("stable after traffic"),
        initial
    );
    let debug = format!("{managed:?}");
    assert!(debug.contains("ManagedOllamaIsolationLease"));
    assert!(!debug.contains(model_fixture.package.source().locator()));

    let cancelled_operation = CancellationToken::new();
    cancelled_operation.cancel();
    managed
        .close(&cancelled_operation)
        .expect("mandatory cleanup and final model revalidation");
}

#[test]
#[ignore = "requires privileged Linux managed isolation"]
fn relationship_and_admitted_launch_substitutions_fail_before_start() {
    if std::env::var_os(REQUIRE_LIVE).is_none() {
        return;
    }
    let cancellation = CancellationToken::new();
    let (model_fixture, model_lease) = verified_model();
    let (runtime, mut runtime_lease, _admitted, isolation) = verified_runtime();
    let changed_package = runtime.with_version("0.32.14");
    let admitted = VerifiedAdmittedRuntime::exact_launch_test_fixture(
        &runtime.package,
        crate::managed_ollama_v0_32_15_launch_spec().redacted_digest(),
    );
    let error = PackageAttestationService::launch_managed_ollama_v0_32_15(
        &changed_package,
        &mut runtime_lease,
        &admitted,
        &isolation,
        authorization(&model_fixture, &model_lease),
        &cancellation,
    )
    .expect_err("changed runtime relationship cannot start a process");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::RelationshipMismatch
    ));

    let wrong_launch = VerifiedAdmittedRuntime::exact_launch_test_fixture(
        &runtime.package,
        Digest::sha256(b"unauthorized managed launch profile"),
    );
    let error = PackageAttestationService::launch_managed_ollama_v0_32_15(
        &runtime.package,
        &mut runtime_lease,
        &wrong_launch,
        &isolation,
        authorization(&model_fixture, &model_lease),
        &cancellation,
    )
    .expect_err("another admitted launch profile cannot start a process");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::UnauthorizedLaunch
    ));
}

#[test]
#[ignore = "requires privileged Linux managed isolation"]
fn cancelled_launch_fails_before_materialization() {
    if std::env::var_os(REQUIRE_LIVE).is_none() {
        return;
    }
    let (model_fixture, model_lease) = verified_model();
    let (runtime, mut runtime_lease, admitted, isolation) = verified_runtime();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let error = PackageAttestationService::launch_managed_ollama_v0_32_15(
        &runtime.package,
        &mut runtime_lease,
        &admitted,
        &isolation,
        authorization(&model_fixture, &model_lease),
        &cancelled,
    )
    .expect_err("cancelled launch cannot materialize private input");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::Package(PackageAttestationError::Cancelled)
    ));
}

#[test]
#[ignore = "requires privileged Linux managed isolation"]
fn post_launch_model_tree_drift_is_rejected_and_cleanup_still_runs() {
    if std::env::var_os(REQUIRE_LIVE).is_none() {
        return;
    }
    let cancellation = CancellationToken::new();
    let (model_fixture, model_lease) = verified_model();
    let (runtime, mut runtime_lease, admitted, isolation) = verified_runtime();
    let managed = PackageAttestationService::launch_managed_ollama_v0_32_15(
        &runtime.package,
        &mut runtime_lease,
        &admitted,
        &isolation,
        authorization(&model_fixture, &model_lease),
        &cancellation,
    )
    .expect("launch exact fixture before drift");
    fs::write(
        super::support::set_root(&model_fixture).join("unexpected-after-launch.bin"),
        b"unexpected",
    )
    .expect("drift model tree after launch");
    let error = managed
        .reobserve(&cancellation)
        .expect_err("reobserve must reject model tree drift");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::ModelAuthority(ManagedOllamaModelAuthorityError::Revalidation(
            ModelLicenseControlError::Lease(_)
        ))
    ));
    let close = managed
        .close(&cancellation)
        .expect_err("cleanup retains final authority failure");
    assert!(matches!(
        close,
        ManagedOllamaCloseError::ModelAuthority(ManagedOllamaModelAuthorityError::Revalidation(
            ModelLicenseControlError::Lease(_)
        ))
    ));
}

fn verified_model() -> (Fixture, crate::VerifiedManagedOllamaModelPackageLease) {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    (fixture, lease)
}

fn authorization<'lease>(
    fixture: &Fixture,
    lease: &'lease crate::VerifiedManagedOllamaModelPackageLease,
) -> VerifiedManagedOllamaLaunchPlan<'lease> {
    PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(
        lease,
        input(fixture, lease),
        approved_license(lease, crate::ModelLicensePermission::LocalGeneration),
    )
    .expect("exact model authorities join")
}

fn verified_runtime() -> (
    RuntimeFixture,
    RuntimePackageLease,
    VerifiedAdmittedRuntime,
    PreparedIsolation,
) {
    let runtime = RuntimeFixture::new(required_path(FIXTURE_BINARY));
    let runtime_lease = runtime.lease();
    let admitted = VerifiedAdmittedRuntime::exact_launch_test_fixture(
        &runtime.package,
        crate::managed_ollama_v0_32_15_launch_spec().redacted_digest(),
    );
    let helper = required_path(ISOLATION_HELPER);
    let helper_bytes = fs::read(&helper).expect("read retained isolation helper fixture");
    let isolation = PreparedIsolation::prepare(
        &helper,
        &Digest::sha256(&helper_bytes),
        u64::try_from(helper_bytes.len()).expect("helper byte size"),
        IsolationPolicy::new(
            Duration::from_secs(10),
            Duration::from_secs(5),
            8,
            16,
            4_096,
            256,
            64,
        )
        .expect("closed live isolation policy"),
        &CancellationToken::new(),
    )
    .expect("prepare complete native isolation");
    (runtime, runtime_lease, admitted, isolation)
}

struct RuntimeFixture {
    _directory: tempfile::TempDir,
    repository: ArtifactRepository,
    manifest: ArtifactSetManifest,
    package: RuntimePackageManifest,
}

impl RuntimeFixture {
    fn new(fixture_binary: PathBuf) -> Self {
        let directory = tempfile::tempdir_in("/tmp").expect("runtime fixture root");
        let source_root = directory.path().join("source");
        let files = [
            (
                "bin/ollama",
                fs::read(fixture_binary).expect("read fixture binary"),
            ),
            ("legal/license.txt", b"fixture runtime license".to_vec()),
            (
                "provenance/source.json",
                b"{\"source\":\"managed launch live fixture\"}".to_vec(),
            ),
        ];
        for (path, bytes) in &files {
            let target = source_root.join(path);
            fs::create_dir_all(target.parent().expect("runtime member parent"))
                .expect("create runtime source directory");
            fs::write(&target, bytes).expect("write runtime source member");
            if *path == "bin/ollama" {
                fs::set_permissions(target, fs::Permissions::from_mode(0o755))
                    .expect("make runtime fixture executable");
            }
        }
        let manifest = ArtifactSetManifest::new(
            files
                .iter()
                .map(|(path, bytes)| {
                    ArtifactSetMember::new(
                        ArtifactId::from_digest(Digest::sha256(bytes)),
                        u64::try_from(bytes.len()).expect("runtime member bytes"),
                        relative(path),
                    )
                })
                .collect(),
        )
        .expect("exact runtime artifact set");
        let total_bytes = manifest.total_byte_size();
        let repository =
            ArtifactRepository::new(directory.path().join("repository")).expect("repository");
        repository
            .import_set(
                &OfflineArtifactSetImportRequest {
                    source_root,
                    manifest: manifest.clone(),
                },
                artifact_limits(total_bytes),
                &CancellationToken::new(),
            )
            .expect("import exact runtime fixture");
        let roles = [
            (
                RuntimePackageMemberRole::Entrypoint,
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            (
                RuntimePackageMemberRole::LicenseText,
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
            (
                RuntimePackageMemberRole::ProvenanceRecord,
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ];
        let members = manifest
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
        let package = RuntimePackageManifest::new(
            &manifest,
            "ollama",
            "0.32.15",
            Some("managed-launch-live-fixture".to_owned()),
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxGnuLibc,
            )
            .expect("exact managed Ollama target"),
            PackageSource::new(
                PackageSourceKind::LocalArchive,
                "local-managed-launch-live-fixture",
                "managed-launch-live-fixture",
                Digest::sha256(b"managed launch live fixture provenance"),
            )
            .expect("runtime fixture source"),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"managed launch live fixture transformation"),
            },
            members,
        )
        .expect("exact managed Ollama runtime package");
        Self {
            _directory: directory,
            repository,
            manifest,
            package,
        }
    }

    fn with_version(&self, version: &str) -> RuntimePackageManifest {
        RuntimePackageManifest::new(
            &self.manifest,
            self.package.runtime_family(),
            version,
            Some("managed-launch-live-fixture".to_owned()),
            self.package.target().clone(),
            self.package.source().clone(),
            self.package.transformation().clone(),
            self.package.members().to_vec(),
        )
        .expect("structurally valid changed runtime version")
    }

    fn lease(&self) -> RuntimePackageLease {
        let total_bytes = self
            .package
            .members()
            .iter()
            .map(|member| member.byte_size())
            .sum();
        let artifact_set = self
            .repository
            .lease_set(
                self.package.artifact_set_id(),
                lease_limits(total_bytes),
                &CancellationToken::new(),
            )
            .expect("lease exact runtime fixture");
        PackageAttestationService::attest_runtime(
            artifact_set,
            &self.package,
            RuntimePackageLeaseLimits {
                maximum_code_members: 1,
                maximum_code_member_bytes: total_bytes,
                maximum_code_bytes: total_bytes,
            },
            &CancellationToken::new(),
        )
        .expect("attest exact runtime fixture")
    }
}

fn artifact_limits(total_bytes: u64) -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 3,
        maximum_member_bytes: total_bytes,
        maximum_total_bytes: total_bytes,
        maximum_tree_entries: 8,
        maximum_storage_entries: 3,
        maximum_staging_entries: 3,
    }
}

fn lease_limits(total_bytes: u64) -> RuntimeArtifactSetLeaseLimits {
    RuntimeArtifactSetLeaseLimits {
        maximum_members: 3,
        maximum_member_bytes: total_bytes,
        maximum_total_bytes: total_bytes,
        maximum_tree_entries: 8,
        maximum_storage_entries: 3,
    }
}

fn relative(path: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(path).expect("canonical runtime fixture path")
}

fn required_path(name: &str) -> PathBuf {
    let path = std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must name the retained live fixture executable"));
    assert!(Path::new(&path).is_file(), "{name} must name a file");
    path
}
