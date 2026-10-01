//! Private command-line interface for deterministic candidate validation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    ffi::OsString,
    io::{self, IsTerminal, Write},
    process::ExitCode,
};

use clap::{CommandFactory, Parser, error::ErrorKind};
use rewrite_types::CancellationToken;

use crate::args::{Cli, Command};
use crate::contract::{CommandName, ErrorEnvelope, ReportFormat, SuccessEnvelope};
use crate::failure::RunFailure;

mod args;

mod check;
mod completions;
pub mod contract;
mod doctor;
mod edit_level;
pub use edit_level::EditLevelArg;
mod failure;
mod file_input;
mod identity;
mod inspect_source;
mod lint;
mod man;
mod model;
mod render;
mod rewrite;
mod tui;
mod version;

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    let cli = match Cli::try_parse_from(&arguments) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            return if error.print().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Err(_) => {
            let format = requested_format(&arguments);
            let failure = RunFailure::usage();
            if write_failure(&failure, format).is_err() {
                return ExitCode::FAILURE;
            }
            return failure.exit_code;
        }
    };
    match run(cli) {
        Ok(code) => code,
        Err((error, format)) => {
            if write_failure(&error, format).is_err() {
                return ExitCode::FAILURE;
            }
            error.exit_code
        }
    }
}

#[expect(
    clippy::result_large_err,
    reason = "RunFailure carries the typed CLI report contract"
)]
#[expect(
    clippy::too_many_lines,
    reason = "each command arm owns its clap-derived request"
)]
fn run(cli: Cli) -> Result<ExitCode, (RunFailure, ReportFormat)> {
    let format = ReportFormat::from_invocation(cli.format, io::stdout().is_terminal());
    match cli.command {
        Command::Tui(request) => tui::run(&request, cli.format).map_err(|error| (error, format)),
        Command::Rewrite {
            source,
            output,
            in_place,
            backup,
            artifact_id,
            protected_terms,
            fail_on_abstain,
            raw_terminal,
            yes,
            diff,
            dry_run,
            trace,
            recursive,
            output_dir,
        } => rewrite::run(&rewrite::RewriteRequest {
            source,
            output,
            in_place: check::replace::InPlaceFlags {
                requested: in_place,
                backup,
            },
            data_directory: cli.data_dir,
            artifact_id,
            protected_terms,
            fail_on_abstain,
            raw_terminal,
            confirmed: yes,
            inspection: check::CheckInspection {
                diff,
                dry_run,
                trace,
            },
            directory: rewrite::DirectoryFlags {
                recursive,
                output_dir,
            },
            format,
        })
        .map_err(|error| (error, format)),
        Command::Check {
            source,
            candidate,
            recursive,
            protected_terms,
            fail_on_abstain,
            output,
            in_place,
            backup,
            raw_terminal,
            yes,
            diff,
            dry_run,
            trace,
            edit_level,
            max_chars,
            min_chars,
            max_expansion_pct,
            max_lines,
            preserve_line_count,
        } => {
            let character_budget =
                if max_chars.is_some() || min_chars.is_some() || max_expansion_pct.is_some() {
                    Some(rewrite_types::CharacterBudget {
                        min_characters: min_chars,
                        max_characters: max_chars,
                        max_expansion_percent: max_expansion_pct,
                    })
                } else {
                    None
                };
            let line_budget = if max_lines.is_some() || preserve_line_count {
                Some(rewrite_types::LineBudget {
                    max_lines,
                    preserve_line_count,
                })
            } else {
                None
            };
            let layout = if character_budget.is_some() || line_budget.is_some() {
                Some(rewrite_types::LayoutConstraints {
                    character_budget,
                    line_budget,
                })
            } else {
                None
            };
            check::run(
                check::CheckRequest {
                    source,
                    candidate,
                    traversal: if recursive {
                        check::CheckTraversal::Recursive
                    } else {
                        check::CheckTraversal::Flat
                    },
                    protected_terms,
                    fail_on_abstain,
                    output,
                    in_place: check::replace::InPlaceFlags {
                        requested: in_place,
                        backup,
                    },
                    raw_terminal,
                    confirmed: yes,
                    inspection: check::CheckInspection {
                        diff,
                        dry_run,
                        trace,
                    },
                    layout,
                    edit_level: edit_level.map(Into::into),
                },
                format,
            )
            .map_err(|error| (error, format))
        }
        Command::Inspect { source, recursive } => {
            let (command, output, exit_code) =
                inspect_source::run(&source, recursive).map_err(|error| (error, format))?;
            write_model_report(command, &output, format)
                .map_err(|_| (RunFailure::operational(command), format))?;
            Ok(exit_code)
        }
        Command::Model { command } => {
            let command_name = command.name();
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            ctrlc::try_set_handler(move || signal_cancellation.cancel())
                .map_err(|_| (RunFailure::operational(command_name), format))?;
            let success = model::run(command, cli.data_dir, &cancellation)
                .map_err(|error| (RunFailure::from_model(error), format))?;
            write_model_report(command_name, &success.output, format)
                .map_err(|_| (RunFailure::operational(command_name), format))?;
            Ok(success.exit_code)
        }
        Command::Version => {
            let (command, output, exit_code) = version::run();
            write_model_report(command, &output, format)
                .map_err(|_| (RunFailure::operational(command), format))?;
            Ok(exit_code)
        }
        Command::Doctor => {
            let (command, output, exit_code) =
                doctor::run(cli.data_dir).map_err(|error| (error, format))?;
            write_model_report(command, &output, format)
                .map_err(|_| (RunFailure::operational(command), format))?;
            Ok(exit_code)
        }
        Command::Completions { shell } => {
            let mut command = Cli::command();
            let (command_name, output) = completions::run(shell, &mut command);
            write_model_report(command_name, &output, format)
                .map_err(|_| (RunFailure::operational(command_name), format))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Man => {
            let command = Cli::command();
            let (command_name, output) = man::run(&command).map_err(|error| (error, format))?;
            write_model_report(command_name, &output, format)
                .map_err(|_| (RunFailure::operational(command_name), format))?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Lint {
            source,
            recursive,
            candidate,
            fail_on_findings,
        } => lint::run(
            &lint::LintRequest {
                source,
                recursive,
                candidate,
                fail_on_findings,
            },
            format,
        )
        .map_err(|error| (error, format)),
    }
}

fn write_model_report(
    command: CommandName,
    output: &model::ModelOutput,
    format: ReportFormat,
) -> io::Result<()> {
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes =
                render::to_safe_pretty_json(&SuccessEnvelope::new(command, &output.value))
                    .map_err(io::Error::other)?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => output.text.as_bytes().to_vec(),
    };
    io::stdout().lock().write_all(&bytes)
}

fn write_failure(error: &RunFailure, format: ReportFormat) -> io::Result<()> {
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes =
                render::to_safe_pretty_json(&ErrorEnvelope::new(error.command, error.body.clone()))
                    .map_err(io::Error::other)?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => format!("error: {}\n", error.message).into_bytes(),
    };
    io::stderr().lock().write_all(&bytes)
}

fn requested_format(arguments: &[OsString]) -> ReportFormat {
    let mut values = arguments.iter().filter_map(|value| value.to_str());
    while let Some(value) = values.next() {
        if value == "--" {
            break;
        }
        if value == "--format" || value == "-f" {
            return match values.next() {
                Some("text") => ReportFormat::Text,
                _ => ReportFormat::Json,
            };
        }
        if value == "--format=text" || value == "-f=text" || value == "-ftext" {
            return ReportFormat::Text;
        }
        if value == "--format=json" || value == "-f=json" || value == "-fjson" {
            return ReportFormat::Json;
        }
    }
    ReportFormat::from_invocation(None, io::stdout().is_terminal())
}
