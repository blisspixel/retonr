use rewrite_model::{
    ArtifactSetId, ArtifactSetRelativePath, PackageSource, PackageSourceKind,
    PackageTransformation, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
    RuntimePackageLoadPolicy, RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;
use serde::Deserialize;

use super::error::{RuntimeReconstructionError, RuntimeReconstructionResult};
use crate::json::validate_unique_json;

/// Reviewed runtime-package layout contract version.
pub const RUNTIME_LAYOUT_SCHEMA_VERSION: u32 = 1;
/// Only family admitted by this reconstruction slice.
pub const ADMITTED_RUNTIME_FAMILY: &str = "ollama";

const HARD_LAYOUT_BYTES: usize = 1_048_576;
const DEFAULT_LAYOUT_BYTES: usize = 64 * 1024;
const HARD_MEMBERS: usize = rewrite_model::MAX_ARTIFACT_SET_MEMBERS;
const DEFAULT_MEMBERS: usize = 64;
const HARD_MEMBER_BYTES: u64 = 256 * 1024 * 1024 * 1024;
const DEFAULT_MEMBER_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Fixed ceilings applied before runtime layout bytes are allocated or hashed.
///
/// Explicit values may only lower the defaults. Values above a default are rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLayoutLimits {
    /// Maximum reviewed layout JSON bytes.
    pub layout_bytes: usize,
    /// Maximum declared members.
    pub maximum_members: usize,
    /// Maximum bytes admitted for any one member.
    pub maximum_member_bytes: u64,
}

impl Default for RuntimeLayoutLimits {
    fn default() -> Self {
        Self {
            layout_bytes: DEFAULT_LAYOUT_BYTES,
            maximum_members: DEFAULT_MEMBERS,
            maximum_member_bytes: DEFAULT_MEMBER_BYTES,
        }
    }
}

impl RuntimeLayoutLimits {
    /// Validates that every configured ceiling is nonzero and no greater than
    /// the crate's fixed defaults.
    ///
    /// Call this before using a ceiling to allocate or read caller-controlled
    /// input outside this crate.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeReconstructionError::LimitExceeded`] when any ceiling is
    /// zero or exceeds its fixed default.
    pub fn validate(self) -> RuntimeReconstructionResult<Self> {
        if self.layout_bytes == 0
            || self.layout_bytes > DEFAULT_LAYOUT_BYTES
            || self.layout_bytes > HARD_LAYOUT_BYTES
            || self.maximum_members == 0
            || self.maximum_members > DEFAULT_MEMBERS
            || self.maximum_members > HARD_MEMBERS
            || self.maximum_member_bytes == 0
            || self.maximum_member_bytes > DEFAULT_MEMBER_BYTES
            || self.maximum_member_bytes > HARD_MEMBER_BYTES
        {
            return Err(RuntimeReconstructionError::LimitExceeded);
        }
        Ok(self)
    }
}

/// One declared runtime-package member and its exact byte identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageLayoutMember {
    relative_path: ArtifactSetRelativePath,
    roles: Vec<RuntimePackageMemberRole>,
    load_policy: RuntimePackageLoadPolicy,
    byte_size: u64,
    digest: Digest,
}

impl RuntimePackageLayoutMember {
    /// Returns the portable member path.
    #[must_use]
    pub fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns declared roles in layout order.
    #[must_use]
    pub fn roles(&self) -> &[RuntimePackageMemberRole] {
        &self.roles
    }

    /// Returns the declared static load policy.
    #[must_use]
    pub const fn load_policy(&self) -> RuntimePackageLoadPolicy {
        self.load_policy
    }

    /// Returns the exact declared byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the exact declared SHA-256 digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }
}

/// Reviewed layout for one Ollama Linux runtime package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageLayout {
    runtime_family: String,
    reported_version: String,
    build_revision: String,
    target: RuntimeTarget,
    source: PackageSource,
    transformation: PackageTransformation,
    members: Vec<RuntimePackageLayoutMember>,
}

impl RuntimePackageLayout {
    /// Parses one reviewed layout and revalidates the first admitted contract.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeReconstructionError`] for oversized, malformed, duplicated,
    /// unsupported, or noncanonical layouts.
    pub fn parse(bytes: &[u8], limits: RuntimeLayoutLimits) -> RuntimeReconstructionResult<Self> {
        let limits = limits.validate()?;
        if bytes.len() > limits.layout_bytes {
            return Err(RuntimeReconstructionError::LayoutTooLarge);
        }
        validate_unique_json(bytes).map_err(|()| RuntimeReconstructionError::InvalidLayout)?;
        let wire: LayoutWire =
            serde_json::from_slice(bytes).map_err(|_| RuntimeReconstructionError::InvalidLayout)?;
        from_wire(wire, limits)
    }

    /// Returns the admitted runtime family.
    #[must_use]
    pub fn runtime_family(&self) -> &str {
        &self.runtime_family
    }

    /// Returns the exact reported version.
    #[must_use]
    pub fn reported_version(&self) -> &str {
        &self.reported_version
    }

    /// Returns the exact source revision bound into the package.
    #[must_use]
    pub fn build_revision(&self) -> &str {
        &self.build_revision
    }

    /// Returns the admitted native target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the exact package source.
    #[must_use]
    pub const fn source(&self) -> &PackageSource {
        &self.source
    }

    /// Returns the transformation disposition.
    #[must_use]
    pub const fn transformation(&self) -> &PackageTransformation {
        &self.transformation
    }

    /// Returns members in canonical path order.
    #[must_use]
    pub fn members(&self) -> &[RuntimePackageLayoutMember] {
        &self.members
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayoutWire {
    schema_version: u32,
    runtime_family: String,
    reported_version: String,
    build_revision: String,
    target: TargetWire,
    source: SourceWire,
    transformation: TransformationWire,
    members: Vec<MemberWire>,
    observed_tree: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetWire {
    operating_system: RuntimeOperatingSystem,
    architecture: RuntimeArchitecture,
    abi: RuntimeAbi,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceWire {
    schema_version: u32,
    kind: PackageSourceKind,
    locator: String,
    revision: String,
    provenance_digest: Digest,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TransformationWire {
    Untransformed {
        evidence_digest: Digest,
    },
    Transformed {
        source_artifact_set_id: ArtifactSetId,
        tool_evidence_digest: Digest,
        parameters_digest: Digest,
        log_digest: Digest,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire {
    relative_path: String,
    roles: Vec<RuntimePackageMemberRole>,
    load_policy: RuntimePackageLoadPolicy,
    byte_size: u64,
    digest: Digest,
}

fn from_wire(
    wire: LayoutWire,
    limits: RuntimeLayoutLimits,
) -> RuntimeReconstructionResult<RuntimePackageLayout> {
    if wire.schema_version != RUNTIME_LAYOUT_SCHEMA_VERSION {
        return Err(RuntimeReconstructionError::UnsupportedLayout);
    }
    if wire.runtime_family != ADMITTED_RUNTIME_FAMILY {
        return Err(RuntimeReconstructionError::UnsupportedLayout);
    }
    if !valid_identity_text(&wire.reported_version) || !valid_identity_text(&wire.build_revision) {
        return Err(RuntimeReconstructionError::InvalidLayout);
    }
    let target = RuntimeTarget::new(
        wire.target.operating_system,
        wire.target.architecture,
        wire.target.abi,
    )
    .map_err(|_| RuntimeReconstructionError::UnsupportedTarget)?;
    if !matches!(
        (
            target.operating_system(),
            target.architecture(),
            target.abi()
        ),
        (
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc
        )
    ) {
        return Err(RuntimeReconstructionError::UnsupportedTarget);
    }
    if wire.members.is_empty() || wire.members.len() > limits.maximum_members {
        return Err(RuntimeReconstructionError::LimitExceeded);
    }
    let members = wire
        .members
        .into_iter()
        .map(|member| layout_member(member, limits.maximum_member_bytes))
        .collect::<RuntimeReconstructionResult<Vec<_>>>()?;
    require_canonical_member_order(&members)?;
    require_observed_tree(&members, &wire.observed_tree)?;
    require_required_roles(
        &members,
        matches!(&wire.transformation, TransformationWire::Transformed { .. }),
    )?;
    if wire.source.schema_version != rewrite_model::PACKAGE_SOURCE_SCHEMA_VERSION {
        return Err(RuntimeReconstructionError::UnsupportedLayout);
    }
    let source = PackageSource::new(
        wire.source.kind,
        wire.source.locator,
        wire.source.revision,
        wire.source.provenance_digest,
    )
    .map_err(|_| RuntimeReconstructionError::InvalidLayout)?;
    let transformation = match wire.transformation {
        TransformationWire::Untransformed { evidence_digest } => {
            PackageTransformation::Untransformed { evidence_digest }
        }
        TransformationWire::Transformed {
            source_artifact_set_id,
            tool_evidence_digest,
            parameters_digest,
            log_digest,
        } => PackageTransformation::Transformed {
            source_artifact_set_id,
            tool_evidence_digest,
            parameters_digest,
            log_digest,
        },
    };
    Ok(RuntimePackageLayout {
        runtime_family: wire.runtime_family,
        reported_version: wire.reported_version,
        build_revision: wire.build_revision,
        target,
        source,
        transformation,
        members,
    })
}

fn layout_member(
    wire: MemberWire,
    maximum_member_bytes: u64,
) -> RuntimeReconstructionResult<RuntimePackageLayoutMember> {
    if wire.byte_size == 0 || wire.byte_size > maximum_member_bytes {
        return Err(RuntimeReconstructionError::LimitExceeded);
    }
    validate_declared_roles(&wire.roles, wire.load_policy)?;
    let relative_path = ArtifactSetRelativePath::new(wire.relative_path)
        .map_err(|_| RuntimeReconstructionError::InvalidMember)?;
    Ok(RuntimePackageLayoutMember {
        relative_path,
        roles: wire.roles,
        load_policy: wire.load_policy,
        byte_size: wire.byte_size,
        digest: wire.digest,
    })
}

fn validate_declared_roles(
    roles: &[RuntimePackageMemberRole],
    policy: RuntimePackageLoadPolicy,
) -> RuntimeReconstructionResult<()> {
    if roles.is_empty()
        || roles.len() > 8
        || roles
            .windows(2)
            .any(|pair| role_rank(pair[0]) >= role_rank(pair[1]))
    {
        return Err(RuntimeReconstructionError::InvalidMember);
    }
    let entrypoint = roles.contains(&RuntimePackageMemberRole::Entrypoint);
    let dependency = roles.contains(&RuntimePackageMemberRole::NativeDependency);
    let worker = roles.contains(&RuntimePackageMemberRole::WorkerExecutable);
    let utility = roles.contains(&RuntimePackageMemberRole::UtilityExecutable);
    let has_other = roles.iter().any(|role| {
        !matches!(
            role,
            RuntimePackageMemberRole::Entrypoint | RuntimePackageMemberRole::NativeDependency
        )
    });
    let policy_valid = if entrypoint {
        roles.len() == 1 && policy == RuntimePackageLoadPolicy::RequiredAtReady
    } else if dependency {
        !has_other && policy != RuntimePackageLoadPolicy::MustNotBeCodeLoaded
    } else if worker {
        roles.len() == 1 && policy == RuntimePackageLoadPolicy::BackendConditional
    } else if utility {
        roles.len() == 1 && policy == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
    } else {
        policy == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
    };
    if policy_valid {
        Ok(())
    } else {
        Err(RuntimeReconstructionError::InvalidMember)
    }
}

const fn role_rank(role: RuntimePackageMemberRole) -> u8 {
    match role {
        RuntimePackageMemberRole::Entrypoint => 0,
        RuntimePackageMemberRole::NativeDependency => 1,
        RuntimePackageMemberRole::HelperExecutable => 2,
        RuntimePackageMemberRole::RuntimeResource => 3,
        RuntimePackageMemberRole::DefaultConfiguration => 4,
        RuntimePackageMemberRole::BuildConfiguration => 5,
        RuntimePackageMemberRole::LicenseText => 6,
        RuntimePackageMemberRole::ProvenanceRecord => 7,
        RuntimePackageMemberRole::TransformationRecord => 8,
        RuntimePackageMemberRole::WorkerExecutable => 9,
        RuntimePackageMemberRole::UtilityExecutable => 10,
    }
}

fn require_canonical_member_order(
    members: &[RuntimePackageLayoutMember],
) -> RuntimeReconstructionResult<()> {
    let mut prior: Option<&[u8]> = None;
    for member in members {
        let path = member.relative_path.as_str().as_bytes();
        if prior.is_some_and(|previous| previous >= path) {
            return Err(RuntimeReconstructionError::InvalidLayout);
        }
        prior = Some(path);
    }
    Ok(())
}

fn require_observed_tree(
    members: &[RuntimePackageLayoutMember],
    observed_tree: &[String],
) -> RuntimeReconstructionResult<()> {
    if observed_tree.len() != members.len() {
        return Err(RuntimeReconstructionError::ObservedTreeMismatch);
    }
    if observed_tree
        .iter()
        .zip(members)
        .all(|(path, member)| path == member.relative_path.as_str())
    {
        Ok(())
    } else {
        Err(RuntimeReconstructionError::ObservedTreeMismatch)
    }
}

fn require_required_roles(
    members: &[RuntimePackageLayoutMember],
    transformed: bool,
) -> RuntimeReconstructionResult<()> {
    let mut entrypoints = 0usize;
    let mut helpers = 0usize;
    let mut workers = 0usize;
    let mut license = false;
    let mut provenance = false;
    let mut transformation_records = 0usize;
    for member in members {
        entrypoints += usize::from(member.roles.contains(&RuntimePackageMemberRole::Entrypoint));
        helpers += usize::from(
            member
                .roles
                .contains(&RuntimePackageMemberRole::HelperExecutable),
        );
        workers += usize::from(
            member
                .roles
                .contains(&RuntimePackageMemberRole::WorkerExecutable),
        );
        license |= member
            .roles
            .contains(&RuntimePackageMemberRole::LicenseText);
        provenance |= member
            .roles
            .contains(&RuntimePackageMemberRole::ProvenanceRecord);
        transformation_records += usize::from(
            member
                .roles
                .contains(&RuntimePackageMemberRole::TransformationRecord),
        );
    }
    if entrypoints == 1
        && helpers == 1
        && workers == 1
        && license
        && provenance
        && transformation_records == usize::from(transformed)
    {
        Ok(())
    } else {
        Err(RuntimeReconstructionError::InvalidLayout)
    }
}

fn valid_identity_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}
