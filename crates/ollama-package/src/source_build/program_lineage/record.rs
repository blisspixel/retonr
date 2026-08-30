use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use crate::json::validate_unique_json;

use super::{
    RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID, RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION,
    RetainedProgramBuildRecipeId, RetainedProgramLineageId,
};
use crate::source_build::{
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputError,
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputRole, sequence_digest,
};

mod wire;

use wire::{
    Authority, BuildHostWire, BuildProgramName, DependencyArchiveKind, LineageWire,
    MemberMeasurementWire, PreparationToolName, RepositoryArchiveKind, RustComponentName,
    RustComponentWire, RustDistributionWire, SignatureDisposition, ToolEvidenceWire,
    WorkspaceIdentitySource, WorkspaceState,
};

pub(super) const MAXIMUM_LINEAGE_BYTES: u64 = 256 * 1024;
pub(super) const MAXIMUM_TOOL_EVIDENCE_BYTES: u64 = 256 * 1024;
const LINEAGE_DOMAIN: &[u8] = b"runtime-source-build/retained-program-lineage/v2";

const RECIPE_PATH: &str = "lineage/build-recipe-v2.json";
const CARGO_LOCK_PATH: &str = "lineage/source/Cargo.lock";
const CARGO_CRATES_PATH: &str = "lineage/source/cargo-crates.tar";
const CARGO_VENDOR_PATH: &str = "lineage/source/cargo-vendor.tar";
const RETONR_SOURCE_PATH: &str = "lineage/source/retonr-source.tar";
const ALPINE_ROOT: &str = "lineage/host/alpine/";
const ALPINE_MINIROOTFS_PATH: &str = "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz";
const ALPINE_CHECKSUM_PATH: &str =
    "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.sha256";
const ALPINE_SIGNATURE_PATH: &str =
    "lineage/host/alpine/alpine-minirootfs-3.23.3-x86_64.tar.gz.asc";
const ALPINE_RELEASE_KEY_PATH: &str = "lineage/host/alpine/ncopa.asc";
const ALPINE_LIBGCC_PATH: &str = "lineage/host/alpine/libgcc-15.2.0-r2.apk";
const ALPINE_BUSYBOX_APK_PATH: &str = "lineage/host/alpine/busybox-static-1.37.0-r30.apk";
const BUSYBOX_EXECUTABLE_PATH: &str = "toolchains/busybox";
const CHANNEL_PATH: &str = "lineage/rust/channel-rust-1.97.1.toml";
const CHANNEL_CHECKSUM_PATH: &str = "lineage/rust/channel-rust-1.97.1.toml.sha256";
const CHANNEL_SIGNATURE_PATH: &str = "lineage/rust/channel-rust-1.97.1.toml.asc";
const TRUST_ROOT_PATH: &str = "lineage/rust/rust-key.gpg.ascii";
const ARCHIVE_TOOL_PATH: &str = "lineage/tools/rewrite-runtime-source-archive";
const MANIFEST_TOOL_PATH: &str = "lineage/tools/rewrite-runtime-source-manifest";
const BUILDER_PATH: &str = "scripts/build";
const HELPER_PATH: &str = "helper/isolation";
const TOOL_EVIDENCE_PATH: &str = "metadata/tool-evidence.json";
const LINEAGE_PATH: &str = "metadata/retained-program-lineage.json";

pub(super) fn parse_and_verify(
    bytes: &[u8],
    tool_evidence_bytes: &[u8],
    manifest: &RuntimeSourceBuildInputManifest,
    _recipe_id: &RetainedProgramBuildRecipeId,
) -> Result<RetainedProgramLineageId, RuntimeSourceBuildInputError> {
    let record: LineageWire = parse_canonical(bytes, MAXIMUM_LINEAGE_BYTES)?;
    validate_header(&record)?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::CanonicalBuildRecipe,
        RECIPE_PATH,
        &record.build_recipe,
    )?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::CargoLockfile,
        CARGO_LOCK_PATH,
        &record.cargo_lock,
    )?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::CargoVendorSource,
        CARGO_VENDOR_PATH,
        &record.dependency_source_archive.payload,
    )?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::CargoRawCrateSource,
        CARGO_CRATES_PATH,
        &record.dependency_source_archive.raw_crate_source_archive,
    )?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::RetonrRepositorySource,
        RETONR_SOURCE_PATH,
        &record.repository_source_archive.payload,
    )?;
    validate_archive_records(&record)?;
    verify_build_host(manifest, &record.build_host)?;
    verify_programs(manifest, &record)?;
    verify_rust(manifest, &record.rust_distribution)?;
    verify_tool_evidence(manifest, tool_evidence_bytes, bytes, &record)?;
    Ok(RetainedProgramLineageId(sequence_digest(
        LINEAGE_DOMAIN,
        [bytes],
    )))
}

fn verify_build_host(
    manifest: &RuntimeSourceBuildInputManifest,
    host: &BuildHostWire,
) -> Result<(), RuntimeSourceBuildInputError> {
    if host.architecture != "x86_64"
        || host.distribution != "alpine"
        || host.version != "3.23.3"
        || host.schema_version != 1
        || host.minirootfs_signature_disposition != SignatureDisposition::ReviewRequired
        || host.release_key_fingerprint != "0482D84022F52DF1C4E7CD43293ACD0907D9495A"
        || host.libgcc.package_name != "libgcc"
        || host.libgcc.version != "15.2.0-r2"
        || host.libgcc.signature_disposition != SignatureDisposition::ReviewRequired
        || host.busybox_static.package_name != "busybox-static"
        || host.busybox_static.version != "1.37.0-r30"
        || host.busybox_static.signature_disposition != SignatureDisposition::ReviewRequired
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    for (role, path, measurement) in [
        (
            RuntimeSourceBuildInputRole::AlpineMinirootfs,
            ALPINE_MINIROOTFS_PATH,
            &host.minirootfs,
        ),
        (
            RuntimeSourceBuildInputRole::AlpineMinirootfsChecksum,
            ALPINE_CHECKSUM_PATH,
            &host.minirootfs_checksum,
        ),
        (
            RuntimeSourceBuildInputRole::AlpineMinirootfsSignature,
            ALPINE_SIGNATURE_PATH,
            &host.minirootfs_signature,
        ),
        (
            RuntimeSourceBuildInputRole::AlpineReleaseTrustRoot,
            ALPINE_RELEASE_KEY_PATH,
            &host.release_key,
        ),
        (
            RuntimeSourceBuildInputRole::AlpineLibgccPackage,
            ALPINE_LIBGCC_PATH,
            &host.libgcc.package,
        ),
        (
            RuntimeSourceBuildInputRole::AlpineBusyboxStaticPackage,
            ALPINE_BUSYBOX_APK_PATH,
            &host.busybox_static.package,
        ),
    ] {
        if !path.starts_with(ALPINE_ROOT) {
            return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
        }
        join(manifest, role, path, measurement)?;
    }
    join(
        manifest,
        RuntimeSourceBuildInputRole::PosixShell,
        BUSYBOX_EXECUTABLE_PATH,
        &host.busybox_executable,
    )
}

fn validate_archive_records(record: &LineageWire) -> Result<(), RuntimeSourceBuildInputError> {
    let dependencies = &record.dependency_source_archive;
    let repository = &record.repository_source_archive;
    if dependencies.archive_kind != DependencyArchiveKind::CargoVendor
        || dependencies.archive_root != "cargo-vendor"
        || dependencies.cargo_lock != record.cargo_lock
        || dependencies.raw_crate_archive_kind != DependencyArchiveKind::CargoCrates
        || dependencies.raw_crate_archive_root != "cargo-crates"
        || repository.archive_kind != RepositoryArchiveKind::RetonrSource
        || repository.archive_root != "retonr-source"
        || repository.workspace_provenance.base_commit != "6a9a00bc1af7181fae6489f5856653ccc7c5bb4b"
        || repository.workspace_provenance.identity_source != WorkspaceIdentitySource::Archive
        || repository.workspace_provenance.workspace_state != WorkspaceState::DirtySnapshot
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    Ok(())
}

fn validate_header(record: &LineageWire) -> Result<(), RuntimeSourceBuildInputError> {
    if record.authority != Authority::None
        || record.procedure_id != RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID
        || record.procedure_version != RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION
        || record.schema_version != 2
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    Ok(())
}

fn verify_programs(
    manifest: &RuntimeSourceBuildInputManifest,
    record: &LineageWire,
) -> Result<(), RuntimeSourceBuildInputError> {
    let [helper, builder] = record.build_programs.as_slice() else {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    };
    if helper.name != BuildProgramName::IsolationHelper
        || builder.name != BuildProgramName::SourceBuilder
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    join(
        manifest,
        RuntimeSourceBuildInputRole::IsolationHelper,
        HELPER_PATH,
        &helper.payload,
    )?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::BuildScript,
        BUILDER_PATH,
        &builder.payload,
    )?;

    let [archive, manifest_tool] = record.preparation_tools.as_slice() else {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    };
    if archive.name != PreparationToolName::SourceArchivePreparation
        || manifest_tool.name != PreparationToolName::SourceManifestPreparation
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    join(
        manifest,
        RuntimeSourceBuildInputRole::SourceArchivePreparationTool,
        ARCHIVE_TOOL_PATH,
        &archive.payload,
    )?;
    join(
        manifest,
        RuntimeSourceBuildInputRole::SourceManifestPreparationTool,
        MANIFEST_TOOL_PATH,
        &manifest_tool.payload,
    )
}

fn verify_rust(
    manifest: &RuntimeSourceBuildInputManifest,
    rust: &RustDistributionWire,
) -> Result<(), RuntimeSourceBuildInputError> {
    if rust.channel != "1.97.1"
        || rust.host_target != "x86_64-unknown-linux-musl"
        || rust.target_standard_library != "x86_64-unknown-linux-musl"
        || rust.release_key_fingerprint != "108F66205EAEB0AAA8DD5E1C85AB96E6FA1BE5FE"
        || rust.signature_disposition != SignatureDisposition::ReviewRequired
        || rust.channel_manifest_digest != rust.channel_manifest.digest
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    for (role, path, measurement) in [
        (
            RuntimeSourceBuildInputRole::RustChannelManifest,
            CHANNEL_PATH,
            &rust.channel_manifest,
        ),
        (
            RuntimeSourceBuildInputRole::RustChannelManifestChecksum,
            CHANNEL_CHECKSUM_PATH,
            &rust.channel_manifest_checksum,
        ),
        (
            RuntimeSourceBuildInputRole::RustChannelManifestSignature,
            CHANNEL_SIGNATURE_PATH,
            &rust.channel_manifest_signature,
        ),
        (
            RuntimeSourceBuildInputRole::RustReleaseTrustRoot,
            TRUST_ROOT_PATH,
            &rust.release_key,
        ),
    ] {
        join(manifest, role, path, measurement)?;
    }
    verify_rust_components(manifest, &rust.components)
}

fn verify_rust_components(
    manifest: &RuntimeSourceBuildInputManifest,
    components: &[RustComponentWire],
) -> Result<(), RuntimeSourceBuildInputError> {
    const EXPECTED: [(
        RustComponentName,
        &str,
        &str,
        &str,
        RuntimeSourceBuildInputRole,
    ); 3] = [
        (
            RustComponentName::Cargo,
            "x86_64-unknown-linux-musl",
            "lineage/rust/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
            "https://static.rust-lang.org/dist/2026-07-16/cargo-1.97.1-x86_64-unknown-linux-musl.tar.gz",
            RuntimeSourceBuildInputRole::RustCargoDistribution,
        ),
        (
            RustComponentName::RustStd,
            "x86_64-unknown-linux-musl",
            "lineage/rust/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
            "https://static.rust-lang.org/dist/2026-07-16/rust-std-1.97.1-x86_64-unknown-linux-musl.tar.gz",
            RuntimeSourceBuildInputRole::RustTargetStandardLibraryDistribution,
        ),
        (
            RustComponentName::Rustc,
            "x86_64-unknown-linux-musl",
            "lineage/rust/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
            "https://static.rust-lang.org/dist/2026-07-16/rustc-1.97.1-x86_64-unknown-linux-musl.tar.gz",
            RuntimeSourceBuildInputRole::RustCompilerDistribution,
        ),
    ];
    if components.len() != EXPECTED.len() {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    for (component, (name, target, path, url, role)) in components.iter().zip(EXPECTED) {
        if component.name != name || component.target != target || component.url != url {
            return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
        }
        join(manifest, role, path, &component.payload)?;
        if name == RustComponentName::RustStd {
            join(
                manifest,
                RuntimeSourceBuildInputRole::RustHostStandardLibraryDistribution,
                path,
                &component.payload,
            )?;
        }
    }
    Ok(())
}

fn verify_tool_evidence(
    manifest: &RuntimeSourceBuildInputManifest,
    bytes: &[u8],
    lineage_bytes: &[u8],
    record: &LineageWire,
) -> Result<(), RuntimeSourceBuildInputError> {
    let evidence: ToolEvidenceWire = parse_canonical(bytes, MAXIMUM_TOOL_EVIDENCE_BYTES)?;
    let retained = evidence.retained_program_lineage_binding;
    if evidence.schema_version != 1
        || retained.authority != Authority::None
        || retained.profile_id != "retonr:runtime-source-build:production-lineage"
        || retained.profile_version != 2
        || retained.rust_host_target != "x86_64-unknown-linux-musl"
        || retained.rustc != "1.97.1"
        || retained.lineage_record.byte_size
            != u64::try_from(lineage_bytes.len()).unwrap_or(u64::MAX)
        || retained.lineage_record.digest != Digest::sha256(lineage_bytes)
        || retained.builder != record.build_programs[1].payload.without_path()
        || retained.isolation_helper != record.build_programs[0].payload.without_path()
        || retained.source_archive_preparation != record.preparation_tools[0].payload.without_path()
        || retained.source_manifest_preparation
            != record.preparation_tools[1].payload.without_path()
    {
        return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
    }
    let lineage_component = find_role(
        manifest,
        RuntimeSourceBuildInputRole::RetainedProgramLineage,
    )?;
    let tool_component = find_role(manifest, RuntimeSourceBuildInputRole::ToolEvidence)?;
    if lineage_component.relative_path().as_str() != LINEAGE_PATH
        || tool_component.relative_path().as_str() != TOOL_EVIDENCE_PATH
        || retained.lineage_record.byte_size != lineage_component.byte_size()
        || retained.lineage_record.digest != *lineage_component.digest()
    {
        return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
    }
    Ok(())
}

fn join(
    manifest: &RuntimeSourceBuildInputManifest,
    role: RuntimeSourceBuildInputRole,
    expected_path: &str,
    measurement: &MemberMeasurementWire,
) -> Result<(), RuntimeSourceBuildInputError> {
    let component = find_role(manifest, role)?;
    let relative_path = rewrite_model::ArtifactSetRelativePath::new(&measurement.relative_path)
        .map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    if measurement.relative_path != expected_path
        || component.relative_path() != &relative_path
        || component.byte_size() != measurement.byte_size
        || component.digest() != &measurement.digest
    {
        return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
    }
    Ok(())
}

fn find_role(
    manifest: &RuntimeSourceBuildInputManifest,
    role: RuntimeSourceBuildInputRole,
) -> Result<&RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputError> {
    let mut matches = manifest
        .components()
        .iter()
        .filter(|component| component.roles().contains(&role));
    let component = matches
        .next()
        .ok_or(RuntimeSourceBuildInputError::ProgramLineageMismatch)?;
    if matches.next().is_some() {
        return Err(RuntimeSourceBuildInputError::ProgramLineageMismatch);
    }
    Ok(component)
}

fn parse_canonical<T>(bytes: &[u8], maximum: u64) -> Result<T, RuntimeSourceBuildInputError>
where
    T: for<'de> Deserialize<'de> + Serialize,
{
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    validate_unique_json(bytes)
        .map_err(|()| RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)?;
    if serde_json::to_vec(&value)
        .map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)?
        != bytes
    {
        return Err(RuntimeSourceBuildInputError::InvalidProgramLineage);
    }
    serde_json::from_value(value).map_err(|_| RuntimeSourceBuildInputError::InvalidProgramLineage)
}
