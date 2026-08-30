use std::{fs, io::Read as _, path::PathBuf};

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_ollama_package::{
    RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION, RuntimeSourceBuildInputError,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

use super::{
    RuntimeSourceBuildBundleError, RuntimeSourceBuildBundleLimits, RuntimeSourceBuildBundleSource,
    RuntimeSourceBuildBundleVerifier,
};

struct BundleFixture {
    _root: TempDir,
    manifest_path: PathBuf,
    component_root: PathBuf,
    component_paths: Vec<PathBuf>,
    manifest_bytes: Vec<u8>,
    selection: RuntimeSourceBuildBundleSource,
}

#[test]
fn verifies_one_exact_offline_source_build_bundle() {
    let fixture = write_bundle_fixture();
    let verified = RuntimeSourceBuildBundleVerifier::verify(
        &fixture.selection,
        RuntimeSourceBuildBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("exact bundle verifies");
    assert_eq!(verified.manifest_bytes(), fixture.manifest_bytes);
    assert_eq!(verified.inputs().manifest().components().len(), 16);
    assert_eq!(
        verified.inputs().manifest().artifact_set().members().len(),
        16
    );
    assert!(fixture.selection.manifest_path().is_absolute());
    assert!(fixture.selection.component_root().is_absolute());

    let lease = RuntimeSourceBuildBundleVerifier::acquire(
        &fixture.selection,
        RuntimeSourceBuildBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("exact bundle lease");
    assert_eq!(lease.manifest_bytes(), fixture.manifest_bytes);
    assert_eq!(
        lease.plan().build_program_path().as_str(),
        "scripts/build.sh"
    );
    assert_eq!(
        lease.plan().isolation_helper_path().as_str(),
        "helper/isolation"
    );
    assert_eq!(
        lease.plan().source_inputs_id(),
        &lease.inputs().manifest().artifact_set().artifact_set_id()
    );
    assert_eq!(
        lease.plan().source_manifest_digest(),
        lease.inputs().manifest().manifest_digest()
    );
    lease
        .revalidate(&CancellationToken::new())
        .expect("unchanged lease revalidates");
    let (mut build_program, mut isolation_helper, component_root, component_files) = lease
        .build_capabilities(&CancellationToken::new())
        .expect("clone exact build capabilities")
        .into_files();
    let mut program_bytes = Vec::new();
    build_program
        .read_to_end(&mut program_bytes)
        .expect("read cloned build program");
    let mut helper_bytes = Vec::new();
    isolation_helper
        .read_to_end(&mut helper_bytes)
        .expect("read cloned isolation helper");
    assert_eq!(program_bytes, fixture_bytes("scripts/build.sh"));
    assert_eq!(helper_bytes, fixture_bytes("helper/isolation"));
    assert!(component_root.metadata().expect("component root").is_dir());
    assert_eq!(
        component_files.len(),
        lease.inputs().manifest().artifact_set().members().len()
    );
    for ((path, digest, bytes, file), expected) in component_files
        .iter()
        .zip(lease.inputs().manifest().artifact_set().members())
    {
        assert_eq!(path, expected.relative_path());
        assert_eq!(digest, expected.artifact_id().digest());
        assert_eq!(*bytes, expected.byte_size());
        assert!(file.metadata().expect("component metadata").is_file());
    }
}

#[test]
fn overlapping_selection_is_rejected_without_opening_paths() {
    let root = tempdir().expect("temporary root");
    let component_root = root.path().join("components");
    let nested_manifest = component_root.join("manifest.json");
    assert!(matches!(
        RuntimeSourceBuildBundleSource::new(&nested_manifest, &component_root),
        Err(RuntimeSourceBuildBundleError::UnsafeSource)
    ));
}

#[test]
fn limits_and_cancellation_win_before_source_access() {
    let root = tempdir().expect("temporary root");
    let selection = RuntimeSourceBuildBundleSource::new(
        root.path().join("missing.json"),
        root.path().join("missing-components"),
    )
    .expect("nonoverlapping selection");
    let invalid = RuntimeSourceBuildBundleLimits {
        maximum_tree_entries: 0,
        ..RuntimeSourceBuildBundleLimits::default()
    };
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(&selection, invalid, &CancellationToken::new()),
        Err(RuntimeSourceBuildBundleError::LimitExceeded)
    ));

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &selection,
            RuntimeSourceBuildBundleLimits::default(),
            &cancellation
        ),
        Err(RuntimeSourceBuildBundleError::Cancelled)
    ));
}

#[test]
fn extra_missing_and_empty_entries_fail_exact_tree_validation() {
    for mode in ["extra-file", "extra-directory", "missing"] {
        let fixture = write_bundle_fixture();
        match mode {
            "extra-file" => {
                fs::write(fixture.component_root.join("extra.bin"), b"extra")
                    .expect("write extra file");
            }
            "extra-directory" => {
                fs::create_dir(fixture.component_root.join("extra"))
                    .expect("create extra directory");
            }
            "missing" => {
                fs::remove_file(&fixture.component_paths[0]).expect("remove declared component");
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                RuntimeSourceBuildBundleVerifier::verify(
                    &fixture.selection,
                    RuntimeSourceBuildBundleLimits::default(),
                    &CancellationToken::new()
                ),
                Err(RuntimeSourceBuildBundleError::SourceTreeMismatch)
            ),
            "{mode}"
        );
    }
}

#[test]
fn changed_size_and_digest_fail_closed() {
    let shortened = write_bundle_fixture();
    let mut bytes = fs::read(&shortened.component_paths[0]).expect("read component");
    bytes.pop();
    fs::write(&shortened.component_paths[0], bytes).expect("shorten component");
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &shortened.selection,
            RuntimeSourceBuildBundleLimits::default(),
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::SourceTreeMismatch)
    ));

    let changed = write_bundle_fixture();
    let mut bytes = fs::read(&changed.component_paths[0]).expect("read component");
    *bytes.last_mut().expect("nonempty component") ^= 1;
    fs::write(&changed.component_paths[0], bytes).expect("change component");
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &changed.selection,
            RuntimeSourceBuildBundleLimits::default(),
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::Input(
            RuntimeSourceBuildInputError::ComponentDigestMismatch
        ))
    ));
}

#[test]
fn manifest_and_tree_entry_ceilings_are_enforced() {
    let fixture = write_bundle_fixture();
    let manifest_limit = RuntimeSourceBuildBundleLimits {
        inputs: rewrite_ollama_package::RuntimeSourceBuildInputLimits {
            manifest_bytes: fixture.manifest_bytes.len() - 1,
            ..rewrite_ollama_package::RuntimeSourceBuildInputLimits::default()
        },
        ..RuntimeSourceBuildBundleLimits::default()
    };
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &fixture.selection,
            manifest_limit,
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::Input(
            RuntimeSourceBuildInputError::LimitExceeded
        ))
    ));
    let tree_limit = RuntimeSourceBuildBundleLimits {
        maximum_tree_entries: 1,
        ..RuntimeSourceBuildBundleLimits::default()
    };
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &fixture.selection,
            tree_limit,
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::LimitExceeded)
    ));
}

#[test]
fn multiply_linked_manifest_and_component_are_rejected() {
    let manifest = write_bundle_fixture();
    fs::hard_link(
        &manifest.manifest_path,
        manifest.manifest_path.with_extension("alias"),
    )
    .expect("create manifest hard link");
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &manifest.selection,
            RuntimeSourceBuildBundleLimits::default(),
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::UnsafeSource)
    ));

    let component = write_bundle_fixture();
    fs::hard_link(
        &component.component_paths[0],
        component.component_paths[0].with_extension("alias"),
    )
    .expect("create component hard link");
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &component.selection,
            RuntimeSourceBuildBundleLimits::default(),
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::SourceTreeMismatch)
    ));
}

#[test]
fn tree_mutation_after_pinning_is_detected() {
    let fixture = write_bundle_fixture();
    let lease = RuntimeSourceBuildBundleVerifier::acquire(
        &fixture.selection,
        RuntimeSourceBuildBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("pin exact bundle");
    fs::write(fixture.component_root.join("late-extra.bin"), b"extra").expect("mutate pinned tree");
    assert!(matches!(
        lease.revalidate(&CancellationToken::new()),
        Err(RuntimeSourceBuildBundleError::SourceChanged)
    ));
}

#[cfg(unix)]
#[test]
fn source_name_replacement_after_pinning_is_detected() {
    let fixture = write_bundle_fixture();
    let lease = RuntimeSourceBuildBundleVerifier::acquire(
        &fixture.selection,
        RuntimeSourceBuildBundleLimits::default(),
        &CancellationToken::new(),
    )
    .expect("pin exact bundle");
    let selected = &fixture.component_paths[0];
    let bytes = fs::read(selected).expect("read component");
    fs::rename(selected, selected.with_extension("displaced")).expect("displace component");
    fs::write(selected, bytes).expect("replace component name");
    assert!(matches!(
        lease.revalidate(&CancellationToken::new()),
        Err(RuntimeSourceBuildBundleError::SourceChanged)
    ));
}

#[cfg(unix)]
#[test]
fn component_symlink_is_rejected() {
    use std::os::unix::fs::symlink;

    let fixture = write_bundle_fixture();
    let selected = &fixture.component_paths[0];
    let target = selected.with_extension("target");
    fs::rename(selected, &target).expect("move selected component");
    symlink(&target, selected).expect("create component symlink");
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &fixture.selection,
            RuntimeSourceBuildBundleLimits::default(),
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::UnsafeSource)
    ));
}

#[cfg(windows)]
#[test]
fn component_reparse_point_is_rejected() {
    use std::os::windows::fs::symlink_file;

    let fixture = write_bundle_fixture();
    let selected = &fixture.component_paths[0];
    let target = selected.with_extension("target");
    fs::rename(selected, &target).expect("move selected component");
    if let Err(error) = symlink_file(&target, selected) {
        if crate::symlink_test_support::skip_unavailable_link("source-build component", &error) {
            return;
        }
        panic!("create source-build component reparse fixture: {error}");
    }
    assert!(matches!(
        RuntimeSourceBuildBundleVerifier::verify(
            &fixture.selection,
            RuntimeSourceBuildBundleLimits::default(),
            &CancellationToken::new()
        ),
        Err(RuntimeSourceBuildBundleError::UnsafeSource)
    ));
}

fn write_bundle_fixture() -> BundleFixture {
    let root = tempdir().expect("temporary fixture root");
    let manifest_path = root.path().join("review/source-build-inputs.json");
    let component_root = root.path().join("components");
    let value = manifest_value();
    let manifest_bytes = serde_json::to_vec(&value).expect("serialize canonical manifest");
    let components = value["components"].as_array().expect("components");
    let mut component_paths = Vec::new();
    for component in components {
        let relative = component["relative_path"].as_str().expect("relative path");
        let path = component_root.join(relative);
        fs::create_dir_all(path.parent().expect("component parent"))
            .expect("create component parent");
        fs::write(&path, fixture_bytes(relative)).expect("write component");
        component_paths.push(path);
    }
    fs::create_dir_all(manifest_path.parent().expect("manifest parent"))
        .expect("create manifest parent");
    fs::write(&manifest_path, &manifest_bytes).expect("write manifest");
    let selection = RuntimeSourceBuildBundleSource::new(&manifest_path, &component_root)
        .expect("valid bundle selection");
    BundleFixture {
        _root: root,
        manifest_path,
        component_root,
        component_paths,
        manifest_bytes,
        selection,
    }
}

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
        "schema_version": RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION
    })
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
