//! Generated shell-completion scripts for the local CLI.

use clap::Command;
use clap_complete::{Shell, generate};
use serde::Serialize;

use crate::contract::CommandName;
use crate::model::ModelOutput;

#[derive(Serialize)]
struct CompletionsReport {
    shell: String,
    script: String,
}

/// Builds a completion script for `shell` from the live CLI definition.
pub(crate) fn run(shell: Shell, command: &mut Command) -> (CommandName, ModelOutput) {
    let script = script_for(shell, command);
    let report = CompletionsReport {
        shell: shell.to_string(),
        script: script.clone(),
    };
    (
        CommandName::Completions,
        ModelOutput {
            value: serde_json::to_value(&report).expect("completions report serializes"),
            text: script,
            findings: false,
        },
    )
}

fn script_for(shell: Shell, command: &mut Command) -> String {
    let mut buffer = Vec::new();
    generate(shell, command, "retonr", &mut buffer);
    String::from_utf8(buffer).expect("clap_complete emits UTF-8")
}

#[cfg(test)]
mod tests {
    use clap::Command;
    use clap_complete::Shell;

    use super::script_for;

    fn fixture_command() -> Command {
        Command::new("retonr").subcommand(Command::new("check"))
    }

    #[test]
    fn bash_script_names_the_binary() {
        let mut command = fixture_command();
        let script = script_for(Shell::Bash, &mut command);
        assert!(script.contains("retonr"));
        assert!(!script.is_empty());
    }

    #[test]
    fn powershell_script_names_the_binary() {
        let mut command = fixture_command();
        let script = script_for(Shell::PowerShell, &mut command);
        assert!(script.contains("retonr"));
        assert!(!script.is_empty());
    }
}
