use std::{
    ffi::{OsStr, OsString},
    fs::File,
    os::fd::{AsRawFd as _, OwnedFd},
    process::Stdio,
};

use crate::{ControlledBuildOutput, ControlledBuildProcessStatus};
use rustix::{
    io::{FdFlags, fcntl_dupfd_cloexec, fcntl_setfd},
    process::fchdir,
};

use super::{
    linux_build_mount::{BUILD_INPUT_ROOT, BUILD_OUTPUT_ROOT},
    linux_fd_exec::{PRIVATE_DESCRIPTOR_MINIMUM, retained_fd_command},
    linux_helper_setup::HelperFailure,
    linux_startup::StartupDrains,
};

const BUILD_CAPABILITY_ABI: &str = "2";
const BUILD_CAPABILITY_PREFIX: &str = "RETONR_CONTROLLED_BUILD_";

pub(super) struct PreparedBuildTarget {
    program: OwnedFd,
    input: OwnedFd,
    output: OwnedFd,
    null: File,
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
}

impl PreparedBuildTarget {
    pub(super) fn prepare(
        program: &File,
        input: &File,
        output: &File,
        arguments: &[OsString],
        environment: &[(OsString, OsString)],
    ) -> Result<Self, HelperFailure> {
        let program = duplicate_private(program)?;
        let input = duplicate_private(input)?;
        let output = duplicate_private(output)?;
        let null = File::open("/dev/null").map_err(|_| HelperFailure::InvalidLaunch)?;
        Ok(Self {
            program,
            input,
            output,
            null,
            arguments: arguments.to_vec(),
            environment: environment.to_vec(),
        })
    }

    pub(super) fn run(self) -> Result<ControlledBuildOutput, HelperFailure> {
        fcntl_setfd(&self.input, FdFlags::empty()).map_err(|_| HelperFailure::InvalidLaunch)?;
        fcntl_setfd(&self.output, FdFlags::empty()).map_err(|_| HelperFailure::InvalidLaunch)?;
        fchdir(&self.output).map_err(|_| HelperFailure::InvalidLaunch)?;
        let mut environment = self.environment;
        if environment.iter().any(|(key, _value)| {
            key.as_encoded_bytes()
                .starts_with(BUILD_CAPABILITY_PREFIX.as_bytes())
        }) {
            return Err(HelperFailure::InvalidLaunch);
        }
        environment.extend([
            (
                OsString::from("RETONR_CONTROLLED_BUILD_INPUT_FD"),
                OsString::from(self.input.as_raw_fd().to_string()),
            ),
            (
                OsString::from("RETONR_CONTROLLED_BUILD_OUTPUT_FD"),
                OsString::from(self.output.as_raw_fd().to_string()),
            ),
            (
                OsString::from("RETONR_CONTROLLED_BUILD_INPUT_ROOT"),
                OsString::from(BUILD_INPUT_ROOT),
            ),
            (
                OsString::from("RETONR_CONTROLLED_BUILD_OUTPUT_ROOT"),
                OsString::from(BUILD_OUTPUT_ROOT),
            ),
            (
                OsString::from("RETONR_CONTROLLED_BUILD_CAPABILITY_ABI"),
                OsString::from(BUILD_CAPABILITY_ABI),
            ),
        ]);
        let mut command = retained_fd_command(
            &self.program,
            OsStr::new("retonr-controlled-build-target"),
            &self.arguments,
            &environment,
        )?;
        command
            .stdin(Stdio::from(self.null))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|_| HelperFailure::InvalidLaunch)?;
        drop(self.program);
        drop(self.input);
        drop(self.output);
        drop(command);
        let standard_output = child.stdout.take().ok_or(HelperFailure::InvalidLaunch)?;
        let standard_error = child.stderr.take().ok_or(HelperFailure::InvalidLaunch)?;
        let drains = StartupDrains::start(standard_output, standard_error);
        let status = child.wait().map_err(|_| HelperFailure::InvalidLaunch)?;
        let status = if status.success() {
            ControlledBuildProcessStatus::Success
        } else if let Some(code) = status.code() {
            ControlledBuildProcessStatus::ExitCode(code)
        } else {
            use std::os::unix::process::ExitStatusExt as _;

            ControlledBuildProcessStatus::Signal(
                status.signal().ok_or(HelperFailure::InvalidLaunch)?,
            )
        };
        Ok(ControlledBuildOutput::new(status, drains.finish()))
    }
}

fn duplicate_private(file: &impl std::os::fd::AsFd) -> Result<OwnedFd, HelperFailure> {
    fcntl_dupfd_cloexec(file, PRIVATE_DESCRIPTOR_MINIMUM).map_err(|_| HelperFailure::InvalidLaunch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_abi_is_explicit_and_versioned() {
        assert_eq!(BUILD_CAPABILITY_ABI, "2");
    }
}
