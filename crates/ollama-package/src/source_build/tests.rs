use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
};
use rewrite_types::Digest;
use serde_json::{Value, json};

use super::*;

mod program_lineage;
mod verification;

fn manifest_value() -> Value {
    let components = vec![
        component("helper/isolation", &["isolation_helper"]),
        component("legal/licenses.json", &["license_evidence"]),
        component("metadata/build-parameters.json", &["build_parameters"]),
        component("metadata/source-provenance.json", &["source_provenance"]),
        component("metadata/tool-evidence.json", &["tool_evidence"]),
        component("modules/checksums.txt", &["go_checksum_set"]),
        component("modules/example.zip", &["go_module"]),
        component("packages/native.pkg", &["native_package", "posix_shell"]),
        component("patches/source.patch", &["source_patch"]),
        component("patches/source2.patch", &["source_patch"]),
        component("scripts/build.sh", &["build_script"]),
        component("sources/llama-cpp.tar.gz", &["llama_cpp_source"]),
        component("sources/ollama.tar.gz", &["ollama_source"]),
        component("toolchains/build-tools.tar", &["cmake", "ninja"]),
        component(
            "toolchains/cc.tar",
            &[
                "c_compiler",
                "cxx_compiler",
                "assembler",
                "linker",
                "standard_library",
            ],
        ),
        component("toolchains/go.tar.gz", &["go_toolchain"]),
    ];
    let artifact_set = artifact_set(&components);
    json!({
        "artifact_set_id": artifact_set.artifact_set_id(),
        "components": components,
        "policy": {
            "accelerator": "cpu_only",
            "build_arguments": ["--build-runtime", "--cpu-only", "--offline"],
            "cpu_feature_policy": "x86-64-v2",
            "environment": controlled_environment(),
            "locale": "C.UTF-8",
            "network_access": "denied",
            "source_date_epoch": 1_725_000_000_u64,
            "target": {
                "abi": "linux_gnu_libc",
                "architecture": "x86_64",
                "operating_system": "linux"
            },
            "timezone": "UTC"
        },
        "schema_version": RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION
    })
}

fn controlled_environment() -> Value {
    json!([
        {"name": "CGO_ENABLED", "value": "1"},
        {"name": "GOAMD64", "value": "v2"},
        {"name": "GOARCH", "value": "amd64"},
        {"name": "GOOS", "value": "linux"},
        {"name": "GOPROXY", "value": "off"},
        {"name": "GOSUMDB", "value": "off"},
        {"name": "LC_ALL", "value": "C.UTF-8"},
        {"name": "SOURCE_DATE_EPOCH", "value": "1725000000"},
        {"name": "TZ", "value": "UTC"}
    ])
}

fn component(path: &str, roles: &[&str]) -> Value {
    let bytes = fixture_bytes(path);
    json!({
        "byte_size": bytes.len(),
        "digest": Digest::sha256(&bytes),
        "name": path.replace('/', "-"),
        "relative_path": path,
        "revision": "fixture-v1",
        "roles": roles,
        "source_locator": format!("https://example.invalid/{path}")
    })
}

fn artifact_set(components: &[Value]) -> ArtifactSetManifest {
    ArtifactSetManifest::new(
        components
            .iter()
            .map(|component| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(
                        serde_json::from_value(component["digest"].clone()).expect("digest"),
                    ),
                    component["byte_size"].as_u64().expect("byte size"),
                    ArtifactSetRelativePath::new(
                        component["relative_path"].as_str().expect("relative path"),
                    )
                    .expect("valid fixture path"),
                )
            })
            .collect(),
    )
    .expect("fixture artifact set")
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("manifest serializes")
}

fn parse(value: &Value) -> Result<RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputError> {
    RuntimeSourceBuildInputManifest::parse(
        &canonical(value),
        RuntimeSourceBuildInputLimits::default(),
    )
}

fn assert_legacy_execution_constraints(
    plan: &RuntimeSourceBuildPlan,
    manifest: &RuntimeSourceBuildInputManifest,
) {
    let execution = plan.execution_policy();
    assert_eq!(execution.startup_timeout_seconds(), 30);
    assert_eq!(execution.shutdown_timeout_seconds(), 30);
    assert_eq!(execution.execution_timeout_seconds(), 10_800);
    assert_eq!(execution.maximum_arguments(), 256);
    assert_eq!(execution.maximum_environment_variables(), 256);
    assert_eq!(execution.maximum_value_bytes(), 65_536);
    assert_eq!(execution.maximum_open_files(), 4_096);
    assert_eq!(execution.maximum_processes(), 4_096);
    assert_eq!(execution.isolation_policy_digest().as_str().len(), 64);
    assert_eq!(
        plan.expected_launch_digest(),
        &plan::expected_launch_digest_for_test(manifest, 16 * 1024 * 1024 * 1024)
    );
    assert_ne!(
        plan.expected_launch_digest(),
        &plan::expected_launch_digest_for_test(manifest, 12 * 1024 * 1024 * 1024)
    );
}

#[test]
fn exact_controlled_build_closure_and_policy_parse() {
    let value = manifest_value();
    let manifest_bytes = canonical(&value);
    let manifest = parse(&value).expect("exact fixture parses");
    assert_eq!(manifest.components().len(), 16);
    assert_eq!(
        manifest.artifact_set().artifact_set_id(),
        serde_json::from_value::<ArtifactSetId>(value["artifact_set_id"].clone())
            .expect("artifact set id")
    );
    assert_eq!(
        manifest.policy().target().operating_system(),
        rewrite_model::RuntimeOperatingSystem::Linux
    );
    assert_eq!(
        manifest.policy().network_access(),
        RuntimeSourceBuildNetworkPolicy::Denied
    );
    assert_eq!(
        manifest.policy().accelerator(),
        RuntimeSourceBuildAcceleratorPolicy::CpuOnly
    );
    assert_eq!(manifest.policy().cpu_feature_policy(), "x86-64-v2");
    assert_eq!(manifest.policy().locale(), "C.UTF-8");
    assert_eq!(manifest.policy().timezone(), "UTC");
    assert_eq!(manifest.policy().source_date_epoch(), 1_725_000_000);
    assert_eq!(manifest.policy().environment().len(), 9);
    assert_eq!(manifest.policy().environment()[0].name(), "CGO_ENABLED");
    assert_eq!(manifest.policy().environment()[0].value(), "1");
    assert_eq!(
        manifest.policy().build_arguments(),
        ["--build-runtime", "--cpu-only", "--offline"]
    );
    assert_ne!(
        manifest.policy().environment_digest(),
        manifest.policy().build_arguments_digest()
    );
    assert_eq!(
        manifest.components()[0].relative_path().as_str(),
        "helper/isolation"
    );
    assert!(manifest.components()[0].byte_size() > 0);
    assert_eq!(
        manifest.components()[0].roles(),
        &[RuntimeSourceBuildInputRole::IsolationHelper]
    );
    assert_eq!(manifest.components()[0].name(), "helper-isolation");
    assert_eq!(manifest.components()[0].revision(), "fixture-v1");
    assert_eq!(
        manifest.components()[0].source_locator(),
        "https://example.invalid/helper/isolation"
    );
    assert_eq!(
        manifest.components()[0].digest(),
        manifest.artifact_set().members()[0].artifact_id().digest()
    );
    assert_eq!(
        manifest.manifest_digest(),
        &sequence_digest(
            b"runtime-source-build/input-manifest/v1",
            [manifest_bytes.as_slice()]
        )
    );
    let plan = RuntimeSourceBuildPlan::for_legacy_read_only_verification(&manifest);
    assert_eq!(
        plan.source_inputs_id(),
        &manifest.artifact_set().artifact_set_id()
    );
    assert_eq!(plan.source_manifest_digest(), manifest.manifest_digest());
    assert_eq!(plan.build_program_path().as_str(), "scripts/build.sh");
    assert_eq!(plan.isolation_helper_path().as_str(), "helper/isolation");
    assert_eq!(plan.target(), manifest.policy().target());
    assert_eq!(plan.policy(), manifest.policy());
    assert_ne!(plan.build_program_digest(), plan.isolation_helper_digest());
    assert_eq!(
        plan.build_program_bytes(),
        fixture_bytes("scripts/build.sh").len() as u64
    );
    assert_eq!(
        plan.isolation_helper_bytes(),
        fixture_bytes("helper/isolation").len() as u64
    );
    assert_eq!(plan.controlled_build_capability_abi(), 2);
    assert_legacy_execution_constraints(&plan, &manifest);
    assert_eq!(plan.plan_digest().as_str().len(), 64);
}

#[test]
fn component_semantic_relabeling_changes_manifest_and_plan_identity() {
    let baseline = parse(&manifest_value()).expect("baseline manifest");
    let baseline_plan = RuntimeSourceBuildPlan::for_legacy_read_only_verification(&baseline);
    for (pointer, replacement) in [
        ("/components/0/name", json!("relabeled-isolation-helper")),
        ("/components/0/revision", json!("fixture-v2")),
        (
            "/components/0/source_locator",
            json!("urn:fixture:relabeled-isolation-helper"),
        ),
        (
            "/components/1/roles",
            json!(["source_patch", "license_evidence"]),
        ),
    ] {
        let mut relabeled = manifest_value();
        *relabeled.pointer_mut(pointer).expect("fixture pointer") = replacement;
        let relabeled = parse(&relabeled).expect("semantic relabeling remains valid");
        let relabeled_plan = RuntimeSourceBuildPlan::for_legacy_read_only_verification(&relabeled);
        assert_eq!(
            relabeled.artifact_set().artifact_set_id(),
            baseline.artifact_set().artifact_set_id(),
            "{pointer} must preserve byte identity"
        );
        assert_eq!(relabeled.policy(), baseline.policy(), "{pointer}");
        assert_ne!(
            relabeled.manifest_digest(),
            baseline.manifest_digest(),
            "{pointer}"
        );
        assert_eq!(
            relabeled_plan.source_inputs_id(),
            baseline_plan.source_inputs_id(),
            "{pointer}"
        );
        assert_ne!(
            relabeled_plan.source_manifest_digest(),
            baseline_plan.source_manifest_digest(),
            "{pointer}"
        );
        assert_ne!(
            relabeled_plan.plan_digest(),
            baseline_plan.plan_digest(),
            "{pointer}"
        );
    }
}

#[test]
fn encoding_schema_and_unknown_fields_fail_closed() {
    let value = manifest_value();
    let pretty = serde_json::to_vec_pretty(&value).expect("pretty manifest");
    assert_eq!(
        RuntimeSourceBuildInputManifest::parse(&pretty, RuntimeSourceBuildInputLimits::default()),
        Err(RuntimeSourceBuildInputError::NoncanonicalEncoding)
    );
    assert_eq!(
        RuntimeSourceBuildInputManifest::parse(
            br#"{"schema_version":1,"schema_version":1}"#,
            RuntimeSourceBuildInputLimits::default()
        ),
        Err(RuntimeSourceBuildInputError::InvalidEncoding)
    );
    let mut schema = value.clone();
    schema["schema_version"] = json!(2);
    assert_eq!(
        parse(&schema),
        Err(RuntimeSourceBuildInputError::UnsupportedSchema)
    );
    let mut unknown = value;
    unknown["unknown"] = json!(true);
    assert_eq!(
        parse(&unknown),
        Err(RuntimeSourceBuildInputError::InvalidEncoding)
    );
}

#[test]
fn policy_is_exactly_offline_cpu_only_linux_x86_64() {
    let baseline = manifest_value();
    for (pointer, replacement) in [
        ("/policy/target/architecture", json!("aarch64")),
        ("/policy/cpu_feature_policy", json!("contains space")),
        ("/policy/cpu_feature_policy", json!("x86-64-v3")),
        ("/policy/locale", json!("en_US.UTF-8")),
        ("/policy/timezone", json!("America/Los_Angeles")),
        ("/policy/source_date_epoch", json!(0)),
    ] {
        let mut value = baseline.clone();
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        assert_eq!(
            parse(&value),
            Err(RuntimeSourceBuildInputError::UnsupportedPolicy),
            "{pointer}"
        );
    }
}

#[test]
fn explicit_environment_and_arguments_are_bounded_and_self_binding() {
    let baseline = manifest_value();
    for (pointer, replacement) in [
        ("/policy/environment/0/name", json!("CGO-ENABLED")),
        ("/policy/environment/0/value", json!("0")),
        (
            "/policy/environment/3/value",
            json!("https://proxy.invalid"),
        ),
        ("/policy/environment/5/value", json!("C")),
        ("/policy/environment/6/value", json!("1")),
        ("/policy/build_arguments/0", json!("")),
    ] {
        let mut value = baseline.clone();
        *value.pointer_mut(pointer).expect("fixture pointer") = replacement;
        assert_eq!(
            parse(&value),
            Err(RuntimeSourceBuildInputError::UnsupportedPolicy),
            "{pointer}"
        );
    }
    let mut proxy = baseline.clone();
    proxy["policy"]["environment"]
        .as_array_mut()
        .expect("environment")
        .insert(
            0,
            json!({"name": "ALL_PROXY", "value": "http://proxy.invalid"}),
        );
    assert_eq!(
        parse(&proxy),
        Err(RuntimeSourceBuildInputError::UnsupportedPolicy)
    );
    let mut reordered = baseline;
    reordered["policy"]["environment"]
        .as_array_mut()
        .expect("environment")
        .swap(0, 1);
    assert_eq!(
        parse(&reordered),
        Err(RuntimeSourceBuildInputError::UnsupportedPolicy)
    );

    let original = parse(&manifest_value()).expect("original manifest");
    let mut changed = manifest_value();
    changed["policy"]["build_arguments"][0] = json!("--changed-build");
    let changed = parse(&changed).expect("changed explicit argument remains valid");
    assert_ne!(
        RuntimeSourceBuildPlan::for_legacy_read_only_verification(&original).plan_digest(),
        RuntimeSourceBuildPlan::for_legacy_read_only_verification(&changed).plan_digest()
    );
}

#[test]
fn component_and_role_order_and_complete_closure_are_required() {
    let baseline = manifest_value();
    let mut reordered = baseline.clone();
    reordered["components"]
        .as_array_mut()
        .expect("components")
        .swap(0, 1);
    assert_eq!(
        parse(&reordered),
        Err(RuntimeSourceBuildInputError::InvalidComponent)
    );

    let mut role_order = baseline.clone();
    role_order["components"][8]["roles"] = json!(["ninja", "cmake"]);
    assert_eq!(
        parse(&role_order),
        Err(RuntimeSourceBuildInputError::InvalidComponent)
    );

    let mut missing = baseline.clone();
    missing["components"][7]["roles"] = json!(["license_evidence"]);
    assert_eq!(
        parse(&missing),
        Err(RuntimeSourceBuildInputError::IncompleteClosure)
    );

    let mut duplicate_singleton = baseline;
    duplicate_singleton["components"][0]["roles"] = json!(["ollama_source", "isolation_helper"]);
    assert_eq!(
        parse(&duplicate_singleton),
        Err(RuntimeSourceBuildInputError::IncompleteClosure)
    );
}

#[test]
fn declared_artifact_set_identity_and_limits_are_enforced() {
    let mut wrong_identity = manifest_value();
    wrong_identity["artifact_set_id"] =
        json!(ArtifactSetId::from_digest(Digest::sha256(b"wrong set")));
    assert_eq!(
        parse(&wrong_identity),
        Err(RuntimeSourceBuildInputError::InvalidArtifactSet)
    );

    let bytes = canonical(&manifest_value());
    for limits in [
        RuntimeSourceBuildInputLimits {
            manifest_bytes: 0,
            ..RuntimeSourceBuildInputLimits::default()
        },
        RuntimeSourceBuildInputLimits {
            maximum_components: 10,
            ..RuntimeSourceBuildInputLimits::default()
        },
        RuntimeSourceBuildInputLimits {
            maximum_component_bytes: 1,
            ..RuntimeSourceBuildInputLimits::default()
        },
        RuntimeSourceBuildInputLimits {
            maximum_total_bytes: 1,
            ..RuntimeSourceBuildInputLimits::default()
        },
    ] {
        assert_eq!(
            RuntimeSourceBuildInputManifest::parse(&bytes, limits),
            Err(RuntimeSourceBuildInputError::LimitExceeded)
        );
    }
}

fn fixture_bytes(path: &str) -> Vec<u8> {
    if matches!(path, "scripts/build.sh" | "helper/isolation") {
        let entry = if path == "scripts/build.sh" {
            0x1_000_u64
        } else {
            0x2_000_u64
        };
        synthetic_static_elf(entry)
    } else {
        format!("fixture bytes for {path}").into_bytes()
    }
}

fn synthetic_static_elf(entry: u64) -> Vec<u8> {
    const HEADER: usize = 64;
    const PROGRAM_HEADER: usize = 56;
    const PROGRAM_HEADERS: u16 = 2;
    let mut bytes = vec![0_u8; HEADER + usize::from(PROGRAM_HEADERS) * PROGRAM_HEADER];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[16..18].copy_from_slice(&3_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&entry.to_le_bytes());
    bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(
        &u16::try_from(HEADER)
            .expect("ELF header size fits u16")
            .to_le_bytes(),
    );
    bytes[54..56].copy_from_slice(
        &u16::try_from(PROGRAM_HEADER)
            .expect("ELF program header size fits u16")
            .to_le_bytes(),
    );
    bytes[56..58].copy_from_slice(&PROGRAM_HEADERS.to_le_bytes());
    let file_bytes = bytes.len() as u64;
    bytes[HEADER..HEADER + 4].copy_from_slice(&1_u32.to_le_bytes());
    bytes[HEADER + 4..HEADER + 8].copy_from_slice(&5_u32.to_le_bytes());
    bytes[HEADER + 16..HEADER + 24].copy_from_slice(&entry.to_le_bytes());
    bytes[HEADER + 32..HEADER + 40].copy_from_slice(&file_bytes.to_le_bytes());
    bytes[HEADER + 40..HEADER + 48].copy_from_slice(&file_bytes.to_le_bytes());
    bytes[HEADER + 48..HEADER + 56].copy_from_slice(&0x1_000_u64.to_le_bytes());
    let stack = HEADER + PROGRAM_HEADER;
    bytes[stack..stack + 4].copy_from_slice(&0x6474_e551_u32.to_le_bytes());
    bytes[stack + 4..stack + 8].copy_from_slice(&6_u32.to_le_bytes());
    bytes[stack + 48..stack + 56].copy_from_slice(&16_u64.to_le_bytes());
    bytes
}
