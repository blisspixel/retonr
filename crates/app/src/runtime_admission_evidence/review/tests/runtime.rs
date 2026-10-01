use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    RuntimePackageManifestId,
};
use rewrite_ollama_package::{
    MemberOpenError, RuntimeLayoutLimits, reconstruct_runtime_package_with_limits,
};
use rewrite_types::Digest;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io::Cursor};

struct RuntimeMemberFixture {
    path: &'static str,
    roles: Value,
    load_policy: &'static str,
    bytes: &'static [u8],
}
fn source_fixture_declarations() -> [(&'static str, Vec<&'static str>); 16] {
    [
        ("helper/isolation", vec!["isolation_helper"]),
        ("legal/licenses.json", vec!["license_evidence"]),
        ("metadata/build-parameters.json", vec!["build_parameters"]),
        ("metadata/source-provenance.json", vec!["source_provenance"]),
        ("metadata/tool-evidence.json", vec!["tool_evidence"]),
        ("modules/checksums.txt", vec!["go_checksum_set"]),
        ("modules/example.zip", vec!["go_module"]),
        ("packages/native.pkg", vec!["native_package", "posix_shell"]),
        ("patches/source.patch", vec!["source_patch"]),
        ("patches/source2.patch", vec!["source_patch"]),
        ("scripts/build.sh", vec!["build_script"]),
        ("sources/llama-cpp.tar.gz", vec!["llama_cpp_source"]),
        ("sources/ollama.tar.gz", vec!["ollama_source"]),
        ("toolchains/build-tools.tar", vec!["cmake", "ninja"]),
        (
            "toolchains/cc.tar",
            vec![
                "c_compiler",
                "cxx_compiler",
                "assembler",
                "linker",
                "standard_library",
            ],
        ),
        ("toolchains/go.tar.gz", vec!["go_toolchain"]),
    ]
}

pub(crate) fn source_fixture() -> (ArtifactSetId, Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let source_inputs = source_fixture_declarations();
    let components = source_inputs
        .iter()
        .map(|(path, roles)| {
            let bytes = source_input_bytes(path);
            json!({
                "byte_size": bytes.len(),
                "digest": Digest::sha256(&bytes),
                "name": path.replace('/', "-"),
                "relative_path": path,
                "revision": if roles.contains(&"ollama_source") {
                    "b7871fc0d1d82fe109536efa3e0e8e411c766c75"
                } else {
                    "fixture-v1"
                },
                "roles": roles,
                "source_locator": format!("https://example.invalid/{path}")
            })
        })
        .collect::<Vec<_>>();
    let source_input_bytes = source_inputs
        .iter()
        .map(|(path, _roles)| ((*path).to_owned(), source_input_bytes(path)))
        .collect::<BTreeMap<_, _>>();
    let source_set = ArtifactSetManifest::new(
        components
            .iter()
            .map(|component| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(
                        serde_json::from_value(component["digest"].clone())
                            .expect("component digest"),
                    ),
                    component["byte_size"].as_u64().expect("component size"),
                    ArtifactSetRelativePath::new(
                        component["relative_path"].as_str().expect("component path"),
                    )
                    .expect("source input path"),
                )
            })
            .collect(),
    )
    .expect("source input artifact set");
    let source_build_inputs_id = source_set.artifact_set_id();
    let manifest = canonical(&json!({
        "artifact_set_id": source_build_inputs_id,
        "components": components,
        "policy": {
            "accelerator": "cpu_only",
            "build_arguments": ["--build-runtime", "--cpu-only", "--offline"],
            "cpu_feature_policy": "x86-64-v2",
            "environment": [
                {"name": "CGO_ENABLED", "value": "1"},
                {"name": "GOAMD64", "value": "v2"},
                {"name": "GOARCH", "value": "amd64"},
                {"name": "GOOS", "value": "linux"},
                {"name": "GOPROXY", "value": "off"},
                {"name": "GOSUMDB", "value": "off"},
                {"name": "LC_ALL", "value": "C.UTF-8"},
                {"name": "SOURCE_DATE_EPOCH", "value": "1725000000"},
                {"name": "TZ", "value": "UTC"}
            ],
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
        "schema_version": rewrite_ollama_package::RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION
    }));
    (source_build_inputs_id, manifest, source_input_bytes)
}

fn source_input_bytes(path: &str) -> Vec<u8> {
    if matches!(path, "scripts/build.sh" | "helper/isolation") {
        let entry = if path == "scripts/build.sh" {
            0x1_000_u64
        } else {
            0x2_000_u64
        };
        synthetic_static_elf(entry)
    } else {
        match path {
            "metadata/build-parameters.json" => b"build parameters".to_vec(),
            "metadata/source-provenance.json" => b"source provenance".to_vec(),
            "metadata/tool-evidence.json" => b"build tools".to_vec(),
            _ => format!("fixture bytes for {path}").into_bytes(),
        }
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

pub(crate) fn runtime_fixture(
    source_build_inputs_id: &ArtifactSetId,
) -> (Vec<u8>, BTreeMap<String, Vec<u8>>, RuntimePackageManifestId) {
    let mut members = BTreeMap::new();
    let mut member_declarations = Vec::new();
    let mut observed_tree = Vec::new();
    for member in runtime_members() {
        members.insert(member.path.to_owned(), member.bytes.to_vec());
        member_declarations.push(json!({
            "byte_size": member.bytes.len(),
            "digest": Digest::sha256(member.bytes),
            "load_policy": member.load_policy,
            "relative_path": member.path,
            "roles": member.roles
        }));
        observed_tree.push(member.path);
    }
    let layout = canonical(&json!({
        "build_revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
        "members": member_declarations,
        "observed_tree": observed_tree,
        "reported_version": "0.32.15-retonr.1",
        "runtime_family": "ollama",
        "schema_version": 1,
        "source": {
            "kind": "repository_revision",
            "locator": "https://github.com/ollama/ollama",
            "provenance_digest": Digest::sha256(b"source provenance"),
            "revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
            "schema_version": 1
        },
        "target": {
            "abi": "linux_gnu_libc",
            "architecture": "x86_64",
            "operating_system": "linux"
        },
        "transformation": {
            "kind": "transformed",
            "log_digest": Digest::sha256(b""),
            "parameters_digest": Digest::sha256(b"build parameters"),
            "source_artifact_set_id": source_build_inputs_id,
            "tool_evidence_digest": Digest::sha256(b"build tools")
        }
    }));
    let reconstructed = reconstruct_runtime_package_with_limits(
        &layout,
        &RuntimeLayoutLimits::default(),
        |path| {
            members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(MemberOpenError)
        },
        || false,
    )
    .expect("fixture runtime reconstructs");
    let runtime_package_manifest_id = reconstructed
        .runtime_package()
        .runtime_package_manifest_id();
    (layout, members, runtime_package_manifest_id)
}

fn runtime_members() -> [RuntimeMemberFixture; 7] {
    [
        RuntimeMemberFixture {
            path: "bin/ollama",
            roles: json!(["entrypoint"]),
            load_policy: "required_at_ready",
            bytes: b"ollama-entrypoint\n",
        },
        RuntimeMemberFixture {
            path: "helper/retonr-isolation",
            roles: json!(["helper_executable"]),
            load_policy: "must_not_be_code_loaded",
            bytes: b"isolation-helper\n",
        },
        RuntimeMemberFixture {
            path: "legal/license.txt",
            roles: json!(["license_text"]),
            load_policy: "must_not_be_code_loaded",
            bytes: b"runtime license\n",
        },
        RuntimeMemberFixture {
            path: "lib/ollama/libggml-cpu.so",
            roles: json!(["native_dependency"]),
            load_policy: "backend_conditional",
            bytes: b"ggml-cpu\n",
        },
        RuntimeMemberFixture {
            path: "lib/ollama/llama-server",
            roles: json!(["worker_executable"]),
            load_policy: "backend_conditional",
            bytes: b"llama-server\n",
        },
        RuntimeMemberFixture {
            path: "provenance/source.txt",
            roles: json!(["provenance_record"]),
            load_policy: "must_not_be_code_loaded",
            bytes: b"source provenance\n",
        },
        RuntimeMemberFixture {
            path: "review/transformation.json",
            roles: json!(["transformation_record"]),
            load_policy: "must_not_be_code_loaded",
            bytes: b"transformation evidence\n",
        },
    ]
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("canonical fixture JSON")
}
