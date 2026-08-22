use serde::Deserialize;

use rewrite_types::Digest;

use crate::runtime_identity::{append_digest, append_text, append_u32};
use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetRelativePath, PackageSource, PackageTransformation,
    RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};

use super::{
    MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageManifestError, RuntimePackageMember, RuntimePackageMemberRole,
};

impl RuntimePackageManifest {
    /// Parses bounded JSON and revalidates all byte and semantic relationships.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimePackageManifestError`] for malformed or inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        artifact_set: &ArtifactSetManifest,
    ) -> Result<Self, RuntimePackageManifestError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct TargetWire {
            operating_system: RuntimeOperatingSystem,
            architecture: RuntimeArchitecture,
            abi: RuntimeAbi,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct MemberWire {
            artifact_id: ArtifactId,
            byte_size: u64,
            relative_path: String,
            roles: Vec<RuntimePackageMemberRole>,
            load_policy: RuntimePackageLoadPolicy,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            artifact_set_id: crate::ArtifactSetId,
            runtime_family: String,
            reported_version: String,
            build_revision: Option<String>,
            target: TargetWire,
            source: serde_json::Value,
            transformation: PackageTransformation,
            members: Vec<MemberWire>,
        }

        if bytes.len() > MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES {
            return Err(RuntimePackageManifestError::EncodedManifestTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| RuntimePackageManifestError::InvalidEncoding)?;
        if wire.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(RuntimePackageManifestError::ArtifactSetMismatch);
        }
        let target = RuntimeTarget::new(
            wire.target.operating_system,
            wire.target.architecture,
            wire.target.abi,
        )
        .map_err(|_| RuntimePackageManifestError::InvalidTarget)?;
        let source_bytes = serde_json::to_vec(&wire.source)
            .map_err(|_| RuntimePackageManifestError::InvalidSource)?;
        let source = PackageSource::from_json_bytes(&source_bytes)
            .map_err(|_| RuntimePackageManifestError::InvalidSource)?;
        let members = wire
            .members
            .into_iter()
            .map(|member| {
                Ok(RuntimePackageMember::new(
                    member.artifact_id,
                    member.byte_size,
                    ArtifactSetRelativePath::new(member.relative_path)
                        .map_err(|_| RuntimePackageManifestError::InvalidMemberPath)?,
                    member.roles,
                    member.load_policy,
                ))
            })
            .collect::<Result<Vec<_>, RuntimePackageManifestError>>()?;
        RuntimePackageManifest::from_wire(
            wire.schema_version,
            artifact_set,
            wire.runtime_family,
            wire.reported_version,
            wire.build_revision,
            target,
            source,
            wire.transformation,
            members,
        )
    }

    pub(super) fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:runtime-package-manifest:v1\0");
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.artifact_set_id.digest());
        append_text(&mut output, &self.runtime_family);
        append_text(&mut output, &self.reported_version);
        match self.build_revision.as_deref() {
            Some(value) => {
                output.push(1);
                append_text(&mut output, value);
            }
            None => output.push(0),
        }
        output.push(target_os_byte(self.target.operating_system()));
        output.push(target_arch_byte(self.target.architecture()));
        output.push(target_abi_byte(self.target.abi()));
        self.source.append_identity(&mut output);
        self.transformation.append_canonical(&mut output);
        append_u32(
            &mut output,
            u32::try_from(self.members.len()).expect("validated member count fits u32"),
        );
        for member in &self.members {
            append_member(&mut output, member);
        }
        output
    }
}

pub(super) fn subset_digest<F>(
    domain: &[u8],
    manifest: &RuntimePackageManifest,
    include: F,
) -> Digest
where
    F: Fn(RuntimePackageMemberRole) -> bool,
{
    let selected = manifest
        .members
        .iter()
        .filter(|member| member.roles.iter().copied().any(&include))
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    output.extend_from_slice(domain);
    append_u32(&mut output, 1);
    append_digest(&mut output, manifest.artifact_set_id.digest());
    append_u32(
        &mut output,
        u32::try_from(selected.len()).expect("selected count fits u32"),
    );
    for member in selected {
        append_member(&mut output, member);
    }
    Digest::sha256(&output)
}

fn append_member(output: &mut Vec<u8>, member: &RuntimePackageMember) {
    append_text(output, member.relative_path.as_str());
    append_digest(output, member.artifact_id.digest());
    output.extend_from_slice(&member.byte_size.to_be_bytes());
    append_u32(
        output,
        u32::try_from(member.roles.len()).expect("validated role count fits u32"),
    );
    output.extend(member.roles.iter().copied().map(role_byte));
    output.push(load_policy_byte(member.load_policy));
}

pub(super) const fn role_byte(value: RuntimePackageMemberRole) -> u8 {
    match value {
        RuntimePackageMemberRole::Entrypoint => 0,
        RuntimePackageMemberRole::NativeDependency => 1,
        RuntimePackageMemberRole::HelperExecutable => 2,
        RuntimePackageMemberRole::RuntimeResource => 3,
        RuntimePackageMemberRole::DefaultConfiguration => 4,
        RuntimePackageMemberRole::BuildConfiguration => 5,
        RuntimePackageMemberRole::LicenseText => 6,
        RuntimePackageMemberRole::ProvenanceRecord => 7,
        RuntimePackageMemberRole::TransformationRecord => 8,
        // Appended to preserve every existing version 1 package identity.
        RuntimePackageMemberRole::WorkerExecutable => 9,
        RuntimePackageMemberRole::UtilityExecutable => 10,
    }
}

const fn load_policy_byte(value: RuntimePackageLoadPolicy) -> u8 {
    match value {
        RuntimePackageLoadPolicy::RequiredAtReady => 0,
        RuntimePackageLoadPolicy::BackendConditional => 1,
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded => 2,
    }
}

const fn target_os_byte(value: RuntimeOperatingSystem) -> u8 {
    match value {
        RuntimeOperatingSystem::Windows => 0,
        RuntimeOperatingSystem::MacOs => 1,
        RuntimeOperatingSystem::Linux => 2,
    }
}

const fn target_arch_byte(value: RuntimeArchitecture) -> u8 {
    match value {
        RuntimeArchitecture::X86_64 => 0,
        RuntimeArchitecture::Aarch64 => 1,
    }
}

const fn target_abi_byte(value: RuntimeAbi) -> u8 {
    match value {
        RuntimeAbi::WindowsMsvc => 0,
        RuntimeAbi::WindowsGnu => 1,
        RuntimeAbi::LinuxGnuLibc => 2,
        RuntimeAbi::LinuxMusl => 3,
        RuntimeAbi::Darwin => 4,
    }
}
