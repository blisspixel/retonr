#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use rewrite_ollama_package::{
    CargoSourceClosureLimits, RetainedProgramLicenseEvidenceLimits,
    RuntimeSourceBuildInputOpenError, RuntimeSourceBuildPlan, verify_cargo_source_closure,
    verify_retained_program_license_evidence_closure, verify_retained_program_upstream_closure,
};
#[cfg(target_os = "linux")]
use rewrite_runtime_isolation::{
    AuthenticatedBusyboxExecutable, ControlledBuildInputFile, ControlledBuildLaunchSpec,
    ControlledBuildOutputTree, IsolationPolicy, PreparedIsolation, RetainedProgramBootstrapAttempt,
    RetainedProgramBootstrapCapabilities, RetainedProgramBootstrapInputKind,
    RetainedProgramBootstrapInputMeasurement, RetainedProgramBootstrapLaunchSpec,
    RetainedProgramBootstrapSignedInputs,
};
use rewrite_types::CancellationToken;
#[cfg(target_os = "linux")]
use rewrite_types::Digest;

#[cfg(target_os = "linux")]
use super::linux::{compile_closure_id, expected_program_tree, validate_live_pair};
#[cfg(target_os = "linux")]
use super::{
    BootstrapAttemptLease, ExecutableClosureAuthority, PinnedRuntimeSourceBuildOutput,
    ProductionExecutableClosureAuthority,
};
use super::{
    ExecutableRuntimeSourceBuildBundleLease, RetainedProgramBootstrapOutputSources,
    RetainedProgramExecutableClosureError,
};
use crate::{RuntimeSourceBuildBundleLease, RuntimeSourceBuildExecutionError};
#[cfg(target_os = "linux")]
use crate::{RuntimeSourceBuildOutputSource, runtime_source_build_execution::paths_overlap};

#[cfg(target_os = "linux")]
const RECIPE_PATH: &str = "lineage/build-recipe-v2.json";
#[cfg(target_os = "linux")]
const BUSYBOX_PATH: &str = "toolchains/busybox";
#[cfg(target_os = "linux")]
const SIGNED_INPUT_KINDS: [RetainedProgramBootstrapInputKind; 13] = [
    RetainedProgramBootstrapInputKind::AlpineMinirootfs,
    RetainedProgramBootstrapInputKind::AlpineMinirootfsSignature,
    RetainedProgramBootstrapInputKind::AlpineMinirootfsChecksum,
    RetainedProgramBootstrapInputKind::AlpineReleaseTrustRoot,
    RetainedProgramBootstrapInputKind::AlpineLibgccPackage,
    RetainedProgramBootstrapInputKind::AlpineBusyboxStaticPackage,
    RetainedProgramBootstrapInputKind::RustChannelManifest,
    RetainedProgramBootstrapInputKind::RustChannelManifestSignature,
    RetainedProgramBootstrapInputKind::RustChannelManifestChecksum,
    RetainedProgramBootstrapInputKind::RustReleaseTrustRoot,
    RetainedProgramBootstrapInputKind::RustCargoDistribution,
    RetainedProgramBootstrapInputKind::RustCompilerDistribution,
    RetainedProgramBootstrapInputKind::RustStandardLibraryDistribution,
];

/// Application-owned verifier and live bootstrap orchestrator.
#[derive(Clone, Copy, Debug, Default)]
pub struct RetainedProgramExecutableClosureVerifier;

impl RetainedProgramExecutableClosureVerifier {
    /// Verifies all static closures and executes two independent Linux bootstraps.
    ///
    /// The returned opaque lease is the only fresh-build authority. Canonical
    /// JSON, static closure results, bootstrap digests, and build plans remain
    /// inert when separated from this live value.
    ///
    /// # Errors
    ///
    /// Returns [`RetainedProgramExecutableClosureError`] for any byte drift,
    /// incomplete static closure, unsafe output boundary, unavailable isolation,
    /// failed root postcondition, or non-identical four-program rebuild.
    #[cfg(target_os = "linux")]
    pub fn establish(
        bundle: RuntimeSourceBuildBundleLease,
        outputs: &RetainedProgramBootstrapOutputSources,
        cancellation: &CancellationToken,
    ) -> Result<ExecutableRuntimeSourceBuildBundleLease, RetainedProgramExecutableClosureError>
    {
        validate_output_selections(&bundle, outputs)?;
        bundle.revalidate(cancellation)?;
        let upstream = verify_retained_program_upstream_closure(
            bundle.inputs(),
            |path| {
                bundle
                    .clone_component_for_evidence(path)
                    .map_err(|_| RuntimeSourceBuildInputOpenError)
            },
            || cancellation.is_cancelled(),
        )?;
        bundle.revalidate(cancellation)?;
        let cargo = verify_cargo_source_closure(
            bundle.inputs(),
            CargoSourceClosureLimits::default(),
            |path| {
                bundle
                    .clone_component_for_evidence(path)
                    .map_err(|_| RuntimeSourceBuildInputOpenError)
            },
            || cancellation.is_cancelled(),
        )?;
        bundle.revalidate(cancellation)?;
        let license = verify_retained_program_license_evidence_closure(
            bundle.inputs(),
            &cargo,
            &upstream,
            RetainedProgramLicenseEvidenceLimits::default(),
            |path| {
                bundle
                    .clone_component_for_evidence(path)
                    .map_err(|_| RuntimeSourceBuildInputOpenError)
            },
            || cancellation.is_cancelled(),
        )?;
        bundle.revalidate(cancellation)?;
        let expected_tree = expected_program_tree(&bundle)?;
        let primary = run_bootstrap(
            &bundle,
            outputs.primary(),
            RetainedProgramBootstrapAttempt::Primary,
            &expected_tree,
            cancellation,
        )?;
        bundle.revalidate(cancellation)?;
        let rebuild = run_bootstrap(
            &bundle,
            outputs.rebuild(),
            RetainedProgramBootstrapAttempt::Rebuild,
            &expected_tree,
            cancellation,
        )?;
        validate_live_pair(&primary.execution, &rebuild.execution, &expected_tree)?;
        bundle.revalidate(cancellation)?;
        let closure_id = compile_closure_id(
            &bundle,
            &cargo,
            &license,
            &upstream,
            &primary.execution,
            &rebuild.execution,
            &expected_tree,
        )?;
        let plan = RuntimeSourceBuildPlan::for_verified_retained_program_closure(
            bundle.inputs(),
            closure_id.digest().clone(),
        )
        .map_err(|_| RetainedProgramExecutableClosureError::ClosureMismatch)?;
        let executable = ExecutableRuntimeSourceBuildBundleLease {
            bundle,
            plan,
            closure_id,
            authority: ExecutableClosureAuthority::Production(Box::new(
                ProductionExecutableClosureAuthority {
                    cargo,
                    license,
                    upstream,
                    primary,
                    rebuild,
                },
            )),
        };
        executable.revalidate(cancellation)?;
        Ok(executable)
    }

    /// Returns deterministic unsupported-platform failure without opening inputs.
    ///
    /// # Errors
    ///
    /// Always returns the managed-isolation unsupported-platform error.
    #[cfg(not(target_os = "linux"))]
    pub fn establish(
        _bundle: RuntimeSourceBuildBundleLease,
        _outputs: &RetainedProgramBootstrapOutputSources,
        _cancellation: &CancellationToken,
    ) -> Result<ExecutableRuntimeSourceBuildBundleLease, RetainedProgramExecutableClosureError>
    {
        Err(RuntimeSourceBuildExecutionError::from(
            rewrite_runtime_isolation::IsolationError::UnsupportedPlatform,
        )
        .into())
    }
}

#[cfg(target_os = "linux")]
fn validate_output_selections(
    bundle: &RuntimeSourceBuildBundleLease,
    outputs: &RetainedProgramBootstrapOutputSources,
) -> Result<(), RuntimeSourceBuildExecutionError> {
    if paths_overlap(outputs.primary().path(), outputs.rebuild().path()) {
        return Err(RuntimeSourceBuildExecutionError::OutputOverlap);
    }
    if bundle.overlaps_path(outputs.primary().path())
        || bundle.overlaps_path(outputs.rebuild().path())
    {
        return Err(RuntimeSourceBuildExecutionError::UnsafeOutput);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_bootstrap(
    bundle: &RuntimeSourceBuildBundleLease,
    source: &RuntimeSourceBuildOutputSource,
    attempt: RetainedProgramBootstrapAttempt,
    expected_tree: &ControlledBuildOutputTree,
    cancellation: &CancellationToken,
) -> Result<BootstrapAttemptLease, RetainedProgramExecutableClosureError> {
    if cancellation.is_cancelled() {
        return Err(RuntimeSourceBuildExecutionError::Cancelled.into());
    }
    let plan = bundle.plan();
    let mut output = PinnedRuntimeSourceBuildOutput::open(source, cancellation)?;
    let capabilities = bundle.build_capabilities(cancellation)?;
    let (program, helper, input_root, component_files) = capabilities.into_files();
    let input_files = component_files
        .into_iter()
        .map(|(path, digest, bytes, file)| {
            ControlledBuildInputFile::new(path.as_str(), digest, bytes, file)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut controlled = ControlledBuildLaunchSpec::new(
        plan.build_program_path().as_str(),
        plan.build_program_digest().clone(),
        plan.build_program_bytes(),
        Duration::from_secs(plan.execution_policy().execution_timeout_seconds()),
    )?;
    for argument in plan.policy().build_arguments() {
        controlled.push_argument(argument);
    }
    for variable in plan.policy().environment() {
        controlled.insert_environment(variable.name(), variable.value());
    }
    if controlled.redacted_digest_with_inputs(&input_files) != *plan.expected_launch_digest() {
        return Err(RetainedProgramExecutableClosureError::ClosureMismatch);
    }
    let bootstrap = RetainedProgramBootstrapLaunchSpec::new(
        attempt,
        measurement(bundle, RECIPE_PATH)?.0,
        measurement(bundle, RECIPE_PATH)?.1,
        signed_inputs(bundle)?,
        authenticated_busybox(bundle)?,
        controlled,
    )?;
    let prepared = PreparedIsolation::prepare_retained(
        helper,
        plan.isolation_helper_digest(),
        plan.isolation_helper_bytes(),
        isolation_policy(plan)?,
        cancellation,
    )?;
    let execution = prepared.run_retained_program_bootstrap(
        &bootstrap,
        RetainedProgramBootstrapCapabilities::new(
            program,
            input_root,
            input_files,
            output.clone_empty_root(cancellation)?,
        ),
        cancellation,
    )?;
    let helper_tree = execution
        .controlled_build()
        .output()
        .tree()
        .ok_or(RetainedProgramExecutableClosureError::ClosureMismatch)?;
    let sealed_tree = output.seal(helper_tree, cancellation)?;
    if &sealed_tree != expected_tree || helper_tree != expected_tree {
        return Err(RetainedProgramExecutableClosureError::ClosureMismatch);
    }
    Ok(BootstrapAttemptLease { execution, output })
}

#[cfg(target_os = "linux")]
fn isolation_policy(
    plan: &RuntimeSourceBuildPlan,
) -> Result<IsolationPolicy, RuntimeSourceBuildExecutionError> {
    let execution = plan.execution_policy();
    let policy = IsolationPolicy::new(
        Duration::from_secs(execution.startup_timeout_seconds()),
        Duration::from_secs(execution.shutdown_timeout_seconds()),
        usize::try_from(execution.maximum_arguments())
            .map_err(|_| RuntimeSourceBuildExecutionError::PlanMismatch)?,
        usize::try_from(execution.maximum_environment_variables())
            .map_err(|_| RuntimeSourceBuildExecutionError::PlanMismatch)?,
        usize::try_from(execution.maximum_value_bytes())
            .map_err(|_| RuntimeSourceBuildExecutionError::PlanMismatch)?,
        execution.maximum_open_files(),
        execution.maximum_processes(),
    )?;
    if policy.redacted_digest() != execution.isolation_policy_digest() {
        return Err(RuntimeSourceBuildExecutionError::PlanMismatch);
    }
    Ok(policy)
}

#[cfg(target_os = "linux")]
fn signed_inputs(
    bundle: &RuntimeSourceBuildBundleLease,
) -> Result<RetainedProgramBootstrapSignedInputs, RetainedProgramExecutableClosureError> {
    let measurements = SIGNED_INPUT_KINDS
        .into_iter()
        .map(|kind| {
            let (digest, bytes) = measurement(bundle, kind.relative_path())?;
            RetainedProgramBootstrapInputMeasurement::new(kind, digest, bytes)
                .map_err(RetainedProgramExecutableClosureError::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    RetainedProgramBootstrapSignedInputs::new(measurements)
        .map_err(RetainedProgramExecutableClosureError::from)
}

#[cfg(target_os = "linux")]
fn authenticated_busybox(
    bundle: &RuntimeSourceBuildBundleLease,
) -> Result<AuthenticatedBusyboxExecutable, RetainedProgramExecutableClosureError> {
    let (digest, bytes) = measurement(bundle, BUSYBOX_PATH)?;
    AuthenticatedBusyboxExecutable::new(digest, bytes)
        .map_err(RetainedProgramExecutableClosureError::from)
}

#[cfg(target_os = "linux")]
fn measurement(
    bundle: &RuntimeSourceBuildBundleLease,
    path: &str,
) -> Result<(Digest, u64), RetainedProgramExecutableClosureError> {
    let mut matches = bundle
        .inputs()
        .manifest()
        .components()
        .iter()
        .filter(|component| component.relative_path().as_str() == path);
    let component = matches
        .next()
        .ok_or(RetainedProgramExecutableClosureError::ClosureMismatch)?;
    if matches.next().is_some() {
        return Err(RetainedProgramExecutableClosureError::ClosureMismatch);
    }
    Ok((component.digest().clone(), component.byte_size()))
}
