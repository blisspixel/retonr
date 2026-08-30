use std::fs;

use rewrite_model::{
    ModelPackageManifest, PackageSource, PackageSourceKind, PackageTransformation,
};
use rewrite_ollama_package::{OllamaLocalArchiveFoundationError, ReconstructionLimits};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    ManagedOllamaInputError, ManagedOllamaModelPackageError, PackageAttestationError,
    PackageAttestationService,
};

#[path = "managed_ollama_model/launch_authority.rs"]
mod launch_authority;
#[cfg(all(target_os = "linux", feature = "managed-launch-live-fixture"))]
#[path = "managed_ollama_model/managed_launch_live.rs"]
mod managed_launch_live;
#[path = "managed_ollama_model/support.rs"]
mod support;

use support::{
    Fixture, MODEL_LIMITS, install_alternate_raw_manifest, rebuild_package, removal_limits,
    set_root, verify,
};

#[test]
fn exact_foundation_constructs_revalidates_and_prepares_verified_inputs() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    assert_eq!(lease.artifact_set_id(), &fixture.set.artifact_set_id());
    assert_eq!(
        lease.model_package_manifest_id(),
        &fixture.package.model_package_manifest_id()
    );
    assert_eq!(
        lease.package_source_id(),
        &fixture.package.source().package_source_id()
    );
    assert_eq!(lease.member_count(), 6);
    assert_eq!(lease.byte_size(), fixture.set.total_byte_size());
    assert_eq!(lease.license_member_count(), 1);
    assert_eq!(lease.foundation_id().digest().as_str().len(), 64);
    lease
        .revalidate(&CancellationToken::new())
        .expect("verified foundation revalidates");

    let plan = PackageAttestationService::prepare_verified_managed_ollama_inputs(
        &lease,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("verified retained package prepares exact inputs");
    assert_eq!(plan.foundation_id(), lease.foundation_id());
    assert_eq!(plan.evidence().artifact_set_id(), lease.artifact_set_id());
    assert_eq!(
        plan.evidence().model_package_manifest_id(),
        lease.model_package_manifest_id()
    );
    assert_eq!(
        plan.model_target().artifact_id(),
        plan.evidence().model_artifact_id()
    );
    let debug = format!("{plan:?}");
    assert!(debug.contains("VerifiedManagedOllamaInputPlan"));
    assert!(debug.contains(lease.foundation_id().digest().as_str()));
    assert!(!debug.contains(fixture.package.source().locator()));
    assert!(!debug.contains(fixture.directory.path().to_string_lossy().as_ref()));
}

#[test]
fn shared_license_and_template_bytes_preserve_logical_foundation_members() {
    let fixture = Fixture::new(true);
    let lease = verify(&fixture);
    assert_eq!(lease.license_member_count(), 1);
    let plan = PackageAttestationService::prepare_verified_managed_ollama_inputs(
        &lease,
        &fixture.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("shared CAS content remains a valid logical foundation");
    assert_eq!(plan.evidence().member_count(), 5);
    assert_eq!(plan.foundation_id(), lease.foundation_id());
}

#[test]
fn package_and_artifact_set_substitution_fail_before_authority() {
    let fixture = Fixture::new(false);
    let changed_contract = ModelPackageManifest::new(
        &fixture.set,
        "substituted-ollama-contract",
        fixture.package.format_contract_schema_version(),
        fixture.package.source().clone(),
        fixture.package.transformation().clone(),
        fixture.package.members().to_vec(),
        fixture.package.weight_layout().clone(),
        fixture.package.embedded_components().to_vec(),
    )
    .expect("structurally valid substituted package");
    let error = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease(),
        &changed_contract,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("package substitution must fail");
    assert!(matches!(
        error,
        ManagedOllamaModelPackageError::Foundation(
            OllamaLocalArchiveFoundationError::ModelPackageMismatch
        )
    ));

    let other = Fixture::new(true);
    let error = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease(),
        &other.package,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("artifact-set substitution must fail");
    assert!(matches!(
        error,
        ManagedOllamaModelPackageError::Package(PackageAttestationError::ModelRelationship(_))
    ));
}

#[test]
fn source_and_transformation_substitution_fail_closed() {
    let fixture = Fixture::new(false);
    let source = PackageSource::new(
        PackageSourceKind::UpstreamRelease,
        fixture.package.source().locator(),
        fixture.package.source().revision(),
        fixture.package.source().provenance_digest().clone(),
    )
    .expect("alternate source");
    let source_changed = rebuild_package(
        &fixture.set,
        &fixture.package,
        source,
        fixture.package.transformation().clone(),
        fixture.package.members().to_vec(),
    );
    let source_error = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease(),
        &source_changed,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("source substitution must fail");
    assert!(matches!(
        source_error,
        ManagedOllamaModelPackageError::Foundation(
            OllamaLocalArchiveFoundationError::UnsupportedSource
        )
    ));

    let transformation_changed = rebuild_package(
        &fixture.set,
        &fixture.package,
        fixture.package.source().clone(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"substituted logical binding"),
        },
        fixture.package.members().to_vec(),
    );
    let transformation_error = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease(),
        &transformation_changed,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("transformation substitution must fail");
    assert!(matches!(
        transformation_error,
        ManagedOllamaModelPackageError::Foundation(
            OllamaLocalArchiveFoundationError::TransformationBindingMismatch
        )
    ));
}

#[test]
fn raw_manifest_substitution_fails_source_binding() {
    let fixture = Fixture::new(false);
    let (set, members) = install_alternate_raw_manifest(&fixture);
    let substituted = rebuild_package(
        &set,
        &fixture.package,
        fixture.package.source().clone(),
        fixture.package.transformation().clone(),
        members,
    );
    let error = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease_for(&set),
        &substituted,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("raw manifest substitution must fail");
    assert!(matches!(
        error,
        ManagedOllamaModelPackageError::Foundation(
            OllamaLocalArchiveFoundationError::SourceBindingMismatch
        )
    ));
}

#[test]
fn foundation_id_is_stable_across_installation_generations() {
    let fixture = Fixture::new(false);
    let first = verify(&fixture);
    let first_id = first.foundation_id().clone();
    let first_generation = fixture.key.installation_generation();
    drop(first);
    fixture
        .repository
        .remove_set(&fixture.key, removal_limits(), &CancellationToken::new())
        .expect("remove inactive first installation");
    let second_key = fixture.reinstall("second-installation");
    assert!(second_key.installation_generation() > first_generation);
    let second = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease_for_key(&second_key),
        &fixture.package,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("verify identical reinstall");
    assert_eq!(second.foundation_id(), &first_id);
}

#[test]
fn same_generation_plan_is_bound_only_to_its_originating_live_lease() {
    let first = Fixture::new(false);
    let second = Fixture::new(false);
    let first_lease = verify(&first);
    let second_lease = verify(&second);
    assert_eq!(first_lease.foundation_id(), second_lease.foundation_id());
    assert_eq!(
        first_lease.private_view().installation_generation(),
        second_lease.private_view().installation_generation()
    );

    let second_plan = PackageAttestationService::prepare_verified_managed_ollama_inputs(
        &second_lease,
        &second.reference,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect("prepare second repository input plan");
    assert!(second_plan.binds_exact_package_lease(&second_lease));
    assert!(!second_plan.binds_exact_package_lease(&first_lease));
}

#[test]
fn cancellation_fails_construction_revalidation_and_preparation() {
    let fixture = Fixture::new(false);
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let error = PackageAttestationService::verify_managed_ollama_model_package(
        fixture.lease(),
        &fixture.package,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &cancelled,
    )
    .expect_err("cancelled construction must fail");
    assert!(matches!(
        error,
        ManagedOllamaModelPackageError::Package(PackageAttestationError::Cancelled)
    ));

    let lease = verify(&fixture);
    assert!(matches!(
        lease.revalidate(&cancelled),
        Err(ManagedOllamaModelPackageError::Package(
            PackageAttestationError::Cancelled
        ))
    ));
    assert!(matches!(
        PackageAttestationService::prepare_verified_managed_ollama_inputs(
            &lease,
            &fixture.reference,
            &ReconstructionLimits::default(),
            &cancelled,
        ),
        Err(ManagedOllamaInputError::VerifiedPackage(
            ManagedOllamaModelPackageError::Package(PackageAttestationError::Cancelled)
        ))
    ));
}

#[test]
fn debug_is_redacted_and_private_view_names_the_same_exact_subject() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    let debug = format!("{lease:?}");
    for secret in [
        fixture.package.source().locator(),
        "model/model.gguf",
        "fixture model license",
        fixture.directory.path().to_string_lossy().as_ref(),
        "File",
    ] {
        assert!(!debug.contains(secret), "debug exposed {secret}");
    }
    let view = lease.private_view();
    assert_eq!(
        view.installation_generation(),
        fixture.key.installation_generation()
    );
    assert_eq!(view.artifact_set_manifest(), &fixture.set);
    assert_eq!(view.model_package_manifest(), &fixture.package);
    assert_eq!(
        view.foundation_evidence().artifact_set_id(),
        lease.artifact_set_id()
    );
}

#[test]
fn stale_tree_and_retained_bytes_are_rejected() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    fs::write(
        set_root(&fixture).join("unexpected-member.bin"),
        b"unexpected",
    )
    .expect("add unexpected managed member");
    assert!(matches!(
        lease.revalidate(&CancellationToken::new()),
        Err(ManagedOllamaModelPackageError::Package(
            PackageAttestationError::ArtifactSet(_)
        ))
    ));

    let bytes_fixture = Fixture::new(false);
    let _bytes_lease = verify(&bytes_fixture);
    let model_path = set_root(&bytes_fixture).join("model/model.gguf");
    let original = fs::read(&model_path).expect("read model bytes");
    let mut changed = original.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    let write = fs::write(&model_path, changed);
    #[cfg(windows)]
    assert!(
        write.is_err(),
        "retained Windows handle must deny byte drift"
    );
    #[cfg(unix)]
    {
        write.expect("drift retained model bytes");
        assert!(matches!(
            _bytes_lease.revalidate(&CancellationToken::new()),
            Err(ManagedOllamaModelPackageError::Package(
                PackageAttestationError::MemberBytesConflict
                    | PackageAttestationError::ArtifactSet(_)
            ))
        ));
    }
}

#[test]
fn unexpected_tree_entry_prevents_specialized_construction() {
    let fixture = Fixture::new(false);
    let set_lease = fixture.lease();
    fs::write(
        set_root(&fixture).join("unexpected-before-verification.bin"),
        b"unexpected",
    )
    .expect("add unexpected managed member before verification");
    let error = PackageAttestationService::verify_managed_ollama_model_package(
        set_lease,
        &fixture.package,
        MODEL_LIMITS,
        &ReconstructionLimits::default(),
        &CancellationToken::new(),
    )
    .expect_err("complete tree identity must reject an extra entry");
    assert!(matches!(
        error,
        ManagedOllamaModelPackageError::Package(PackageAttestationError::ArtifactSet(_))
    ));
}

#[cfg(unix)]
#[test]
fn exact_byte_path_replacement_is_rejected_by_retained_identity() {
    let fixture = Fixture::new(false);
    let lease = verify(&fixture);
    let model_path = set_root(&fixture).join("model/model.gguf");
    let replacement = set_root(&fixture).join("model/replacement.gguf");
    fs::copy(&model_path, &replacement).expect("copy exact replacement bytes");
    fs::rename(&replacement, &model_path).expect("replace canonical model object");
    assert!(matches!(
        lease.revalidate(&CancellationToken::new()),
        Err(ManagedOllamaModelPackageError::Package(
            PackageAttestationError::MemberIdentityChanged
        ))
    ));
}
