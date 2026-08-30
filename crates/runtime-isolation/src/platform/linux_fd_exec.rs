use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr, OsString},
    io,
    os::{
        fd::OwnedFd,
        unix::{ffi::OsStrExt as _, process::CommandExt as _},
    },
    process::Command,
};

use rustix::{fs::AtFlags, io::fcntl_dupfd_cloexec, runtime::execveat};

use super::{linux_executable::has_elf_magic, linux_helper_setup::HelperFailure};

pub(super) const PRIVATE_DESCRIPTOR_MINIMUM: i32 = 3;
const EXEC_FAILURE_SENTINEL: &str = "/proc/self/exe";
const MAXIMUM_ARGUMENT_POINTERS: usize = 4_098;
const MAXIMUM_ENVIRONMENT_POINTERS: usize = 1_025;

pub(super) fn retained_fd_command(
    program: &impl std::os::fd::AsFd,
    argv_zero: &OsStr,
    arguments: &[OsString],
    environment: &[(OsString, OsString)],
) -> Result<Command, HelperFailure> {
    if arguments.len().saturating_add(2) > MAXIMUM_ARGUMENT_POINTERS
        || environment.len().saturating_add(1) > MAXIMUM_ENVIRONMENT_POINTERS
    {
        return Err(HelperFailure::InvalidLaunch);
    }
    if !has_elf_magic(program).map_err(|_| HelperFailure::InvalidLaunch)? {
        return Err(HelperFailure::InvalidLaunch);
    }
    let retained_program = fcntl_dupfd_cloexec(program, PRIVATE_DESCRIPTOR_MINIMUM)
        .map_err(|_| HelperFailure::InvalidLaunch)?;
    let environment_count = environment.len();
    let environment = environment
        .iter()
        .cloned()
        .collect::<BTreeMap<OsString, OsString>>();
    if environment.len() != environment_count {
        return Err(HelperFailure::InvalidLaunch);
    }
    let invocation = PreparedInvocation::new(argv_zero, arguments, &environment)?;
    let mut command = Command::new(EXEC_FAILURE_SENTINEL);
    command
        .arg0(argv_zero)
        .args(arguments)
        .env_clear()
        .envs(&environment);
    attach_retained_exec(&mut command, retained_program, invocation);
    Ok(command)
}

#[expect(
    unsafe_code,
    reason = "Command::pre_exec and execveat require one reviewed unsafe attachment"
)]
#[expect(
    clippy::large_stack_arrays,
    reason = "fixed post-fork pointer arrays avoid allocation and locking before execveat"
)]
fn attach_retained_exec(command: &mut Command, program: OwnedFd, invocation: PreparedInvocation) {
    // SAFETY: `invocation` owns immutable CString backing storage and
    // NUL-terminated values for the entire child closure. Each CString pointer
    // is deliberately exposed as `usize` before `fork`. The closure rebuilds
    // typed raw pointers with `with_exposed_provenance` into fixed stack arrays,
    // so no integer allocation is reinterpreted as pointer storage and no Rust
    // reference is reconstructed from an address.
    // `program` owns the executable descriptor until execveat. The closure only
    // invokes execveat and converts its returned errno without allocation or
    // locking.
    unsafe {
        command.pre_exec(move || {
            let mut arguments = [std::ptr::null(); MAXIMUM_ARGUMENT_POINTERS];
            let mut environment = [std::ptr::null(); MAXIMUM_ENVIRONMENT_POINTERS];
            invocation.arguments.write_pointers(&mut arguments);
            invocation.environment.write_pointers(&mut environment);
            let error = execveat(
                &program,
                c"",
                arguments.as_ptr(),
                environment.as_ptr(),
                AtFlags::EMPTY_PATH,
            );
            Err(io::Error::from_raw_os_error(error.raw_os_error()))
        });
    }
}

struct PreparedInvocation {
    arguments: StableCStringArray,
    environment: StableCStringArray,
}

impl PreparedInvocation {
    fn new(
        argv_zero: &OsStr,
        arguments: &[OsString],
        environment: &BTreeMap<OsString, OsString>,
    ) -> Result<Self, HelperFailure> {
        let mut argument_strings = Vec::with_capacity(arguments.len().saturating_add(1));
        argument_strings.push(c_string(argv_zero)?);
        for argument in arguments {
            argument_strings.push(c_string(argument)?);
        }
        let mut environment_strings = Vec::with_capacity(environment.len());
        for (key, value) in environment {
            let mut entry = Vec::with_capacity(
                key.as_encoded_bytes()
                    .len()
                    .saturating_add(value.as_encoded_bytes().len())
                    .saturating_add(1),
            );
            entry.extend_from_slice(key.as_bytes());
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            environment_strings
                .push(CString::new(entry).map_err(|_| HelperFailure::InvalidLaunch)?);
        }
        Ok(Self {
            arguments: StableCStringArray::new(argument_strings),
            environment: StableCStringArray::new(environment_strings),
        })
    }
}

fn c_string(value: &OsStr) -> Result<CString, HelperFailure> {
    CString::new(value.as_bytes()).map_err(|_| HelperFailure::InvalidLaunch)
}

struct StableCStringArray {
    _strings: Vec<CString>,
    addresses: Vec<usize>,
}

impl StableCStringArray {
    fn new(strings: Vec<CString>) -> Self {
        let addresses = strings
            .iter()
            .map(|value| value.as_ptr().expose_provenance())
            .collect::<Vec<_>>();
        Self {
            _strings: strings,
            addresses,
        }
    }

    fn write_pointers(&self, target: &mut [*const u8]) {
        for (slot, address) in target.iter_mut().zip(&self.addresses) {
            *slot = std::ptr::with_exposed_provenance(*address);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::Write as _,
        os::unix::fs::PermissionsExt as _,
        process::Stdio,
    };

    use super::*;

    #[test]
    fn retained_execution_preserves_arguments_environment_cwd_and_stdio() {
        let program = File::open("/bin/sh").expect("open retained shell");
        let directory = tempfile::tempdir_in("/tmp").expect("create retained execution cwd");
        let arguments = vec![
            OsString::from("-c"),
            OsString::from(
                "argv_zero=$(tr '\\000' '\\n' < /proc/$$/cmdline | head -n 1); IFS= read -r input; printf '%s|%s|%s|%s|' \"$argv_zero\" \"$1\" \"$TOKEN\" \"$input\"; pwd; printf stderr-ok >&2",
            ),
            OsString::from("retained-shell"),
            OsString::from("argument value"),
        ];
        let environment = vec![(OsString::from("TOKEN"), OsString::from("environment value"))];
        let mut command = retained_fd_command(
            &program,
            OsStr::new("retained-test-target"),
            &arguments,
            &environment,
        )
        .expect("prepare retained exec");
        command
            .current_dir(directory.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn retained executable");
        drop(command);
        child
            .stdin
            .take()
            .expect("retained stdin")
            .write_all(b"standard input\n")
            .expect("write retained stdin");
        let output = child
            .wait_with_output()
            .expect("wait for retained executable");
        assert!(output.status.success());
        assert_eq!(
            output.stdout,
            format!(
                "retained-test-target|argument value|environment value|standard input|{}\n",
                directory.path().display()
            )
            .as_bytes()
        );
        assert_eq!(output.stderr, b"stderr-ok");
    }

    #[test]
    fn retained_execution_rejects_nul_before_fork() {
        let program = File::open("/bin/true").expect("open retained true");
        assert!(matches!(
            retained_fd_command(
                &program,
                OsStr::new("retained-test-target"),
                &[OsString::from("bad\0argument")],
                &[]
            ),
            Err(HelperFailure::InvalidLaunch)
        ));
        assert!(matches!(
            retained_fd_command(
                &program,
                OsStr::new("retained-test-target"),
                &[],
                &[(OsString::from("TOKEN"), OsString::from("bad\0value"))]
            ),
            Err(HelperFailure::InvalidLaunch)
        ));
        assert!(matches!(
            retained_fd_command(
                &program,
                OsStr::new("retained-test-target"),
                &[],
                &[
                    (OsString::from("TOKEN"), OsString::from("first")),
                    (OsString::from("TOKEN"), OsString::from("second")),
                ]
            ),
            Err(HelperFailure::InvalidLaunch)
        ));
    }

    #[test]
    fn retained_execution_rejects_executable_scripts_before_fork() {
        let directory = tempfile::tempdir().expect("script fixture directory");
        let script = directory.path().join("script");
        fs::write(&script, b"#!/bin/sh\nexit 0\n").expect("write script fixture");
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
            .expect("make script fixture executable");
        let program = File::open(script).expect("open script fixture");
        assert!(matches!(
            retained_fd_command(&program, OsStr::new("retained-script-target"), &[], &[]),
            Err(HelperFailure::InvalidLaunch)
        ));
    }
}
