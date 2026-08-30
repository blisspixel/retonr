use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LineageWire {
    pub(super) authority: Authority,
    pub(super) build_host: BuildHostWire,
    pub(super) build_programs: Vec<BuildProgramWire>,
    pub(super) build_recipe: MemberMeasurementWire,
    pub(super) cargo_lock: MemberMeasurementWire,
    pub(super) dependency_source_archive: DependencySourceArchiveWire,
    pub(super) preparation_tools: Vec<PreparationToolWire>,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) repository_source_archive: RepositorySourceArchiveWire,
    pub(super) rust_distribution: RustDistributionWire,
    pub(super) schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuildHostWire {
    pub(super) architecture: String,
    pub(super) busybox_executable: MemberMeasurementWire,
    pub(super) busybox_static: AlpinePackageWire,
    pub(super) distribution: String,
    pub(super) libgcc: AlpinePackageWire,
    pub(super) minirootfs: MemberMeasurementWire,
    pub(super) minirootfs_checksum: MemberMeasurementWire,
    pub(super) minirootfs_signature: MemberMeasurementWire,
    pub(super) minirootfs_signature_disposition: SignatureDisposition,
    pub(super) release_key: MemberMeasurementWire,
    pub(super) release_key_fingerprint: String,
    pub(super) schema_version: u32,
    pub(super) version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AlpinePackageWire {
    pub(super) package: MemberMeasurementWire,
    pub(super) package_name: String,
    pub(super) signature_disposition: SignatureDisposition,
    pub(super) version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DependencySourceArchiveWire {
    pub(super) archive_kind: DependencyArchiveKind,
    pub(super) archive_root: String,
    pub(super) cargo_lock: MemberMeasurementWire,
    pub(super) payload: MemberMeasurementWire,
    pub(super) raw_crate_archive_kind: DependencyArchiveKind,
    pub(super) raw_crate_archive_root: String,
    pub(super) raw_crate_source_archive: MemberMeasurementWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DependencyArchiveKind {
    CargoCrates,
    CargoVendor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RepositorySourceArchiveWire {
    pub(super) archive_kind: RepositoryArchiveKind,
    pub(super) archive_root: String,
    pub(super) payload: MemberMeasurementWire,
    pub(super) workspace_provenance: WorkspaceProvenanceWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RepositoryArchiveKind {
    RetonrSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WorkspaceProvenanceWire {
    pub(super) base_commit: String,
    pub(super) identity_source: WorkspaceIdentitySource,
    pub(super) workspace_state: WorkspaceState,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkspaceIdentitySource {
    Archive,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkspaceState {
    DirtySnapshot,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Authority {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BuildProgramWire {
    pub(super) name: BuildProgramName,
    pub(super) payload: MemberMeasurementWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BuildProgramName {
    IsolationHelper,
    SourceBuilder,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PreparationToolWire {
    pub(super) name: PreparationToolName,
    pub(super) payload: MemberMeasurementWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PreparationToolName {
    SourceArchivePreparation,
    SourceManifestPreparation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RustDistributionWire {
    pub(super) channel: String,
    pub(super) channel_manifest: MemberMeasurementWire,
    pub(super) channel_manifest_checksum: MemberMeasurementWire,
    pub(super) channel_manifest_digest: Digest,
    pub(super) channel_manifest_signature: MemberMeasurementWire,
    pub(super) components: Vec<RustComponentWire>,
    pub(super) host_target: String,
    pub(super) release_key: MemberMeasurementWire,
    pub(super) release_key_fingerprint: String,
    pub(super) signature_disposition: SignatureDisposition,
    pub(super) target_standard_library: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RustComponentWire {
    pub(super) name: RustComponentName,
    pub(super) payload: MemberMeasurementWire,
    pub(super) target: String,
    pub(super) url: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RustComponentName {
    Cargo,
    RustStd,
    Rustc,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SignatureDisposition {
    ReviewRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MemberMeasurementWire {
    pub(super) byte_size: u64,
    pub(super) digest: Digest,
    pub(super) relative_path: String,
}

impl MemberMeasurementWire {
    pub(super) fn without_path(&self) -> MeasurementWire {
        MeasurementWire {
            byte_size: self.byte_size,
            digest: self.digest.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct ToolEvidenceWire {
    pub(super) retained_program_lineage_binding: RetainedProgramLineageBindingWire,
    pub(super) schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RetainedProgramLineageBindingWire {
    pub(super) authority: Authority,
    pub(super) builder: MeasurementWire,
    pub(super) isolation_helper: MeasurementWire,
    pub(super) lineage_record: MeasurementWire,
    pub(super) profile_id: String,
    pub(super) profile_version: u32,
    pub(super) rust_host_target: String,
    pub(super) rustc: String,
    pub(super) source_archive_preparation: MeasurementWire,
    pub(super) source_manifest_preparation: MeasurementWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasurementWire {
    pub(super) byte_size: u64,
    pub(super) digest: Digest,
}
