use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{Read, Write as _},
    path::Path,
    process::{Command, Stdio},
    thread,
};

use crate::BuildError;

const TOOL_DIAGNOSTIC_TAIL_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug)]
pub(super) struct BuildEnvironment {
    values: BTreeMap<OsString, OsString>,
}

impl BuildEnvironment {
    pub(super) fn from_controlled_process() -> Result<Self, BuildError> {
        let mut values = BTreeMap::new();
        for (name, value) in std::env::vars_os() {
            let text = name.to_string_lossy();
            if text.starts_with("RETONR_CONTROLLED_BUILD_") {
                continue;
            }
            if name.is_empty() || values.insert(name, value).is_some() {
                return Err(BuildError::InvalidCapability);
            }
        }
        Ok(Self { values })
    }

    pub(super) fn insert(&mut self, name: impl Into<OsString>, value: impl Into<OsString>) {
        self.values.insert(name.into(), value.into());
    }

    pub(super) fn run(
        &self,
        stage: &'static str,
        program: &Path,
        current_directory: &Path,
        arguments: &[OsString],
    ) -> Result<(), BuildError> {
        let mut child = Command::new(program)
            .args(arguments)
            .current_dir(current_directory)
            .env_clear()
            .envs(&self.values)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| BuildError::ToolFailed)?;
        let standard_output = child.stdout.take().ok_or(BuildError::ToolFailed)?;
        let standard_error = child.stderr.take().ok_or(BuildError::ToolFailed)?;
        let output_drain = thread::spawn(move || drain_tail(standard_output));
        let error_drain = thread::spawn(move || drain_tail(standard_error));
        let status = child.wait().map_err(|_| BuildError::ToolFailed)?;
        let standard_output = output_drain.join().map_err(|_| BuildError::ToolFailed)??;
        let standard_error = error_drain.join().map_err(|_| BuildError::ToolFailed)??;
        if status.success() {
            Ok(())
        } else {
            write_diagnostic_tail(std::io::stdout(), stage, &standard_output)?;
            write_diagnostic_tail(std::io::stderr(), stage, &standard_error)?;
            let message = format!("runtime-source-build-tool-failed:{stage}\n");
            let _ = std::io::stderr().write_all(message.as_bytes());
            Err(BuildError::ToolFailed)
        }
    }
}

fn drain_tail(mut input: impl Read) -> Result<Vec<u8>, BuildError> {
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut tail = Vec::new();
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|_| BuildError::ToolFailed)?;
        if read == 0 {
            return Ok(tail);
        }
        retain_tail(&mut tail, &buffer[..read]);
    }
}

fn retain_tail(tail: &mut Vec<u8>, bytes: &[u8]) {
    if bytes.len() >= TOOL_DIAGNOSTIC_TAIL_BYTES {
        tail.clear();
        tail.extend_from_slice(&bytes[bytes.len() - TOOL_DIAGNOSTIC_TAIL_BYTES..]);
        return;
    }
    let combined = tail.len() + bytes.len();
    if combined > TOOL_DIAGNOSTIC_TAIL_BYTES {
        tail.drain(..combined - TOOL_DIAGNOSTIC_TAIL_BYTES);
    }
    tail.extend_from_slice(bytes);
}

fn write_diagnostic_tail(
    mut destination: impl std::io::Write,
    stage: &str,
    bytes: &[u8],
) -> Result<(), BuildError> {
    if !bytes.is_empty() {
        writeln!(
            destination,
            "runtime-source-build-tool-diagnostic-tail:{stage}"
        )
        .and_then(|()| destination.write_all(bytes))
        .and_then(|()| {
            if bytes.ends_with(b"\n") {
                Ok(())
            } else {
                destination.write_all(b"\n")
            }
        })
        .map_err(|_| BuildError::ToolFailed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_only_the_bounded_diagnostic_tail() {
        let input = (0..TOOL_DIAGNOSTIC_TAIL_BYTES + 17)
            .map(|value| u8::try_from(value % 251).expect("bounded byte"))
            .collect::<Vec<_>>();
        let observed = drain_tail(std::io::Cursor::new(&input)).expect("drain input");
        assert_eq!(observed, input[17..]);
    }

    #[test]
    fn fragmented_diagnostic_tail_matches_one_shot_input() {
        let mut fragmented = Vec::new();
        for _ in 0..3 {
            retain_tail(&mut fragmented, &[7; TOOL_DIAGNOSTIC_TAIL_BYTES / 2]);
        }
        assert_eq!(fragmented, vec![7; TOOL_DIAGNOSTIC_TAIL_BYTES]);
    }
}

pub(super) fn owned_values(values: impl IntoIterator<Item = String>) -> Vec<OsString> {
    values.into_iter().map(OsString::from).collect()
}
