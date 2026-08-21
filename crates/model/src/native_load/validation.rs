use std::collections::BTreeSet;

use crate::{RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest};

use super::{
    MAX_NATIVE_LOAD_CANONICAL_BYTES, MAX_NATIVE_LOAD_COMPONENTS, MAX_OBSERVATION_CONTRACT_BYTES,
    NATIVE_LOAD_OBSERVATION_SCHEMA_VERSION, NativeLoadEvidenceClass, NativeLoadObservation,
    NativeLoadObservationError, NativeLoadObservationInput, NativeLoadOrigin,
    NativeLoadVisibilityScope, NativeMappingClass,
};

impl NativeLoadObservation {
    pub(super) fn from_wire(
        schema_version: u32,
        package: &RuntimePackageManifest,
        input: NativeLoadObservationInput,
    ) -> Result<Self, NativeLoadObservationError> {
        if schema_version != NATIVE_LOAD_OBSERVATION_SCHEMA_VERSION {
            return Err(NativeLoadObservationError::UnsupportedSchema(
                schema_version,
            ));
        }
        if !valid_machine_id(&input.observation_contract_id)
            || input.observation_contract_schema_version == 0
        {
            return Err(NativeLoadObservationError::InvalidMetadata);
        }
        let evidence_matches_target = matches!(
            (package.target().operating_system(), input.evidence_class),
            (
                RuntimeOperatingSystem::Linux,
                NativeLoadEvidenceClass::LinuxProcMapFiles
            )
        );
        if !evidence_matches_target {
            return Err(NativeLoadObservationError::EvidenceClassTargetMismatch);
        }
        if input.components.is_empty() || input.components.len() > MAX_NATIVE_LOAD_COMPONENTS {
            return Err(NativeLoadObservationError::InvalidComponentCount);
        }
        if input.visibility_scope == NativeLoadVisibilityScope::FileBackedExecutableMappings
            && input
                .components
                .iter()
                .any(|component| component.mapping_class() == NativeMappingClass::DataMapped)
        {
            return Err(NativeLoadObservationError::VisibilityScopeViolation);
        }
        validate_components(package, &input.components)?;
        let observation = Self {
            schema_version,
            runtime_package_manifest_id: package.runtime_package_manifest_id(),
            evidence_class: input.evidence_class,
            visibility_scope: input.visibility_scope,
            process_evidence_digest: input.process_evidence_digest,
            observation_contract_id: input.observation_contract_id,
            observation_contract_schema_version: input.observation_contract_schema_version,
            components: input.components,
        };
        if observation.canonical_bytes().len() > MAX_NATIVE_LOAD_CANONICAL_BYTES {
            return Err(NativeLoadObservationError::CanonicalEncodingTooLarge);
        }
        Ok(observation)
    }
}

fn validate_components(
    package: &RuntimePackageManifest,
    components: &[super::NativeLoadedComponent],
) -> Result<(), NativeLoadObservationError> {
    let mut prior = None;
    let mut origins = BTreeSet::new();
    let mut packaged_paths = BTreeSet::new();
    for component in components {
        let key = component_key(component);
        if prior.as_ref().is_some_and(|value| value >= &key)
            || !origins.insert(origin_key(component))
        {
            return Err(NativeLoadObservationError::InvalidComponentOrder);
        }
        prior = Some(key);
        let NativeLoadOrigin::PackagedMember { relative_path } = component.origin() else {
            if component.mapping_class() == NativeMappingClass::ExecutableImage {
                return Err(NativeLoadObservationError::LoadPolicyViolation);
            }
            continue;
        };
        let Some(member) = package
            .members()
            .iter()
            .find(|member| member.relative_path() == relative_path)
        else {
            return Err(NativeLoadObservationError::PackagedComponentMismatch);
        };
        if member.artifact_id() != component.artifact_id()
            || member.byte_size() != component.byte_size()
        {
            return Err(NativeLoadObservationError::PackagedComponentMismatch);
        }
        let is_entrypoint = member == package.entrypoint();
        let expected_mapping = if is_entrypoint {
            NativeMappingClass::ExecutableImage
        } else {
            NativeMappingClass::ExecutableMapped
        };
        if !member.is_code()
            || member.load_policy() == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
            || component.mapping_class() != expected_mapping
        {
            return Err(NativeLoadObservationError::LoadPolicyViolation);
        }
        packaged_paths.insert(relative_path.as_str());
    }
    let all_required_present = package.members().iter().all(|member| {
        !member.is_code()
            || member.load_policy() != RuntimePackageLoadPolicy::RequiredAtReady
            || packaged_paths.contains(member.relative_path().as_str())
    });
    if !all_required_present {
        return Err(NativeLoadObservationError::MissingRequiredComponent);
    }
    Ok(())
}

fn component_key(component: &super::NativeLoadedComponent) -> Vec<u8> {
    let mut key = Vec::new();
    match component.origin() {
        NativeLoadOrigin::PackagedMember { relative_path } => {
            key.push(0);
            key.extend_from_slice(relative_path.as_str().as_bytes());
            key.push(0);
        }
        NativeLoadOrigin::ExternalPlatformComponent => key.push(1),
    }
    key.extend_from_slice(component.artifact_id().digest().as_str().as_bytes());
    key.extend_from_slice(&component.byte_size().to_be_bytes());
    key.push(super::codec::mapping_class_byte(component.mapping_class()));
    key.extend_from_slice(component.object_evidence_digest().as_str().as_bytes());
    key
}

fn origin_key(component: &super::NativeLoadedComponent) -> Vec<u8> {
    let mut key = Vec::new();
    match component.origin() {
        NativeLoadOrigin::PackagedMember { relative_path } => {
            key.push(0);
            key.extend_from_slice(relative_path.as_str().as_bytes());
        }
        NativeLoadOrigin::ExternalPlatformComponent => {
            key.push(1);
            key.extend_from_slice(component.artifact_id().digest().as_str().as_bytes());
            key.extend_from_slice(component.object_evidence_digest().as_str().as_bytes());
        }
    }
    key
}

fn valid_machine_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OBSERVATION_CONTRACT_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}
