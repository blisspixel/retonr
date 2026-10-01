use std::{ffi::OsString, fs::File, process::Stdio};

use crate::{ControlledBuildOutput, ControlledBuildProcessStatus};

use super::{
    linux_fd_exec::retained_fd_command, linux_helper_setup::HelperFailure,
    linux_helper_support::validate_executable, linux_startup::StartupDrains,
};

mod outputs;
mod recipe;
mod streams;

pub(super) struct PreparedBootstrapTarget {
    cargo: File,
    target: File,
    output: File,
    null: File,
    environment: Vec<(OsString, OsString)>,
}

impl PreparedBootstrapTarget {
    pub(super) fn prepare(
        program: &File,
        _runtime_arguments: &[OsString],
        _runtime_environment: &[(OsString, OsString)],
    ) -> Result<Self, HelperFailure> {
        // The runtime program is joined to the request, but bootstrap builds the
        // four programs from the independently authenticated fixed recipe.
        validate_executable(program)?;
        let input = File::open("/inputs").map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let toolchain =
            File::open("/toolchain").map_err(|_| HelperFailure::BootstrapRootVerification)?;
        let cargo = outputs::open_regular(&toolchain, "bin/cargo", true)?;
        validate_executable(&cargo)?;
        Ok(Self {
            cargo,
            target: File::open("/target").map_err(|_| HelperFailure::BootstrapRootVerification)?,
            output: File::open("/output").map_err(|_| HelperFailure::BootstrapRootVerification)?,
            null: File::open("/dev/null").map_err(|_| HelperFailure::InvalidLaunch)?,
            environment: recipe::read_environment(&input)?,
        })
    }

    pub(super) fn run(self) -> Result<ControlledBuildOutput, HelperFailure> {
        self.run_at(std::path::Path::new("/source"))
    }

    fn run_at(self, source: &std::path::Path) -> Result<ControlledBuildOutput, HelperFailure> {
        let result = execute_builds(&self.environment, |build, environment| {
            let arguments = build.arguments();
            let mut command = retained_fd_command(
                &self.cargo,
                std::ffi::OsStr::new(recipe::CARGO),
                &arguments,
                environment,
            )?;
            command
                .current_dir(source)
                .stdin(Stdio::from(
                    self.null
                        .try_clone()
                        .map_err(|_| HelperFailure::InvalidLaunch)?,
                ))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = command.spawn().map_err(|_| HelperFailure::InvalidLaunch)?;
            drop(command);
            let drains = StartupDrains::start(
                child.stdout.take().ok_or(HelperFailure::InvalidLaunch)?,
                child.stderr.take().ok_or(HelperFailure::InvalidLaunch)?,
            );
            let status = child.wait().map_err(|_| HelperFailure::InvalidLaunch)?;
            Ok(ControlledBuildOutput::new(
                process_status(status)?,
                drains.finish(),
            ))
        })?;
        if result.status() == ControlledBuildProcessStatus::Success {
            outputs::publish(&self.target, &self.output)?;
        }
        Ok(result)
    }
}

fn execute_builds(
    environment: &[(OsString, OsString)],
    mut run: impl FnMut(
        recipe::Build,
        &[(OsString, OsString)],
    ) -> Result<ControlledBuildOutput, HelperFailure>,
) -> Result<ControlledBuildOutput, HelperFailure> {
    let mut capture = streams::Capture::default();
    for build in recipe::BUILDS {
        let result = run(build, environment)?;
        capture.append(result.streams());
        if result.status() != ControlledBuildProcessStatus::Success {
            return Ok(ControlledBuildOutput::new(
                result.status(),
                capture.finish(),
            ));
        }
    }
    Ok(ControlledBuildOutput::new(
        ControlledBuildProcessStatus::Success,
        capture.finish(),
    ))
}

fn process_status(
    status: std::process::ExitStatus,
) -> Result<ControlledBuildProcessStatus, HelperFailure> {
    if status.success() {
        Ok(ControlledBuildProcessStatus::Success)
    } else if let Some(code) = status.code() {
        Ok(ControlledBuildProcessStatus::ExitCode(code))
    } else {
        use std::os::unix::process::ExitStatusExt as _;
        Ok(ControlledBuildProcessStatus::Signal(
            status.signal().ok_or(HelperFailure::InvalidLaunch)?,
        ))
    }
}

#[cfg(test)]
mod tests;
