use std::{collections::BTreeMap, io::Cursor};

use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
};
use rewrite_ollama_package::{
    MemberOpenError, RuntimePackageReviewEvidenceOpenError, RuntimePackageReviewV2Limits,
    RuntimeSourceBuildInputOpenError, VerifiedRuntimePackageReviewV2,
    verify_runtime_package_review_v2,
};
use rewrite_types::Digest;
use serde_json::{Value, json};

const SOURCE_PATH: &str = "evidence/source-build-inputs.json";
const LAYOUT_PATH: &str = "layout/runtime-package.json";

pub(super) fn verified_review() -> VerifiedRuntimePackageReviewV2 {
    let (source_build_inputs_id, source_manifest, source_inputs) = source_fixture();
    let (layout, members, runtime_package_manifest_id) = runtime_fixture(&source_build_inputs_id);
    let evidence = BTreeMap::from([
        (
            "evidence/build-tools.json".to_owned(),
            canonical(&json!({"schema_version":1,"tools":["go","cmake","ninja"]})),
        ),
        (
            "evidence/execution.json".to_owned(),
            canonical(&json!({"cloud_disabled":true,"managed_startup":"passed"})),
        ),
        (SOURCE_PATH.to_owned(), source_manifest),
        (LAYOUT_PATH.to_owned(), layout),
    ]);
    let review = review_bytes(
        &evidence,
        &source_build_inputs_id,
        &runtime_package_manifest_id,
    );
    verify_runtime_package_review_v2(
        &review,
        &RuntimePackageReviewV2Limits::default(),
        |path| {
            evidence
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimePackageReviewEvidenceOpenError)
        },
        |path| {
            source_inputs
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildInputOpenError)
        },
        |path| {
            members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(MemberOpenError)
        },
        || false,
    )
    .expect("verified all-pass review")
}

fn source_fixture() -> (ArtifactSetId, Vec<u8>, BTreeMap<String, Vec<u8>>) {
    let declarations = [
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
    ];
    let inputs = declarations
        .iter()
        .map(|(path, _)| ((*path).to_owned(), source_bytes(path)))
        .collect::<BTreeMap<_, _>>();
    let components = declarations
        .iter()
        .map(|(path, roles)| {
            let bytes = inputs.get(*path).expect("source bytes");
            json!({
                "byte_size":bytes.len(),
                "digest":Digest::sha256(bytes),
                "name":path.replace('/',"-"),
                "relative_path":path,
                "revision":if roles.contains(&"ollama_source") {
                    "b7871fc0d1d82fe109536efa3e0e8e411c766c75"
                } else { "fixture-v1" },
                "roles":roles,
                "source_locator":format!("https://example.invalid/{path}")
            })
        })
        .collect::<Vec<_>>();
    let artifact_set = ArtifactSetManifest::new(
        components
            .iter()
            .map(|component| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(
                        serde_json::from_value(component["digest"].clone()).expect("digest"),
                    ),
                    component["byte_size"].as_u64().expect("size"),
                    path(component["relative_path"].as_str().expect("path")),
                )
            })
            .collect(),
    )
    .expect("source set");
    let artifact_set_id = artifact_set.artifact_set_id();
    let manifest = canonical(&json!({
        "artifact_set_id":artifact_set_id,
        "components":components,
        "policy":{
            "accelerator":"cpu_only",
            "build_arguments":["--build-runtime","--cpu-only","--offline"],
            "cpu_feature_policy":"x86-64-v2",
            "environment":[
                {"name":"CGO_ENABLED","value":"1"},
                {"name":"GOAMD64","value":"v2"},
                {"name":"GOARCH","value":"amd64"},
                {"name":"GOOS","value":"linux"},
                {"name":"GOPROXY","value":"off"},
                {"name":"GOSUMDB","value":"off"},
                {"name":"LC_ALL","value":"C.UTF-8"},
                {"name":"SOURCE_DATE_EPOCH","value":"1725000000"},
                {"name":"TZ","value":"UTC"}
            ],
            "locale":"C.UTF-8",
            "network_access":"denied",
            "source_date_epoch":1_725_000_000_u64,
            "target":{"abi":"linux_gnu_libc","architecture":"x86_64","operating_system":"linux"},
            "timezone":"UTC"
        },
        "schema_version":1
    }));
    (artifact_set_id, manifest, inputs)
}

fn source_bytes(path: &str) -> Vec<u8> {
    match path {
        "scripts/build.sh" => synthetic_static_elf(0x1_000),
        "helper/isolation" => synthetic_static_elf(0x2_000),
        "metadata/build-parameters.json" => b"build parameters".to_vec(),
        "metadata/source-provenance.json" => b"source provenance".to_vec(),
        "metadata/tool-evidence.json" => b"build tools".to_vec(),
        _ => format!("fixture bytes for {path}").into_bytes(),
    }
}

fn runtime_fixture(
    source_build_inputs_id: &ArtifactSetId,
) -> (
    Vec<u8>,
    BTreeMap<String, Vec<u8>>,
    rewrite_model::RuntimePackageManifestId,
) {
    let specifications = [
        (
            "bin/ollama",
            "entrypoint",
            "required_at_ready",
            b"ollama-entrypoint\n".as_slice(),
        ),
        (
            "helper/retonr-isolation",
            "helper_executable",
            "must_not_be_code_loaded",
            b"isolation-helper\n".as_slice(),
        ),
        (
            "legal/license.txt",
            "license_text",
            "must_not_be_code_loaded",
            b"runtime license\n".as_slice(),
        ),
        (
            "lib/ollama/libggml-cpu.so",
            "native_dependency",
            "backend_conditional",
            b"ggml-cpu\n".as_slice(),
        ),
        (
            "lib/ollama/llama-server",
            "worker_executable",
            "backend_conditional",
            b"llama-server\n".as_slice(),
        ),
        (
            "provenance/source.txt",
            "provenance_record",
            "must_not_be_code_loaded",
            b"source provenance\n".as_slice(),
        ),
        (
            "review/transformation.json",
            "transformation_record",
            "must_not_be_code_loaded",
            b"transformation evidence\n".as_slice(),
        ),
    ];
    let members = specifications
        .iter()
        .map(|(path, _, _, bytes)| ((*path).to_owned(), bytes.to_vec()))
        .collect::<BTreeMap<_, _>>();
    let member_declarations = specifications
        .iter()
        .map(|(path, role, policy, bytes)| {
            json!({
                "byte_size":bytes.len(),"digest":Digest::sha256(bytes),"load_policy":policy,
                "relative_path":path,"roles":[role]
            })
        })
        .collect::<Vec<_>>();
    let layout = canonical(&json!({
        "build_revision":"b7871fc0d1d82fe109536efa3e0e8e411c766c75",
        "members":member_declarations,
        "observed_tree":specifications.iter().map(|item| item.0).collect::<Vec<_>>(),
        "reported_version":"0.32.15",
        "runtime_family":"ollama",
        "schema_version":1,
        "source":{
            "kind":"repository_revision","locator":"https://github.com/ollama/ollama",
            "provenance_digest":Digest::sha256(b"source provenance"),
            "revision":"b7871fc0d1d82fe109536efa3e0e8e411c766c75","schema_version":1
        },
        "target":{"abi":"linux_gnu_libc","architecture":"x86_64","operating_system":"linux"},
        "transformation":{
            "kind":"transformed","log_digest":Digest::sha256(b""),
            "parameters_digest":Digest::sha256(b"build parameters"),
            "source_artifact_set_id":source_build_inputs_id,
            "tool_evidence_digest":Digest::sha256(b"build tools")
        }
    }));
    let reconstructed = rewrite_ollama_package::reconstruct_runtime_package_with_limits(
        &layout,
        &rewrite_ollama_package::RuntimeLayoutLimits::default(),
        |path| {
            members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(MemberOpenError)
        },
        || false,
    )
    .expect("runtime reconstruction");
    let package_id = reconstructed
        .runtime_package()
        .runtime_package_manifest_id();
    (layout, members, package_id)
}

fn review_bytes(
    evidence: &BTreeMap<String, Vec<u8>>,
    source_id: &ArtifactSetId,
    package_id: &rewrite_model::RuntimePackageManifestId,
) -> Vec<u8> {
    let declarations = evidence
        .iter()
        .map(|(path, bytes)| {
            json!({
                "byte_size":bytes.len(),
                "class":match path.as_str() {
                    "evidence/build-tools.json" => "build_tool",
                    "evidence/execution.json" => "execution",
                    SOURCE_PATH => "fetched_input",
                    LAYOUT_PATH => "build_output",
                    _ => unreachable!()
                },
                "digest":Digest::sha256(bytes),"relative_path":path
            })
        })
        .collect::<Vec<_>>();
    canonical(&json!({
        "build_revision":"b7871fc0d1d82fe109536efa3e0e8e411c766c75",
        "checks":[
            {"check":"source_lineage","evidence":["evidence/build-tools.json",SOURCE_PATH],"status":"passed"},
            {"check":"transformation","evidence":["evidence/build-tools.json",LAYOUT_PATH],"status":"passed"},
            {"check":"license","evidence":[SOURCE_PATH],"status":"passed"},
            {"check":"native_closure","evidence":["evidence/execution.json",LAYOUT_PATH],"status":"passed"},
            {"check":"managed_startup","evidence":["evidence/execution.json"],"status":"passed"},
            {"check":"cloud_disable","evidence":["evidence/execution.json"],"status":"passed"}
        ],
        "disposition":{
            "layout_digest":Digest::sha256(evidence.get(LAYOUT_PATH).expect("layout")),
            "runtime_layout":LAYOUT_PATH,"runtime_package_manifest_id":package_id,"status":"admitted"
        },
        "evidence":declarations,
        "reported_version":"0.32.15","runtime_family":"ollama","schema_version":2,
        "source_build_inputs":{"artifact_set_id":source_id,"evidence_path":SOURCE_PATH},
        "target":{"abi":"linux_gnu_libc","architecture":"x86_64","operating_system":"linux"}
    }))
}

fn synthetic_static_elf(entry: u64) -> Vec<u8> {
    const HEADER: usize = 64;
    const PH: usize = 56;
    let mut bytes = vec![0_u8; HEADER + 2 * PH];
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4..7].copy_from_slice(&[2, 1, 1]);
    bytes[16..18].copy_from_slice(&3_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&entry.to_le_bytes());
    bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(
        &u16::try_from(HEADER)
            .expect("ELF header size")
            .to_le_bytes(),
    );
    bytes[54..56].copy_from_slice(
        &u16::try_from(PH)
            .expect("ELF program header size")
            .to_le_bytes(),
    );
    bytes[56..58].copy_from_slice(&2_u16.to_le_bytes());
    let size = bytes.len() as u64;
    bytes[HEADER..HEADER + 4].copy_from_slice(&1_u32.to_le_bytes());
    bytes[HEADER + 4..HEADER + 8].copy_from_slice(&5_u32.to_le_bytes());
    bytes[HEADER + 16..HEADER + 24].copy_from_slice(&entry.to_le_bytes());
    bytes[HEADER + 32..HEADER + 40].copy_from_slice(&size.to_le_bytes());
    bytes[HEADER + 40..HEADER + 48].copy_from_slice(&size.to_le_bytes());
    bytes[HEADER + 48..HEADER + 56].copy_from_slice(&0x1_000_u64.to_le_bytes());
    let stack = HEADER + PH;
    bytes[stack..stack + 4].copy_from_slice(&0x6474_e551_u32.to_le_bytes());
    bytes[stack + 4..stack + 8].copy_from_slice(&6_u32.to_le_bytes());
    bytes[stack + 48..stack + 56].copy_from_slice(&16_u64.to_le_bytes());
    bytes
}

fn canonical(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("canonical JSON")
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value.to_owned()).expect("fixture path")
}
