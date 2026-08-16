//! Private command-line interface for deterministic candidate validation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Parser, Subcommand, error::ErrorKind};
use rewrite_types::CancellationToken;

use crate::contract::{CommandName, ErrorEnvelope, ReportFormat, SuccessEnvelope};
use crate::failure::RunFailure;
use crate::model::{ModelCommand, ModelFailure};

mod check;
pub mod contract;
mod failure;
mod model;

/// Fidelity-gated rewriting prototype.
#[derive(Debug, Parser)]
#[command(name = "retonr", version, about)]
struct Cli {
    /// Versioned JSON or concise human-readable output.
    #[arg(long, value_enum, default_value_t = ReportFormat::Json, global = true)]
    format: ReportFormat,
    /// Explicit fixed repository root required by every model command.
    #[arg(long, value_name = "DIRECTORY", global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

/// Supported prototype operations.
#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a supplied plain-text candidate without using a model.
    Check {
        /// UTF-8 source file to protect and validate against, or - for standard input.
        #[arg(value_name = "SOURCE")]
        source: PathBuf,
        /// UTF-8 file containing the complete proposed replacement, or - for standard input.
        #[arg(value_name = "CANDIDATE")]
        candidate: PathBuf,
        /// Exact term that must be preserved. May be repeated.
        #[arg(long = "protect", value_name = "TERM")]
        protected_terms: Vec<String>,
        /// Return exit code 3 when validation safely abstains.
        #[arg(long)]
        fail_on_abstain: bool,
        /// Write the exact accepted bytes to a new file, or to - for standard output.
        ///
        /// An existing destination is never replaced and the source is never modified.
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Permit exact unescaped bytes on a terminal. Requires --yes.
        #[arg(long)]
        raw_terminal: bool,
        /// Confirm the raw terminal output opt-in.
        #[arg(long)]
        yes: bool,
    },
    /// Administer exact local model artifacts without network access.
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
}

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

fn run(cli: Cli) -> Result<ExitCode, (RunFailure, ReportFormat)> {
    let format = cli.format;
    match cli.command {
        Command::Check {
            source,
            candidate,
            protected_terms,
            fail_on_abstain,
            output,
            raw_terminal,
            yes,
        } => check::run(
            check::CheckRequest {
                source,
                candidate,
                protected_terms,
                fail_on_abstain,
                output,
                raw_terminal,
                confirmed: yes,
            },
            format,
        )
        .map_err(|error| (error, format)),
        Command::Model { command } => {
            let command_name = command.name();
            let data_directory = cli.data_dir.ok_or_else(|| {
                (
                    RunFailure::from_model(ModelFailure::missing_data_directory(command_name)),
                    format,
                )
            })?;
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            ctrlc::try_set_handler(move || signal_cancellation.cancel())
                .map_err(|_| (RunFailure::operational(command_name), format))?;
            let success = model::run(command, data_directory, &cancellation)
                .map_err(|error| (RunFailure::from_model(error), format))?;
            write_model_report(command_name, &success.output, format)
                .map_err(|_| (RunFailure::operational(command_name), format))?;
            Ok(success.exit_code)
        }
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
                serde_json::to_vec_pretty(&SuccessEnvelope::new(command, &output.value))
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
                serde_json::to_vec_pretty(&ErrorEnvelope::new(error.command, error.body.clone()))
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
        if value == "--format" {
            return if values.next() == Some("text") {
                ReportFormat::Text
            } else {
                ReportFormat::Json
            };
        }
        if value == "--format=text" {
            return ReportFormat::Text;
        }
    }
    ReportFormat::Json
}
