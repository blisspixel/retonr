use std::{
    env,
    fs::File,
    path::Path,
    process::{Child, Command, Stdio},
};

use crate::{IsolationPolicy, IsolationResult, LaunchSpec, error::native};

const INTERNAL_PREFIX: &str = "REWRITE_ISOLATION_INTERNAL_";

pub(super) fn helper_command(helper: &File) -> Command {
    use std::os::fd::AsRawFd as _;

    let mut command = Command::new(format!("/proc/self/fd/{}", helper.as_raw_fd()));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear();
    apply_coverage_environment(&mut command);
    command
}

pub(super) fn helper_command_with_input(helper: &File, input: Stdio) -> Command {
    use std::os::fd::AsRawFd as _;

    let mut command = Command::new(format!("/proc/self/fd/{}", helper.as_raw_fd()));
    command
        .stdin(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_clear();
    apply_coverage_environment(&mut command);
    command
}

fn apply_coverage_environment(command: &mut Command) {
    if env::var_os("CARGO_LLVM_COV").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    let Some(profile) = env::var_os("LLVM_PROFILE_FILE") else {
        return;
    };
    if profile.as_encoded_bytes().len() <= 4_096 && Path::new(&profile).is_absolute() {
        command.env("LLVM_PROFILE_FILE", profile);
    }
}

pub(super) fn apply_policy_environment(command: &mut Command, policy: IsolationPolicy) {
    command
        .env(
            format!("{INTERNAL_PREFIX}MAX_OPEN_FILES"),
            policy.maximum_open_files().to_string(),
        )
        .env(
            format!("{INTERNAL_PREFIX}MAX_PROCESSES"),
            policy.maximum_processes().to_string(),
        )
        .env(
            format!("{INTERNAL_PREFIX}STARTUP_TIMEOUT_MILLIS"),
            policy.startup_timeout().as_millis().to_string(),
        );
}

pub(super) fn apply_target_environment(command: &mut Command, specification: &LaunchSpec) {
    command.env(
        format!("{INTERNAL_PREFIX}ENV_COUNT"),
        specification.environment().len().to_string(),
    );
    for (index, (key, value)) in specification.environment().iter().enumerate() {
        command
            .env(format!("{INTERNAL_PREFIX}ENV_{index}_KEY"), key)
            .env(format!("{INTERNAL_PREFIX}ENV_{index}_VALUE"), value);
    }
    if let Some(directory) = specification.current_directory() {
        command.env(format!("{INTERNAL_PREFIX}CURRENT_DIRECTORY"), directory);
    }
}

pub(super) fn spawn_helper(command: &mut Command) -> IsolationResult<Child> {
    command
        .spawn()
        .map_err(|error| native("spawn-isolation-helper", &error))
}
