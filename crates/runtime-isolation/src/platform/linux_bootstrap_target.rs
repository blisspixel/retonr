use std::{
    ffi::OsString,
    fs::File,
    os::fd::{AsRawFd as _, OwnedFd},
    process::{Command, Stdio},
};

use rustix::{
    io::{FdFlags, fcntl_dupfd_cloexec, fcntl_setfd},
    process::chdir,
};

use crate::{ControlledBuildOutput, ControlledBuildProcessStatus};

use super::{linux_helper_setup::HelperFailure, linux_startup::StartupDrains};

const SAFE_DESCRIPTOR_MINIMUM: i32 = 64;

pub(super) struct PreparedBootstrapTarget {
    program: OwnedFd,
    null: File,
    arguments: Vec<OsString>,
    environment: Vec<(OsString, OsString)>,
}

impl PreparedBootstrapTarget {
    pub(super) fn prepare(
        program: &File,
        arguments: &[OsString],
        environment: &[(OsString, OsString)],
    ) -> Result<Self, HelperFailure> {
        let program = fcntl_dupfd_cloexec(program, SAFE_DESCRIPTOR_MINIMUM)
            .map_err(|_| HelperFailure::InvalidLaunch)?;
        let null = File::open("/dev/null").map_err(|_| HelperFailure::InvalidLaunch)?;
        Ok(Self {
            program,
            null,
            arguments: arguments.to_vec(),
            environment: environment.to_vec(),
        })
    }

    pub(super) fn run(self) -> Result<ControlledBuildOutput, HelperFailure> {
        fcntl_setfd(&self.program, FdFlags::CLOEXEC).map_err(|_| HelperFailure::InvalidLaunch)?;
        chdir("/source").map_err(|_| HelperFailure::InvalidLaunch)?;
        let mut command = Command::new(format!("/proc/self/fd/{}", self.program.as_raw_fd()));
        command
            .args(&self.arguments)
            .stdin(Stdio::from(self.null))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        for (key, value) in self.environment {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|_| HelperFailure::InvalidLaunch)?;
        drop(self.program);
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
