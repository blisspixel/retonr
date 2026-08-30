use std::{fs, os::unix::fs::PermissionsExt as _};

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_ollama_package::RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION;
use rewrite_types::Digest;
use serde_json::json;
use tempfile::{TempDir, tempdir_in};

use crate::RuntimeSourceBuildBundleSource;

struct Component {
    path: &'static str,
    roles: &'static [&'static str],
    bytes: Vec<u8>,
}

pub(super) struct BundleFixture {
    _root: TempDir,
    pub(super) selection: RuntimeSourceBuildBundleSource,
}

pub(super) fn write_bundle_fixture(
    helper: &std::path::Path,
    build_program: &std::path::Path,
) -> BundleFixture {
    write_bundle_fixture_with_arguments(helper, build_program, &["controlled-fixture"])
}

fn write_bundle_fixture_with_arguments(
    helper: &std::path::Path,
    build_program: &std::path::Path,
    build_arguments: &[&str],
) -> BundleFixture {
    let root = tempdir_in("/tmp").expect("bundle fixture root under host temporary root");
    let component_root = root.path().join("components");
    let manifest_path = root.path().join("review/source-build-inputs.json");
    let mut components = fixture_components(build_program, helper);
    let artifact_set = artifact_set(&components);
    let source_id = artifact_set.artifact_set_id();
    for component in &components {
        let path = component_root.join(component.path);
        fs::create_dir_all(path.parent().expect("component parent"))
            .expect("create component parent");
        fs::write(&path, &component.bytes).expect("write component");
        if matches!(component.path, "scripts/build.sh" | "helper/isolation") {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("make component executable");
        }
    }
    let declarations = components
        .drain(..)
        .map(|component| {
            json!({
                "byte_size": component.bytes.len(),
                "digest": Digest::sha256(&component.bytes),
                "name": component.path.replace('/', "-"),
                "relative_path": component.path,
                "revision": if component.roles.contains(&"ollama_source") {
                    "b7871fc0d1d82fe109536efa3e0e8e411c766c75"
                } else {
                    "fixture-v1"
                },
                "roles": component.roles,
                "source_locator": format!("https://example.invalid/{}", component.path)
            })
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&json!({
        "artifact_set_id": source_id,
        "components": declarations,
        "policy": {
            "accelerator": "cpu_only",
            "build_arguments": build_arguments,
            "cpu_feature_policy": "x86-64-v2",
            "environment": [
                {"name": "CGO_ENABLED", "value": "1"},
                {"name": "GOAMD64", "value": "v2"},
                {"name": "GOARCH", "value": "amd64"},
                {"name": "GOOS", "value": "linux"},
                {"name": "GOPROXY", "value": "off"},
                {"name": "GOSUMDB", "value": "off"},
                {"name": "LC_ALL", "value": "C.UTF-8"},
                {"name": "RETONR_FIXTURE_SOURCE_ID", "value": source_id},
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
        "schema_version": RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION
    }))
    .expect("encode source-build manifest");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest parent");
    fs::write(&manifest_path, manifest).expect("write source-build manifest");
    let selection = RuntimeSourceBuildBundleSource::new(&manifest_path, &component_root)
        .expect("select source-build bundle");
    BundleFixture {
        _root: root,
        selection,
    }
}

fn fixture_components(build_program: &std::path::Path, helper: &std::path::Path) -> Vec<Component> {
    let declarations: [(&str, &[&str]); 16] = [
        ("helper/isolation", &["isolation_helper"]),
        ("legal/licenses.json", &["license_evidence"]),
        ("metadata/build-parameters.json", &["build_parameters"]),
        ("metadata/source-provenance.json", &["source_provenance"]),
        ("metadata/tool-evidence.json", &["tool_evidence"]),
        ("modules/checksums.txt", &["go_checksum_set"]),
        ("modules/example.zip", &["go_module"]),
        ("packages/native.pkg", &["native_package", "posix_shell"]),
        ("patches/source.patch", &["source_patch"]),
        ("patches/source2.patch", &["source_patch"]),
        ("scripts/build.sh", &["build_script"]),
        ("sources/llama-cpp.tar.gz", &["llama_cpp_source"]),
        ("sources/ollama.tar.gz", &["ollama_source"]),
        ("toolchains/build-tools.tar", &["cmake", "ninja"]),
        (
            "toolchains/cc.tar",
            &[
                "c_compiler",
                "cxx_compiler",
                "assembler",
                "linker",
                "standard_library",
            ],
        ),
        ("toolchains/go.tar.gz", &["go_toolchain"]),
    ];
    declarations
        .into_iter()
        .map(|(path, roles)| Component {
            path,
            roles,
            bytes: match path {
                "scripts/build.sh" => fs::read(build_program).expect("read build fixture"),
                "helper/isolation" => fs::read(helper).expect("read isolation helper"),
                "metadata/build-parameters.json" => b"fixture build parameters".to_vec(),
                "metadata/source-provenance.json" => b"fixture source provenance".to_vec(),
                "metadata/tool-evidence.json" => b"fixture build tools".to_vec(),
                _ => format!("fixture bytes for {path}").into_bytes(),
            },
        })
        .collect()
}

fn artifact_set(components: &[Component]) -> ArtifactSetManifest {
    ArtifactSetManifest::new(
        components
            .iter()
            .map(|component| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(Digest::sha256(&component.bytes)),
                    u64::try_from(component.bytes.len()).expect("component size"),
                    ArtifactSetRelativePath::new(component.path).expect("component path"),
                )
            })
            .collect(),
    )
    .expect("source-build artifact set")
}
