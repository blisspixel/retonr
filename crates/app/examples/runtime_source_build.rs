#![forbid(unsafe_code)]

use std::{error::Error, ffi::OsString, io::Write as _, path::PathBuf, process::ExitCode};

use rewrite_app::{
    RetainedProgramBootstrapOutputSources, RetainedProgramExecutableClosureVerifier,
    RuntimeSourceBuildBundleLimits, RuntimeSourceBuildBundleSource,
    RuntimeSourceBuildBundleVerifier, RuntimeSourceBuildEvidenceBundleDestination,
    RuntimeSourceBuildEvidenceBundleLimits, RuntimeSourceBuildEvidenceBundlePublisher,
    RuntimeSourceBuildEvidenceBundleSource, RuntimeSourceBuildEvidenceBundleVerifier,
    RuntimeSourceBuildExecutionError, RuntimeSourceBuildExecutor, RuntimeSourceBuildOutputSource,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let message = format!("runtime-source-build-runner-error:{error}\n");
            let _ = std::io::stderr().write_all(message.as_bytes());
            ExitCode::from(70)
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next().ok_or("missing program")?;
    let operation = argument(&mut arguments)?;
    let cancellation = CancellationToken::new();
    if operation.to_str() == Some("verify") {
        let source =
            RuntimeSourceBuildEvidenceBundleSource::new(PathBuf::from(argument(&mut arguments)?))?;
        require_end(&mut arguments)?;
        let reacquired = RuntimeSourceBuildEvidenceBundleVerifier::acquire(
            &source,
            RuntimeSourceBuildEvidenceBundleLimits::default(),
            &cancellation,
        )?;
        return write_success(&json!({
            "byte_identical": reacquired.report().is_byte_identical(),
            "evidence_artifact_set_id": reacquired.manifest().artifact_set_id(),
            "source_inputs_id": reacquired.plan().source_inputs_id(),
            "status": "verification_success"
        }));
    }
    let manifest = PathBuf::from(argument(&mut arguments)?);
    let components = PathBuf::from(argument(&mut arguments)?);
    let selection = RuntimeSourceBuildBundleSource::new(manifest, components)?;
    let bundle = RuntimeSourceBuildBundleVerifier::acquire(
        &selection,
        RuntimeSourceBuildBundleLimits::default(),
        &cancellation,
    )?;
    match operation.to_str() {
        Some("single") => {
            let bootstrap_outputs = RetainedProgramBootstrapOutputSources::new(
                output(argument(&mut arguments)?)?,
                output(argument(&mut arguments)?)?,
            );
            let output = output(argument(&mut arguments)?)?;
            require_end(&mut arguments)?;
            let bundle = RetainedProgramExecutableClosureVerifier::establish(
                bundle,
                &bootstrap_outputs,
                &cancellation,
            )?;
            let execution = RuntimeSourceBuildExecutor::execute(&bundle, &output, &cancellation)
                .map_err(report_execution_failure)?;
            let streams = execution.managed().output().streams();
            write_success(&json!({
                "source_inputs_id": bundle.plan().source_inputs_id(),
                "standard_error_digest": Digest::sha256(streams.standard_error()),
                "standard_output_digest": Digest::sha256(streams.standard_output()),
                "status": "single_success"
            }))
        }
        Some("pair") => {
            let bootstrap_outputs = RetainedProgramBootstrapOutputSources::new(
                output(argument(&mut arguments)?)?,
                output(argument(&mut arguments)?)?,
            );
            let primary = output(argument(&mut arguments)?)?;
            let rebuild = output(argument(&mut arguments)?)?;
            let evidence = RuntimeSourceBuildEvidenceBundleDestination::new(PathBuf::from(
                argument(&mut arguments)?,
            ))?;
            require_end(&mut arguments)?;
            let bundle = RetainedProgramExecutableClosureVerifier::establish(
                bundle,
                &bootstrap_outputs,
                &cancellation,
            )?;
            let pair = RuntimeSourceBuildExecutor::execute_pair(
                &bundle,
                &primary,
                &rebuild,
                &cancellation,
            )
            .map_err(report_execution_failure)?;
            let published = RuntimeSourceBuildEvidenceBundlePublisher::publish(
                &bundle,
                pair.primary(),
                pair.rebuild(),
                &evidence,
                RuntimeSourceBuildEvidenceBundleLimits::default(),
                &cancellation,
            )?;
            published.revalidate(&cancellation)?;
            let source = RuntimeSourceBuildEvidenceBundleSource::new(evidence.path())?;
            let reacquired = RuntimeSourceBuildEvidenceBundleVerifier::acquire(
                &source,
                RuntimeSourceBuildEvidenceBundleLimits::default(),
                &cancellation,
            )?;
            write_success(&json!({
                "byte_identical": reacquired.report().is_byte_identical(),
                "evidence_artifact_set_id": reacquired.manifest().artifact_set_id(),
                "source_inputs_id": reacquired.plan().source_inputs_id(),
                "status": "pair_success"
            }))
        }
        _ => Err("unsupported operation".into()),
    }
}

fn argument(arguments: &mut impl Iterator<Item = OsString>) -> Result<OsString, Box<dyn Error>> {
    arguments.next().ok_or_else(|| "missing argument".into())
}

fn require_end(arguments: &mut impl Iterator<Item = OsString>) -> Result<(), Box<dyn Error>> {
    if arguments.next().is_none() {
        Ok(())
    } else {
        Err("unexpected argument".into())
    }
}

fn output(path: OsString) -> Result<RuntimeSourceBuildOutputSource, Box<dyn Error>> {
    Ok(RuntimeSourceBuildOutputSource::new(PathBuf::from(path))?)
}

fn report_execution_failure(
    error: RuntimeSourceBuildExecutionError,
) -> RuntimeSourceBuildExecutionError {
    if let RuntimeSourceBuildExecutionError::BuildFailed(failure) = &error {
        let summary = format!(
            "controlled-build-failure:status={:?}:stdout_bytes={}:stderr_bytes={}\n",
            failure.status(),
            failure.streams().standard_output().len(),
            failure.streams().standard_error().len()
        );
        let _ = std::io::stderr().write_all(summary.as_bytes());
        let _ = std::io::stderr().write_all(failure.streams().standard_output());
        let _ = std::io::stderr().write_all(failure.streams().standard_error());
    }
    error
}

fn write_success(value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    let mut bytes = serde_json::to_vec(&value)?;
    bytes.push(b'\n');
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
