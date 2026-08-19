use std::path::Path;

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
use rusqlite::Connection;
use tempfile::tempdir;

use super::{reserve_file, schema_version};
use crate::ArtifactStateStore;

struct EvidenceFixture {
    artifact_set: ArtifactSetManifest,
    build: RuntimeBuildIdentity,
    state: EffectiveRuntimeState,
    package: EffectivePackageEvidence,
    qualification: QualificationRecordV2,
}

#[test]
fn bytes_survive_verified_backup_and_schema_four_migration() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("schema-three-evidence.db");
    let backup = directory.path().join("schema-three-evidence-backup.db");
    seed_schema_three_evidence(&source);
    let before = evidence_rows(&Connection::open(&source).expect("open schema-three source"));

    let mut backup_file = reserve_file(&backup);
    let mut session = ArtifactStateStore::begin_existing_migration(&source)
        .expect("begin schema-three migration");
    assert_eq!(session.schema_status().found, 3);
    session
        .backup_to(&mut backup_file, 16 * 1024 * 1024, || false)
        .expect("retain verified schema-three backup");
    session.migrate().expect("migrate schema three to five");

    let backup_connection = Connection::open(&backup).expect("open retained schema-three backup");
    assert_eq!(schema_version(&backup), 3);
    assert_eq!(evidence_rows(&backup_connection), before);
    assert!(!table_exists(&backup_connection, "installed_artifact_sets"));
    assert!(!table_exists(&backup_connection, "artifact_set_removals"));

    let migrated = Connection::open(&source).expect("open migrated schema-five source");
    assert_eq!(schema_version(&source), 5);
    assert_eq!(evidence_rows(&migrated), before);
    assert!(table_exists(&migrated, "installed_artifact_sets"));
    assert!(table_exists(&migrated, "artifact_set_removals"));
    let installed_sets: u32 = migrated
        .query_row("SELECT COUNT(*) FROM installed_artifact_sets", [], |row| {
            row.get(0)
        })
        .expect("count migrated artifact-set installations");
    assert_eq!(installed_sets, 0);
}

fn seed_schema_three_evidence(path: &Path) {
    let fixture = evidence_fixture();
    let mut store = ArtifactStateStore::open(path).expect("create current evidence store");
    store
        .put_artifact_set_manifest(&fixture.artifact_set)
        .expect("store artifact-set manifest");
    store
        .put_runtime_build_identity(&fixture.build)
        .expect("store runtime build");
    store
        .put_effective_runtime_state(&fixture.state)
        .expect("store effective state");
    store
        .put_effective_package_evidence(&fixture.package)
        .expect("store effective package");
    store
        .put_qualification_v2(&fixture.qualification)
        .expect("store qualification v2");
    drop(store);

    Connection::open(path)
        .expect("open evidence store for schema-three fixture")
        .execute_batch(
            "DROP TABLE artifact_set_removals;
             DROP TABLE installed_artifact_sets;
             PRAGMA user_version = 3;",
        )
        .expect("restore exact schema-three shape");
    assert_eq!(
        ArtifactStateStore::inspect_existing_schema(path)
            .expect("validate populated schema-three fixture")
            .found,
        3
    );
}

fn evidence_fixture() -> EvidenceFixture {
    let artifact_set = ArtifactSetManifest::new(vec![ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(b"schema-three-weights")),
        20,
        ArtifactSetRelativePath::new("model.gguf").expect("portable member path"),
    )])
    .expect("artifact-set fixture");
    let build = runtime_build();
    let state = EffectiveRuntimeState::new(
        &build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: "llama-server-snapshot".to_owned(),
            provider_snapshot_schema_version: 1,
            provider_snapshot_digest: digest("provider-snapshot"),
            launch_policy_digest: digest("launch-policy"),
            loaded_components_digest: digest("loaded-components"),
            effective_configuration_digest: digest("effective-configuration"),
            platform_digest: digest("platform"),
            execution_class_digest: digest("execution-class"),
            isolation_policy_digest: digest("isolation-policy"),
            effective_context_tokens: 8_192,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
        },
    )
    .expect("effective-state fixture");
    let package = effective_package(&artifact_set, &build, &state);
    let qualification = qualification(&artifact_set, &build, &state, &package);
    EvidenceFixture {
        artifact_set,
        build,
        state,
        package,
        qualification,
    }
}

fn runtime_build() -> RuntimeBuildIdentity {
    RuntimeBuildIdentity::new(RuntimeBuildIdentityInput {
        mode: RuntimeBuildMode::ManagedProcess,
        runtime_family: "llama-server".to_owned(),
        reported_version: "fixture-v3".to_owned(),
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
        build_configuration_digest: digest("build-configuration"),
    })
    .expect("runtime-build fixture")
}

fn effective_package(
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
) -> EffectivePackageEvidence {
    let member = EffectivePackageMemberEvidence::new(
        artifact_set.members()[0].relative_path().clone(),
        vec![EffectivePackageMemberPurpose::ModelWeights],
    )
    .expect("member evidence");
    EffectivePackageEvidence::new(
        artifact_set,
        build,
        state,
        EffectivePackageEvidenceInput {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: "managed-package-attestor".to_owned(),
            evidence_contract_schema_version: 1,
            member_evidence: vec![member],
            artifact_set_completeness_evidence_digest: digest("completeness"),
            acquisition_evidence_digest: digest("acquisition"),
            license_review_evidence_digest: digest("license-review"),
            transformation: PackageTransformationDisposition::Untransformed {
                evidence_digest: digest("untransformed"),
            },
            runtime_load_closure_evidence_digest: digest("load-closure"),
            exclusion_isolation_evidence_digest: digest("exclusion-isolation"),
        },
    )
    .expect("effective-package fixture")
}

fn qualification(
    artifact_set: &ArtifactSetManifest,
    build: &RuntimeBuildIdentity,
    state: &EffectiveRuntimeState,
    package: &EffectivePackageEvidence,
) -> QualificationRecordV2 {
    QualificationRecordV2::new(
        artifact_set,
        build,
        state,
        package,
        QualificationRecordV2Input {
            source_byte_limit: 1_048_576,
            context_token_limit: 8_192,
            prompt_template_digest: digest("prompt-template"),
            claim_output_contract_digest: digest("claim-output-contract"),
            claim_operation_contract_digest: digest("claim-operation-contract"),
            request_policy_digest: digest("request-policy"),
            threshold_policy_digest: digest("threshold-policy"),
            language_policy_digest: digest("language-policy"),
            hardware_envelope_digest: digest("hardware-envelope"),
            qualification_suite_digest: digest("qualification-suite"),
            qualification_result_evidence_digest: digest("qualification-result"),
            license_decision: LicenseDecision::LocalUseOnly,
            status: QualificationStatus::Qualified,
        },
    )
    .expect("qualification-v2 fixture")
}

fn evidence_rows(connection: &Connection) -> Vec<Vec<String>> {
    let queries = [
        (
            "artifact_set_manifests",
            "SELECT artifact_set_id, record_json FROM artifact_set_manifests",
        ),
        (
            "runtime_build_identities",
            "SELECT runtime_build_id, record_json FROM runtime_build_identities",
        ),
        (
            "effective_runtime_states",
            "SELECT effective_runtime_state_id, runtime_build_id, record_json FROM effective_runtime_states",
        ),
        (
            "effective_package_evidence",
            "SELECT effective_package_evidence_id, artifact_set_id, runtime_build_id, effective_runtime_state_id, record_json FROM effective_package_evidence",
        ),
        (
            "qualification_v2_records",
            "SELECT qualification_v2_id, artifact_set_id, effective_package_evidence_id, runtime_build_id, effective_runtime_state_id, record_json FROM qualification_v2_records",
        ),
    ];
    let mut rows = Vec::new();
    for (table, query) in queries {
        let mut statement = connection.prepare(query).expect("prepare evidence query");
        let column_count = statement.column_count();
        let table_rows = statement
            .query_map([], |row| {
                let mut values = Vec::with_capacity(column_count + 1);
                values.push(table.to_owned());
                for column in 0..column_count {
                    values.push(row.get::<_, String>(column)?);
                }
                Ok(values)
            })
            .expect("query evidence rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("read evidence rows");
        assert_eq!(table_rows.len(), 1, "{table}");
        rows.extend(table_rows);
    }
    rows
}

fn table_exists(connection: &Connection, table: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1
             )",
            [table],
            |row| row.get(0),
        )
        .expect("inspect table presence")
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}
