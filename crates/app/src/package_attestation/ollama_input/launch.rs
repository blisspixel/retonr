use std::{
    cell::RefCell,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Instant,
};

use rewrite_model::{
    RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem, RuntimePackageManifest,
};
use rewrite_runtime_attestor::RetainedModelWeight;
use rewrite_runtime_isolation::{
    IsolationError, IsolationEvidence, LaunchSpec, MANAGED_RUNTIME_INPUT_ROOT_V1,
    ManagedLoopbackChannel, PreparedIsolation, PreparedIsolationSubjectToken,
    RetainedIsolationLease,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    ModelLicenseControlId, VerifiedAdmittedRuntime, VerifiedApprovedModelLicenseControl,
    VerifiedManagedOllamaModelPackageLease,
};

use super::super::{
    ManagedOllamaLiveSubjectToken, ManagedOllamaSubjectBinding, PackageAttestationService,
    RuntimePackageIdentityToken, RuntimePackageLease,
};
use super::{
    ManagedOllamaInputEvidence, ManagedOllamaInputPlan, ManagedOllamaModelTarget,
    VerifiedManagedOllamaInputPlan,
};

mod authority;
mod error;
mod finalization;
mod lease;
#[cfg(test)]
mod test_support;
use authority::revalidate_exact_model_authority;
pub use authority::{ManagedOllamaModelAuthorityError, VerifiedManagedOllamaLaunchPlan};
pub use error::{
    ManagedOllamaCloseError, ManagedOllamaLaunchError, ManagedOllamaLaunchFinalizationFailures,
    ManagedOllamaPostAcquisitionFailure,
};
use finalization::finalize_post_acquisition_binding_failure;
#[cfg(test)]
use lease::combine_close_results;

/// Exact target-visible endpoint for the closed Ollama v0.32.15 server profile.
pub const MANAGED_OLLAMA_V0_32_15_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 11_434);

const MANAGED_OLLAMA_EXECUTABLE: &str = "/bin/ollama";
const MANAGED_OLLAMA_ENTRYPOINT: &str = "bin/ollama";
const MANAGED_OLLAMA_RUNTIME_FAMILY: &str = "ollama";
const MANAGED_OLLAMA_VERSION: &str = "0.32.15";

/// Constructs the single closed Ollama v0.32.15 CPU-only server launch profile.
///
/// [`LaunchSpec`] starts with an empty environment, so only the variables listed
/// here reach the server or its worker descendants. Generation launches build a
/// fresh copy internally and do not accept a caller-modifiable specification.
#[must_use]
pub fn managed_ollama_v0_32_15_launch_spec() -> LaunchSpec {
    let mut specification = LaunchSpec::new(MANAGED_OLLAMA_EXECUTABLE);
    specification.push_argument("serve");
    specification.insert_environment("HOME", "/tmp");
    specification.insert_environment("TMPDIR", "/tmp");
    specification.insert_environment("OLLAMA_HOST", "127.0.0.1:11434");
    specification.insert_environment("OLLAMA_MODELS", MANAGED_RUNTIME_INPUT_ROOT_V1);
    specification.insert_environment("OLLAMA_NO_CLOUD", "1");
    specification.insert_environment("OLLAMA_NOPRUNE", "1");
    specification.insert_environment("OLLAMA_NUM_PARALLEL", "1");
    specification.insert_environment("OLLAMA_MAX_LOADED_MODELS", "1");
    specification.insert_environment("OLLAMA_MAX_QUEUE", "1");
    specification.insert_environment("OLLAMA_FLASH_ATTENTION", "0");
    specification.insert_environment("OLLAMA_VULKAN", "0");
    specification.insert_environment("CUDA_VISIBLE_DEVICES", "-1");
    specification.insert_environment("HIP_VISIBLE_DEVICES", "-1");
    specification.insert_environment("ROCR_VISIBLE_DEVICES", "-1");
    specification.insert_environment("GGML_VK_VISIBLE_DEVICES", "-1");
    specification.insert_environment("GPU_DEVICE_ORDINAL", "-1");
    specification
}

/// App-owned retained Ollama process, private input tree, and model-package lease join.
///
/// The raw input tree is consumed during construction and cannot be extracted.
/// The exact retained model weight remains available only by shared reference for
/// the separate worker observer. The concrete originating model-package lease is
/// retained and revalidated after live traffic.
pub struct ManagedOllamaIsolationLease<'lease> {
    isolation: ManagedOllamaRetainedIsolation,
    initial_isolation: Option<IsolationEvidence>,
    plain_launch_spec_digest: Digest,
    input_bound_launch_spec_digest: Digest,
    isolation_policy_digest: Digest,
    input_evidence: ManagedOllamaInputEvidence,
    model_target: ManagedOllamaModelTarget,
    retained_model_weight: RetainedModelWeight<'lease>,
    model_package_lease: &'lease VerifiedManagedOllamaModelPackageLease,
    license: VerifiedApprovedModelLicenseControl<'lease>,
    license_control_id: ModelLicenseControlId,
    subjects: ManagedOllamaSubjectBinding,
    prepared_isolation_subject: PreparedIsolationSubjectToken,
}

enum ManagedOllamaRetainedIsolation {
    Production(RefCell<RetainedIsolationLease>),
    #[cfg(test)]
    SealedFixture,
}

impl PackageAttestationService {
    /// Consumes one verified model-input and license join into the closed server launch.
    ///
    /// The plain launch digest must exactly equal the independently verified digest
    /// carried by `admitted_runtime`. The returned capability retains the concrete
    /// model foundation and consumed license authority through cleanup. It exposes
    /// no raw input tree, file handle, or lifetime-erasing conversion.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaLaunchError`] before launch for any profile, package,
    /// lease, input, or authority mismatch, and after launch if private input
    /// materialization does not bind exactly to the planned layout.
    pub fn launch_managed_ollama_v0_32_15<'lease>(
        package: &RuntimePackageManifest,
        runtime_package_lease: &mut RuntimePackageLease,
        admitted_runtime: &VerifiedAdmittedRuntime,
        isolation: &PreparedIsolation,
        authorization: VerifiedManagedOllamaLaunchPlan<'lease>,
        cancellation: &CancellationToken,
    ) -> Result<ManagedOllamaIsolationLease<'lease>, ManagedOllamaLaunchError> {
        Self::launch_managed_ollama_v0_32_15_with_deadline(
            package,
            runtime_package_lease,
            admitted_runtime,
            isolation,
            authorization,
            cancellation,
            None,
        )
    }

    /// Consumes one verified model-input and license join into the closed server
    /// launch while preserving an already-captured absolute operation deadline.
    ///
    /// Cleanup after any acquired-process failure retains independent fresh
    /// cancellation and shutdown bounds.
    ///
    /// # Errors
    ///
    /// Returns an operation-deadline error at or after the supplied deadline, or
    /// the same authority and isolation errors as
    /// [`Self::launch_managed_ollama_v0_32_15`].
    pub fn launch_managed_ollama_v0_32_15_until<'lease>(
        package: &RuntimePackageManifest,
        runtime_package_lease: &mut RuntimePackageLease,
        admitted_runtime: &VerifiedAdmittedRuntime,
        isolation: &PreparedIsolation,
        authorization: VerifiedManagedOllamaLaunchPlan<'lease>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedOllamaIsolationLease<'lease>, ManagedOllamaLaunchError> {
        Self::launch_managed_ollama_v0_32_15_with_deadline(
            package,
            runtime_package_lease,
            admitted_runtime,
            isolation,
            authorization,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn launch_managed_ollama_v0_32_15_with_deadline<'lease>(
        package: &RuntimePackageManifest,
        runtime_package_lease: &mut RuntimePackageLease,
        admitted_runtime: &VerifiedAdmittedRuntime,
        isolation: &PreparedIsolation,
        authorization: VerifiedManagedOllamaLaunchPlan<'lease>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedOllamaIsolationLease<'lease>, ManagedOllamaLaunchError> {
        ensure_operation_active(cancellation, operation_deadline)?;
        let relationship_result = validate_launch_relationships(
            package,
            runtime_package_lease,
            admitted_runtime,
            &authorization,
        );
        operation_precedence(relationship_result, cancellation, operation_deadline)?;
        let specification = managed_ollama_v0_32_15_launch_spec();
        let plain_launch_spec_digest = specification.redacted_digest();
        let authorization_result = validate_authorized_launch(
            admitted_runtime.startup_launch_spec_digest(),
            &plain_launch_spec_digest,
        );
        operation_precedence(authorization_result, cancellation, operation_deadline)?;
        let executable_result = runtime_package_lease
            .clone_entrypoint_for_launch(cancellation)
            .map_err(ManagedOllamaLaunchError::Package);
        let executable = operation_precedence(executable_result, cancellation, operation_deadline)?;
        let model_result = revalidate_exact_model_authority(
            authorization.package(),
            authorization.license(),
            cancellation,
        )
        .map_err(ManagedOllamaLaunchError::ModelAuthority);
        operation_precedence(model_result, cancellation, operation_deadline)?;
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
        let isolation_result = match operation_deadline {
            Some(deadline) => isolation.launch_retained_with_inputs_until(
                &specification,
                executable,
                tree,
                cancellation,
                deadline,
            ),
            None => isolation.launch_retained_with_inputs(
                &specification,
                executable,
                tree,
                cancellation,
            ),
        }
        .map_err(ManagedOllamaLaunchError::Isolation);
        let isolation_lease = match isolation_result {
            Ok(isolation_lease) => isolation_lease,
            Err(error) => {
                return operation_precedence(Err(error), cancellation, operation_deadline);
            }
        };
        finish_acquired_launch(
            AcquiredManagedOllamaLaunch {
                isolation_lease,
                plain_launch_spec_digest,
                input_evidence,
                model_target,
                retained_model_weight,
                model_package_lease,
                license,
                license_control_id,
            },
            runtime_package_lease,
            isolation,
            cancellation,
            operation_deadline,
        )
    }
}

struct AcquiredManagedOllamaLaunch<'lease> {
    isolation_lease: RetainedIsolationLease,
    plain_launch_spec_digest: Digest,
    input_evidence: ManagedOllamaInputEvidence,
    model_target: ManagedOllamaModelTarget,
    retained_model_weight: RetainedModelWeight<'lease>,
    model_package_lease: &'lease VerifiedManagedOllamaModelPackageLease,
    license: VerifiedApprovedModelLicenseControl<'lease>,
    license_control_id: ModelLicenseControlId,
}

fn finish_acquired_launch<'lease>(
    acquired: AcquiredManagedOllamaLaunch<'lease>,
    runtime_package: &mut RuntimePackageLease,
    prepared_isolation: &PreparedIsolation,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<ManagedOllamaIsolationLease<'lease>, ManagedOllamaLaunchError> {
    let AcquiredManagedOllamaLaunch {
        isolation_lease,
        plain_launch_spec_digest,
        input_evidence,
        model_target,
        retained_model_weight,
        model_package_lease,
        license,
        license_control_id,
    } = acquired;
    let initial = isolation_lease.initial_evidence();
    let binding_result = validate_isolation_input_binding(
        &initial,
        isolation_lease.launch_spec_digest(),
        &plain_launch_spec_digest,
        &input_evidence,
    );
    if let Some(primary) = finalization::post_acquisition_failure(
        binding_result.is_err(),
        cancellation.is_cancelled(),
        operation_deadline,
        Instant::now(),
    ) {
        return Err(finalize_post_acquisition_by_primary(
            primary,
            isolation_lease,
            model_package_lease,
            &license,
            runtime_package,
        ));
    }
    let input_bound_launch_spec_digest = isolation_lease.launch_spec_digest().clone();
    let isolation_policy_digest = isolation_lease.isolation_policy_digest().clone();
    let subjects = ManagedOllamaSubjectBinding::new(
        runtime_package.identity_token(),
        model_package_lease.identity_token(),
    );
    let prepared_isolation_subject = prepared_isolation.subject_token();
    if let Some(primary) = finalization::post_acquisition_failure(
        false,
        cancellation.is_cancelled(),
        operation_deadline,
        Instant::now(),
    ) {
        return Err(finalize_post_acquisition_by_primary(
            primary,
            isolation_lease,
            model_package_lease,
            &license,
            runtime_package,
        ));
    }
    Ok(ManagedOllamaIsolationLease {
        input_bound_launch_spec_digest,
        isolation_policy_digest,
        isolation: ManagedOllamaRetainedIsolation::Production(RefCell::new(isolation_lease)),
        initial_isolation: Some(initial),
        plain_launch_spec_digest,
        input_evidence,
        model_target,
        retained_model_weight,
        model_package_lease,
        license,
        license_control_id,
        subjects,
        prepared_isolation_subject,
    })
}

fn finalize_post_acquisition_by_primary(
    primary: ManagedOllamaPostAcquisitionFailure,
    isolation: RetainedIsolationLease,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    license: &VerifiedApprovedModelLicenseControl<'_>,
    runtime_package: &mut RuntimePackageLease,
) -> ManagedOllamaLaunchError {
    match primary {
        ManagedOllamaPostAcquisitionFailure::InputBindingMismatch => {
            finalize_post_acquisition_binding_failure(
                isolation,
                model_package,
                license,
                runtime_package,
            )
        }
        ManagedOllamaPostAcquisitionFailure::Cancelled => {
            finalization::finalize_post_acquisition_cancellation(
                isolation,
                model_package,
                license,
                runtime_package,
            )
        }
        ManagedOllamaPostAcquisitionFailure::DeadlineExceeded => {
            finalization::finalize_post_acquisition_deadline(
                isolation,
                model_package,
                license,
                runtime_package,
            )
        }
    }
}

fn ensure_operation_active(
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<(), ManagedOllamaLaunchError> {
    if operation_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err(ManagedOllamaLaunchError::Isolation(
            IsolationError::OperationDeadlineExceeded,
        ))
    } else if cancellation.is_cancelled() {
        Err(ManagedOllamaLaunchError::Isolation(
            IsolationError::Cancelled,
        ))
    } else {
        Ok(())
    }
}

fn operation_precedence<T>(
    result: Result<T, ManagedOllamaLaunchError>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<T, ManagedOllamaLaunchError> {
    if operation_deadline.is_none() {
        return result;
    }
    ensure_operation_active(cancellation, operation_deadline)?;
    result
}

fn mandatory_final_cancellation(_operation: &CancellationToken) -> CancellationToken {
    CancellationToken::new()
}

fn validate_launch_relationships(
    package: &RuntimePackageManifest,
    runtime_package_lease: &RuntimePackageLease,
    admitted_runtime: &VerifiedAdmittedRuntime,
    authorization: &VerifiedManagedOllamaLaunchPlan<'_>,
) -> Result<(), ManagedOllamaLaunchError> {
    let target = package.target();
    let runtime_evidence = runtime_package_lease.evidence();
    let model_package_lease = authorization.package();
    let plan = authorization.input();
    let plan_evidence = plan.evidence();
    if package.runtime_family() != MANAGED_OLLAMA_RUNTIME_FAMILY
        || package.reported_version() != MANAGED_OLLAMA_VERSION
        || target.operating_system() != RuntimeOperatingSystem::Linux
        || target.architecture() != RuntimeArchitecture::X86_64
        || target.abi() != RuntimeAbi::LinuxGnuLibc
        || package.entrypoint().relative_path().as_str() != MANAGED_OLLAMA_ENTRYPOINT
        || package.artifact_set_id() != runtime_evidence.artifact_set_id()
        || package.runtime_package_manifest_id() != *runtime_evidence.runtime_package_manifest_id()
        || package.entrypoint().artifact_id() != runtime_evidence.entrypoint_artifact_id()
        || admitted_runtime.runtime_package_manifest_id()
            != runtime_evidence.runtime_package_manifest_id()
        || model_package_lease.artifact_set_id() != plan_evidence.artifact_set_id()
        || model_package_lease.model_package_manifest_id()
            != plan_evidence.model_package_manifest_id()
        || model_package_lease.private_view().installation_generation()
            != plan_evidence.installation_generation()
        || plan.model_target().artifact_id() != plan_evidence.model_artifact_id()
        || plan.model_target().target_digest() != plan_evidence.model_target_digest()
        || plan.retained_model_weight().artifact_id() != plan_evidence.model_artifact_id()
    {
        return Err(ManagedOllamaLaunchError::RelationshipMismatch);
    }
    Ok(())
}

fn validate_authorized_launch(
    admitted_plain_launch_spec_digest: &Digest,
    closed_plain_launch_spec_digest: &Digest,
) -> Result<(), ManagedOllamaLaunchError> {
    if admitted_plain_launch_spec_digest == closed_plain_launch_spec_digest {
        Ok(())
    } else {
        Err(ManagedOllamaLaunchError::UnauthorizedLaunch)
    }
}

fn validate_isolation_input_binding(
    isolation: &IsolationEvidence,
    input_bound_launch_spec_digest: &Digest,
    plain_launch_spec_digest: &Digest,
    planned: &ManagedOllamaInputEvidence,
) -> Result<(), ManagedOllamaLaunchError> {
    let observed = isolation.runtime_inputs();
    if observed.is_empty()
        || observed.member_count() != planned.member_count()
        || observed.total_bytes() != planned.total_bytes()
        || observed.layout_digest() != planned.input_layout_digest()
        || &observed.input_bound_launch_digest(plain_launch_spec_digest)
            != input_bound_launch_spec_digest
    {
        return Err(ManagedOllamaLaunchError::IsolationInputMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "launch/tests.rs"]
mod tests;

#[cfg(test)]
mod deadline_tests {
    use std::time::{Duration, Instant};

    use rewrite_runtime_isolation::IsolationError;
    use rewrite_types::CancellationToken;

    use super::{ManagedOllamaLaunchError, ensure_operation_active, operation_precedence};

    #[test]
    fn app_deadline_is_open_only_strictly_before_it() {
        let cancellation = CancellationToken::new();
        assert!(
            ensure_operation_active(&cancellation, Some(Instant::now() + Duration::from_secs(1)),)
                .is_ok()
        );
        for deadline in [
            Instant::now(),
            Instant::now()
                .checked_sub(Duration::from_nanos(1))
                .expect("test instant supports one-nanosecond subtraction"),
        ] {
            assert!(matches!(
                ensure_operation_active(&cancellation, Some(deadline)),
                Err(ManagedOllamaLaunchError::Isolation(
                    IsolationError::OperationDeadlineExceeded
                ))
            ));
        }
    }

    #[test]
    fn expired_deadline_precedes_cancellation() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            ensure_operation_active(&cancellation, Some(Instant::now())),
            Err(ManagedOllamaLaunchError::Isolation(
                IsolationError::OperationDeadlineExceeded
            ))
        ));
    }

    #[test]
    fn cancellation_precedes_other_post_boundary_error_before_deadline() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            operation_precedence::<()>(
                Err(ManagedOllamaLaunchError::UnauthorizedLaunch),
                &cancellation,
                Some(Instant::now() + Duration::from_secs(1)),
            ),
            Err(ManagedOllamaLaunchError::Isolation(
                IsolationError::Cancelled
            ))
        ));
    }
}
