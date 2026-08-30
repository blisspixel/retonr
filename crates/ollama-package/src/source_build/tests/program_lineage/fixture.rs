use std::collections::BTreeMap;

use rewrite_types::Digest;
use serde_json::{Value, json};

use crate::source_build::{
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputRole,
    VerifiedRetainedProgramLineage, VerifiedRuntimeSourceBuildInputs,
    program_lineage::{
        RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID, RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION,
        profile::PRODUCTION_COMPONENTS, verify_lineage,
    },
    tests::{artifact_set, controlled_environment, fixture_bytes, synthetic_static_elf},
};

pub(super) const RECIPE_PATH: &str = "lineage/build-recipe-v2.json";
pub(super) const LINEAGE_PATH: &str = "metadata/retained-program-lineage.json";
pub(super) const TOOL_PATH: &str = "metadata/tool-evidence.json";

pub(super) struct LineageFixture {
    manifest: Value,
    retained: BTreeMap<RuntimeSourceBuildInputRole, Vec<u8>>,
}

impl LineageFixture {
    pub(super) fn new() -> Self {
        let components = PRODUCTION_COMPONENTS
            .iter()
            .map(component)
            .collect::<Vec<_>>();
        let mut fixture = Self {
            manifest: json!({
                "artifact_set_id": artifact_set(&components).artifact_set_id(),
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
                "schema_version": 1
            }),
            retained: BTreeMap::new(),
        };
        fixture.retained.insert(
            RuntimeSourceBuildInputRole::CanonicalBuildRecipe,
            recipe_bytes(),
        );
        fixture.rebuild_lineage().expect("production fixture");
        fixture
    }

    pub(super) fn manifest(&self) -> RuntimeSourceBuildInputManifest {
        self.parsed_manifest().expect("valid production manifest")
    }

    pub(super) fn retained(&self, path: &str) -> &[u8] {
        let role = match path {
            RECIPE_PATH => RuntimeSourceBuildInputRole::CanonicalBuildRecipe,
            LINEAGE_PATH => RuntimeSourceBuildInputRole::RetainedProgramLineage,
            TOOL_PATH => RuntimeSourceBuildInputRole::ToolEvidence,
            _ => panic!("unknown retained path"),
        };
        &self.retained[&role]
    }

    pub(super) const fn retained_map(&self) -> &BTreeMap<RuntimeSourceBuildInputRole, Vec<u8>> {
        &self.retained
    }

    pub(super) fn verify(
        &self,
    ) -> Result<VerifiedRetainedProgramLineage, RuntimeSourceBuildInputError> {
        verify_lineage(&self.manifest(), &self.retained)
    }

    pub(super) fn verified_inputs(
        &self,
    ) -> Result<VerifiedRuntimeSourceBuildInputs, RuntimeSourceBuildInputError> {
        Ok(VerifiedRuntimeSourceBuildInputs {
            manifest: self.parsed_manifest()?,
            program_lineage: Some(self.verify()?),
        })
    }

    pub(super) fn replace_recipe(
        &mut self,
        bytes: Vec<u8>,
    ) -> Result<(), RuntimeSourceBuildInputError> {
        self.retained
            .insert(RuntimeSourceBuildInputRole::CanonicalBuildRecipe, bytes);
        self.rebuild_lineage()
    }

    pub(super) fn mutate_lineage(
        &mut self,
        pointer: &str,
        replacement: Value,
    ) -> Result<(), RuntimeSourceBuildInputError> {
        let mut lineage: Value = serde_json::from_slice(
            &self.retained[&RuntimeSourceBuildInputRole::RetainedProgramLineage],
        )
        .expect("lineage JSON");
        *lineage.pointer_mut(pointer).expect("lineage pointer") = replacement;
        self.store_lineage(&lineage)
    }

    pub(super) fn mutate_tool(
        &mut self,
        pointer: &str,
        replacement: Value,
    ) -> Result<(), RuntimeSourceBuildInputError> {
        let mut tool: Value =
            serde_json::from_slice(&self.retained[&RuntimeSourceBuildInputRole::ToolEvidence])
                .expect("tool JSON");
        *tool.pointer_mut(pointer).expect("tool pointer") = replacement;
        self.store_tool(&tool)
    }

    pub(super) fn rename_tool_binding(&mut self) -> Result<(), RuntimeSourceBuildInputError> {
        let mut tool: Value =
            serde_json::from_slice(&self.retained[&RuntimeSourceBuildInputRole::ToolEvidence])
                .expect("tool JSON");
        let binding = tool
            .as_object_mut()
            .expect("tool object")
            .remove("retained_program_lineage_binding")
            .expect("binding");
        tool["retained_programs"] = binding;
        self.store_tool(&tool)
    }

    pub(super) fn replace_retained_record(
        &mut self,
        path: &str,
        bytes: &[u8],
    ) -> Result<(), RuntimeSourceBuildInputError> {
        let role = if path == LINEAGE_PATH {
            RuntimeSourceBuildInputRole::RetainedProgramLineage
        } else {
            RuntimeSourceBuildInputRole::ToolEvidence
        };
        self.retained.insert(role, bytes.to_owned());
        self.set_measurement(path, bytes);
        self.refresh_artifact_set();
        self.parsed_manifest().map(|_| ())
    }

    pub(super) fn mutate_component(
        &mut self,
        path: &str,
        field: &str,
        replacement: Value,
    ) -> Result<(), RuntimeSourceBuildInputError> {
        self.component_mut(path)[field] = replacement;
        self.refresh_artifact_set();
        self.parsed_manifest().map(|_| ())
    }

    pub(super) fn add_extra_source_patch(&mut self) -> Result<(), RuntimeSourceBuildInputError> {
        let bytes = b"extra patch";
        self.manifest["components"]
            .as_array_mut()
            .expect("components")
            .push(json!({
                "byte_size": bytes.len(),
                "digest": Digest::sha256(bytes),
                "name": "unexpected production patch",
                "relative_path": "patches/unexpected.patch",
                "revision": "v1",
                "roles": ["source_patch"],
                "source_locator": "urn:unexpected"
            }));
        self.sort_components();
        self.refresh_artifact_set();
        self.parsed_manifest().map(|_| ())
    }

    fn rebuild_lineage(&mut self) -> Result<(), RuntimeSourceBuildInputError> {
        let recipe = self.retained[&RuntimeSourceBuildInputRole::CanonicalBuildRecipe].clone();
        self.set_measurement(RECIPE_PATH, &recipe);
        let lineage = self.lineage_value();
        self.store_lineage(&lineage)
    }

    fn store_lineage(&mut self, lineage: &Value) -> Result<(), RuntimeSourceBuildInputError> {
        let bytes = serde_json::to_vec(&lineage).expect("lineage serialization");
        self.retained.insert(
            RuntimeSourceBuildInputRole::RetainedProgramLineage,
            bytes.clone(),
        );
        self.set_measurement(LINEAGE_PATH, &bytes);
        let tool = self.tool_value();
        self.store_tool(&tool)
    }

    fn store_tool(&mut self, tool: &Value) -> Result<(), RuntimeSourceBuildInputError> {
        let bytes = serde_json::to_vec(&tool).expect("tool serialization");
        self.retained
            .insert(RuntimeSourceBuildInputRole::ToolEvidence, bytes.clone());
        self.set_measurement(TOOL_PATH, &bytes);
        self.refresh_artifact_set();
        self.parsed_manifest().map(|_| ())
    }

    fn lineage_value(&self) -> Value {
        let rust = |path: &str, name: &str, target: &str, url: &str| json!({"name":name, "payload":self.member(path), "target":target, "url":url});
        let channel = self.member("lineage/rust/channel-rust-1.97.1.toml");
        json!({
            "authority": "none",
            "build_host": {
                "architecture": "x86_64",
                "busybox_executable": self.member("toolchains/busybox"),
                "busybox_static": {
                    "package": self.member("lineage/host/alpine/busybox-static-1.37.0-r30.apk"),
                    "package_name": "busybox-static",
                    "signature_disposition": "review_required",
                    "version": "1.37.0-r30"
                },
                "distribution": "alpine",
                "libgcc": {
                    "package": self.member("lineage/host/alpine/libgcc-15.2.0-r2.apk"),
                    "package_name": "libgcc",
                    "signature_disposition": "review_required",
                    "version": "15.2.0-r2"
                },
                "minirootfs": self.member("lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz"),
                "minirootfs_checksum": self.member("lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256"),
                "minirootfs_signature": self.member("lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc"),
                "minirootfs_signature_disposition": "review_required",
                "release_key": self.member("lineage/host/alpine/ncopa.asc"),
                "release_key_fingerprint": "0482D84022F52DF1C4E7CD43293ACD0907D9495A",
                "schema_version": 1,
                "version": "3.23.3"
            },
            "build_programs": [
                {"name":"isolation_helper", "payload":self.member("helper/isolation")},
                {"name":"source_builder", "payload":self.member("scripts/build")}
            ],
            "build_recipe": self.member(RECIPE_PATH),
            "cargo_lock": self.member("lineage/source/Cargo.lock"),
            "dependency_source_archive": {
                "archive_kind": "cargo_vendor",
                "archive_root": "cargo-vendor",
                "cargo_lock": self.member("lineage/source/Cargo.lock"),
                "payload": self.member("lineage/source/cargo-vendor.tar")
                ,"raw_crate_archive_kind": "cargo_crates",
                "raw_crate_archive_root": "cargo-crates",
                "raw_crate_source_archive": self.member("lineage/source/cargo-crates.tar")
            },
            "preparation_tools": [
                {"name":"source_archive_preparation", "payload":self.member("lineage/tools/rewrite-runtime-source-archive")},
                {"name":"source_manifest_preparation", "payload":self.member("lineage/tools/rewrite-runtime-source-manifest")}
            ],
            "procedure_id": RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID,
            "procedure_version": RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION,
            "repository_source_archive": {
                "archive_kind": "retonr_source",
                "archive_root": "retonr-source",
                "payload": self.member("lineage/source/retonr-source.tar"),
                "workspace_provenance": {
                    "base_commit": "6a9a00bc1af7181fae6489f5856653ccc7c5bb4b",
                    "identity_source": "archive",
                    "workspace_state": "dirty_snapshot"
                }
            },
            "rust_distribution": {
                "channel": "1.97.1",
                "channel_manifest": channel.clone(),
                "channel_manifest_checksum": self.member("lineage/rust/channel-rust-1.97.1.toml.sha256"),
                "channel_manifest_digest": channel["digest"],
                "channel_manifest_signature": self.member("lineage/rust/channel-rust-1.97.1.toml.asc"),
                "components": [
                    rust("lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz", "cargo", "x86_64-unknown-linux-musl", "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz"),
                    rust("lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz", "rust_std", "x86_64-unknown-linux-musl", "https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz"),
                    rust("lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz", "rustc", "x86_64-unknown-linux-musl", "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz")
                ],
                "host_target": "x86_64-unknown-linux-musl",
                "release_key": self.member("lineage/rust/rust-key.gpg.ascii"),
                "release_key_fingerprint": "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE",
                "signature_disposition": "review_required",
                "target_standard_library": "x86_64-unknown-linux-musl"
            },
            "schema_version": 2
        })
    }

    fn tool_value(&self) -> Value {
        json!({
            "retained_program_lineage_binding": {
                "authority": "none",
                "builder": self.measurement("scripts/build"),
                "isolation_helper": self.measurement("helper/isolation"),
                "lineage_record": self.measurement(LINEAGE_PATH),
                "profile_id": "retonr:runtime-source-build:production-lineage",
                "profile_version": 2,
                "rust_host_target": "x86_64-unknown-linux-musl",
                "rustc": "1.97.1",
                "source_archive_preparation": self.measurement("lineage/tools/rewrite-runtime-source-archive"),
                "source_manifest_preparation": self.measurement("lineage/tools/rewrite-runtime-source-manifest")
            },
            "schema_version": 1
        })
    }

    fn member(&self, path: &str) -> Value {
        let component = self.component(path);
        json!({
            "byte_size": component["byte_size"],
            "digest": component["digest"],
            "relative_path": path
        })
    }

    fn measurement(&self, path: &str) -> Value {
        let component = self.component(path);
        json!({"byte_size":component["byte_size"], "digest":component["digest"]})
    }

    fn component(&self, path: &str) -> &Value {
        self.manifest["components"]
            .as_array()
            .expect("components")
            .iter()
            .find(|component| component["relative_path"] == path)
            .expect("fixture component")
    }

    fn component_mut(&mut self, path: &str) -> &mut Value {
        self.manifest["components"]
            .as_array_mut()
            .expect("components")
            .iter_mut()
            .find(|component| component["relative_path"] == path)
            .expect("fixture component")
    }

    fn set_measurement(&mut self, path: &str, bytes: &[u8]) {
        let component = self.component_mut(path);
        component["byte_size"] = json!(bytes.len());
        component["digest"] = json!(Digest::sha256(bytes));
    }

    fn sort_components(&mut self) {
        self.manifest["components"]
            .as_array_mut()
            .expect("components")
            .sort_by(|left, right| {
                left["relative_path"]
                    .as_str()
                    .cmp(&right["relative_path"].as_str())
            });
    }

    fn refresh_artifact_set(&mut self) {
        let components = self.manifest["components"].as_array().expect("components");
        self.manifest["artifact_set_id"] = json!(artifact_set(components).artifact_set_id());
    }

    fn parsed_manifest(
        &self,
    ) -> Result<RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputError> {
        RuntimeSourceBuildInputManifest::parse(
            &serde_json::to_vec(&self.manifest).expect("manifest serialization"),
            crate::source_build::RuntimeSourceBuildInputLimits::default(),
        )
    }
}

fn component(expected: &crate::source_build::program_lineage::profile::ExpectedComponent) -> Value {
    let bytes = input_bytes(expected.path);
    let (byte_size, digest) = expected.official.map_or_else(
        || (bytes.len() as u64, Digest::sha256(&bytes)),
        |(byte_size, digest)| {
            (
                byte_size,
                Digest::from_sha256_hex(digest).expect("official digest"),
            )
        },
    );
    json!({
        "byte_size": byte_size,
        "digest": digest,
        "name": expected.name,
        "relative_path": expected.path,
        "revision": expected.revision,
        "roles": expected.roles,
        "source_locator": expected.locator
    })
}

fn input_bytes(path: &str) -> Vec<u8> {
    match path {
        RECIPE_PATH => recipe_bytes(),
        "scripts/build" => synthetic_static_elf(0x1_000),
        "lineage/tools/rewrite-runtime-source-archive" => synthetic_static_elf(0x3_000),
        "lineage/tools/rewrite-runtime-source-manifest" => synthetic_static_elf(0x4_000),
        _ => fixture_bytes(path),
    }
}

fn recipe_bytes() -> Vec<u8> {
    include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../support/runtime-source-build/retained-program-build-recipe-v2.json"
    ))
    .to_vec()
}
