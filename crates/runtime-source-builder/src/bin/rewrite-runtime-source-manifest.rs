#![forbid(unsafe_code)]

use std::{
    env,
    fs::{File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    process::ExitCode,
};

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_ollama_package::{
    RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION, RuntimeSourceBuildInputLimits,
    RuntimeSourceBuildInputOpenError, verify_runtime_source_build_inputs,
};
use rewrite_types::Digest;
use serde_json::{Value, json};

#[path = "runtime_source_manifest/canonical_json.rs"]
mod canonical_json;
#[path = "runtime_source_manifest/component_specs.rs"]
mod component_specs;
#[path = "runtime_source_manifest/evidence.rs"]
mod evidence;
#[path = "runtime_source_manifest/license_inventory.rs"]
mod license_inventory;
#[path = "runtime_source_manifest/manifest_support.rs"]
mod manifest_support;
#[path = "runtime_source_manifest/program_lineage.rs"]
mod program_lineage;
#[path = "runtime_source_archive/object.rs"]
mod retained_object;

use component_specs::{COMPONENT_SPECS, ComponentSpec};
use manifest_support::{measure_file, parameter_parallel_jobs, role_digest};

const RETAINED_OLLAMA_RUNTIME_VERSION: &str = "0.32.15";

fn main() -> ExitCode {
    if run().is_ok() {
        ExitCode::SUCCESS
    } else {
        let _ = std::io::stderr().write_all(b"runtime-source-manifest-error\n");
        ExitCode::from(70)
    }
}

fn run() -> Result<(), ()> {
    let mut arguments = env::args_os();
    let _program = arguments.next().ok_or(())?;
    let operation = arguments.next().ok_or(())?;
    let component_root = PathBuf::from(arguments.next().ok_or(())?);
    if !component_root.is_absolute() {
        return Err(());
    }
    let component_root = component_root.canonicalize().map_err(|_| ())?;
    if !component_root.is_dir() {
        return Err(());
    }
    match operation.to_str() {
        Some("evidence") if arguments.next().is_none() => evidence::write(&component_root),
        Some("licenses") => {
            let review_root = PathBuf::from(arguments.next().ok_or(())?);
            if arguments.next().is_some() || !review_root.is_absolute() {
                return Err(());
            }
            license_inventory::write(&component_root, &review_root)
        }
        Some("license-review-template") => {
            let destination = PathBuf::from(arguments.next().ok_or(())?);
            if arguments.next().is_some()
                || !destination.is_absolute()
                || destination.exists()
                || destination.starts_with(&component_root)
            {
                return Err(());
            }
            license_inventory::write_template(&component_root, &destination)
        }
        Some("manifest") => {
            let destination = PathBuf::from(arguments.next().ok_or(())?);
            if arguments.next().is_some()
                || !destination.is_absolute()
                || destination.exists()
                || destination.starts_with(&component_root)
            {
                return Err(());
            }
            compile_manifest(&component_root, &destination)
        }
        _ => Err(()),
    }
}

fn compile_manifest(component_root: &Path, destination: &Path) -> Result<(), ()> {
    let components = COMPONENT_SPECS
        .into_iter()
        .map(|specification| component(component_root, specification))
        .collect::<Result<Vec<_>, _>>()?;
    let artifact_set = ArtifactSetManifest::new(
        components
            .iter()
            .map(|component| {
                ArtifactSetMember::new(
                    ArtifactId::from_digest(component.digest.clone()),
                    component.byte_size,
                    component.relative_path.clone(),
                )
            })
            .collect(),
    )
    .map_err(|_| ())?;
    let source_inputs_id = artifact_set.artifact_set_id();
    let source_provenance = role_digest(&components, "source_provenance")?;
    let tool_evidence = role_digest(&components, "tool_evidence")?;
    let parameters = role_digest(&components, "build_parameters")?;
    let parallel_jobs = parameter_parallel_jobs(component_root)?;
    let component_values = components.iter().map(Component::value).collect::<Vec<_>>();
    let manifest = serde_json::to_vec(&json!({
        "artifact_set_id": source_inputs_id,
        "components": component_values,
        "policy": {
            "accelerator": "cpu_only",
            "build_arguments": [
                "--build-runtime",
                "--cpu-only",
                "--offline",
                "--build-revision",
                "b7871fc0d1d82fe109536efa3e0e8e411c766c75",
                "--jobs",
                parallel_jobs,
                "--parameters-digest",
                parameters,
                "--reported-version",
                RETAINED_OLLAMA_RUNTIME_VERSION,
                "--source-inputs-id",
                source_inputs_id,
                "--source-provenance-digest",
                source_provenance,
                "--tool-evidence-digest",
                tool_evidence
            ],
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
    }))
    .map_err(|_| ())?;
    verify_runtime_source_build_inputs(
        &manifest,
        RuntimeSourceBuildInputLimits::default(),
        |path| {
            File::open(component_root.join(path.as_str()))
                .map_err(|_| RuntimeSourceBuildInputOpenError)
        },
        || false,
    )
    .map_err(|_| ())?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|_| ())?;
    output.write_all(&manifest).map_err(|_| ())
}

struct Component {
    relative_path: ArtifactSetRelativePath,
    name: &'static str,
    revision: &'static str,
    source_locator: &'static str,
    roles: &'static [&'static str],
    byte_size: u64,
    digest: Digest,
}

impl Component {
    fn value(&self) -> Value {
        json!({
            "byte_size": self.byte_size,
            "digest": self.digest,
            "name": self.name,
            "relative_path": self.relative_path,
            "revision": self.revision,
            "roles": self.roles,
            "source_locator": self.source_locator
        })
    }
}

fn component(root: &Path, specification: ComponentSpec) -> Result<Component, ()> {
    let relative_path =
        ArtifactSetRelativePath::new(specification.relative_path).map_err(|_| ())?;
    let path = root.join(relative_path.as_str());
    let metadata = path.symlink_metadata().map_err(|_| ())?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(());
    }
    let measurement = measure_file(&path)?;
    if measurement.byte_size != metadata.len() {
        return Err(());
    }
    Ok(Component {
        relative_path,
        name: specification.name,
        revision: specification.revision,
        source_locator: specification.source_locator,
        roles: specification.roles,
        byte_size: measurement.byte_size,
        digest: measurement.digest,
    })
}

#[cfg(test)]
mod tests {
    use super::RETAINED_OLLAMA_RUNTIME_VERSION;

    #[test]
    fn retained_manifest_uses_the_stable_runtime_api_version() {
        assert_eq!(RETAINED_OLLAMA_RUNTIME_VERSION, "0.32.15");
    }
}
