use std::path::PathBuf;

#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(all(test, target_os = "linux"))]
use std::fs;
#[cfg(all(test, target_os = "linux"))]
use std::sync::atomic::{AtomicBool, Ordering};

use rewrite_model::ArtifactSetId;
use rewrite_ollama_package::RuntimeSourceBuildPlan;
#[cfg(not(target_os = "linux"))]
use rewrite_runtime_isolation::IsolationError;
use rewrite_runtime_isolation::{
    ControlledBuildExecution, ControlledBuildProcessStatus, ManagedStartupOutput,
};
#[cfg(target_os = "linux")]
use rewrite_runtime_isolation::{
    ControlledBuildInputFile, ControlledBuildLaunchSpec, IsolationPolicy, PreparedIsolation,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{ExecutableRuntimeSourceBuildBundleLease, artifact_storage::ManagedTreeLimits};

mod error;
pub(crate) mod output;
pub use error::RuntimeSourceBuildExecutionError;
use error::{ensure_active, map_completed_output, map_output};
use output::PinnedRuntimeSourceBuildOutput;

/// Hard ceiling for files plus directories in one controlled-build output tree.
pub const MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES: usize = 4_096;
#[cfg(all(test, target_os = "linux"))]
static SUBSTITUTE_OUTPUT_AFTER_HANDOFF: AtomicBool = AtomicBool::new(false);

/// Caller-selected empty directory for one controlled build attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildOutputSource {
    path: PathBuf,
}

impl RuntimeSourceBuildOutputSource {
    /// Forms one absolute output-root selection without opening or creating it.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildExecutionError::InvalidOutput`] when the path
    /// cannot be made absolute.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RuntimeSourceBuildExecutionError> {
        let path = std::path::absolute(path.into())
            .map_err(RuntimeSourceBuildExecutionError::InvalidOutput)?;
        Ok(Self { path })
    }

    /// Returns the selected absolute path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Successful controlled build attempt and its retained output-root object.
pub struct RuntimeSourceBuildExecution {
    managed: ControlledBuildExecution,
    pub(crate) output: PinnedRuntimeSourceBuildOutput,
    provenance: RuntimeSourceBuildExecutionProvenance,
}

struct RuntimeSourceBuildExecutionProvenance {
    source_inputs_id: ArtifactSetId,
    source_manifest_digest: Digest,
    build_plan_digest: Digest,
    expected_launch_digest: Digest,
}

/// Two independently prepared and executed controlled builds.
pub struct RuntimeSourceBuildExecutionPair {
    primary: RuntimeSourceBuildExecution,
    rebuild: RuntimeSourceBuildExecution,
}

/// Bounded process result retained when the controlled build program fails.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildFailure {
    status: ControlledBuildProcessStatus,
    streams: ManagedStartupOutput,
}

impl RuntimeSourceBuildFailure {
    /// Returns the exact failed process status.
    #[must_use]
    pub const fn status(&self) -> ControlledBuildProcessStatus {
        self.status
    }

    /// Returns bounded captured output whose debug representation excludes bytes.
    #[must_use]
    pub const fn streams(&self) -> &ManagedStartupOutput {
        &self.streams
    }
}

impl RuntimeSourceBuildExecutionPair {
    /// Returns the primary controlled build.
    #[must_use]
    pub const fn primary(&self) -> &RuntimeSourceBuildExecution {
        &self.primary
    }

    /// Returns the independently recreated controlled build.
    #[must_use]
    pub const fn rebuild(&self) -> &RuntimeSourceBuildExecution {
        &self.rebuild
    }

    /// Consumes the pair into primary and rebuild results in that order.
    #[must_use]
    pub fn into_parts(self) -> (RuntimeSourceBuildExecution, RuntimeSourceBuildExecution) {
        (self.primary, self.rebuild)
    }
}

impl RuntimeSourceBuildExecution {
    /// Returns the managed namespace, filesystem, process, and bounded-log result.
    #[must_use]
    pub const fn managed(&self) -> &ControlledBuildExecution {
        &self.managed
    }

    /// Clones the exact retained output-root object without exposing its local path.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildExecutionError`] for cancellation, replacement,
    /// or a descriptor-clone failure.
    pub fn clone_output_root(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<std::fs::File, RuntimeSourceBuildExecutionError> {
        self.output.validate_sealed(
            ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES)
                .map_err(map_output)?,
            cancellation,
        )?;
        self.output.root.clone_handle().map_err(map_output)
    }

    /// Rechecks that the held and selected output directory retain one identity.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildExecutionError`] for cancellation or replacement.
    pub fn revalidate_output(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeSourceBuildExecutionError> {
        self.output.validate_sealed(
            ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES)
                .map_err(map_output)?,
            cancellation,
        )
    }

    pub(crate) fn revalidate_plan(
        &self,
        plan: &RuntimeSourceBuildPlan,
    ) -> Result<(), RuntimeSourceBuildExecutionError> {
        let isolation = self.managed.isolation();
        if self.provenance.source_inputs_id == *plan.source_inputs_id()
            && self.provenance.source_manifest_digest == *plan.source_manifest_digest()
            && self.provenance.build_plan_digest == *plan.plan_digest()
            && self.provenance.expected_launch_digest == *isolation.launch_digest()
            && isolation.preparation().helper_digest() == plan.isolation_helper_digest()
            && isolation.preparation().helper_bytes() == plan.isolation_helper_bytes()
        {
            Ok(())
        } else {
            Err(RuntimeSourceBuildExecutionError::PlanMismatch)
        }
    }
}

/// Application-owned compiler for one retained controlled build attempt.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeSourceBuildExecutor;

impl RuntimeSourceBuildExecutor {
    /// Runs two independent attempts into distinct, nonoverlapping output roots.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildExecutionError`] before launch when the output
    /// selections overlap, or propagates any primary or rebuild execution failure.
    #[cfg(target_os = "linux")]
    pub fn execute_pair(
        bundle: &ExecutableRuntimeSourceBuildBundleLease,
        primary_output: &RuntimeSourceBuildOutputSource,
        rebuild_output: &RuntimeSourceBuildOutputSource,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildExecutionPair, RuntimeSourceBuildExecutionError> {
        if paths_overlap(primary_output.path(), rebuild_output.path()) {
            return Err(RuntimeSourceBuildExecutionError::OutputOverlap);
        }
        let primary = Self::execute(bundle, primary_output, cancellation)?;
        let rebuild = Self::execute(bundle, rebuild_output, cancellation)?;
        primary.revalidate_plan(bundle.plan())?;
        rebuild.revalidate_plan(bundle.plan())?;
        Ok(RuntimeSourceBuildExecutionPair { primary, rebuild })
    }

    /// Returns deterministic unsupported-platform failure without opening inputs.
    ///
    /// # Errors
    ///
    /// Always returns [`RuntimeSourceBuildExecutionError::Isolation`] wrapping
    /// [`IsolationError::UnsupportedPlatform`].
    #[cfg(not(target_os = "linux"))]
    pub fn execute_pair(
        _bundle: &ExecutableRuntimeSourceBuildBundleLease,
        _primary_output: &RuntimeSourceBuildOutputSource,
        _rebuild_output: &RuntimeSourceBuildOutputSource,
        _cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildExecutionPair, RuntimeSourceBuildExecutionError> {
        Err(IsolationError::UnsupportedPlatform.into())
    }

    /// Executes the reviewed build plan from retained bundle objects only.
    ///
    /// The selected output directory must already exist and be empty. On Linux,
    /// the method verifies the retained helper identity, prepares managed
    /// isolation, applies the exact reviewed arguments and cleared environment,
    /// runs the program, then revalidates both input and output boundaries.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildExecutionError`] for unsupported platforms,
    /// cancellation, drift, a nonempty or unsafe output root, plan divergence,
    /// or any managed isolation failure.
    #[cfg(target_os = "linux")]
    pub fn execute(
        bundle: &ExecutableRuntimeSourceBuildBundleLease,
        output: &RuntimeSourceBuildOutputSource,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildExecution, RuntimeSourceBuildExecutionError> {
        ensure_active(cancellation)?;
        let plan = bundle.plan();
        bundle.revalidate_for_execution(cancellation)?;
        if bundle.bundle().overlaps_path(output.path()) {
            return Err(RuntimeSourceBuildExecutionError::UnsafeOutput);
        }
        let mut output = PinnedRuntimeSourceBuildOutput::open(output, cancellation)?;
        if plan.controlled_build_capability_abi() != 2 {
            return Err(RuntimeSourceBuildExecutionError::PlanMismatch);
        }
        let isolation_policy = controlled_build_isolation_policy(plan)?;
        let capabilities = bundle.bundle().build_capabilities(cancellation)?;
        let (program, helper, input_root, component_files) = capabilities.into_files();
        let input_files = component_files
            .into_iter()
            .map(|(path, digest, bytes, file)| {
                ControlledBuildInputFile::new(path.as_str(), digest, bytes, file)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prepared = PreparedIsolation::prepare_retained(
            helper,
            plan.isolation_helper_digest(),
            plan.isolation_helper_bytes(),
            isolation_policy,
            cancellation,
        )?;
        let preparation = prepared.preparation_evidence();
        if preparation.helper_digest() != plan.isolation_helper_digest()
            || preparation.helper_bytes() != plan.isolation_helper_bytes()
        {
            return Err(RuntimeSourceBuildExecutionError::PlanMismatch);
        }
        let (specification, expected_launch_digest) =
            controlled_build_launch_specification(plan, &input_files)?;
        let managed = prepared.run_controlled_build_retained(
            &specification,
            program,
            input_root,
            input_files,
            output.root.clone_handle().map_err(map_output)?,
            cancellation,
        )?;
        validate_controlled_build_result(&managed, plan, &expected_launch_digest)?;
        output = seal_controlled_build_output(bundle, output, &managed, cancellation)?;
        Ok(RuntimeSourceBuildExecution {
            managed,
            output,
            provenance: RuntimeSourceBuildExecutionProvenance {
                source_inputs_id: plan.source_inputs_id().clone(),
                source_manifest_digest: plan.source_manifest_digest().clone(),
                build_plan_digest: plan.plan_digest().clone(),
                expected_launch_digest,
            },
        })
    }

    /// Returns deterministic unsupported-platform failure without opening inputs.
    ///
    /// # Errors
    ///
    /// Always returns [`RuntimeSourceBuildExecutionError::Isolation`] wrapping
    /// [`IsolationError::UnsupportedPlatform`].
    #[cfg(not(target_os = "linux"))]
    pub fn execute(
        _bundle: &ExecutableRuntimeSourceBuildBundleLease,
        _output: &RuntimeSourceBuildOutputSource,
        _cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildExecution, RuntimeSourceBuildExecutionError> {
        Err(IsolationError::UnsupportedPlatform.into())
    }
}

#[cfg(target_os = "linux")]
fn controlled_build_isolation_policy(
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
fn controlled_build_launch_specification(
    plan: &RuntimeSourceBuildPlan,
    input_files: &[ControlledBuildInputFile],
) -> Result<(ControlledBuildLaunchSpec, Digest), RuntimeSourceBuildExecutionError> {
    let mut specification = ControlledBuildLaunchSpec::new(
        plan.build_program_path().as_str(),
        plan.build_program_digest().clone(),
        plan.build_program_bytes(),
        Duration::from_secs(plan.execution_policy().execution_timeout_seconds()),
    )?;
    for argument in plan.policy().build_arguments() {
        specification.push_argument(argument);
    }
    for variable in plan.policy().environment() {
        specification.insert_environment(variable.name(), variable.value());
    }
    let digest = specification.redacted_digest_with_inputs(input_files);
    if &digest != plan.expected_launch_digest() {
        return Err(RuntimeSourceBuildExecutionError::PlanMismatch);
    }
    Ok((specification, digest))
}

#[cfg(target_os = "linux")]
fn validate_controlled_build_result(
    managed: &ControlledBuildExecution,
    plan: &RuntimeSourceBuildPlan,
    expected_launch_digest: &Digest,
) -> Result<(), RuntimeSourceBuildExecutionError> {
    if managed.output().status() != ControlledBuildProcessStatus::Success {
        return Err(RuntimeSourceBuildExecutionError::BuildFailed(
            RuntimeSourceBuildFailure {
                status: managed.output().status(),
                streams: managed.output().streams().clone(),
            },
        ));
    }
    if managed.output().streams().standard_output_truncated()
        || managed.output().streams().standard_error_truncated()
    {
        return Err(RuntimeSourceBuildExecutionError::OutputLimitExceeded);
    }
    let isolation = managed.isolation();
    if isolation.preparation().helper_digest() != plan.isolation_helper_digest()
        || isolation.preparation().helper_bytes() != plan.isolation_helper_bytes()
        || isolation.isolation_policy_digest() != &plan.execution_policy().isolation_policy_digest()
        || isolation.launch_digest() != expected_launch_digest
        || isolation.landlock_abi() < 3
    {
        return Err(RuntimeSourceBuildExecutionError::PlanMismatch);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn seal_controlled_build_output(
    bundle: &ExecutableRuntimeSourceBuildBundleLease,
    mut output: PinnedRuntimeSourceBuildOutput,
    managed: &ControlledBuildExecution,
    cancellation: &CancellationToken,
) -> Result<PinnedRuntimeSourceBuildOutput, RuntimeSourceBuildExecutionError> {
    bundle.revalidate_for_execution(cancellation)?;
    #[cfg(test)]
    inject_output_substitution(output.path())?;
    let helper_tree = managed
        .output()
        .tree()
        .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
    let sealed_tree = output.seal(helper_tree, cancellation)?;
    if managed.output().tree() != Some(&sealed_tree) {
        return Err(RuntimeSourceBuildExecutionError::OutputChanged);
    }
    bundle.revalidate_for_execution(cancellation)?;
    output.validate_sealed(
        ManagedTreeLimits::new(MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES).map_err(map_output)?,
        cancellation,
    )?;
    Ok(output)
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn inject_controlled_build_output_substitution_once() {
    SUBSTITUTE_OUTPUT_AFTER_HANDOFF.store(true, Ordering::SeqCst);
}

#[cfg(all(test, target_os = "linux"))]
fn inject_output_substitution(
    path: &std::path::Path,
) -> Result<(), RuntimeSourceBuildExecutionError> {
    if SUBSTITUTE_OUTPUT_AFTER_HANDOFF.swap(false, Ordering::SeqCst) {
        let member = path.join("sbom.json");
        let mut bytes = fs::read(&member).map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
        let first = bytes
            .first_mut()
            .ok_or(RuntimeSourceBuildExecutionError::OutputChanged)?;
        *first ^= 1;
        fs::remove_file(&member).map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
        fs::write(member, bytes).map_err(RuntimeSourceBuildExecutionError::OutputIo)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    path_is_within(left, right) || path_is_within(right, left)
}

#[cfg(target_os = "linux")]
fn path_is_within(path: &std::path::Path, ancestor: &std::path::Path) -> bool {
    let path = path.components().collect::<Vec<_>>();
    let ancestor = ancestor.components().collect::<Vec<_>>();
    path.len() >= ancestor.len()
        && path
            .iter()
            .zip(&ancestor)
            .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
}

#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    use super::*;
    use crate::ArtifactInventoryError;

    #[test]
    fn output_selection_is_absolute_and_platform_failure_is_inert() {
        let output = RuntimeSourceBuildOutputSource::new("missing-output")
            .expect("form absolute output selection");
        assert!(output.path().is_absolute());
    }

    #[test]
    fn completed_tree_limit_has_its_exact_public_error() {
        assert!(matches!(
            map_completed_output(ArtifactInventoryError::StorageEntryLimitExceeded),
            RuntimeSourceBuildExecutionError::OutputTreeLimitExceeded
        ));
        assert!(matches!(
            map_output(ArtifactInventoryError::StorageEntryLimitExceeded),
            RuntimeSourceBuildExecutionError::OutputNotEmpty
        ));
    }
}
