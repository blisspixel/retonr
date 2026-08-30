use std::fs;

use rewrite_ollama_package::ReconstructionLimits;
use rewrite_types::CancellationToken;
use serde_json::json;

use crate::{
    MODEL_LICENSE_REVIEW_PROCEDURE_ID, MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
    MODEL_LICENSE_REVIEW_SCHEMA_VERSION, ManagedOllamaLaunchError,
    ManagedOllamaModelAuthorityError, ModelLicenseControlCompiler, ModelLicenseControlError,
    ModelLicenseControlVerifier, ModelLicensePermission, PackageAttestationService,
    ProductionModelLicenseApprovalPolicy, VerifiedApprovedModelLicenseControl,
    VerifiedManagedOllamaModelPackageLease,
};

use super::support::{Fixture, set_root, verify};

#[test]
fn exact_input_and_local_license_create_one_redacted_launch_authority() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    let input = input(&fixture, &lease);
    let license = approved_license(&lease, ModelLicensePermission::LocalGeneration);
    let expected_control = license.control_id().clone();
    let authority =
        PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(&lease, input, license)
            .expect("exact authorities join");

    assert_eq!(authority.foundation_id(), lease.foundation_id());
    assert_eq!(authority.model_license_control_id(), &expected_control);
    assert_eq!(
        authority.model_target().artifact_id(),
        authority.input_evidence().model_artifact_id()
    );
    assert!(authority.model_byte_size() > 0);
    let debug = format!("{authority:?}");
    assert!(debug.contains("VerifiedManagedOllamaLaunchPlan"));
    for secret in [
        fixture.package.source().locator(),
        "legal/license.txt",
        "fixture model license",
        fixture.directory.path().to_string_lossy().as_ref(),
    ] {
        assert!(!debug.contains(secret), "debug exposed {secret}");
    }
    authority
        .revalidate_for_materialization_test(&CancellationToken::new())
        .expect("exact authority passes the materialization checkpoint");
    assert!(authority.verify_internal_handoff_for_test(&lease, &expected_control));
}

#[test]
fn input_from_another_live_lease_is_rejected_before_revalidation() {
    let selected_fixture = Fixture::new(false);
    let other_fixture = Fixture::new(false);
    let selected = verify(&selected_fixture);
    let other = verify(&other_fixture);
    let other_input = input(&other_fixture, &other);
    let selected_license = approved_license(&selected, ModelLicensePermission::LocalGeneration);
    fs::write(
        set_root(&selected_fixture).join("unexpected-after-authority.bin"),
        b"unexpected",
    )
    .expect("make selected lease stale");

    let error = PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(
        &selected,
        other_input,
        selected_license,
    )
    .expect_err("another input capability must not join");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::RelationshipMismatch
    ));
}

#[test]
fn license_from_another_live_lease_is_rejected() {
    let selected_fixture = Fixture::new(false);
    let other_fixture = Fixture::new(false);
    let selected = verify(&selected_fixture);
    let other = verify(&other_fixture);
    let selected_input = input(&selected_fixture, &selected);
    let other_license = approved_license(&other, ModelLicensePermission::LocalGeneration);

    let error = PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(
        &selected,
        selected_input,
        other_license,
    )
    .expect_err("another license capability must not join");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::ModelAuthority(ManagedOllamaModelAuthorityError::Revalidation(
            ModelLicenseControlError::LeaseMismatch
        ))
    ));
}

#[test]
fn redistribution_permission_cannot_authorize_generation() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    let input = input(&fixture, &lease);
    let redistribution = approved_license(&lease, ModelLicensePermission::PackageRedistribution);

    let error = PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(
        &lease,
        input,
        redistribution,
    )
    .expect_err("redistribution is not local-generation authority");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::ModelAuthority(ManagedOllamaModelAuthorityError::Revalidation(
            ModelLicenseControlError::PermissionMismatch
        ))
    ));
}

#[test]
fn self_authored_control_remains_inert_under_empty_production_policy() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    let review = reviewer_json(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = ModelLicenseControlCompiler::compile(
        &lease,
        ModelLicensePermission::LocalGeneration,
        &review,
        &CancellationToken::new(),
    )
    .expect("compile inert local control");
    assert!(matches!(
        ModelLicenseControlVerifier::verify(
            compiled.canonical_bytes(),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &ProductionModelLicenseApprovalPolicy::new(),
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::ApprovalPolicyDenied)
    ));
}

#[test]
fn stale_specialized_lease_is_rejected_at_materialization_not_structural_join() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    let input = input(&fixture, &lease);
    let license = approved_license(&lease, ModelLicensePermission::LocalGeneration);
    fs::write(
        set_root(&fixture).join("unexpected-after-input.bin"),
        b"unexpected",
    )
    .expect("make package tree stale");

    let authority =
        PackageAttestationService::authorize_managed_ollama_v0_32_15_launch(&lease, input, license)
            .expect("structural join does not duplicate executable-boundary hashing");
    let error = authority
        .revalidate_for_materialization_test(&CancellationToken::new())
        .expect_err("stale retained tree must fail before private input materialization");
    assert!(matches!(
        error,
        ManagedOllamaLaunchError::ModelAuthority(ManagedOllamaModelAuthorityError::Revalidation(
            ModelLicenseControlError::Lease(_)
        ))
    ));
}

pub(super) fn input<'lease>(
    fixture: &Fixture,
    lease: &'lease VerifiedManagedOllamaModelPackageLease,
) -> crate::VerifiedManagedOllamaInputPlan<'lease> {
    PackageAttestationService::prepare_verified_managed_ollama_inputs(
        lease,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("prepare exact verified input")
}

pub(super) fn approved_license(
    lease: &VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
) -> VerifiedApprovedModelLicenseControl<'_> {
    let review = reviewer_json(lease, permission);
    let compiled =
        ModelLicenseControlCompiler::compile(lease, permission, &review, &CancellationToken::new())
            .expect("compile exact review");
    let policy = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        compiled.control_id().clone(),
        permission,
    );
    ModelLicenseControlVerifier::verify(
        compiled.canonical_bytes(),
        lease,
        permission,
        &policy,
        &CancellationToken::new(),
    )
    .expect("verify exact approved control")
}

fn reviewer_json(
    lease: &VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
) -> Vec<u8> {
    let view = lease.private_view();
    let foundation = view.foundation_evidence();
    let package = view.model_package_manifest();
    let source = package.source();
    let provenance = foundation.provenance_manifest();
    let license_members = foundation
        .license_members()
        .iter()
        .map(|member| {
            json!({
                "artifact_id": member.artifact_id(),
                "byte_size": member.byte_size(),
                "relative_path": member.relative_path().as_str(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&json!({
        "decision": "approved",
        "foundation": {
            "artifact_set_id": foundation.artifact_set_id().digest(),
            "descriptor_mapping_digest": foundation.descriptor_mapping_digest(),
            "foundation_id": lease.foundation_id().digest(),
            "logical_binding_digest": foundation.logical_binding_digest(),
            "model_package_manifest_id": foundation.model_package_manifest_id().digest(),
            "package_source_id": foundation.package_source_id().digest(),
            "provenance_manifest": {
                "artifact_id": provenance.artifact_id(),
                "byte_size": provenance.byte_size(),
                "relative_path": provenance.relative_path().as_str(),
            },
        },
        "license_members": license_members,
        "permission": permission,
        "procedure_id": MODEL_LICENSE_REVIEW_PROCEDURE_ID,
        "procedure_version": MODEL_LICENSE_REVIEW_PROCEDURE_VERSION,
        "schema_version": MODEL_LICENSE_REVIEW_SCHEMA_VERSION,
        "source": {
            "kind": source.kind(),
            "locator": source.locator(),
            "provenance_digest": source.provenance_digest(),
            "revision": source.revision(),
        },
    }))
    .expect("canonical reviewer JSON")
}
