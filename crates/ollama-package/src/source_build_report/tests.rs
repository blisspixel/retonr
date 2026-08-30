use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};

use rewrite_model::ArtifactSetRelativePath;
use rewrite_types::Digest;
use serde_json::{Value, json};

use super::*;
use crate::{
    MemberOpenError, RuntimeSourceBuildInputLimits, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildPlan, reconstruct_runtime_package_with_limits,
    review_v2::{controlled_runtime_fixture, controlled_source_fixture},
};

struct Fixture {
    report: Vec<u8>,
    source_inputs: RuntimeSourceBuildInputManifest,
    plan: RuntimeSourceBuildPlan,
    evidence: BTreeMap<(RuntimeSourceBuildAttempt, String), Vec<u8>>,
    primary_members: BTreeMap<String, Vec<u8>>,
    rebuild_members: BTreeMap<String, Vec<u8>>,
    primary_entrypoint_mode: u32,
    rebuild_entrypoint_mode: u32,
}

mod contract;
mod verification;

fn fixture(different: bool) -> Fixture {
    let (_source_id, source_bytes, _components) = controlled_source_fixture();
    let source_inputs = RuntimeSourceBuildInputManifest::parse(
        &source_bytes,
        RuntimeSourceBuildInputLimits::default(),
    )
    .expect("source manifest");
    let plan = RuntimeSourceBuildPlan::for_legacy_read_only_verification(&source_inputs);
    let (primary_layout, primary_members, _) = controlled_runtime_fixture(plan.source_inputs_id());
    let mut rebuild_layout = primary_layout.clone();
    let mut rebuild_members = primary_members.clone();
    if different {
        let selected = rebuild_members.get_mut("bin/ollama").expect("entrypoint");
        *selected.last_mut().expect("nonempty entrypoint") ^= 1;
        let mut value: Value = serde_json::from_slice(&rebuild_layout).expect("layout value");
        value["members"][0]["digest"] = json!(Digest::sha256(selected));
        rebuild_layout = canonical(&value);
    }
    let evidence = evidence_map(primary_layout, rebuild_layout);
    let mut fixture = Fixture {
        report: Vec::new(),
        source_inputs,
        plan,
        evidence,
        primary_members,
        rebuild_members,
        primary_entrypoint_mode: 0o755,
        rebuild_entrypoint_mode: 0o755,
    };
    rebuild_report(&mut fixture);
    fixture
}

fn evidence_map(
    primary_layout: Vec<u8>,
    rebuild_layout: Vec<u8>,
) -> BTreeMap<(RuntimeSourceBuildAttempt, String), Vec<u8>> {
    let mut evidence = BTreeMap::new();
    for (attempt, name, layout) in [
        (
            RuntimeSourceBuildAttempt::Primary,
            "primary",
            primary_layout,
        ),
        (
            RuntimeSourceBuildAttempt::Rebuild,
            "rebuild",
            rebuild_layout,
        ),
    ] {
        for (kind, file, bytes) in [
            ("runtime_layout", "runtime-layout.json", layout),
            ("sbom", "sbom.json", br#"{"packages":[]}"#.to_vec()),
            (
                "provenance",
                "provenance.json",
                br#"{"builder":"fixture"}"#.to_vec(),
            ),
            (
                "transformation",
                "transformation.json",
                br#"{"steps":[]}"#.to_vec(),
            ),
            ("standard_output", "stdout.bin", Vec::new()),
            ("standard_error", "stderr.bin", Vec::new()),
        ] {
            let path = format!("attempts/{name}/{file}");
            let _ = kind;
            evidence.insert((attempt, path), bytes);
        }
    }
    evidence
}

fn rebuild_report(fixture: &mut Fixture) {
    rebuild_output_tree_sidecar(fixture, RuntimeSourceBuildAttempt::Primary);
    rebuild_output_tree_sidecar(fixture, RuntimeSourceBuildAttempt::Rebuild);
    rebuild_execution_receipt(fixture, RuntimeSourceBuildAttempt::Primary);
    rebuild_execution_receipt(fixture, RuntimeSourceBuildAttempt::Rebuild);
    let primary = reconstruct(
        fixture
            .evidence
            .get(&(
                RuntimeSourceBuildAttempt::Primary,
                "attempts/primary/runtime-layout.json".to_owned(),
            ))
            .expect("primary layout"),
        &fixture.primary_members,
    );
    let rebuild = reconstruct(
        fixture
            .evidence
            .get(&(
                RuntimeSourceBuildAttempt::Rebuild,
                "attempts/rebuild/runtime-layout.json".to_owned(),
            ))
            .expect("rebuild layout"),
        &fixture.rebuild_members,
    );
    let primary_set = primary.artifact_set().artifact_set_id();
    let rebuild_set = rebuild.artifact_set().artifact_set_id();
    let primary_package = primary.runtime_package().runtime_package_manifest_id();
    let rebuild_package = rebuild.runtime_package().runtime_package_manifest_id();
    let primary_output_tree = output_tree(fixture, RuntimeSourceBuildAttempt::Primary);
    let rebuild_output_tree = output_tree(fixture, RuntimeSourceBuildAttempt::Rebuild);
    let comparison = if primary_set == rebuild_set
        && primary_package == rebuild_package
        && primary_output_tree == rebuild_output_tree
    {
        json!({
            "artifact_set_id": primary_set,
            "output_tree_digest": primary_output_tree.digest(),
            "runtime_package_manifest_id": primary_package,
            "status": "byte_identical"
        })
    } else {
        json!({
            "primary_artifact_set_id": primary_set,
            "primary_output_tree_digest": primary_output_tree.digest(),
            "primary_runtime_package_manifest_id": primary_package,
            "rebuild_artifact_set_id": rebuild_set,
            "rebuild_output_tree_digest": rebuild_output_tree.digest(),
            "rebuild_runtime_package_manifest_id": rebuild_package,
            "status": "different"
        })
    };
    let attempts = [
        attempt_value(fixture, RuntimeSourceBuildAttempt::Primary, "primary"),
        attempt_value(fixture, RuntimeSourceBuildAttempt::Rebuild, "rebuild"),
    ];
    fixture.report = canonical(&json!({
        "attempts": attempts,
        "build_plan_digest": fixture.plan.plan_digest(),
        "comparison": comparison,
        "schema_version": RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION,
        "source_build_inputs_id": fixture.plan.source_inputs_id()
    }));
}

fn attempt_value(fixture: &Fixture, attempt: RuntimeSourceBuildAttempt, name: &str) -> Value {
    let declarations = [
        ("runtime_layout", "runtime-layout.json"),
        ("sbom", "sbom.json"),
        ("provenance", "provenance.json"),
        ("transformation", "transformation.json"),
        ("output_tree", "output-tree.json"),
        ("execution_receipt", "execution-receipt.json"),
        ("standard_output", "stdout.bin"),
        ("standard_error", "stderr.bin"),
    ]
    .into_iter()
    .map(|(kind, file)| {
        let path = format!("attempts/{name}/{file}");
        let bytes = fixture
            .evidence
            .get(&(attempt, path.clone()))
            .expect("attempt evidence");
        json!({
            "byte_size": bytes.len(),
            "digest": Digest::sha256(bytes),
            "kind": kind,
            "relative_path": path
        })
    })
    .collect::<Vec<_>>();
    let receipt_path = format!("attempts/{name}/execution-receipt.json");
    let receipt = fixture
        .evidence
        .get(&(attempt, receipt_path))
        .expect("execution receipt");
    let output_tree = output_tree(fixture, attempt);
    json!({
        "accelerator": "cpu_only",
        "attempt": match attempt {
            RuntimeSourceBuildAttempt::Primary => "primary",
            RuntimeSourceBuildAttempt::Rebuild => "rebuild"
        },
        "build_arguments_digest": fixture.plan.policy().build_arguments_digest(),
        "environment_digest": fixture.plan.policy().environment_digest(),
        "evidence": declarations,
        "execution_receipt_digest": Digest::sha256(receipt),
        "network_access": "denied",
        "output_tree_digest": output_tree.digest()
    })
}

fn rebuild_execution_receipt(fixture: &mut Fixture, attempt: RuntimeSourceBuildAttempt) {
    let name = attempt_name(attempt);
    let output_tree = output_tree(fixture, attempt);
    let bytes = execution_receipt::tests::fixture_receipt(
        attempt,
        &fixture.source_inputs,
        &fixture.plan,
        &output_tree,
    );
    fixture.evidence.insert(
        (attempt, format!("attempts/{name}/execution-receipt.json")),
        bytes,
    );
}

fn replace_execution_receipt(
    fixture: &mut Fixture,
    attempt: RuntimeSourceBuildAttempt,
    bytes: &[u8],
) {
    let (attempt_index, name) = match attempt {
        RuntimeSourceBuildAttempt::Primary => (0, "primary"),
        RuntimeSourceBuildAttempt::Rebuild => (1, "rebuild"),
    };
    fixture.evidence.insert(
        (attempt, format!("attempts/{name}/execution-receipt.json")),
        bytes.to_owned(),
    );
    let mut report = report_value(fixture);
    let evidence = &mut report["attempts"][attempt_index]["evidence"][5];
    evidence["byte_size"] = json!(bytes.len());
    evidence["digest"] = json!(Digest::sha256(bytes));
    report["attempts"][attempt_index]["execution_receipt_digest"] = json!(Digest::sha256(bytes));
    fixture.report = canonical(&report);
}

fn rebuild_output_tree_sidecar(fixture: &mut Fixture, attempt: RuntimeSourceBuildAttempt) {
    let (members, entrypoint_mode, name) = match attempt {
        RuntimeSourceBuildAttempt::Primary => (
            &fixture.primary_members,
            fixture.primary_entrypoint_mode,
            "primary",
        ),
        RuntimeSourceBuildAttempt::Rebuild => (
            &fixture.rebuild_members,
            fixture.rebuild_entrypoint_mode,
            "rebuild",
        ),
    };
    let mut entries = Vec::new();
    let mut directories = BTreeSet::new();
    for file in [
        "runtime-layout.json",
        "sbom.json",
        "provenance.json",
        "transformation.json",
    ] {
        let bytes = fixture
            .evidence
            .get(&(attempt, format!("attempts/{name}/{file}")))
            .expect("build evidence");
        entries.push(
            RuntimeSourceBuildOutputTreeEntry::regular_file(
                ArtifactSetRelativePath::new(file).expect("evidence path"),
                0o644,
                u64::try_from(bytes.len()).expect("evidence length fits u64"),
                Digest::sha256(bytes),
            )
            .expect("evidence tree entry"),
        );
    }
    for (path, bytes) in members {
        collect_parent_directories(path, &mut directories);
        let mode = if path == "bin/ollama" {
            entrypoint_mode
        } else if matches!(
            path.as_str(),
            "helper/retonr-isolation" | "lib/ollama/llama-server"
        ) {
            0o755
        } else {
            0o644
        };
        entries.push(
            RuntimeSourceBuildOutputTreeEntry::regular_file(
                ArtifactSetRelativePath::new(path.clone()).expect("member path"),
                mode,
                u64::try_from(bytes.len()).expect("member length fits u64"),
                Digest::sha256(bytes),
            )
            .expect("member tree entry"),
        );
    }
    entries.extend(directories.into_iter().map(|path| {
        RuntimeSourceBuildOutputTreeEntry::directory(
            ArtifactSetRelativePath::new(path).expect("directory path"),
            0o755,
        )
        .expect("directory tree entry")
    }));
    let tree = RuntimeSourceBuildOutputTree::compile(entries).expect("output tree sidecar");
    fixture.evidence.insert(
        (attempt, format!("attempts/{name}/output-tree.json")),
        tree.canonical_bytes().to_vec(),
    );
}

fn collect_parent_directories(path: &str, directories: &mut BTreeSet<String>) {
    let mut prefix = String::new();
    let components = path.split('/').collect::<Vec<_>>();
    for component in components.iter().take(components.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        directories.insert(prefix.clone());
    }
}

fn output_tree(
    fixture: &Fixture,
    attempt: RuntimeSourceBuildAttempt,
) -> RuntimeSourceBuildOutputTree {
    let name = attempt_name(attempt);
    RuntimeSourceBuildOutputTree::parse(
        fixture
            .evidence
            .get(&(attempt, format!("attempts/{name}/output-tree.json")))
            .expect("output tree evidence"),
    )
    .expect("valid output tree evidence")
}

fn verify(
    fixture: &Fixture,
) -> Result<VerifiedRuntimeSourceBuildReport, RuntimeSourceBuildReportError> {
    verify_bytes(fixture, &fixture.report)
}

fn compile_fixture(
    fixture: &Fixture,
) -> Result<CompiledRuntimeSourceBuildReport, RuntimeSourceBuildReportError> {
    compile_runtime_source_build_report(
        &fixture.source_inputs,
        &fixture.plan,
        &RuntimeSourceBuildReportLimits::default(),
        |attempt, path| {
            fixture
                .evidence
                .get(&(attempt, path.as_str().to_owned()))
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildReportOpenError)
        },
        |attempt, path| open_member(fixture, attempt, path),
        || false,
    )
}

fn verify_bytes(
    fixture: &Fixture,
    bytes: &[u8],
) -> Result<VerifiedRuntimeSourceBuildReport, RuntimeSourceBuildReportError> {
    verify_runtime_source_build_report(
        bytes,
        &fixture.source_inputs,
        &fixture.plan,
        &RuntimeSourceBuildReportLimits::default(),
        |attempt, path| {
            fixture
                .evidence
                .get(&(attempt, path.as_str().to_owned()))
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildReportOpenError)
        },
        |attempt, path| open_member(fixture, attempt, path),
        || false,
    )
}

fn open_member(
    fixture: &Fixture,
    attempt: RuntimeSourceBuildAttempt,
    path: &ArtifactSetRelativePath,
) -> Result<Cursor<Vec<u8>>, MemberOpenError> {
    let members = match attempt {
        RuntimeSourceBuildAttempt::Primary => &fixture.primary_members,
        RuntimeSourceBuildAttempt::Rebuild => &fixture.rebuild_members,
    };
    members
        .get(path.as_str())
        .cloned()
        .map(Cursor::new)
        .ok_or(MemberOpenError)
}

fn reconstruct(
    layout: &[u8],
    members: &BTreeMap<String, Vec<u8>>,
) -> crate::ReconstructedRuntimePackage {
    reconstruct_runtime_package_with_limits(
        layout,
        &crate::RuntimeLayoutLimits::default(),
        |path| {
            members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(MemberOpenError)
        },
        || false,
    )
    .expect("fixture reconstructs")
}

fn report_value(fixture: &Fixture) -> Value {
    serde_json::from_slice(&fixture.report).expect("report value")
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("fixture serializes")
}
