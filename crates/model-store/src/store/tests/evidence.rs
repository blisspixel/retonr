use tempfile::tempdir;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, ComputeBackend,
    EffectivePackageEvidence, EffectivePackageEvidenceInput, EffectivePackageEvidenceMode,
    EffectivePackageMemberEvidence, EffectivePackageMemberPurpose, EffectiveRuntimeState,
    EffectiveRuntimeStateInput, ExecutionPlacement, LicenseDecision,
    PackageTransformationDisposition, QualificationRecordV2, QualificationRecordV2Input,
    QualificationStatus, RuntimeAbi, RuntimeArchitecture, RuntimeBuildIdentity,
    RuntimeBuildIdentityInput, RuntimeBuildMode, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::{ArtifactStateStore, WriteDisposition};
use crate::StoreError;

struct EvidenceFixture {
    artifact_set: ArtifactSetManifest,
    build: RuntimeBuildIdentity,
    state: EffectiveRuntimeState,
    package: EffectivePackageEvidence,
    qualification: QualificationRecordV2,
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn evidence_fixture() -> EvidenceFixture {
    let artifact_set = ArtifactSetManifest::new(vec![
        artifact_member("config.json", b"{}"),
        artifact_member("model.gguf", b"weights"),
    ])
    .expect("artifact set");
    let build = runtime_build("b10417");
    let state = EffectiveRuntimeState::new(
        &build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "llama-server-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("provider snapshot"),
            launch_policy_digest: digest("launch"),
            loaded_components_digest: digest("loaded components"),
            effective_configuration_digest: digest("effective configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("execution class"),
            isolation_policy_digest: digest("isolation"),
            effective_context_tokens: 32_768,
            compute_backend: ComputeBackend::Cuda,
            placement: ExecutionPlacement::AcceleratorOnly,
        },
    )
    .expect("runtime state");
    let package = effective_package(&artifact_set, &build, &state, "acquisition");
    let qualification = QualificationRecordV2::new(
        &artifact_set,
        &build,
        &state,
        &package,
        QualificationRecordV2Input {
            source_byte_limit: 16_777_216,
            context_token_limit: 32_768,
            prompt_template_digest: digest("prompt template"),
            claim_output_contract_digest: digest("claim output contract"),
            claim_operation_contract_digest: digest("claim operation contract"),
            request_policy_digest: digest("request policy"),
            threshold_policy_digest: digest("threshold policy"),
            language_policy_digest: digest("language policy"),
            hardware_envelope_digest: digest("hardware envelope"),
            qualification_suite_digest: digest("qualification suite"),
            qualification_result_evidence_digest: digest("qualification result"),
            license_decision: LicenseDecision::LocalUseOnly,
            status: QualificationStatus::Qualified,
        },
    )
    .expect("qualification v2");
    EvidenceFixture {
        artifact_set,
        build,
        state,
        package,
        qualification,
    }
}

fn artifact_member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size"),
        ArtifactSetRelativePath::new(path).expect("member path"),
    )
}

fn runtime_build(version: &str) -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode: RuntimeBuildMode::ManagedProcess,
        runtime_family: "llama-server".to_owned(),
        reported_version: version.to_owned(),
        build_revision: Some("0123456789abcdef".to_owned()),
        target: RuntimeTarget::new(
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsMsvc,
        )
        .expect("runtime target"),
        package_manifest_digest: digest("package"),
        entrypoint_digest: digest("entrypoint"),
        packaged_dependencies_digest: digest("dependencies"),
        build_configuration_digest: digest("build configuration"),
    })
    .expect("runtime build")
}

fn effective_package(
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
    acquisition: &str,
) -> EffectivePackageEvidence {
    let member_evidence = artifact_set
        .members()
        .iter()
        .map(|member| {
            let purpose = if member.relative_path().as_str() == "model.gguf" {
                EffectivePackageMemberPurpose::ModelWeights
            } else {
                EffectivePackageMemberPurpose::ModelConfiguration
            };
            EffectivePackageMemberEvidence::new(member.relative_path().clone(), vec![purpose])
                .expect("member evidence")
        })
        .collect();
    EffectivePackageEvidence::new(
        artifact_set,
        build,
        state,
        EffectivePackageEvidenceInput {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: "managed-package-attestor".to_owned(),
            evidence_contract_schema_version: 1,
            member_evidence,
            artifact_set_completeness_evidence_digest: digest("completeness"),
            acquisition_evidence_digest: digest(acquisition),
            license_review_evidence_digest: digest("license review"),
            transformation: PackageTransformationDisposition::Untransformed {
                evidence_digest: digest("untransformed"),
            },
            runtime_load_closure_evidence_digest: digest("load closure"),
            exclusion_isolation_evidence_digest: digest("exclusion isolation"),
        },
    )
    .expect("effective package")
}

fn store_dependencies(store: &mut ArtifactStateStore, fixture: &EvidenceFixture) {
    store
        .put_artifact_set_manifest(&fixture.artifact_set)
        .expect("store artifact set");
    store
        .put_runtime_build_identity(&fixture.build)
        .expect("store runtime build");
    store
        .put_effective_runtime_state(&fixture.state)
        .expect("store runtime state");
    store
        .put_effective_package_evidence(&fixture.package)
        .expect("store package evidence");
}

#[test]
fn persists_and_recovers_the_complete_inert_evidence_chain() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("state.db");
    let fixture = evidence_fixture();
    {
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        store_dependencies(&mut store, &fixture);
        assert_eq!(
            store
                .put_qualification_v2(&fixture.qualification)
                .expect("store qualification"),
            WriteDisposition::Inserted
        );
        assert_eq!(
            store
                .put_qualification_v2(&fixture.qualification)
                .expect("repeat qualification"),
            WriteDisposition::AlreadyPresent
        );
        assert!(
            store
                .recover_active_bindings(|_| true)
                .expect("bindings")
                .is_empty()
        );
    }

    let store = ArtifactStateStore::open_existing_read_only(&path).expect("reopen store");
    assert_eq!(
        store
            .artifact_set_manifest(&fixture.artifact_set.artifact_set_id())
            .expect("artifact set"),
        Some(fixture.artifact_set)
    );
    assert_eq!(
        store
            .runtime_build_identity(&fixture.build.runtime_build_id())
            .expect("runtime build"),
        Some(fixture.build)
    );
    assert_eq!(
        store
            .effective_runtime_state(&fixture.state.effective_runtime_state_id())
            .expect("runtime state"),
        Some(fixture.state)
    );
    assert_eq!(
        store
            .effective_package_evidence(&fixture.package.effective_package_evidence_id())
            .expect("package evidence"),
        Some(fixture.package)
    );
    assert_eq!(
        store
            .qualification_v2(&fixture.qualification.qualification_v2_id())
            .expect("qualification"),
        Some(fixture.qualification)
    );
}

#[test]
fn dependent_evidence_requires_prior_exact_records() {
    let directory = tempdir().expect("temporary directory");
    let fixture = evidence_fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");

    assert!(matches!(
        store.put_effective_runtime_state(&fixture.state),
        Err(StoreError::MissingRecord)
    ));
    store
        .put_runtime_build_identity(&fixture.build)
        .expect("store build");
    store
        .put_effective_runtime_state(&fixture.state)
        .expect("store state");
    assert!(matches!(
        store.put_effective_package_evidence(&fixture.package),
        Err(StoreError::MissingRecord)
    ));
    store
        .put_artifact_set_manifest(&fixture.artifact_set)
        .expect("store set");
    assert!(matches!(
        store.put_qualification_v2(&fixture.qualification),
        Err(StoreError::MissingRecord)
    ));
    store
        .put_effective_package_evidence(&fixture.package)
        .expect("store package");
    store
        .put_qualification_v2(&fixture.qualification)
        .expect("store qualification");
}

#[test]
fn indexed_reference_corruption_fails_closed() {
    let directory = tempdir().expect("temporary directory");
    let fixture = evidence_fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store_dependencies(&mut store, &fixture);
    let other_build = runtime_build("b10418");
    store
        .put_runtime_build_identity(&other_build)
        .expect("store alternate build");
    store
        .connection()
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable test foreign keys");
    store
        .connection()
        .execute(
            "UPDATE effective_runtime_states SET runtime_build_id = ?1
             WHERE effective_runtime_state_id = ?2",
            rusqlite::params![
                other_build.runtime_build_id().digest().as_str(),
                fixture.state.effective_runtime_state_id().digest().as_str()
            ],
        )
        .expect("corrupt indexed build");

    assert!(matches!(
        store.effective_runtime_state(&fixture.state.effective_runtime_state_id()),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        store.put_qualification_v2(&fixture.qualification),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn noncanonical_record_bytes_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let fixture = evidence_fixture();
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store
        .put_artifact_set_manifest(&fixture.artifact_set)
        .expect("store artifact set");
    store
        .connection()
        .execute(
            "UPDATE artifact_set_manifests SET record_json = ' ' || record_json
             WHERE artifact_set_id = ?1",
            [fixture.artifact_set.artifact_set_id().digest().as_str()],
        )
        .expect("make JSON noncanonical");
    assert!(matches!(
        store.artifact_set_manifest(&fixture.artifact_set.artifact_set_id()),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn qualification_indexed_subject_mismatch_fails_closed() {
    let directory = tempdir().expect("temporary directory");
    let fixture = evidence_fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    store_dependencies(&mut store, &fixture);
    store
        .put_qualification_v2(&fixture.qualification)
        .expect("store qualification");
    let alternate = effective_package(
        &fixture.artifact_set,
        &fixture.build,
        &fixture.state,
        "alternate acquisition",
    );
    store
        .put_effective_package_evidence(&alternate)
        .expect("store alternate package");
    store
        .connection()
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable test foreign keys");
    store
        .connection()
        .execute(
            "UPDATE qualification_v2_records SET effective_package_evidence_id = ?1
             WHERE qualification_v2_id = ?2",
            rusqlite::params![
                alternate.effective_package_evidence_id().digest().as_str(),
                fixture
                    .qualification
                    .qualification_v2_id()
                    .digest()
                    .as_str()
            ],
        )
        .expect("corrupt indexed package");

    assert!(matches!(
        store.qualification_v2(&fixture.qualification.qualification_v2_id()),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        store.put_qualification_v2(&fixture.qualification),
        Err(StoreError::CorruptRecord)
    ));
}
