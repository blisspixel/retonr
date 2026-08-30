use rewrite_ollama_package::{
    CargoSourceClosureError, RetainedProgramLicenseEvidenceError,
    RetainedProgramUpstreamClosureError, RuntimeSourceBuildPlan, VerifiedCargoSourceClosure,
    VerifiedRetainedProgramLicenseEvidenceClosure, VerifiedRetainedProgramUpstreamClosure,
};
use rewrite_runtime_isolation::RetainedProgramBootstrapExecution;
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use crate::{
    RuntimeSourceBuildBundleError, RuntimeSourceBuildBundleLease, RuntimeSourceBuildExecutionError,
    RuntimeSourceBuildOutputSource,
    artifact_storage::ManagedTreeLimits,
    runtime_source_build_execution::{
        MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES, output::PinnedRuntimeSourceBuildOutput,
    },
};

mod establish;
#[cfg(target_os = "linux")]
mod linux;
pub use establish::RetainedProgramExecutableClosureVerifier;

/// Domain-separated identity of one complete static and live retained-program closure.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetainedProgramExecutableClosureId(Digest);

impl RetainedProgramExecutableClosureId {
    /// Returns the digest defining this exact live closure identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Two caller-selected empty output directories for independent bootstrap attempts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedProgramBootstrapOutputSources {
    primary: RuntimeSourceBuildOutputSource,
    rebuild: RuntimeSourceBuildOutputSource,
}

impl RetainedProgramBootstrapOutputSources {
    /// Groups distinct primary and rebuild output selections.
    #[must_use]
    pub const fn new(
        primary: RuntimeSourceBuildOutputSource,
        rebuild: RuntimeSourceBuildOutputSource,
    ) -> Self {
        Self { primary, rebuild }
    }

    /// Returns the primary bootstrap output selection.
    #[must_use]
    pub const fn primary(&self) -> &RuntimeSourceBuildOutputSource {
        &self.primary
    }

    /// Returns the rebuild bootstrap output selection.
    #[must_use]
    pub const fn rebuild(&self) -> &RuntimeSourceBuildOutputSource {
        &self.rebuild
    }
}

/// Failure to establish the complete retained-program executable closure.
#[derive(Debug, Error)]
pub enum RetainedProgramExecutableClosureError {
    /// The retained bundle changed or could not provide an exact capability.
    #[error(transparent)]
    Bundle(#[from] RuntimeSourceBuildBundleError),
    /// The complete Cargo source closure did not verify.
    #[error(transparent)]
    Cargo(#[from] CargoSourceClosureError),
    /// The complete mechanical license-material closure did not verify.
    #[error(transparent)]
    License(#[from] RetainedProgramLicenseEvidenceError),
    /// A bootstrap output boundary or execution failed.
    #[error(transparent)]
    Execution(#[from] RuntimeSourceBuildExecutionError),
    /// The authenticated Rust and Alpine upstream closure did not verify.
    #[error(transparent)]
    Upstream(#[from] RetainedProgramUpstreamClosureError),
    /// Static and live closure identities did not join exactly.
    #[error("retained-program static and live closures do not match")]
    ClosureMismatch,
}

/// Opaque application-owned authority for fresh controlled runtime builds.
///
/// This type is intentionally neither cloneable nor serializable. Its authority
/// exists only while it retains the exact bundle, all three strong static
/// closures, both live bootstrap executions, and both independently sealed
/// bootstrap output roots.
pub struct ExecutableRuntimeSourceBuildBundleLease {
    bundle: RuntimeSourceBuildBundleLease,
    plan: RuntimeSourceBuildPlan,
    closure_id: RetainedProgramExecutableClosureId,
    authority: ExecutableClosureAuthority,
}

enum ExecutableClosureAuthority {
    #[cfg_attr(
        not(target_os = "linux"),
        expect(
            dead_code,
            reason = "production construction is intentionally Linux-only"
        )
    )]
    Production {
        cargo: VerifiedCargoSourceClosure,
        license: VerifiedRetainedProgramLicenseEvidenceClosure,
        upstream: VerifiedRetainedProgramUpstreamClosure,
        primary: BootstrapAttemptLease,
        rebuild: BootstrapAttemptLease,
    },
    #[cfg(all(test, target_os = "linux"))]
    InertFixture,
}

struct BootstrapAttemptLease {
    execution: RetainedProgramBootstrapExecution,
    output: PinnedRuntimeSourceBuildOutput,
}

impl ExecutableRuntimeSourceBuildBundleLease {
    /// Returns the exact live retained-program closure identity.
    #[must_use]
    pub const fn closure_id(&self) -> &RetainedProgramExecutableClosureId {
        &self.closure_id
    }

    /// Returns the inert build description bound to this live closure.
    #[must_use]
    pub const fn plan(&self) -> &RuntimeSourceBuildPlan {
        &self.plan
    }

    /// Returns mechanical Cargo source-closure reviewer facts.
    #[must_use]
    pub const fn cargo_closure(&self) -> &VerifiedCargoSourceClosure {
        match &self.authority {
            ExecutableClosureAuthority::Production { cargo, .. } => cargo,
            #[cfg(all(test, target_os = "linux"))]
            ExecutableClosureAuthority::InertFixture => {
                panic!("an inert test fixture has no Cargo closure")
            }
        }
    }

    /// Returns mechanical license-material closure reviewer facts.
    #[must_use]
    pub const fn license_closure(&self) -> &VerifiedRetainedProgramLicenseEvidenceClosure {
        match &self.authority {
            ExecutableClosureAuthority::Production { license, .. } => license,
            #[cfg(all(test, target_os = "linux"))]
            ExecutableClosureAuthority::InertFixture => {
                panic!("an inert test fixture has no license closure")
            }
        }
    }

    /// Returns authenticated upstream reviewer facts.
    #[must_use]
    pub const fn upstream_closure(&self) -> &VerifiedRetainedProgramUpstreamClosure {
        match &self.authority {
            ExecutableClosureAuthority::Production { upstream, .. } => upstream,
            #[cfg(all(test, target_os = "linux"))]
            ExecutableClosureAuthority::InertFixture => {
                panic!("an inert test fixture has no upstream closure")
            }
        }
    }

    /// Returns the primary live bootstrap result.
    #[must_use]
    pub const fn primary_bootstrap(&self) -> &RetainedProgramBootstrapExecution {
        match &self.authority {
            ExecutableClosureAuthority::Production { primary, .. } => &primary.execution,
            #[cfg(all(test, target_os = "linux"))]
            ExecutableClosureAuthority::InertFixture => {
                panic!("an inert test fixture has no bootstrap execution")
            }
        }
    }

    /// Returns the independent rebuild live bootstrap result.
    #[must_use]
    pub const fn rebuild_bootstrap(&self) -> &RetainedProgramBootstrapExecution {
        match &self.authority {
            ExecutableClosureAuthority::Production { rebuild, .. } => &rebuild.execution,
            #[cfg(all(test, target_os = "linux"))]
            ExecutableClosureAuthority::InertFixture => {
                panic!("an inert test fixture has no bootstrap execution")
            }
        }
    }

    /// Revalidates every retained filesystem boundary without releasing authority.
    ///
    /// # Errors
    ///
    /// Returns [`RetainedProgramExecutableClosureError`] for cancellation or any
    /// bundle or bootstrap-output identity drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RetainedProgramExecutableClosureError> {
        self.revalidate_for_execution(cancellation)?;
        Ok(())
    }

    pub(crate) fn revalidate_for_execution(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildExecutionError> {
        self.bundle.revalidate(cancellation)?;
        match &self.authority {
            ExecutableClosureAuthority::Production {
                primary, rebuild, ..
            } => {
                let limits = ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES)
                    .map_err(|_| RuntimeSourceBuildExecutionError::PlanMismatch)?;
                primary.output.validate_sealed(limits, cancellation)?;
                rebuild.output.validate_sealed(limits, cancellation)?;
            }
            #[cfg(all(test, target_os = "linux"))]
            ExecutableClosureAuthority::InertFixture => {}
        }
        self.bundle.revalidate(cancellation)?;
        Ok(())
    }

    pub(crate) const fn bundle(&self) -> &RuntimeSourceBuildBundleLease {
        &self.bundle
    }

    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn from_inert_bundle_for_test(bundle: RuntimeSourceBuildBundleLease) -> Self {
        let plan = bundle.plan().clone();
        Self {
            bundle,
            plan,
            closure_id: RetainedProgramExecutableClosureId(Digest::sha256(
                b"retonr/inert-runtime-source-build-test-fixture/v1",
            )),
            authority: ExecutableClosureAuthority::InertFixture,
        }
    }
}

impl From<rewrite_runtime_isolation::IsolationError> for RetainedProgramExecutableClosureError {
    fn from(error: rewrite_runtime_isolation::IsolationError) -> Self {
        Self::Execution(RuntimeSourceBuildExecutionError::Isolation(error))
    }
}

#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    use super::*;

    #[test]
    fn output_pair_preserves_order() {
        let primary = RuntimeSourceBuildOutputSource::new("bootstrap-primary").expect("primary");
        let rebuild = RuntimeSourceBuildOutputSource::new("bootstrap-rebuild").expect("rebuild");
        let outputs = RetainedProgramBootstrapOutputSources::new(primary.clone(), rebuild.clone());
        assert_eq!(outputs.primary(), &primary);
        assert_eq!(outputs.rebuild(), &rebuild);
    }
}
