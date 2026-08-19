//! Generated manual page for the local CLI.

use clap::Command;
use clap_mangen::Man;
use serde::Serialize;

use crate::contract::CommandName;
use crate::failure::RunFailure;
use crate::model::ModelOutput;

#[derive(Serialize)]
struct ManReport {
    name: &'static str,
    section: &'static str,
    page: String,
}

/// Renders a section-1 manual page from the live CLI definition.
pub(crate) fn run(command: &Command) -> Result<(CommandName, ModelOutput), RunFailure> {
    let page = render(command)?;
    let report = ManReport {
        name: "retonr",
        section: "1",
        page: page.clone(),
    };
    Ok((
        CommandName::Man,
        ModelOutput {
            value: serde_json::to_value(&report).expect("man report serializes"),
            text: page,
            findings: false,
        },
    ))
}

fn render(command: &Command) -> Result<String, RunFailure> {
    let mut buffer = Vec::new();
    Man::new(command.clone())
        .title("retonr")
        .section("1")
        .render(&mut buffer)
        .map_err(|_| RunFailure::operational(CommandName::Man))?;
    String::from_utf8(buffer).map_err(|_| RunFailure::operational(CommandName::Man))
}

#[cfg(test)]
mod tests {
    use clap::Command;

    use super::render;

    #[test]
    fn page_is_a_section_one_manual() {
        let command = Command::new("retonr")
            .about("Fidelity-gated rewriting prototype.")
            .subcommand(Command::new("check"));
        let page = render(&command).expect("man page renders");
        assert!(page.contains(".TH"));
        assert!(page.to_ascii_lowercase().contains("retonr"));
        assert!(page.contains("\n.SH NAME\n") || page.contains(".SH NAME"));
    }
}
