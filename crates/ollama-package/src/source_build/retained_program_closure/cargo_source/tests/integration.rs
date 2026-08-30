use std::{collections::BTreeMap, io::Cursor};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::{
    super::{
        CARGO_SOURCE_CLOSURE_PROCEDURE_ID, CARGO_SOURCE_CLOSURE_PROCEDURE_VERSION,
        CargoSourceClosureError, CargoSourceClosureLimits, verify_cargo_source_closure,
    },
    Fixture,
};
use crate::source_build::{
    RuntimeSourceBuildAcceleratorPolicy, RuntimeSourceBuildInputComponent,
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
    RuntimeSourceBuildNetworkPolicy, RuntimeSourceBuildPolicy, VerifiedRuntimeSourceBuildInputs,
};

#[test]
fn public_verifier_binds_all_identities_and_reviewer_facts() {
    let fixture = Fixture::valid();
    let inputs = inputs(&fixture);
    let members = members(&fixture);
    let closure = verify_cargo_source_closure(
        &inputs,
        CargoSourceClosureLimits::default(),
        |path| {
            members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildInputOpenError)
        },
        || false,
    )
    .expect("complete fixture verifies");
    assert_eq!(CARGO_SOURCE_CLOSURE_PROCEDURE_VERSION, 1);
    assert_eq!(
        CARGO_SOURCE_CLOSURE_PROCEDURE_ID,
        "retonr:runtime-source-build:cargo-source-closure"
    );
    assert_eq!(closure.cargo_lock_digest(), &Digest::sha256(&fixture.lock));
    assert_eq!(
        closure.repository_archive_id(),
        &Digest::sha256(&fixture.repository)
    );
    assert_eq!(
        closure.raw_crate_archive_id(),
        &Digest::sha256(&fixture.raw)
    );
    assert_eq!(
        closure.vendor_archive_id(),
        &Digest::sha256(&fixture.vendor)
    );
    assert_ne!(closure.closure_id(), closure.repository_tree_id());
    assert_ne!(closure.raw_crate_tree_id(), closure.vendor_tree_id());
    assert_eq!(
        closure.source_input_set_id(),
        inputs.manifest().artifact_set().artifact_set_id().digest()
    );
    let facts = closure.reviewer_facts();
    assert_eq!(facts.registry_package_count(), 1);
    assert_eq!(facts.path_package_count(), 2);
    assert_eq!(facts.dependency_edge_count(), 2);
    assert_eq!(facts.source_file_count(), 2);
}

#[test]
fn public_verifier_rejects_cancellation_roles_open_and_measurement_drift() {
    let fixture = Fixture::valid();
    let inputs = inputs(&fixture);
    assert_eq!(
        verify_cargo_source_closure::<Cursor<Vec<u8>>, _, _>(
            &inputs,
            CargoSourceClosureLimits::default(),
            |_| Err(RuntimeSourceBuildInputOpenError),
            || true,
        ),
        Err(CargoSourceClosureError::Cancelled)
    );
    assert_eq!(
        verify_cargo_source_closure::<Cursor<Vec<u8>>, _, _>(
            &inputs,
            CargoSourceClosureLimits::default(),
            |_| Err(RuntimeSourceBuildInputOpenError),
            || false,
        ),
        Err(CargoSourceClosureError::ComponentUnavailable)
    );
    let mut invalid_roles = inputs.clone();
    invalid_roles.manifest.components[0]
        .roles
        .push(RuntimeSourceBuildInputRole::CargoVendorSource);
    assert_eq!(
        verify_cargo_source_closure::<Cursor<Vec<u8>>, _, _>(
            &invalid_roles,
            CargoSourceClosureLimits::default(),
            |_| Err(RuntimeSourceBuildInputOpenError),
            || false,
        ),
        Err(CargoSourceClosureError::InvalidComponentRoles)
    );
    let members = members(&fixture);
    assert_eq!(
        verify_cargo_source_closure(
            &inputs,
            CargoSourceClosureLimits::default(),
            |path| {
                let mut bytes = members[path.as_str()].clone();
                if path.as_str().ends_with("Cargo.lock") {
                    bytes.push(0);
                }
                Ok::<_, RuntimeSourceBuildInputOpenError>(Cursor::new(bytes))
            },
            || false,
        ),
        Err(CargoSourceClosureError::ComponentMismatch)
    );
}

fn inputs(fixture: &Fixture) -> VerifiedRuntimeSourceBuildInputs {
    let components = component_specs(fixture)
        .into_iter()
        .map(|(path, bytes, role)| RuntimeSourceBuildInputComponent {
            relative_path: ArtifactSetRelativePath::new(path).expect("component path"),
            byte_size: u64::try_from(bytes.len()).expect("fixture length"),
            digest: Digest::sha256(bytes),
            name: "fixture".to_owned(),
            revision: "fixture-v1".to_owned(),
            source_locator: "urn:retonr:fixture".to_owned(),
            roles: vec![role],
        })
        .collect::<Vec<_>>();
    let artifact_members = components
        .iter()
        .map(|component| {
            ArtifactSetMember::new(
                ArtifactId::from_digest(component.digest.clone()),
                component.byte_size,
                component.relative_path.clone(),
            )
        })
        .collect();
    let artifact_set = ArtifactSetManifest::new(artifact_members).expect("artifact set");
    VerifiedRuntimeSourceBuildInputs {
        manifest: RuntimeSourceBuildInputManifest {
            artifact_set,
            policy: fixture_policy(),
            components,
            manifest_digest: Digest::sha256(b"fixture-manifest"),
        },
        program_lineage: None,
    }
}

fn component_specs(fixture: &Fixture) -> [(&'static str, &[u8], RuntimeSourceBuildInputRole); 4] {
    [
        (
            "lineage/source/Cargo.lock",
            &fixture.lock,
            RuntimeSourceBuildInputRole::CargoLockfile,
        ),
        (
            "lineage/source/cargo-crates.tar",
            &fixture.raw,
            RuntimeSourceBuildInputRole::CargoRawCrateSource,
        ),
        (
            "lineage/source/cargo-vendor.tar",
            &fixture.vendor,
            RuntimeSourceBuildInputRole::CargoVendorSource,
        ),
        (
            "lineage/source/retonr-source.tar",
            &fixture.repository,
            RuntimeSourceBuildInputRole::RetonrRepositorySource,
        ),
    ]
}

fn members(fixture: &Fixture) -> BTreeMap<String, Vec<u8>> {
    component_specs(fixture)
        .into_iter()
        .map(|(path, bytes, _)| (path.to_owned(), bytes.to_vec()))
        .collect()
}

fn fixture_policy() -> RuntimeSourceBuildPolicy {
    RuntimeSourceBuildPolicy {
        target: RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxMusl,
        )
        .expect("fixture target"),
        network_access: RuntimeSourceBuildNetworkPolicy::Denied,
        accelerator: RuntimeSourceBuildAcceleratorPolicy::CpuOnly,
        cpu_feature_policy: "fixture".to_owned(),
        locale: "C".to_owned(),
        timezone: "UTC".to_owned(),
        source_date_epoch: super::EPOCH,
        environment: Vec::new(),
        build_arguments: Vec::new(),
        environment_digest: Digest::sha256(b"fixture-environment"),
        build_arguments_digest: Digest::sha256(b"fixture-arguments"),
    }
}
