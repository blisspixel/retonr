use rewrite_types::Digest;

use crate::{NativeLoadObservation, NativeLoadOrigin, NativeLoadedComponent, NativeMappingClass};

use super::{artifact, components, input, observation, runtime_package};

#[test]
fn portable_component_set_excludes_attempt_evidence_and_binds_component_facts() {
    let package = runtime_package("1.0.0");
    let first = observation(&package);
    let mut repeated_components = components();
    for (index, component) in repeated_components.iter_mut().enumerate() {
        *component = NativeLoadedComponent::new(
            component.artifact_id().clone(),
            component.byte_size(),
            component.origin().clone(),
            component.mapping_class(),
            Digest::sha256(format!("repeat object {index}").as_bytes()),
        );
    }
    let mut repeated_input = input(repeated_components);
    repeated_input.process_evidence_digest = Digest::sha256(b"repeat process");
    let repeated =
        NativeLoadObservation::new(&package, repeated_input).expect("repeat observation");
    assert_ne!(
        first.native_load_observation_id(),
        repeated.native_load_observation_id()
    );
    assert_eq!(
        first.portable_component_set_digest(),
        repeated.portable_component_set_digest()
    );

    let mut changed_components = components();
    changed_components[2] = NativeLoadedComponent::new(
        artifact("different-libc"),
        16,
        NativeLoadOrigin::ExternalPlatformComponent,
        NativeMappingClass::ExecutableMapped,
        Digest::sha256(b"different external evidence"),
    );
    let changed = NativeLoadObservation::new(&package, input(changed_components))
        .expect("changed component set");
    assert_ne!(
        first.portable_component_set_digest(),
        changed.portable_component_set_digest()
    );
}
