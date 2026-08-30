use std::{fs, path::Path};

use rewrite_model::{ArtifactSetRelativePath, RuntimePackageLoadPolicy, RuntimePackageMemberRole};
use rewrite_ollama_package::{RuntimeLayoutLimits, RuntimePackageLayout};
use rewrite_types::Digest;
use serde_json::{Value, json};

use crate::{BuildError, arguments::BuildArguments, filesystem::digest_file};

pub(super) const SUCCESS_OUTPUT: &[u8] = b"{\"schema_version\":1,\"status\":\"success\"}\n";
const MAXIMUM_MEMBERS: usize = 64;
const BUILD_STEPS: [&str; 8] = [
    "extract-frozen-inputs",
    "verify-self-contained-tools",
    "apply-ollama-compatibility-patch",
    "configure-llama-cpp-cpu",
    "build-llama-cpp-cpu",
    "install-llama-cpp-cpu",
    "build-ollama-go-entrypoint",
    "assemble-runtime-package",
];

struct Member {
    path: String,
    roles: Vec<RuntimePackageMemberRole>,
    policy: RuntimePackageLoadPolicy,
    byte_size: u64,
    digest: Digest,
}

pub(super) fn write(
    output_root: &Path,
    arguments: &BuildArguments,
    builder_digest: &Digest,
) -> Result<(), BuildError> {
    let provenance = canonical(&json!({
        "build_revision": arguments.build_revision,
        "builder_digest": builder_digest,
        "reported_version": arguments.reported_version,
        "schema_version": 1,
        "source_inputs_id": arguments.source_inputs_id,
        "source_provenance_digest": arguments.source_provenance_digest,
        "target": "x86_64-linux-gnu"
    }))?;
    let transformation = canonical(&json!({
        "accelerator": "cpu_only",
        "build_steps": BUILD_STEPS,
        "network_access": "denied",
        "parameters_digest": arguments.parameters_digest,
        "schema_version": 1,
        "source_inputs_id": arguments.source_inputs_id,
        "tool_evidence_digest": arguments.tool_evidence_digest
    }))?;
    write_member(output_root, "provenance/source-build.json", &provenance)?;
    write_member(output_root, "review/transformation.json", &transformation)?;
    let mut members = collect_members(output_root)?;
    let sbom_components = members
        .iter()
        .map(|member| {
            json!({
                "byte_size": member.byte_size,
                "digest": member.digest,
                "relative_path": member.path,
                "roles": member.roles
            })
        })
        .collect::<Vec<_>>();
    let sbom = canonical(&json!({
        "components": sbom_components,
        "runtime_family": "ollama",
        "schema_version": 1,
        "source_inputs_id": arguments.source_inputs_id
    }))?;
    write_member(output_root, "review/sbom.json", &sbom)?;
    members = collect_members(output_root)?;
    let declarations = members
        .iter()
        .map(|member| {
            json!({
                "byte_size": member.byte_size,
                "digest": member.digest,
                "load_policy": member.policy,
                "relative_path": member.path,
                "roles": member.roles
            })
        })
        .collect::<Vec<_>>();
    let observed_tree = members
        .iter()
        .map(|member| member.path.as_str())
        .collect::<Vec<_>>();
    let layout = canonical(&json!({
        "build_revision": arguments.build_revision,
        "members": declarations,
        "observed_tree": observed_tree,
        "reported_version": arguments.reported_version,
        "runtime_family": "ollama",
        "schema_version": 1,
        "source": {
            "kind": "repository_revision",
            "locator": "https://github.com/ollama/ollama",
            "provenance_digest": arguments.source_provenance_digest,
            "revision": arguments.build_revision,
            "schema_version": 1
        },
        "target": {
            "abi": "linux_gnu_libc",
            "architecture": "x86_64",
            "operating_system": "linux"
        },
        "transformation": {
            "kind": "transformed",
            "log_digest": Digest::sha256(SUCCESS_OUTPUT),
            "parameters_digest": arguments.parameters_digest,
            "source_artifact_set_id": arguments.source_inputs_id,
            "tool_evidence_digest": arguments.tool_evidence_digest
        }
    }))?;
    RuntimePackageLayout::parse(&layout, RuntimeLayoutLimits::default())
        .map_err(|_| BuildError::EvidenceInvalid)?;
    fs::write(output_root.join("runtime-layout.json"), layout)
        .map_err(|_| BuildError::EvidenceInvalid)?;
    fs::write(output_root.join("sbom.json"), sbom).map_err(|_| BuildError::EvidenceInvalid)?;
    fs::write(output_root.join("provenance.json"), provenance)
        .map_err(|_| BuildError::EvidenceInvalid)?;
    fs::write(output_root.join("transformation.json"), transformation)
        .map_err(|_| BuildError::EvidenceInvalid)
}

fn collect_members(output_root: &Path) -> Result<Vec<Member>, BuildError> {
    let mut paths = Vec::new();
    for root in ["bin", "helper", "legal", "lib", "provenance", "review"] {
        collect_files(output_root, &output_root.join(root), &mut paths, 0)?;
    }
    paths.sort_unstable();
    if paths.is_empty() || paths.len() > MAXIMUM_MEMBERS {
        return Err(BuildError::EvidenceInvalid);
    }
    paths
        .into_iter()
        .map(|path| member(output_root, &path))
        .collect()
}

fn collect_files(
    output_root: &Path,
    directory: &Path,
    paths: &mut Vec<String>,
    depth: usize,
) -> Result<(), BuildError> {
    if depth > 16 {
        return Err(BuildError::EvidenceInvalid);
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|_| BuildError::EvidenceInvalid)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| BuildError::EvidenceInvalid)?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let kind = entry.file_type().map_err(|_| BuildError::EvidenceInvalid)?;
        if kind.is_dir() {
            collect_files(output_root, &entry.path(), paths, depth + 1)?;
        } else if kind.is_file() {
            let relative = entry
                .path()
                .strip_prefix(output_root)
                .map_err(|_| BuildError::EvidenceInvalid)?
                .to_string_lossy()
                .replace('\\', "/");
            ArtifactSetRelativePath::new(&relative).map_err(|_| BuildError::EvidenceInvalid)?;
            paths.push(relative);
        } else {
            return Err(BuildError::EvidenceInvalid);
        }
    }
    Ok(())
}

fn member(output_root: &Path, path: &str) -> Result<Member, BuildError> {
    let (roles, policy) = classify(path)?;
    let (byte_size, digest) = digest_file(&output_root.join(path))?;
    if byte_size == 0 {
        return Err(BuildError::EvidenceInvalid);
    }
    Ok(Member {
        path: path.to_owned(),
        roles,
        policy,
        byte_size,
        digest,
    })
}

fn classify(
    path: &str,
) -> Result<(Vec<RuntimePackageMemberRole>, RuntimePackageLoadPolicy), BuildError> {
    use RuntimePackageLoadPolicy::{BackendConditional, MustNotBeCodeLoaded, RequiredAtReady};
    use RuntimePackageMemberRole::{
        BuildConfiguration, Entrypoint, HelperExecutable, LicenseText, NativeDependency,
        ProvenanceRecord, RuntimeResource, TransformationRecord, UtilityExecutable,
        WorkerExecutable,
    };
    match path {
        "bin/ollama" => Ok((vec![Entrypoint], RequiredAtReady)),
        "helper/retonr-isolation" => Ok((vec![HelperExecutable], MustNotBeCodeLoaded)),
        "lib/ollama/llama-server" => Ok((vec![WorkerExecutable], BackendConditional)),
        "lib/ollama/llama-quantize" => Ok((vec![UtilityExecutable], MustNotBeCodeLoaded)),
        "provenance/source-build.json" => Ok((vec![ProvenanceRecord], MustNotBeCodeLoaded)),
        "review/sbom.json" => Ok((vec![BuildConfiguration], MustNotBeCodeLoaded)),
        "review/transformation.json" => Ok((vec![TransformationRecord], MustNotBeCodeLoaded)),
        _ if path.starts_with("legal/") => Ok((vec![LicenseText], MustNotBeCodeLoaded)),
        _ if path.starts_with("lib/ollama/")
            && (Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("so"))
                || path.contains(".so.")) =>
        {
            Ok((vec![NativeDependency], BackendConditional))
        }
        _ if path.starts_with("lib/ollama/") => Ok((vec![RuntimeResource], MustNotBeCodeLoaded)),
        _ => Err(BuildError::EvidenceInvalid),
    }
}

fn write_member(output_root: &Path, path: &str, bytes: &[u8]) -> Result<(), BuildError> {
    let path = output_root.join(path);
    fs::create_dir_all(path.parent().ok_or(BuildError::EvidenceInvalid)?)
        .map_err(|_| BuildError::EvidenceInvalid)?;
    fs::write(path, bytes).map_err(|_| BuildError::EvidenceInvalid)
}

fn canonical(value: &Value) -> Result<Vec<u8>, BuildError> {
    serde_json::to_vec(value).map_err(|_| BuildError::EvidenceInvalid)
}
