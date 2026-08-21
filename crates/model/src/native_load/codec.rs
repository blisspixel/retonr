use serde::Deserialize;

use crate::runtime_identity::{append_digest, append_text, append_u32};
use crate::{ArtifactId, ArtifactSetRelativePath, RuntimePackageManifest};

use super::{
    MAX_NATIVE_LOAD_OBSERVATION_JSON_BYTES, NativeLoadEvidenceClass, NativeLoadObservation,
    NativeLoadObservationError, NativeLoadObservationInput, NativeLoadOrigin,
    NativeLoadVisibilityScope, NativeLoadedComponent, NativeMappingClass,
};

impl NativeLoadObservation {
    /// Parses bounded JSON and revalidates all package relationships.
    ///
    /// # Errors
    ///
    /// Returns [`NativeLoadObservationError`] for malformed or inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        package: &RuntimePackageManifest,
    ) -> Result<Self, NativeLoadObservationError> {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum OriginWire {
            PackagedMember { relative_path: String },
            ExternalPlatformComponent,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ComponentWire {
            artifact_id: ArtifactId,
            byte_size: u64,
            origin: OriginWire,
            mapping_class: NativeMappingClass,
            object_evidence_digest: rewrite_types::Digest,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            runtime_package_manifest_id: crate::RuntimePackageManifestId,
            evidence_class: NativeLoadEvidenceClass,
            visibility_scope: NativeLoadVisibilityScope,
            process_evidence_digest: rewrite_types::Digest,
            observation_contract_id: String,
            observation_contract_schema_version: u32,
            components: Vec<ComponentWire>,
        }

        if bytes.len() > MAX_NATIVE_LOAD_OBSERVATION_JSON_BYTES {
            return Err(NativeLoadObservationError::EncodedObservationTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| NativeLoadObservationError::InvalidEncoding)?;
        if wire.runtime_package_manifest_id != package.runtime_package_manifest_id() {
            return Err(NativeLoadObservationError::PackagedComponentMismatch);
        }
        let components = wire
            .components
            .into_iter()
            .map(|component| {
                let origin = match component.origin {
                    OriginWire::PackagedMember { relative_path } => {
                        NativeLoadOrigin::PackagedMember {
                            relative_path: ArtifactSetRelativePath::new(relative_path)
                                .map_err(|_| NativeLoadObservationError::InvalidMemberPath)?,
                        }
                    }
                    OriginWire::ExternalPlatformComponent => {
                        NativeLoadOrigin::ExternalPlatformComponent
                    }
                };
                Ok(NativeLoadedComponent::new(
                    component.artifact_id,
                    component.byte_size,
                    origin,
                    component.mapping_class,
                    component.object_evidence_digest,
                ))
            })
            .collect::<Result<Vec<_>, NativeLoadObservationError>>()?;
        Self::from_wire(
            wire.schema_version,
            package,
            NativeLoadObservationInput {
                evidence_class: wire.evidence_class,
                visibility_scope: wire.visibility_scope,
                process_evidence_digest: wire.process_evidence_digest,
                observation_contract_id: wire.observation_contract_id,
                observation_contract_schema_version: wire.observation_contract_schema_version,
                components,
            },
        )
    }

    pub(super) fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:native-load-observation:v1\0");
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.runtime_package_manifest_id.digest());
        output.push(evidence_class_byte(self.evidence_class));
        output.push(visibility_scope_byte(self.visibility_scope));
        append_digest(&mut output, &self.process_evidence_digest);
        append_text(&mut output, &self.observation_contract_id);
        append_u32(&mut output, self.observation_contract_schema_version);
        append_u32(
            &mut output,
            u32::try_from(self.components.len()).expect("validated component count fits u32"),
        );
        for component in &self.components {
            append_component(&mut output, component);
        }
        output
    }
}

fn append_component(output: &mut Vec<u8>, component: &NativeLoadedComponent) {
    match component.origin() {
        NativeLoadOrigin::PackagedMember { relative_path } => {
            output.push(0);
            append_text(output, relative_path.as_str());
        }
        NativeLoadOrigin::ExternalPlatformComponent => output.push(1),
    }
    append_digest(output, component.artifact_id().digest());
    output.extend_from_slice(&component.byte_size().to_be_bytes());
    output.push(mapping_class_byte(component.mapping_class()));
    append_digest(output, component.object_evidence_digest());
}

const fn evidence_class_byte(value: NativeLoadEvidenceClass) -> u8 {
    match value {
        NativeLoadEvidenceClass::LinuxProcMapFiles => 1,
    }
}

const fn visibility_scope_byte(value: NativeLoadVisibilityScope) -> u8 {
    match value {
        NativeLoadVisibilityScope::FileBackedExecutableMappings => 0,
        NativeLoadVisibilityScope::FileBackedMappings => 1,
    }
}

pub(super) const fn mapping_class_byte(value: NativeMappingClass) -> u8 {
    match value {
        NativeMappingClass::ExecutableImage => 0,
        NativeMappingClass::ExecutableMapped => 1,
        NativeMappingClass::DataMapped => 2,
    }
}
