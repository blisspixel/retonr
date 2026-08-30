use std::{ffi::OsString, path::Path, process::Command};

use crate::BuildError;

const ZIG_PROGRAM: &str = "RETONR_ZIG_EXECUTABLE";
const ZIG_TARGET: &str = "RETONR_ZIG_TARGET";
const ZIG_HOST_TARGET: &str = "x86_64-linux-musl";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ShimTool {
    CCompiler,
    CxxCompiler,
    HostCxxCompiler,
    Archiver,
    Indexer,
}

pub(super) fn invoked_tool() -> Result<Option<ShimTool>, BuildError> {
    let program = std::env::args_os()
        .next()
        .ok_or(BuildError::InvalidArguments)?;
    classify_program(
        Path::new(&program),
        std::env::var_os("RETONR_CONTROLLED_BUILD_CAPABILITY_ABI").is_some(),
    )
}

fn classify_program(
    program: &Path,
    controlled_launch: bool,
) -> Result<Option<ShimTool>, BuildError> {
    if controlled_launch {
        return Ok(None);
    }
    let name = program
        .file_name()
        .ok_or(BuildError::InvalidArguments)?
        .to_str()
        .ok_or(BuildError::InvalidArguments)?;
    match name {
        "retonr-zig-cc" => Ok(Some(ShimTool::CCompiler)),
        "retonr-zig-cxx" => Ok(Some(ShimTool::CxxCompiler)),
        "retonr-zig-host-cxx" => Ok(Some(ShimTool::HostCxxCompiler)),
        "retonr-zig-ar" => Ok(Some(ShimTool::Archiver)),
        "retonr-zig-ranlib" => Ok(Some(ShimTool::Indexer)),
        "rewrite-runtime-source-builder" | "rewrite-runtime-source-builder.exe" => Ok(None),
        _ => Err(BuildError::InvalidArguments),
    }
}

pub(super) fn run(tool: ShimTool) -> Result<(), BuildError> {
    let zig = std::env::var_os(ZIG_PROGRAM).ok_or(BuildError::InvalidCapability)?;
    let target = std::env::var_os(ZIG_TARGET).ok_or(BuildError::InvalidCapability)?;
    let mut arguments = Vec::new();
    match tool {
        ShimTool::CCompiler => {
            arguments.extend([OsString::from("cc"), OsString::from("-target"), target]);
        }
        ShimTool::CxxCompiler => {
            arguments.extend([OsString::from("c++"), OsString::from("-target"), target]);
        }
        ShimTool::HostCxxCompiler => {
            arguments.extend([
                OsString::from("c++"),
                OsString::from("-target"),
                OsString::from(ZIG_HOST_TARGET),
                OsString::from("-static"),
            ]);
        }
        ShimTool::Archiver => arguments.push(OsString::from("ar")),
        ShimTool::Indexer => arguments.push(OsString::from("ranlib")),
    }
    arguments.extend(std::env::args_os().skip(1));
    let status = Command::new(zig)
        .args(arguments)
        .status()
        .map_err(|_| BuildError::ToolFailed)?;
    if status.success() {
        Ok(())
    } else {
        Err(BuildError::ToolFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shim_names_are_closed_and_unambiguous() {
        let names = [
            ("retonr-zig-cc", ShimTool::CCompiler),
            ("retonr-zig-cxx", ShimTool::CxxCompiler),
            ("retonr-zig-host-cxx", ShimTool::HostCxxCompiler),
            ("retonr-zig-ar", ShimTool::Archiver),
            ("retonr-zig-ranlib", ShimTool::Indexer),
        ];
        assert_eq!(names.len(), 5);
        assert!(names.windows(2).all(|pair| pair[0].0 != pair[1].0));
    }

    #[test]
    fn controlled_launch_marker_selects_the_coordinator_before_fd_argv_zero() {
        assert_eq!(
            classify_program(Path::new("/proc/self/fd/64"), true),
            Ok(None)
        );
        assert_eq!(
            classify_program(Path::new("/proc/self/fd/64"), false),
            Err(BuildError::InvalidArguments)
        );
    }
}
