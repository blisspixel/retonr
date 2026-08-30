use rewrite_model::{ArtifactId, RuntimePackageManifestId};
use rewrite_types::Digest;
use serde::Serialize;

use super::ManagedGenerationWorkerProfile;

/// Schema version for redacted managed generation-worker observations.
pub const MANAGED_GENERATION_WORKER_OBSERVATION_SCHEMA_VERSION: u32 = 1;

/// Redacted point-in-time evidence for one distinct generation worker.
///
/// This record is inert. It does not prove model use, handler execution, native
/// closure, model mapping, effective placement, candidate provenance, or quality.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagedGenerationWorkerEvidence {
    schema_version: u32,
    profile: ManagedGenerationWorkerProfile,
    runtime_package_manifest_id: RuntimePackageManifestId,
    worker_artifact_id: ArtifactId,
    model_artifact_id: ArtifactId,
    process_instance_digest: Digest,
    parent_chain_digest: Digest,
    namespace_and_privilege_digest: Digest,
    portable_configuration_digest: Digest,
    command_digest: Digest,
    evidence_digest: Digest,
}

impl ManagedGenerationWorkerEvidence {
    #[expect(
        clippy::too_many_arguments,
        reason = "the redacted record binds every distinct worker observation"
    )]
    #[cfg(any(target_os = "linux", feature = "test-support"))]
    pub(crate) fn new(
        profile: ManagedGenerationWorkerProfile,
        runtime_package_manifest_id: RuntimePackageManifestId,
        worker_artifact_id: ArtifactId,
        model_artifact_id: ArtifactId,
        process_instance_digest: Digest,
        parent_chain_digest: Digest,
        namespace_and_privilege_digest: Digest,
        portable_configuration_digest: Digest,
        command_digest: Digest,
    ) -> Self {
        let mut material = Vec::with_capacity(640);
        material.extend_from_slice(b"retonr:managed-generation-worker:v1\0");
        material.extend_from_slice(profile.command_contract_id().as_bytes());
        material.push(0);
        material.extend_from_slice(runtime_package_manifest_id.digest().as_str().as_bytes());
        material.extend_from_slice(worker_artifact_id.digest().as_str().as_bytes());
        material.extend_from_slice(model_artifact_id.digest().as_str().as_bytes());
        for digest in [
            &process_instance_digest,
            &parent_chain_digest,
            &namespace_and_privilege_digest,
            &portable_configuration_digest,
            &command_digest,
        ] {
            material.extend_from_slice(digest.as_str().as_bytes());
        }
        let evidence_digest = Digest::sha256(&material);
        Self {
            schema_version: MANAGED_GENERATION_WORKER_OBSERVATION_SCHEMA_VERSION,
            profile,
            runtime_package_manifest_id,
            worker_artifact_id,
            model_artifact_id,
            process_instance_digest,
            parent_chain_digest,
            namespace_and_privilege_digest,
            portable_configuration_digest,
            command_digest,
            evidence_digest,
        }
    }

    /// Constructs inert worker evidence for downstream offline tests.
    ///
    /// This helper is absent unless the `test-support` feature is enabled and
    /// does not construct or imitate a retained worker lease.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn for_test(
        profile: ManagedGenerationWorkerProfile,
        runtime_package_manifest_id: RuntimePackageManifestId,
        worker_artifact_id: ArtifactId,
        model_artifact_id: ArtifactId,
        tag: &str,
    ) -> Self {
        Self::new(
            profile,
            runtime_package_manifest_id,
            worker_artifact_id,
            model_artifact_id,
            test_digest(tag, "process"),
            test_digest(tag, "parent"),
            test_digest(tag, "namespace"),
            test_digest(tag, "configuration"),
            test_digest(tag, "command"),
        )
    }

    /// Returns the worker observation schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the closed worker profile.
    #[must_use]
    pub const fn profile(&self) -> ManagedGenerationWorkerProfile {
        self.profile
    }

    /// Returns the exact runtime-package identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact worker byte identity.
    #[must_use]
    pub const fn worker_artifact_id(&self) -> &ArtifactId {
        &self.worker_artifact_id
    }

    /// Returns the exact model-weight byte identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the portable normalized worker output and execution configuration.
    #[must_use]
    pub const fn portable_configuration_digest(&self) -> &Digest {
        &self.portable_configuration_digest
    }

    /// Returns the digest of the complete redacted worker observation.
    #[must_use]
    pub const fn evidence_digest(&self) -> &Digest {
        &self.evidence_digest
    }
}

/// Redacted native-code relationship evidence for one worker.
///
/// This is inert relationship evidence over file-backed executable mappings only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagedGenerationWorkerNativeLoadEvidence {
    component_count: u32,
    portable_closure_digest: Digest,
    observation_digest: Digest,
}

impl ManagedGenerationWorkerNativeLoadEvidence {
    #[cfg(any(target_os = "linux", feature = "test-support"))]
    pub(crate) const fn new(
        component_count: u32,
        portable_closure_digest: Digest,
        observation_digest: Digest,
    ) -> Self {
        Self {
            component_count,
            portable_closure_digest,
            observation_digest,
        }
    }

    /// Constructs inert native-load evidence for downstream offline tests.
    ///
    /// This helper is absent unless the `test-support` feature is enabled.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn for_test(tag: &str) -> Self {
        Self::new(
            1,
            test_digest(tag, "closure"),
            Digest::sha256(tag.as_bytes()),
        )
    }

    /// Returns the number of distinct executable file objects observed.
    #[must_use]
    pub const fn component_count(&self) -> u32 {
        self.component_count
    }

    /// Returns the portable digest of the actual canonical native closure.
    ///
    /// This digest binds the reviewed worker profile and each observed
    /// component's artifact, byte size, package class, and required flag. It
    /// deliberately excludes process, device, inode, and mapping identities.
    #[must_use]
    pub const fn portable_closure_digest(&self) -> &Digest {
        &self.portable_closure_digest
    }

    /// Returns the digest of the redacted process-specific native observation.
    #[must_use]
    pub const fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }
}

/// Redacted retained-file relationship evidence for one expected GGUF mapping.
///
/// This establishes bounded file mapping only. It never claims model use,
/// resident-page identity, page immutability, or handler execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ManagedGenerationWorkerModelMappingEvidence {
    model_artifact_id: ArtifactId,
    mapping_region_count: u32,
    observation_digest: Digest,
}

impl ManagedGenerationWorkerModelMappingEvidence {
    #[cfg(any(target_os = "linux", feature = "test-support"))]
    pub(crate) const fn new(
        model_artifact_id: ArtifactId,
        mapping_region_count: u32,
        observation_digest: Digest,
    ) -> Self {
        Self {
            model_artifact_id,
            mapping_region_count,
            observation_digest,
        }
    }

    /// Constructs inert model-mapping evidence for downstream offline tests.
    ///
    /// This helper is absent unless the `test-support` feature is enabled.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn for_test(model_artifact_id: ArtifactId, tag: &str) -> Self {
        Self::new(model_artifact_id, 1, Digest::sha256(tag.as_bytes()))
    }

    /// Returns the exact mapped model-weight identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the number of mapping regions backed by the one retained object.
    #[must_use]
    pub const fn mapping_region_count(&self) -> u32 {
        self.mapping_region_count
    }

    /// Returns the digest of the redacted stable mapping relationship.
    #[must_use]
    pub const fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }
}

#[cfg(feature = "test-support")]
fn test_digest(tag: &str, suffix: &str) -> Digest {
    Digest::sha256(format!("{tag}:{suffix}").as_bytes())
}
