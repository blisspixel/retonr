use rewrite_model::{ArtifactId, PackageSource, PackageSourceKind, RuntimePackageMemberRole};
use rewrite_types::Digest;

use super::*;
use crate::{RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput};

mod review_fixture;
mod support;
mod typed_ids;

use support::*;

#[test]
fn exact_canonical_review_binds_every_generation_path_identity() {
    let package = package_with_workers(1);
    let admitted = admitted(&package, b"admission");
    let review = review_bytes(&admitted, &package);

    assert_eq!(
        VerifiedManagedGenerationPath::verify(&review, &admitted, &package, version())
            .expect_err("empty production policy remains fail closed"),
        RuntimeAdmissionAuthorityError::GenerationPathUnreviewed
    );
    assert_eq!(
        VerifiedManagedGenerationPath::verify_with_admission_status(
            &review,
            &admitted,
            &package,
            version(),
            OllamaManagedGenerationAdmissionStatus::Unreviewed,
        )
        .expect_err("unreviewed private status cannot produce authority"),
        RuntimeAdmissionAuthorityError::GenerationPathUnreviewed
    );
    let path = verify_reviewed_path(&review, &admitted, &package).expect("exact reviewed path");

    assert_eq!(path.admitted_runtime_id(), admitted.admitted_runtime_id());
    assert_eq!(
        path.frozen_external_component_set_id(),
        admitted.frozen_external_component_set_id()
    );
    assert_eq!(
        path.runtime_package_manifest_id(),
        &package.runtime_package_manifest_id()
    );
    assert_eq!(path.runtime_version().to_string(), "0.32.15");
    assert_eq!(
        path.source_build_inputs_id(),
        admitted.source_build_inputs_id()
    );
    assert_eq!(path.package_source_id(), admitted.package_source_id());
    assert_eq!(path.review_digest(), &Digest::sha256(&review));
    assert_ne!(path.generation_path_id(), path.review_digest());
    assert!(path.matches_runtime(
        &admitted,
        &package,
        version(),
        admitted.frozen_external_component_set_id()
    ));
    assert!(!path.matches_runtime(
        &admitted,
        &package,
        "0.32.14".parse().expect("other version"),
        admitted.frozen_external_component_set_id()
    ));
    assert!(!path.matches_runtime(
        &admitted,
        &package_with_workers(0),
        version(),
        admitted.frozen_external_component_set_id()
    ));
    assert!(!path.matches_runtime(
        &admitted,
        &package,
        version(),
        &frozen_set_id(b"substituted frozen set")
    ));
    assert_eq!(
        path.worker_artifact_id(),
        package
            .members()
            .iter()
            .find(|member| {
                member
                    .roles()
                    .contains(&RuntimePackageMemberRole::WorkerExecutable)
            })
            .expect("worker")
            .artifact_id()
    );
}

#[test]
fn review_encoding_is_bounded_and_canonical() {
    let package = package_with_workers(1);
    let admitted = admitted(&package, b"admission");
    let mut noncanonical = review_bytes(&admitted, &package);
    noncanonical.push(b' ');
    assert_eq!(
        verify_reviewed_path(&noncanonical, &admitted, &package)
            .expect_err("trailing bytes are noncanonical"),
        RuntimeAdmissionAuthorityError::GenerationPathEncoding
    );
    assert_eq!(
        verify_reviewed_path(&[], &admitted, &package).expect_err("empty review is rejected"),
        RuntimeAdmissionAuthorityError::GenerationPathLimit
    );
    assert_eq!(
        VerifiedManagedGenerationPath::verify_with_admission_status(
            &vec![b'x'; MAX_MANAGED_GENERATION_PATH_REVIEW_BYTES + 1],
            &admitted,
            &package,
            version(),
            OllamaManagedGenerationAdmissionStatus::Reviewed,
        )
        .expect_err("overlong review is rejected"),
        RuntimeAdmissionAuthorityError::GenerationPathLimit
    );
}

#[test]
fn every_reviewed_identity_fails_closed_on_drift() {
    let package = package_with_workers(1);
    let admitted = admitted(&package, b"admission");
    let baseline = serde_json::from_slice::<serde_json::Value>(&review_bytes(&admitted, &package))
        .expect("review value");
    let cases = [
        (
            "admitted_runtime_id",
            serde_json::json!(Digest::sha256(b"other admission")),
        ),
        (
            "frozen_external_component_set_id",
            serde_json::json!(frozen_set_id(b"other frozen set")),
        ),
        (
            "package_source_id",
            serde_json::json!(
                PackageSource::new(
                    PackageSourceKind::RepositoryRevision,
                    "https://example.invalid/other",
                    "other-revision",
                    Digest::sha256(b"other provenance"),
                )
                .expect("other source")
                .package_source_id()
            ),
        ),
        ("procedure_id", serde_json::json!("other-procedure")),
        ("procedure_version", serde_json::json!(2)),
        (
            "reviewed_source_build_inputs_id",
            serde_json::json!(artifact_set_id(b"other source")),
        ),
        (
            "runtime_package_manifest_id",
            serde_json::json!(package_with_workers(0).runtime_package_manifest_id()),
        ),
        ("runtime_version", serde_json::json!("0.32.14")),
        ("schema_version", serde_json::json!(2)),
        (
            "worker_artifact_id",
            serde_json::json!(ArtifactId::from_digest(Digest::sha256(b"other worker"))),
        ),
    ];
    for (field, value) in cases {
        let mut changed = baseline.clone();
        changed[field] = value;
        let bytes = serde_json::to_vec(&changed).expect("canonical changed review");
        assert_eq!(
            verify_reviewed_path(&bytes, &admitted, &package)
                .expect_err("identity drift must fail"),
            RuntimeAdmissionAuthorityError::GenerationPathBinding,
            "{field}"
        );
    }
    assert_eq!(
        VerifiedManagedGenerationPath::verify_with_admission_status(
            &review_bytes(&admitted, &package),
            &admitted,
            &package,
            "0.32.14".parse().expect("other version"),
            OllamaManagedGenerationAdmissionStatus::Reviewed,
        )
        .expect_err("the expected live runtime version must match"),
        RuntimeAdmissionAuthorityError::GenerationPathBinding
    );
}

#[test]
fn path_requires_exactly_one_backend_conditional_worker() {
    for worker_count in [0, 2] {
        let package = package_with_workers(worker_count);
        let admitted = admitted(&package, b"admission");
        let worker = ArtifactId::from_digest(Digest::sha256(b"worker"));
        let review = review_bytes_for_worker(&admitted, &package, &worker);
        assert_eq!(
            verify_reviewed_path(&review, &admitted, &package)
                .expect_err("worker closure must be exact"),
            RuntimeAdmissionAuthorityError::GenerationPathBinding
        );
    }
}

#[test]
fn path_token_is_bound_to_one_admission_join() {
    let package = package_with_workers(1);
    let first = admitted(&package, b"admission one");
    let other = admitted(&package, b"admission two");
    let review = review_bytes(&first, &package);
    assert_eq!(
        verify_reviewed_path(&review, &other, &package)
            .expect_err("another evidence join cannot reuse the path review"),
        RuntimeAdmissionAuthorityError::GenerationPathBinding
    );
    let path = verify_reviewed_path(&review, &first, &package).expect("first admission path");
    assert!(!path.matches_runtime(
        &other,
        &package,
        version(),
        first.frozen_external_component_set_id()
    ));
}

#[test]
fn six_controls_must_share_foundation_and_managed_record() {
    let package = package_with_workers(1);
    let first = admitted(&package, b"first");
    let other = admitted(&package_with_workers(0), b"other");
    let exact = [
        first.foundation_id(),
        first.foundation_id(),
        first.foundation_id(),
        first.foundation_id(),
        first.foundation_id(),
        first.foundation_id(),
    ];
    validate_control_bindings(first.foundation_id(), exact, true, true, true)
        .expect("six exact control foundations");
    assert_eq!(
        validate_control_bindings(first.foundation_id(), exact, false, true, true)
            .expect_err("managed records differ"),
        RuntimeAdmissionAuthorityError::ControlBinding
    );
    assert_eq!(
        validate_control_bindings(first.foundation_id(), exact, true, false, true)
            .expect_err("managed record names another native record"),
        RuntimeAdmissionAuthorityError::ControlBinding
    );
    assert_eq!(
        validate_control_bindings(first.foundation_id(), exact, true, true, false)
            .expect_err("managed record names another frozen set"),
        RuntimeAdmissionAuthorityError::ControlBinding
    );
    let mixed = [
        first.foundation_id(),
        first.foundation_id(),
        other.foundation_id(),
        first.foundation_id(),
        first.foundation_id(),
        first.foundation_id(),
    ];
    assert_eq!(
        validate_control_bindings(first.foundation_id(), mixed, true, true, true)
            .expect_err("cross-foundation control"),
        RuntimeAdmissionAuthorityError::ControlBinding
    );
}

#[test]
fn schema_two_review_requires_all_pass_admission_and_exact_subjects() {
    let package = package_with_workers(1);
    let other_package = package_with_workers(0);
    let package_id = package.runtime_package_manifest_id();
    let other_package_id = other_package.runtime_package_manifest_id();
    let source_id = artifact_set_id(b"source");
    let other_source_id = artifact_set_id(b"other source");
    validate_review_relationships(
        true,
        true,
        Some(&package_id),
        Some(&package_id),
        &package_id,
        &source_id,
        &source_id,
    )
    .expect("exact all-pass review");
    for (all_passed, admitted_disposition, declared, reconstructed) in [
        (false, true, Some(&package_id), Some(&package_id)),
        (true, false, Some(&package_id), Some(&package_id)),
        (true, true, None, Some(&package_id)),
        (true, true, Some(&package_id), None),
    ] {
        assert_eq!(
            validate_review_relationships(
                all_passed,
                admitted_disposition,
                declared,
                reconstructed,
                &package_id,
                &source_id,
                &source_id,
            )
            .expect_err("incomplete review"),
            RuntimeAdmissionAuthorityError::ReviewNotAdmitted
        );
    }
    for (declared, reconstructed, reviewed_source) in [
        (Some(&other_package_id), Some(&package_id), &source_id),
        (Some(&package_id), Some(&other_package_id), &source_id),
        (Some(&package_id), Some(&package_id), &other_source_id),
    ] {
        assert_eq!(
            validate_review_relationships(
                true,
                true,
                declared,
                reconstructed,
                &package_id,
                reviewed_source,
                &source_id,
            )
            .expect_err("review identity drift"),
            RuntimeAdmissionAuthorityError::ReviewBinding
        );
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the admission join and three substitution regressions share one exact fixture"
)]
fn public_admission_join_consumes_all_six_controls_and_verified_review() {
    let review = review_fixture::verified_review();
    let package = review
        .reconstructed_runtime()
        .expect("admitted fixture")
        .runtime_package();
    let foundation =
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            artifact_set_id(b"durable evidence"),
            review
                .source_build_inputs()
                .artifact_set()
                .artifact_set_id(),
            Digest::sha256(b"source manifest"),
            Digest::sha256(b"build plan"),
            Digest::sha256(b"source report"),
            package.runtime_package_manifest_id(),
        ))
        .expect("foundation");
    let binding = VerifiedRuntimeAdmissionFoundationBinding::test_fixture(&foundation);
    let source_lineage = VerifiedPassedRuntimeAdmissionSourceLineageControl::test_fixture(
        foundation.foundation_id().clone(),
    );
    let transformation = VerifiedPassedRuntimeAdmissionTransformationControl::test_fixture(
        foundation.foundation_id().clone(),
    );
    let license = VerifiedPassedRuntimeAdmissionLicenseControl::test_fixture(
        foundation.foundation_id().clone(),
    );
    let (native, startup, cloud) = crate::runtime_admission_runner::test_execution_control_fixtures(
        foundation.foundation_id().clone(),
        OllamaCloudDisableVersionStatus::Reviewed,
        true,
        true,
    );

    let admitted = VerifiedAdmittedRuntime::verify(
        &binding,
        &source_lineage,
        &transformation,
        &license,
        &native,
        &startup,
        &cloud,
        &review,
    )
    .expect("all-pass admission join");
    let alternate_launch_digest = Digest::sha256(b"alternate launch");
    let (alternate_native, alternate_startup, alternate_cloud) =
        crate::runtime_admission_runner::test_execution_control_fixtures_with_launch(
            foundation.foundation_id().clone(),
            OllamaCloudDisableVersionStatus::Reviewed,
            true,
            true,
            alternate_launch_digest.clone(),
        );
    let alternate_launch_admission = VerifiedAdmittedRuntime::verify(
        &binding,
        &source_lineage,
        &transformation,
        &license,
        &alternate_native,
        &alternate_startup,
        &alternate_cloud,
        &review,
    )
    .expect("alternate exact launch has a distinct admission join");
    assert_eq!(
        alternate_launch_admission.startup_launch_spec_digest(),
        &alternate_launch_digest
    );
    assert_ne!(
        admitted.admitted_runtime_id(),
        alternate_launch_admission.admitted_runtime_id()
    );

    let (_, wrong_native_startup, wrong_native_cloud) =
        crate::runtime_admission_runner::test_execution_control_fixtures(
            foundation.foundation_id().clone(),
            OllamaCloudDisableVersionStatus::Reviewed,
            false,
            true,
        );
    assert_eq!(
        VerifiedAdmittedRuntime::verify(
            &binding,
            &source_lineage,
            &transformation,
            &license,
            &native,
            &wrong_native_startup,
            &wrong_native_cloud,
            &review,
        )
        .expect_err("managed controls cannot substitute another native record"),
        RuntimeAdmissionAuthorityError::ControlBinding
    );
    let (_, wrong_frozen_startup, wrong_frozen_cloud) =
        crate::runtime_admission_runner::test_execution_control_fixtures(
            foundation.foundation_id().clone(),
            OllamaCloudDisableVersionStatus::Reviewed,
            true,
            false,
        );
    assert_eq!(
        VerifiedAdmittedRuntime::verify(
            &binding,
            &source_lineage,
            &transformation,
            &license,
            &native,
            &wrong_frozen_startup,
            &wrong_frozen_cloud,
            &review,
        )
        .expect_err("managed controls cannot substitute another frozen set"),
        RuntimeAdmissionAuthorityError::ControlBinding
    );
    let (_, unreviewed_startup, unreviewed_cloud) =
        crate::runtime_admission_runner::test_execution_control_fixtures(
            foundation.foundation_id().clone(),
            OllamaCloudDisableVersionStatus::Unreviewed,
            true,
            true,
        );
    assert_eq!(
        VerifiedAdmittedRuntime::verify(
            &binding,
            &source_lineage,
            &transformation,
            &license,
            &native,
            &unreviewed_startup,
            &unreviewed_cloud,
            &review,
        )
        .expect_err("unreviewed cloud status cannot become authority"),
        RuntimeAdmissionAuthorityError::CloudDisableUnreviewed
    );

    assert_eq!(admitted.foundation_id(), foundation.foundation_id());
    assert_eq!(
        admitted.startup_launch_spec_digest(),
        startup.startup_launch_spec_digest()
    );
    assert_eq!(
        admitted.frozen_external_component_set_id(),
        native.frozen_external_component_set_id()
    );
    assert_eq!(
        admitted.cloud_disable_version_status(),
        OllamaCloudDisableVersionStatus::Reviewed
    );
    assert_eq!(
        admitted.runtime_package_manifest_id(),
        &package.runtime_package_manifest_id()
    );
    assert_eq!(
        admitted.source_build_inputs_id(),
        &review
            .source_build_inputs()
            .artifact_set()
            .artifact_set_id()
    );
    assert_eq!(
        admitted.package_source_id(),
        &package.source().package_source_id()
    );
    let path_review = review_bytes(&admitted, package);
    let path = verify_reviewed_path(&path_review, &admitted, package)
        .expect("admitted runtime has an exact reviewed generation path");
    assert_eq!(path.admitted_runtime_id(), admitted.admitted_runtime_id());
    assert_eq!(path.runtime_version(), version());
    assert!(format!("{admitted:?}").contains("VerifiedAdmittedRuntime"));
}
