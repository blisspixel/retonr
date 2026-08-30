use std::fmt;

use crate::ModelPackageIdentityToken;

use super::{
    CancellationToken, Digest, Instant, IsolationError, IsolationEvidence,
    MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedLoopbackChannel, ManagedOllamaCloseError,
    ManagedOllamaInputEvidence, ManagedOllamaIsolationLease, ManagedOllamaLaunchError,
    ManagedOllamaLiveSubjectToken, ManagedOllamaModelAuthorityError, ManagedOllamaModelTarget,
    ManagedOllamaRetainedIsolation, ModelLicenseControlId, PreparedIsolation, RetainedModelWeight,
    RuntimePackageIdentityToken, RuntimePackageLease, VerifiedManagedOllamaModelPackageLease,
    ensure_operation_active, mandatory_final_cancellation, operation_precedence,
    revalidate_exact_model_authority, validate_isolation_input_binding,
};
impl<'lease> ManagedOllamaIsolationLease<'lease> {
    /// Returns the independently authorized plain closed-launch digest.
    #[must_use]
    pub const fn plain_launch_spec_digest(&self) -> &Digest {
        &self.plain_launch_spec_digest
    }

    /// Returns the input-bound composite digest retained by isolation.
    #[must_use]
    pub const fn input_bound_launch_spec_digest(&self) -> &Digest {
        &self.input_bound_launch_spec_digest
    }

    /// Returns the exact retained isolation-policy digest.
    #[must_use]
    pub const fn isolation_policy_digest(&self) -> &Digest {
        &self.isolation_policy_digest
    }

    /// Returns redacted model-package and private-input binding evidence.
    #[must_use]
    pub const fn input_evidence(&self) -> &ManagedOllamaInputEvidence {
        &self.input_evidence
    }

    /// Returns the exact target-visible GGUF identity and path binding.
    #[must_use]
    pub const fn model_target(&self) -> &ManagedOllamaModelTarget {
        &self.model_target
    }

    /// Returns the exact retained GGUF object for the separate worker observer.
    #[must_use]
    pub const fn retained_model_weight(&self) -> &RetainedModelWeight<'lease> {
        &self.retained_model_weight
    }

    /// Returns the exact approved model-license control retained through cleanup.
    #[must_use]
    pub const fn model_license_control_id(&self) -> &ModelLicenseControlId {
        &self.license_control_id
    }

    pub(crate) fn binds_exact_model_package(
        &self,
        selected: &VerifiedManagedOllamaModelPackageLease,
    ) -> bool {
        std::ptr::eq(self.model_package_lease, selected)
    }

    pub(crate) fn live_subject_token(&self) -> ManagedOllamaLiveSubjectToken {
        self.subjects.live_token()
    }

    pub(crate) fn runtime_package_identity_token(&self) -> RuntimePackageIdentityToken {
        self.subjects.runtime_token()
    }

    pub(crate) fn model_package_identity_token(&self) -> ModelPackageIdentityToken {
        self.subjects.model_token()
    }

    pub(crate) fn binds_model_package_identity_token(
        &self,
        token: &ModelPackageIdentityToken,
    ) -> bool {
        self.subjects.model_token_ref().ptr_eq(token)
    }

    pub(crate) fn binds_live_subject(&self, token: &ManagedOllamaLiveSubjectToken) -> bool {
        self.subjects.binds_live(token)
    }

    pub(crate) fn binds_exact_runtime_package(&self, package: &RuntimePackageLease) -> bool {
        package.binds_identity_token(self.subjects.runtime_token_ref())
    }

    pub(crate) fn binds_exact_prepared_isolation(&self, prepared: &PreparedIsolation) -> bool {
        self.prepared_isolation_subject.binds_exact(prepared)
    }

    /// Revalidates the concrete originating model-package lease without exposing it.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaLaunchError`] for cancellation or canonical model-tree
    /// drift. This narrow operation supports a check immediately after generation,
    /// before any later worker or final-state observation.
    pub fn revalidate_model_package(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedOllamaLaunchError> {
        revalidate_exact_model_authority(self.model_package_lease, &self.license, cancellation)
            .map_err(ManagedOllamaLaunchError::ModelAuthority)
    }

    /// Revalidates the concrete model package under an already-captured deadline.
    ///
    /// # Errors
    ///
    /// Returns an operation-deadline error at or after the supplied deadline, or
    /// the same authority errors as [`Self::revalidate_model_package`].
    pub fn revalidate_model_package_until(
        &self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<(), ManagedOllamaLaunchError> {
        ensure_operation_active(cancellation, Some(operation_deadline))?;
        let result = self.revalidate_model_package(cancellation);
        operation_precedence(result, cancellation, Some(operation_deadline))
    }

    /// Returns launch-time isolation evidence after its private input join passed.
    ///
    /// # Panics
    ///
    /// Panics only for the sealed test fixture, which deliberately has no live
    /// isolation evidence and must not call this production observer accessor.
    #[must_use]
    pub fn initial_evidence(&self) -> IsolationEvidence {
        self.initial_isolation
            .clone()
            .expect("production managed isolation retains initial evidence")
    }

    /// Opens the only allowed server channel at `127.0.0.1:11434`.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaLaunchError`] on cancellation, duplicate use,
    /// target exit, timeout, or namespace-local connection failure.
    pub fn connect_loopback(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedLoopbackChannel, ManagedOllamaLaunchError> {
        self.connect_loopback_with_deadline(cancellation, None)
    }

    /// Opens the only allowed server channel under an already-captured deadline.
    ///
    /// # Errors
    ///
    /// Returns an operation-deadline error at or after the deadline, or the same
    /// retained-channel errors as [`Self::connect_loopback`].
    pub fn connect_loopback_until(
        &self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedLoopbackChannel, ManagedOllamaLaunchError> {
        self.connect_loopback_with_deadline(cancellation, Some(operation_deadline))
    }

    fn connect_loopback_with_deadline(
        &self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedLoopbackChannel, ManagedOllamaLaunchError> {
        let result = (|| match &self.isolation {
            ManagedOllamaRetainedIsolation::Production(isolation) => {
                let mut isolation = isolation
                    .try_borrow_mut()
                    .map_err(|_error| ManagedOllamaLaunchError::CapabilityBusy)?;
                match operation_deadline {
                    Some(deadline) => isolation.connect_loopback_until(
                        MANAGED_OLLAMA_V0_32_15_ENDPOINT,
                        cancellation,
                        deadline,
                    ),
                    None => {
                        isolation.connect_loopback(MANAGED_OLLAMA_V0_32_15_ENDPOINT, cancellation)
                    }
                }
                .map_err(ManagedOllamaLaunchError::Isolation)
            }
            #[cfg(test)]
            ManagedOllamaRetainedIsolation::SealedFixture => Err(
                ManagedOllamaLaunchError::Isolation(IsolationError::UnsupportedPlatform),
            ),
        })();
        operation_precedence(result, cancellation, operation_deadline)
    }

    /// Revalidates process isolation, the private input mapping, and the concrete
    /// originating model-package lease.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaLaunchError`] for cancellation, process or input
    /// drift, canonical model-tree drift, or a lost retained capability.
    pub fn reobserve(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<IsolationEvidence, ManagedOllamaLaunchError> {
        self.reobserve_with_deadline(cancellation, None)
    }

    /// Revalidates isolation and model authority under an absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns an operation-deadline error at or after the deadline, or the same
    /// live-evidence errors as [`Self::reobserve`].
    pub fn reobserve_until(
        &self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<IsolationEvidence, ManagedOllamaLaunchError> {
        self.reobserve_with_deadline(cancellation, Some(operation_deadline))
    }

    fn reobserve_with_deadline(
        &self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<IsolationEvidence, ManagedOllamaLaunchError> {
        ensure_operation_active(cancellation, operation_deadline)?;
        let result = (|| {
            let evidence = match &self.isolation {
                ManagedOllamaRetainedIsolation::Production(isolation) => {
                    let mut isolation = isolation
                        .try_borrow_mut()
                        .map_err(|_error| ManagedOllamaLaunchError::CapabilityBusy)?;
                    match operation_deadline {
                        Some(deadline) => isolation.reobserve_until(cancellation, deadline),
                        None => isolation.reobserve(cancellation),
                    }
                    .map_err(ManagedOllamaLaunchError::Isolation)?
                }
                #[cfg(test)]
                ManagedOllamaRetainedIsolation::SealedFixture => {
                    return Err(ManagedOllamaLaunchError::Isolation(
                        IsolationError::UnsupportedPlatform,
                    ));
                }
            };
            validate_isolation_input_binding(
                &evidence,
                &self.input_bound_launch_spec_digest,
                &self.plain_launch_spec_digest,
                &self.input_evidence,
            )?;
            revalidate_exact_model_authority(self.model_package_lease, &self.license, cancellation)
                .map_err(ManagedOllamaLaunchError::ModelAuthority)?;
            Ok(evidence)
        })();
        operation_precedence(result, cancellation, operation_deadline)
    }

    /// Terminates the process tree, then independently revalidates the originating
    /// model authority before releasing its retained weight and lifetime binding.
    /// A fresh internal cancellation state makes both final operations mandatory
    /// even when the caller's operation token is already cancelled.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaCloseError`] without suppressing either cleanup or
    /// final package-revalidation failure.
    pub fn close(
        self,
        operation_cancellation: &CancellationToken,
    ) -> Result<(), ManagedOllamaCloseError> {
        let Self {
            isolation,
            model_package_lease,
            license,
            retained_model_weight,
            ..
        } = self;
        let final_cancellation = mandatory_final_cancellation(operation_cancellation);
        let isolation_result = match isolation {
            ManagedOllamaRetainedIsolation::Production(isolation) => {
                (*isolation).into_inner().close(&final_cancellation)
            }
            #[cfg(test)]
            ManagedOllamaRetainedIsolation::SealedFixture => Ok(()),
        };
        let authority_result =
            revalidate_exact_model_authority(model_package_lease, &license, &final_cancellation);
        drop(retained_model_weight);
        combine_close_results(isolation_result, authority_result)
    }
}

pub(super) fn combine_close_results(
    isolation_result: Result<(), IsolationError>,
    authority_result: Result<(), ManagedOllamaModelAuthorityError>,
) -> Result<(), ManagedOllamaCloseError> {
    match (isolation_result, authority_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(isolation), Ok(())) => Err(ManagedOllamaCloseError::Isolation(isolation)),
        (Ok(()), Err(authority)) => Err(ManagedOllamaCloseError::ModelAuthority(authority)),
        (Err(isolation), Err(authority)) => {
            Err(ManagedOllamaCloseError::IsolationAndModelAuthority {
                isolation,
                authority,
            })
        }
    }
}

impl fmt::Debug for ManagedOllamaIsolationLease<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedOllamaIsolationLease")
            .field("plain_launch_spec_digest", &self.plain_launch_spec_digest)
            .field(
                "input_bound_launch_spec_digest",
                &self.input_bound_launch_spec_digest,
            )
            .field("input_evidence", &self.input_evidence)
            .field("model_target", &self.model_target)
            .field("license_control_id", &self.license_control_id)
            .finish_non_exhaustive()
    }
}
