//! App-owned observation authority for the current host environment.

use std::fmt;

#[cfg(any(test, feature = "test-support"))]
use rewrite_model::HostEnvironmentV1Input;
use rewrite_model::{
    HostEnvironmentDigestSetV1, HostEnvironmentId, HostEnvironmentV1, HostEnvironmentV1Error,
    RuntimeTarget,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::GenerationQualificationPlatformAssessmentPolicyV1Bindings;

#[cfg(any(target_os = "linux", test))]
#[path = "current_host_environment/cgroup.rs"]
mod cgroup;
#[cfg(any(target_os = "linux", test))]
#[path = "current_host_environment/parser.rs"]
mod parser;

#[cfg(target_os = "linux")]
#[path = "current_host_environment/linux.rs"]
mod platform;
#[cfg(not(target_os = "linux"))]
#[path = "current_host_environment/unsupported.rs"]
mod platform;

trait CurrentHostEnvironmentSource: Send + Sync {
    fn observe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<HostEnvironmentV1, CurrentHostEnvironmentError>;
}

/// Noncloneable, nonserializable authority for one freshly observed current host.
///
/// The retained source is sampled twice during construction and remains
/// available for fresh revalidation. The authority grants no launch,
/// generation, live-use, qualification, or activation permission.
///
/// ```compile_fail
/// use rewrite_app::VerifiedCurrentHostEnvironment;
///
/// fn clone_authority(value: &VerifiedCurrentHostEnvironment) {
///     let _forged: VerifiedCurrentHostEnvironment = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedCurrentHostEnvironment;
///
/// fn serialize_authority(value: &VerifiedCurrentHostEnvironment) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedCurrentHostEnvironment {
    environment: HostEnvironmentV1,
    runtime_target: RuntimeTarget,
    digest_set: HostEnvironmentDigestSetV1,
    source: Box<dyn CurrentHostEnvironmentSource>,
}

impl VerifiedCurrentHostEnvironment {
    /// Returns the inert, privacy-bounded host environment record.
    #[must_use]
    pub const fn environment(&self) -> &HostEnvironmentV1 {
        &self.environment
    }

    /// Returns the content-derived host environment identity.
    #[must_use]
    pub const fn host_environment_id(&self) -> &HostEnvironmentId {
        self.digest_set.host_environment_id()
    }

    /// Returns the exact observed Linux native target.
    #[must_use]
    pub const fn runtime_target(&self) -> RuntimeTarget {
        self.runtime_target
    }

    /// Returns one inseparable typed view of the exact host identity and projections.
    #[must_use]
    pub const fn digest_set(&self) -> &HostEnvironmentDigestSetV1 {
        &self.digest_set
    }

    /// Freshly reobserves the retained current-host source.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, unavailable or constrained host state,
    /// invalid observations, or any drift from the original exact environment.
    pub fn revalidate_current(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        let initial = self.source.observe(cancellation)?;
        ensure_active(cancellation)?;
        let confirmation = self.source.observe(cancellation)?;
        ensure_active(cancellation)?;
        if initial != confirmation {
            return Err(CurrentHostEnvironmentError::ObservationDrift);
        }
        if initial == self.environment {
            Ok(())
        } else {
            Err(CurrentHostEnvironmentError::ObservationDrift)
        }
    }

    fn observe_from_source(
        source: Box<dyn CurrentHostEnvironmentSource>,
        cancellation: &CancellationToken,
    ) -> Result<Self, CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        let initial = source.observe(cancellation)?;
        ensure_active(cancellation)?;
        let confirmation = source.observe(cancellation)?;
        ensure_active(cancellation)?;
        if initial != confirmation {
            return Err(CurrentHostEnvironmentError::ObservationDrift);
        }
        let runtime_target = RuntimeTarget::new(
            initial.operating_system().family(),
            initial.architecture().instruction_set(),
            initial.architecture().abi(),
        )
        .map_err(|_error| CurrentHostEnvironmentError::InvalidObservation)?;
        let digest_set = initial.digest_set();
        Ok(Self {
            environment: initial,
            runtime_target,
            digest_set,
            source,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_test_fixture(
        input: HostEnvironmentV1Input,
    ) -> Result<Self, CurrentHostEnvironmentError> {
        #[cfg(test)]
        {
            Self::observe_from_source(
                Box::new(tests::FixtureEnvironmentSource::stable(input)?),
                &CancellationToken::new(),
            )
        }
        #[cfg(all(feature = "test-support", not(test)))]
        {
            synthetic_test_support::exact(input)
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[cfg_attr(
        all(feature = "test-support", not(test)),
        expect(dead_code, reason = "reserved for synthetic drift integration tests")
    )]
    pub(crate) fn exact_test_fixture_with_revalidations(
        inputs: Vec<HostEnvironmentV1Input>,
    ) -> Result<Self, CurrentHostEnvironmentError> {
        #[cfg(test)]
        {
            Self::observe_from_source(
                Box::new(tests::FixtureEnvironmentSource::sequence(inputs)?),
                &CancellationToken::new(),
            )
        }
        #[cfg(all(feature = "test-support", not(test)))]
        {
            synthetic_test_support::exact_sequence(inputs)
        }
    }
}

impl fmt::Debug for VerifiedCurrentHostEnvironment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCurrentHostEnvironment")
            .field("schema_version", &self.environment.schema_version())
            .finish_non_exhaustive()
    }
}

/// Local observer for the current process host environment.
#[derive(Clone, Copy, Debug, Default)]
pub struct CurrentHostEnvironmentObserver;

impl CurrentHostEnvironmentObserver {
    /// Observes the current host twice and retains its source for fresh checks.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, unsupported build targets, unavailable
    /// bounded evidence, affinity or cgroup constraints, invalid data, or drift.
    pub fn observe(
        cancellation: &CancellationToken,
    ) -> Result<VerifiedCurrentHostEnvironment, CurrentHostEnvironmentError> {
        VerifiedCurrentHostEnvironment::observe_from_source(
            Box::new(platform::ProductionCurrentHostEnvironmentSource),
            cancellation,
        )
    }
}

impl GenerationQualificationPlatformAssessmentPolicyV1Bindings {
    /// Constructs exact named platform-policy bindings from a verified host.
    #[must_use]
    pub fn from_current_host_environment(host: &VerifiedCurrentHostEnvironment) -> Self {
        let digests = host.digest_set();
        Self {
            runtime_target: host.runtime_target(),
            operating_system_digest: digests.operating_system_digest().digest().clone(),
            architecture_digest: digests.architecture_digest().digest().clone(),
            execution_class_digest: digests.execution_class_digest().digest().clone(),
            hardware_envelope_digest: digests.hardware_envelope_digest().digest().clone(),
        }
    }
}

/// Failure while observing or revalidating the current host environment.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CurrentHostEnvironmentError {
    /// Observation was cancelled at a mandatory checkpoint.
    #[error("current-host environment observation was cancelled")]
    Cancelled,
    /// The compiled operating system, architecture, or ABI is outside V1 scope.
    #[error("current-host environment platform is unsupported")]
    UnsupportedPlatform,
    /// One required bounded local observation was unavailable or ambiguous.
    #[error("current-host environment observation is unavailable")]
    ObservationUnavailable,
    /// CPU affinity or cgroup resource constraints prevent whole-host evidence.
    #[error("current-host environment is resource constrained")]
    ConstrainedEnvironment,
    /// Observed bytes could not form the exact privacy-bounded model record.
    #[error("current-host environment observation is invalid")]
    InvalidObservation,
    /// A confirmation or fresh observation differed from the retained environment.
    #[error("current-host environment observation drifted")]
    ObservationDrift,
}

impl From<HostEnvironmentV1Error> for CurrentHostEnvironmentError {
    fn from(_error: HostEnvironmentV1Error) -> Self {
        Self::InvalidObservation
    }
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), CurrentHostEnvironmentError> {
    if cancellation.is_cancelled() {
        Err(CurrentHostEnvironmentError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "current_host_environment/tests.rs"]
mod tests;

#[cfg(all(feature = "test-support", not(test)))]
#[expect(
    dead_code,
    reason = "the stable source is used now; the sequence is retained for drift tests"
)]
#[path = "current_host_environment/synthetic_test_support.rs"]
mod synthetic_test_support;
