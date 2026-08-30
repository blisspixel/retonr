use super::*;

impl PackageAttestationService {
    pub(crate) fn managed_ollama_test_fixture<'lease>(
        package: &RuntimePackageManifest,
        runtime_package_lease: &RuntimePackageLease,
        admitted_runtime: &VerifiedAdmittedRuntime,
        isolation: &PreparedIsolation,
        authorization: VerifiedManagedOllamaLaunchPlan<'lease>,
    ) -> Result<ManagedOllamaIsolationLease<'lease>, ManagedOllamaLaunchError> {
        validate_launch_relationships(
            package,
            runtime_package_lease,
            admitted_runtime,
            &authorization,
        )?;
        revalidate_exact_model_authority(
            authorization.package(),
            authorization.license(),
            &CancellationToken::new(),
        )
        .map_err(ManagedOllamaLaunchError::ModelAuthority)?;
        let (model_package_lease, verified_input, license, license_control_id) =
            authorization.consume();
        let VerifiedManagedOllamaInputPlan { plan, .. } = verified_input;
        let ManagedOllamaInputPlan {
            tree,
            evidence: input_evidence,
            model_target,
            retained_model_weight,
            _lease,
        } = plan;
        drop(tree);
        let plain_launch_spec_digest = managed_ollama_v0_32_15_launch_spec().redacted_digest();
        Ok(ManagedOllamaIsolationLease {
            isolation: ManagedOllamaRetainedIsolation::SealedFixture,
            initial_isolation: None,
            input_bound_launch_spec_digest: Digest::sha256(
                b"sealed-managed-judge-test-input-bound-launch",
            ),
            isolation_policy_digest: isolation.policy_digest(),
            plain_launch_spec_digest,
            input_evidence,
            model_target,
            retained_model_weight,
            model_package_lease,
            license,
            license_control_id,
            subjects: ManagedOllamaSubjectBinding::new(
                runtime_package_lease.identity_token(),
                model_package_lease.identity_token(),
            ),
            prepared_isolation_subject: isolation.subject_token(),
        })
    }
}
