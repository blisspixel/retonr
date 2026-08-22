use std::{fs, path::Path};

use rewrite_ollama_package::{
    ADMITTED_RUNTIME_FAMILY, RUNTIME_LAYOUT_SCHEMA_VERSION, RuntimeLayoutLimits,
};
use rewrite_types::Digest;
use serde_json::json;

use super::super::{OllamaRuntimeImportLimits, ReviewedOllamaRuntimeSource};

pub(super) struct RuntimeFixture {
    pub(super) selection: ReviewedOllamaRuntimeSource,
    pub(super) layout_path: std::path::PathBuf,
    pub(super) member_root: std::path::PathBuf,
    pub(super) member_paths: Vec<std::path::PathBuf>,
}

pub(super) fn import_limits() -> OllamaRuntimeImportLimits {
    OllamaRuntimeImportLimits {
        reconstruction: RuntimeLayoutLimits::default(),
        artifact_set: crate::ArtifactSetImportLimits {
            maximum_members: 8,
            maximum_member_bytes: 1024 * 1024,
            maximum_total_bytes: 2 * 1024 * 1024,
            maximum_tree_entries: 16,
            maximum_storage_entries: 8,
            maximum_staging_entries: 8,
        },
    }
}

pub(super) fn write_runtime_fixture(root: &Path) -> RuntimeFixture {
    let members = [
        (
            "bin/ollama",
            json!(["entrypoint"]),
            "required_at_ready",
            b"ollama-entrypoint\n".as_slice(),
        ),
        (
            "helper/retonr-isolation",
            json!(["helper_executable"]),
            "must_not_be_code_loaded",
            b"isolation-helper\n".as_slice(),
        ),
        (
            "legal/license.txt",
            json!(["license_text"]),
            "must_not_be_code_loaded",
            b"Fixture runtime license\n".as_slice(),
        ),
        (
            "lib/ollama/libggml-cpu.so",
            json!(["native_dependency"]),
            "backend_conditional",
            b"ggml-cpu\n".as_slice(),
        ),
        (
            "lib/ollama/llama-server",
            json!(["worker_executable"]),
            "backend_conditional",
            b"llama-server\n".as_slice(),
        ),
        (
            "provenance/source.txt",
            json!(["provenance_record"]),
            "must_not_be_code_loaded",
            b"reviewed-source\n".as_slice(),
        ),
    ];
    let member_root = root.join("package");
    fs::create_dir(&member_root).expect("create member root");
    let mut declared = Vec::new();
    let mut observed = Vec::new();
    let mut member_paths = Vec::new();
    for (path, roles, policy, bytes) in members {
        let digest = Digest::sha256(bytes);
        let dest = member_root.join(path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).expect("create member parent");
        }
        fs::write(&dest, bytes).expect("write member");
        member_paths.push(dest);
        observed.push(path);
        declared.push(json!({
            "relative_path": path,
            "roles": roles,
            "load_policy": policy,
            "byte_size": bytes.len(),
            "digest": digest.as_str(),
        }));
    }
    let layout = json!({
        "schema_version": RUNTIME_LAYOUT_SCHEMA_VERSION,
        "runtime_family": ADMITTED_RUNTIME_FAMILY,
        "reported_version": "0.32.15",
        "build_revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
        "target": {
            "operating_system": "linux",
            "architecture": "x86_64",
            "abi": "linux_gnu_libc"
        },
        "source": {
            "schema_version": 1,
            "kind": "repository_revision",
            "locator": "https://github.com/ollama/ollama",
            "revision": "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
            "provenance_digest": Digest::sha256(b"runtime-source").as_str()
        },
        "transformation": {
            "kind": "untransformed",
            "evidence_digest": Digest::sha256(b"untransformed").as_str()
        },
        "members": declared,
        "observed_tree": observed
    });
    let review = root.join("review");
    fs::create_dir(&review).expect("create layout parent");
    let layout_path = review.join("runtime-layout.json");
    fs::write(
        &layout_path,
        serde_json::to_vec(&layout).expect("layout serializes"),
    )
    .expect("write layout");
    let selection = ReviewedOllamaRuntimeSource::new(&layout_path, &member_root)
        .expect("fixture source selection");
    RuntimeFixture {
        selection,
        layout_path,
        member_root,
        member_paths,
    }
}
