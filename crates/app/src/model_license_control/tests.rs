use rewrite_model::ModelPackageMemberRole;
use rewrite_types::{CancellationToken, Digest};
use serde_json::{Value, json};

use super::verification::reviewer_json_for_test;
use super::{
    CompiledModelLicenseControl, MODEL_LICENSE_CONTROL_PROCEDURE_ID,
    MODEL_LICENSE_CONTROL_PROCEDURE_VERSION, MODEL_LICENSE_REVIEW_PROCEDURE_ID,
    ModelLicenseControlApprovalPromoter, ModelLicenseControlCompiler, ModelLicenseControlError,
    ModelLicensePermission, ProductionModelLicenseApprovalPolicy,
};

mod adversarial;
mod fixture;

use fixture::Fixture;

const REVIEW_DOMAIN: &[u8] = b"retonr:model-license-review-evidence:v1\0";

#[test]
fn self_contained_control_derives_review_digest_and_live_authority() {
    let fixture = Fixture::new(true);
    let lease = fixture.verify();
    let reviewer = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = compile(&lease, ModelLicensePermission::LocalGeneration, &reviewer);
    let structural = super::ModelLicenseControlVerifier::verify_structural(
        compiled.canonical_bytes(),
        &lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("self-contained control verifies structurally");
    let structural_debug = format!("{structural:?}");
    let policy = ProductionModelLicenseApprovalPolicy::exact_test_policy(
        compiled.control_id().clone(),
        ModelLicensePermission::LocalGeneration,
    );
    assert_eq!(
        policy
            .assess(
                &structural,
                &lease,
                ModelLicensePermission::LocalGeneration,
                &CancellationToken::new(),
            )
            .expect("nonauthorizing assessment"),
        super::ModelLicenseControlAssessmentDisposition::Approved
    );
    let authority = ModelLicenseControlApprovalPromoter::promote(
        structural,
        &policy,
        &CancellationToken::new(),
    )
    .expect("structural proof promotes through exact test policy");

    assert_eq!(compiled.control_id().digest().as_str().len(), 64);
    assert_eq!(compiled.foundation_id(), lease.foundation_id());
    assert_eq!(
        compiled.permission(),
        ModelLicensePermission::LocalGeneration
    );
    assert_eq!(compiled.license_member_count(), 1);
    assert_eq!(authority.control_id(), compiled.control_id());
    assert_eq!(authority.foundation_id(), lease.foundation_id());
    assert_eq!(
        authority.permission(),
        ModelLicensePermission::LocalGeneration
    );
    assert_eq!(
        authority.installation_generation(),
        fixture.key.installation_generation()
    );
    assert_eq!(authority.license_member_count(), 1);
    authority
        .revalidate_for_use(
            &lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        )
        .expect("authority revalidates for the exact live lease");

    let mut digest_material = REVIEW_DOMAIN.to_vec();
    digest_material.extend_from_slice(&reviewer);
    assert_eq!(
        compiled.review_evidence_digest(),
        &Digest::sha256(&digest_material)
    );
    let control: Value = serde_json::from_slice(compiled.canonical_bytes()).expect("control JSON");
    assert_eq!(control["authority"], "none");
    assert_eq!(control["status"], "approved");
    assert_eq!(control["procedure_id"], MODEL_LICENSE_CONTROL_PROCEDURE_ID);
    assert_eq!(
        control["procedure_version"],
        MODEL_LICENSE_CONTROL_PROCEDURE_VERSION
    );
    assert_eq!(
        control["review"],
        serde_json::from_slice::<Value>(&reviewer).expect("reviewer JSON")
    );
    assert_shared_artifact_keeps_one_logical_license_path(&fixture, &control);

    for debug in [
        format!("{compiled:?}"),
        structural_debug,
        format!("{authority:?}"),
    ] {
        for secret in [
            lease
                .private_view()
                .model_package_manifest()
                .source()
                .locator(),
            "legal/license.txt",
            "fixture model license",
            fixture.directory.path().to_string_lossy().as_ref(),
        ] {
            assert!(!debug.contains(secret), "debug exposed {secret}");
        }
    }
}

#[test]
fn structurally_valid_self_authored_control_is_inert_under_production_policy() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let review = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = compile(&lease, ModelLicensePermission::LocalGeneration, &review);
    let production_policy = ProductionModelLicenseApprovalPolicy::new();
    assert_eq!(production_policy.approved_control_count(), 0);
    assert_eq!(
        format!("{production_policy:?}"),
        "ProductionModelLicenseApprovalPolicy { approved_control_count: 0, .. }"
    );
    let structural = super::ModelLicenseControlVerifier::verify_structural(
        compiled.canonical_bytes(),
        &lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("production denial does not erase structural validity");
    assert_eq!(structural.control_id(), compiled.control_id());
    assert_eq!(structural.foundation_id(), lease.foundation_id());
    assert_eq!(
        production_policy
            .assess(
                &structural,
                &lease,
                ModelLicensePermission::LocalGeneration,
                &CancellationToken::new(),
            )
            .expect("denied assessment remains nonauthorizing evidence"),
        super::ModelLicenseControlAssessmentDisposition::Denied
    );
    assert!(matches!(
        ModelLicenseControlApprovalPromoter::promote(
            structural,
            &production_policy,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::ApprovalPolicyDenied)
    ));
    assert!(matches!(
        super::ModelLicenseControlVerifier::verify(
            compiled.canonical_bytes(),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &production_policy,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::ApprovalPolicyDenied)
    ));
}

#[test]
fn permission_is_part_of_review_control_identity_and_authority_use() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let local_review = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let redistribution_review =
        reviewer_json_for_test(&lease, ModelLicensePermission::PackageRedistribution);
    let local = compile(
        &lease,
        ModelLicensePermission::LocalGeneration,
        &local_review,
    );
    let redistribution = compile(
        &lease,
        ModelLicensePermission::PackageRedistribution,
        &redistribution_review,
    );
    assert_ne!(local.control_id(), redistribution.control_id());
    assert_ne!(local.canonical_bytes(), redistribution.canonical_bytes());
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::PackageRedistribution,
            &local_review,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::PermissionMismatch)
    ));
    assert!(matches!(
        ModelLicenseControlVerifier::verify(
            local.canonical_bytes(),
            &lease,
            ModelLicensePermission::PackageRedistribution,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::PermissionMismatch)
    ));
    let authority = ModelLicenseControlVerifier::verify(
        local.canonical_bytes(),
        &lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("local authority");
    assert!(matches!(
        authority.revalidate_for_use(
            &lease,
            ModelLicensePermission::PackageRedistribution,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::PermissionMismatch)
    ));
}

#[test]
fn identical_reinstall_keeps_portable_control_but_requires_fresh_authority() {
    let first_fixture = Fixture::new(false);
    let mut second_fixture = Fixture::new(false);
    second_fixture.advance_generation();
    let first_lease = first_fixture.verify();
    let second_lease = second_fixture.verify();
    let review = reviewer_json_for_test(&first_lease, ModelLicensePermission::LocalGeneration);
    assert_eq!(
        review,
        reviewer_json_for_test(&second_lease, ModelLicensePermission::LocalGeneration)
    );
    let first_control = compile(
        &first_lease,
        ModelLicensePermission::LocalGeneration,
        &review,
    );
    let second_control = compile(
        &second_lease,
        ModelLicensePermission::LocalGeneration,
        &review,
    );
    assert_eq!(first_control, second_control);

    let first_authority = ModelLicenseControlVerifier::verify(
        first_control.canonical_bytes(),
        &first_lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("first live authority");
    let second_authority = ModelLicenseControlVerifier::verify(
        second_control.canonical_bytes(),
        &second_lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("fresh reinstalled authority");
    assert_ne!(
        first_authority.installation_generation(),
        second_authority.installation_generation()
    );
    assert!(matches!(
        first_authority.revalidate_for_use(
            &second_lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::LeaseMismatch)
    ));

    let mut changed_generation = second_authority;
    changed_generation.verified.installation_generation += 1;
    assert!(matches!(
        changed_generation.revalidate_for_use(
            &second_lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::InstallationGenerationMismatch)
    ));
}

#[test]
fn same_generation_from_another_live_lease_is_not_authority_equivalent() {
    let first_fixture = Fixture::new(false);
    let second_fixture = Fixture::new(false);
    let first_lease = first_fixture.verify();
    let second_lease = second_fixture.verify();
    assert_eq!(
        first_lease.private_view().installation_generation(),
        second_lease.private_view().installation_generation()
    );
    let review = reviewer_json_for_test(&first_lease, ModelLicensePermission::LocalGeneration);
    let control = compile(
        &first_lease,
        ModelLicensePermission::LocalGeneration,
        &review,
    );
    let authority = ModelLicenseControlVerifier::verify(
        control.canonical_bytes(),
        &first_lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("first authority");
    second_fixture.add_unexpected_member();
    assert!(matches!(
        authority.revalidate_for_use(
            &second_lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::LeaseMismatch)
    ));
}

#[test]
fn structural_proof_rejects_lease_substitution_and_drift_before_promotion() {
    let first_fixture = Fixture::new(false);
    let second_fixture = Fixture::new(false);
    let first_lease = first_fixture.verify();
    let second_lease = second_fixture.verify();
    let review = reviewer_json_for_test(&first_lease, ModelLicensePermission::LocalGeneration);
    let control = compile(
        &first_lease,
        ModelLicensePermission::LocalGeneration,
        &review,
    );
    let structural = super::ModelLicenseControlVerifier::verify_structural(
        control.canonical_bytes(),
        &first_lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("structural proof");
    assert!(matches!(
        structural.revalidate(
            &second_lease,
            ModelLicensePermission::LocalGeneration,
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::LeaseMismatch)
    ));

    first_fixture.add_unexpected_member();
    assert!(matches!(
        ModelLicenseControlApprovalPromoter::promote(
            structural,
            &ProductionModelLicenseApprovalPolicy::new(),
            &CancellationToken::new(),
        ),
        Err(ModelLicenseControlError::Lease(_))
    ));
}

#[test]
fn cancellation_prevents_compile_verify_and_authority_use() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let review = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let compiled = compile(&lease, ModelLicensePermission::LocalGeneration, &review);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        ModelLicenseControlCompiler::compile(
            &lease,
            ModelLicensePermission::LocalGeneration,
            &review,
            &cancelled,
        ),
        Err(ModelLicenseControlError::Cancelled)
    ));
    assert!(matches!(
        ModelLicenseControlVerifier::verify(
            compiled.canonical_bytes(),
            &lease,
            ModelLicensePermission::LocalGeneration,
            &cancelled,
        ),
        Err(ModelLicenseControlError::Cancelled)
    ));
    let structural = super::ModelLicenseControlVerifier::verify_structural(
        compiled.canonical_bytes(),
        &lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("structural proof before cancellation");
    assert!(matches!(
        ModelLicenseControlApprovalPromoter::promote(
            structural,
            &ProductionModelLicenseApprovalPolicy::new(),
            &cancelled,
        ),
        Err(ModelLicenseControlError::Cancelled)
    ));
    let authority = ModelLicenseControlVerifier::verify(
        compiled.canonical_bytes(),
        &lease,
        ModelLicensePermission::LocalGeneration,
        &CancellationToken::new(),
    )
    .expect("authority before cancellation");
    assert!(matches!(
        authority.revalidate_for_use(&lease, ModelLicensePermission::LocalGeneration, &cancelled,),
        Err(ModelLicenseControlError::Cancelled)
    ));
}

fn compile(
    lease: &crate::VerifiedManagedOllamaModelPackageLease,
    permission: ModelLicensePermission,
    review: &[u8],
) -> CompiledModelLicenseControl {
    ModelLicenseControlCompiler::compile(lease, permission, review, &CancellationToken::new())
        .expect("compile exact model-license control")
}

struct ModelLicenseControlVerifier;

impl ModelLicenseControlVerifier {
    fn verify<'lease>(
        control_json: &[u8],
        lease: &'lease crate::VerifiedManagedOllamaModelPackageLease,
        permission: ModelLicensePermission,
        cancellation: &CancellationToken,
    ) -> Result<super::VerifiedApprovedModelLicenseControl<'lease>, ModelLicenseControlError> {
        let policy = ProductionModelLicenseApprovalPolicy::exact_test_policy(
            super::control_id(control_json),
            permission,
        );
        let structural = super::ModelLicenseControlVerifier::verify_structural(
            control_json,
            lease,
            permission,
            cancellation,
        )?;
        ModelLicenseControlApprovalPromoter::promote(structural, &policy, cancellation)
    }
}

fn value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("fixture JSON")
}

fn assert_shared_artifact_keeps_one_logical_license_path(fixture: &Fixture, control: &Value) {
    let license = fixture
        .package
        .members()
        .iter()
        .find(|member| member.roles() == [ModelPackageMemberRole::LicenseText])
        .expect("license member");
    let template = fixture
        .package
        .members()
        .iter()
        .find(|member| member.roles() == [ModelPackageMemberRole::PromptTemplate])
        .expect("template member");
    assert_eq!(license.artifact_id(), template.artifact_id());
    let reviewed = control["review"]["license_members"]
        .as_array()
        .expect("reviewed license paths");
    assert_eq!(reviewed.len(), 1);
    assert_eq!(reviewed[0]["relative_path"], "legal/license.txt");
    assert_eq!(reviewed[0]["artifact_id"], json!(license.artifact_id()));
}

#[test]
fn reviewer_procedure_is_exact_and_approval_only() {
    let fixture = Fixture::new(false);
    let lease = fixture.verify();
    let review = reviewer_json_for_test(&lease, ModelLicensePermission::LocalGeneration);
    let parsed = value(&review);
    assert_eq!(parsed["procedure_id"], MODEL_LICENSE_REVIEW_PROCEDURE_ID);
    assert_eq!(parsed["decision"], "approved");
}
