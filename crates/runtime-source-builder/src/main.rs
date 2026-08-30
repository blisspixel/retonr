#![forbid(unsafe_code)]

use std::{io::Write as _, process::ExitCode};

#[cfg(any(target_os = "linux", test))]
mod archive;
mod arguments;
mod build;
#[cfg(target_os = "linux")]
mod command;
#[cfg(target_os = "linux")]
mod evidence;
#[cfg(target_os = "linux")]
mod filesystem;
#[cfg(any(target_os = "linux", test))]
mod native_layout;
#[cfg(any(target_os = "linux", test))]
mod patch;
mod shim;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = std::io::stderr().write_all(error.message().as_bytes());
            ExitCode::from(70)
        }
    }
}

fn run() -> Result<(), BuildError> {
    if let Some(tool) = shim::invoked_tool()? {
        shim::run(tool)
    } else {
        #[cfg(target_os = "linux")]
        build::write_stage("argument-validation-started")?;
        let arguments = arguments::BuildArguments::parse(std::env::args_os())?;
        #[cfg(target_os = "linux")]
        build::write_stage("arguments-verified")?;
        build::run(&arguments)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildError {
    InvalidArguments,
    InvalidCapability,
    #[cfg(any(target_os = "linux", test))]
    InvalidInput,
    #[cfg(any(target_os = "linux", test))]
    InvalidArchive,
    #[cfg(any(target_os = "linux", test))]
    UnsafeArchive,
    #[cfg(any(target_os = "linux", test))]
    InvalidPatch,
    #[cfg(target_os = "linux")]
    ToolNotSelfContained,
    ToolFailed,
    #[cfg(any(target_os = "linux", test))]
    OutputInvalid,
    #[cfg(any(target_os = "linux", test))]
    NativeLayoutInvalid,
    #[cfg(target_os = "linux")]
    EvidenceInvalid,
}

impl BuildError {
    const fn message(self) -> &'static str {
        match self {
            Self::InvalidArguments => "runtime-source-build-error:invalid-arguments\n",
            Self::InvalidCapability => "runtime-source-build-error:invalid-capability\n",
            #[cfg(any(target_os = "linux", test))]
            Self::InvalidInput => "runtime-source-build-error:invalid-input\n",
            #[cfg(any(target_os = "linux", test))]
            Self::InvalidArchive => "runtime-source-build-error:invalid-archive\n",
            #[cfg(any(target_os = "linux", test))]
            Self::UnsafeArchive => "runtime-source-build-error:unsafe-archive\n",
            #[cfg(any(target_os = "linux", test))]
            Self::InvalidPatch => "runtime-source-build-error:invalid-patch\n",
            #[cfg(target_os = "linux")]
            Self::ToolNotSelfContained => "runtime-source-build-error:tool-not-self-contained\n",
            Self::ToolFailed => "runtime-source-build-error:tool-failed\n",
            #[cfg(any(target_os = "linux", test))]
            Self::OutputInvalid => "runtime-source-build-error:output-invalid\n",
            #[cfg(any(target_os = "linux", test))]
            Self::NativeLayoutInvalid => "runtime-source-build-error:native-layout-invalid\n",
            #[cfg(target_os = "linux")]
            Self::EvidenceInvalid => "runtime-source-build-error:evidence-invalid\n",
        }
    }
}
