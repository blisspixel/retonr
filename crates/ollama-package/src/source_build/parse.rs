use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;
use serde::Deserialize;

use crate::json::validate_unique_json;

use super::{
    RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION, RuntimeSourceBuildAcceleratorPolicy,
    RuntimeSourceBuildEnvironmentVariable, RuntimeSourceBuildInputComponent,
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputLimits, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildInputRole, RuntimeSourceBuildNetworkPolicy, RuntimeSourceBuildPolicy,
    sequence_digest,
};

const MAX_NAME_BYTES: usize = 128;
const MAX_REVISION_BYTES: usize = 256;
const MAX_LOCATOR_BYTES: usize = 2_048;
const MAX_ENVIRONMENT_ENTRIES: usize = 128;
const MAX_BUILD_ARGUMENTS: usize = 256;
const MAX_ENVIRONMENT_NAME_BYTES: usize = 64;
const MAX_POLICY_VALUE_BYTES: usize = 1_024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestWire {
    schema_version: u32,
    artifact_set_id: ArtifactSetId,
    policy: PolicyWire,
    components: Vec<ComponentWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyWire {
    target: RuntimeTarget,
    network_access: RuntimeSourceBuildNetworkPolicy,
    accelerator: RuntimeSourceBuildAcceleratorPolicy,
    cpu_feature_policy: String,
    locale: String,
    timezone: String,
    source_date_epoch: u64,
    environment: Vec<EnvironmentWire>,
    build_arguments: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentWire {
    name: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentWire {
    relative_path: String,
    byte_size: u64,
    digest: Digest,
    name: String,
    revision: String,
    source_locator: String,
    roles: Vec<RuntimeSourceBuildInputRole>,
}

pub(super) fn parse_runtime_source_build_input_manifest(
    bytes: &[u8],
    limits: RuntimeSourceBuildInputLimits,
) -> Result<RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputError> {
    let limits = limits.validate()?;
    if bytes.len() > limits.manifest_bytes {
        return Err(RuntimeSourceBuildInputError::LimitExceeded);
    }
    validate_unique_json(bytes).map_err(|()| RuntimeSourceBuildInputError::InvalidEncoding)?;
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| RuntimeSourceBuildInputError::InvalidEncoding)?;
    if serde_json::to_vec(&value).map_err(|_| RuntimeSourceBuildInputError::InvalidEncoding)?
        != bytes
    {
        return Err(RuntimeSourceBuildInputError::NoncanonicalEncoding);
    }
    let wire: ManifestWire =
        serde_json::from_value(value).map_err(|_| RuntimeSourceBuildInputError::InvalidEncoding)?;
    if wire.schema_version != RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION {
        return Err(RuntimeSourceBuildInputError::UnsupportedSchema);
    }
    let policy = parse_policy(wire.policy)?;
    let components = parse_components(wire.components, limits)?;
    validate_closure_roles(&components)?;
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
    .map_err(|_| RuntimeSourceBuildInputError::InvalidArtifactSet)?;
    if artifact_set.artifact_set_id() != wire.artifact_set_id {
        return Err(RuntimeSourceBuildInputError::InvalidArtifactSet);
    }
    let manifest_digest = sequence_digest(b"runtime-source-build/input-manifest/v1", [bytes]);
    Ok(RuntimeSourceBuildInputManifest {
        artifact_set,
        policy,
        components,
        manifest_digest,
    })
}

fn parse_policy(
    wire: PolicyWire,
) -> Result<RuntimeSourceBuildPolicy, RuntimeSourceBuildInputError> {
    if !matches!(
        (
            wire.target.operating_system(),
            wire.target.architecture(),
            wire.target.abi()
        ),
        (
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc
        )
    ) || wire.network_access != RuntimeSourceBuildNetworkPolicy::Denied
        || wire.accelerator != RuntimeSourceBuildAcceleratorPolicy::CpuOnly
        || wire.cpu_feature_policy != "x86-64-v2"
        || !matches!(wire.locale.as_str(), "C" | "C.UTF-8")
        || wire.timezone != "UTC"
        || wire.source_date_epoch == 0
    {
        return Err(RuntimeSourceBuildInputError::UnsupportedPolicy);
    }
    let environment = parse_environment(wire.environment, &wire.locale, wire.source_date_epoch)?;
    let build_arguments = parse_build_arguments(wire.build_arguments)?;
    let environment_digest = sequence_digest(
        b"runtime-source-build/environment/v1",
        environment
            .iter()
            .flat_map(|entry| [entry.name.as_bytes(), entry.value.as_bytes()]),
    );
    let build_arguments_digest = sequence_digest(
        b"runtime-source-build/arguments/v1",
        build_arguments.iter().map(String::as_bytes),
    );
    Ok(RuntimeSourceBuildPolicy {
        target: wire.target,
        network_access: wire.network_access,
        accelerator: wire.accelerator,
        cpu_feature_policy: wire.cpu_feature_policy,
        locale: wire.locale,
        timezone: wire.timezone,
        source_date_epoch: wire.source_date_epoch,
        environment,
        build_arguments,
        environment_digest,
        build_arguments_digest,
    })
}

fn parse_environment(
    wire: Vec<EnvironmentWire>,
    locale: &str,
    source_date_epoch: u64,
) -> Result<Vec<RuntimeSourceBuildEnvironmentVariable>, RuntimeSourceBuildInputError> {
    if wire.is_empty() || wire.len() > MAX_ENVIRONMENT_ENTRIES {
        return Err(RuntimeSourceBuildInputError::UnsupportedPolicy);
    }
    let environment = wire
        .into_iter()
        .map(|entry| {
            if !valid_environment_name(&entry.name)
                || !valid_policy_value(&entry.value)
                || forbidden_proxy_name(&entry.name)
            {
                return Err(RuntimeSourceBuildInputError::UnsupportedPolicy);
            }
            Ok(RuntimeSourceBuildEnvironmentVariable {
                name: entry.name,
                value: entry.value,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if environment
        .windows(2)
        .any(|pair| pair[0].name.as_bytes() >= pair[1].name.as_bytes())
    {
        return Err(RuntimeSourceBuildInputError::UnsupportedPolicy);
    }
    for (name, value) in [
        ("CGO_ENABLED", "1".to_owned()),
        ("GOARCH", "amd64".to_owned()),
        ("GOAMD64", "v2".to_owned()),
        ("GOOS", "linux".to_owned()),
        ("GOPROXY", "off".to_owned()),
        ("GOSUMDB", "off".to_owned()),
        ("LC_ALL", locale.to_owned()),
        ("SOURCE_DATE_EPOCH", source_date_epoch.to_string()),
        ("TZ", "UTC".to_owned()),
    ] {
        if environment_value(&environment, name) != Some(value.as_str()) {
            return Err(RuntimeSourceBuildInputError::UnsupportedPolicy);
        }
    }
    Ok(environment)
}

fn parse_build_arguments(
    arguments: Vec<String>,
) -> Result<Vec<String>, RuntimeSourceBuildInputError> {
    if arguments.is_empty()
        || arguments.len() > MAX_BUILD_ARGUMENTS
        || arguments
            .iter()
            .any(|argument| !valid_policy_value(argument))
    {
        Err(RuntimeSourceBuildInputError::UnsupportedPolicy)
    } else {
        Ok(arguments)
    }
}

fn environment_value<'a>(
    environment: &'a [RuntimeSourceBuildEnvironmentVariable],
    name: &str,
) -> Option<&'a str> {
    environment
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.value.as_str())
}

fn valid_environment_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ENVIRONMENT_NAME_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        && value.as_bytes()[0].is_ascii_uppercase()
}

fn valid_policy_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_POLICY_VALUE_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

fn forbidden_proxy_name(name: &str) -> bool {
    matches!(
        name,
        "ALL_PROXY" | "HTTP_PROXY" | "HTTPS_PROXY" | "NO_PROXY"
    )
}

fn parse_components(
    wire: Vec<ComponentWire>,
    limits: RuntimeSourceBuildInputLimits,
) -> Result<Vec<RuntimeSourceBuildInputComponent>, RuntimeSourceBuildInputError> {
    if wire.is_empty() {
        return Err(RuntimeSourceBuildInputError::InvalidComponent);
    }
    if wire.len() > limits.maximum_components {
        return Err(RuntimeSourceBuildInputError::LimitExceeded);
    }
    let mut total_bytes = 0_u64;
    let components = wire
        .into_iter()
        .map(|component| {
            if component.byte_size == 0 || component.byte_size > limits.maximum_component_bytes {
                return Err(RuntimeSourceBuildInputError::LimitExceeded);
            }
            total_bytes = total_bytes
                .checked_add(component.byte_size)
                .ok_or(RuntimeSourceBuildInputError::LimitExceeded)?;
            if total_bytes > limits.maximum_total_bytes {
                return Err(RuntimeSourceBuildInputError::LimitExceeded);
            }
            if !valid_text(&component.name, MAX_NAME_BYTES)
                || !valid_text(&component.revision, MAX_REVISION_BYTES)
                || !valid_locator(&component.source_locator)
                || component.roles.is_empty()
                || component.roles.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(RuntimeSourceBuildInputError::InvalidComponent);
            }
            Ok(RuntimeSourceBuildInputComponent {
                relative_path: ArtifactSetRelativePath::new(component.relative_path)
                    .map_err(|_| RuntimeSourceBuildInputError::InvalidComponent)?,
                byte_size: component.byte_size,
                digest: component.digest,
                name: component.name,
                revision: component.revision,
                source_locator: component.source_locator,
                roles: component.roles,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.windows(2).any(|pair| {
        pair[0].relative_path.as_str().as_bytes() >= pair[1].relative_path.as_str().as_bytes()
    }) {
        return Err(RuntimeSourceBuildInputError::InvalidComponent);
    }
    Ok(components)
}

fn validate_closure_roles(
    components: &[RuntimeSourceBuildInputComponent],
) -> Result<(), RuntimeSourceBuildInputError> {
    const REQUIRED: [RuntimeSourceBuildInputRole; 20] = [
        RuntimeSourceBuildInputRole::OllamaSource,
        RuntimeSourceBuildInputRole::LlamaCppSource,
        RuntimeSourceBuildInputRole::GoToolchain,
        RuntimeSourceBuildInputRole::GoModule,
        RuntimeSourceBuildInputRole::GoChecksumSet,
        RuntimeSourceBuildInputRole::NativePackage,
        RuntimeSourceBuildInputRole::PosixShell,
        RuntimeSourceBuildInputRole::Cmake,
        RuntimeSourceBuildInputRole::Ninja,
        RuntimeSourceBuildInputRole::CCompiler,
        RuntimeSourceBuildInputRole::CxxCompiler,
        RuntimeSourceBuildInputRole::Assembler,
        RuntimeSourceBuildInputRole::Linker,
        RuntimeSourceBuildInputRole::StandardLibrary,
        RuntimeSourceBuildInputRole::BuildScript,
        RuntimeSourceBuildInputRole::BuildParameters,
        RuntimeSourceBuildInputRole::IsolationHelper,
        RuntimeSourceBuildInputRole::SourceProvenance,
        RuntimeSourceBuildInputRole::ToolEvidence,
        RuntimeSourceBuildInputRole::LicenseEvidence,
    ];
    if REQUIRED
        .iter()
        .any(|role| role_count(components, *role) == 0)
    {
        return Err(RuntimeSourceBuildInputError::IncompleteClosure);
    }
    for singleton in [
        RuntimeSourceBuildInputRole::OllamaSource,
        RuntimeSourceBuildInputRole::LlamaCppSource,
        RuntimeSourceBuildInputRole::GoToolchain,
        RuntimeSourceBuildInputRole::GoChecksumSet,
        RuntimeSourceBuildInputRole::PosixShell,
        RuntimeSourceBuildInputRole::BuildScript,
        RuntimeSourceBuildInputRole::BuildParameters,
        RuntimeSourceBuildInputRole::IsolationHelper,
        RuntimeSourceBuildInputRole::SourceProvenance,
        RuntimeSourceBuildInputRole::ToolEvidence,
    ] {
        if role_count(components, singleton) != 1 {
            return Err(RuntimeSourceBuildInputError::IncompleteClosure);
        }
    }
    validate_retained_program_lineage_roles(components)?;
    Ok(())
}

fn validate_retained_program_lineage_roles(
    components: &[RuntimeSourceBuildInputComponent],
) -> Result<(), RuntimeSourceBuildInputError> {
    const LINEAGE_ROLES: [RuntimeSourceBuildInputRole; 22] = [
        RuntimeSourceBuildInputRole::RetainedProgramLineage,
        RuntimeSourceBuildInputRole::CanonicalBuildRecipe,
        RuntimeSourceBuildInputRole::RetonrRepositorySource,
        RuntimeSourceBuildInputRole::CargoLockfile,
        RuntimeSourceBuildInputRole::CargoVendorSource,
        RuntimeSourceBuildInputRole::CargoRawCrateSource,
        RuntimeSourceBuildInputRole::RustChannelManifest,
        RuntimeSourceBuildInputRole::RustChannelManifestChecksum,
        RuntimeSourceBuildInputRole::RustChannelManifestSignature,
        RuntimeSourceBuildInputRole::RustReleaseTrustRoot,
        RuntimeSourceBuildInputRole::RustCargoDistribution,
        RuntimeSourceBuildInputRole::RustHostStandardLibraryDistribution,
        RuntimeSourceBuildInputRole::RustTargetStandardLibraryDistribution,
        RuntimeSourceBuildInputRole::RustCompilerDistribution,
        RuntimeSourceBuildInputRole::SourceArchivePreparationTool,
        RuntimeSourceBuildInputRole::SourceManifestPreparationTool,
        RuntimeSourceBuildInputRole::AlpineMinirootfs,
        RuntimeSourceBuildInputRole::AlpineMinirootfsChecksum,
        RuntimeSourceBuildInputRole::AlpineMinirootfsSignature,
        RuntimeSourceBuildInputRole::AlpineReleaseTrustRoot,
        RuntimeSourceBuildInputRole::AlpineLibgccPackage,
        RuntimeSourceBuildInputRole::AlpineBusyboxStaticPackage,
    ];
    let present = LINEAGE_ROLES
        .iter()
        .filter(|role| role_count(components, **role) != 0)
        .count();
    if present != 0
        && (present != LINEAGE_ROLES.len()
            || LINEAGE_ROLES
                .iter()
                .any(|role| role_count(components, *role) != 1))
    {
        return Err(RuntimeSourceBuildInputError::IncompleteClosure);
    }
    Ok(())
}

fn role_count(
    components: &[RuntimeSourceBuildInputComponent],
    role: RuntimeSourceBuildInputRole,
) -> usize {
    components
        .iter()
        .filter(|component| component.roles.contains(&role))
        .count()
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.is_ascii()
        && value.trim() == value
        && value.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

fn valid_locator(value: &str) -> bool {
    value.len() <= MAX_LOCATOR_BYTES
        && valid_text(value, MAX_LOCATOR_BYTES)
        && ["https://", "oci://", "pkg:", "urn:"]
            .iter()
            .any(|prefix| value.starts_with(prefix))
}
