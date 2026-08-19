//! Private command-line interface for deterministic candidate validation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    ffi::OsString,
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{CommandFactory, Parser, Subcommand, error::ErrorKind};
use clap_complete::Shell;
use rewrite_types::CancellationToken;

use crate::contract::{CommandName, ErrorEnvelope, ReportFormat, SuccessEnvelope};
use crate::failure::RunFailure;
use crate::model::{ModelCommand, ModelFailure};

mod check;
mod completions;
pub mod contract;
mod doctor;
mod failure;
mod identity;
mod inspect_source;
mod man;
mod model;
mod rewrite;
mod version;

const EXAMPLES: &str = "\
Examples:
  retonr inspect draft.txt
  retonr rewrite draft.txt -o rewritten.txt
  retonr rewrite draft.txt -i
  retonr check original.txt candidate.txt --diff
  retonr -D .retonr model list
";

/// Fidelity-gated rewriting prototype.
#[derive(Debug, Parser)]
#[command(name = "retonr", version, about, after_help = EXAMPLES)]
struct Cli {
    /// json for machines, text for humans. A terminal defaults to text.
    #[arg(
        short,
        long,
        value_enum,
        value_name = "json|text",
        global = true,
        hide_possible_values = true
    )]
    format: Option<ReportFormat>,
    /// Explicit repository root. Also read from `RETONR_DATA_DIR`.
    #[arg(
        short = 'D',
        long,
        value_name = "DIRECTORY",
        global = true,
        env = "RETONR_DATA_DIR",
        hide_env_values = true
    )]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

/// Supported prototype operations.
#[derive(Debug, Subcommand)]
enum Command {
    /// Rewrite one UTF-8 source after grounded generation and engine gates.
    ///
    /// A recovered generation binding can attach in-process fake-backend
    /// conformance. The command does not start a runtime or access the network.
    Rewrite {
        /// UTF-8 source file, or - for standard input.
        #[arg(value_name = "SOURCE")]
        source: PathBuf,
        /// Write the accepted bytes to a new file, or to - for standard output.
        ///
        /// An existing destination is never replaced. The source is modified only
        /// with --in-place.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Replace the source after retaining a sibling .retonr-backup.
        ///
        /// Standard input, --output, symlinks, and directories are refused.
        #[arg(short, long)]
        in_place: bool,
        /// Accepted with --in-place. Implied by --in-place.
        #[arg(long, hide = true)]
        backup: bool,
        /// Exact installed artifact that must match the active generation binding.
        #[arg(long, visible_alias = "artifact", value_name = "ARTIFACT_ID")]
        artifact_id: Option<crate::contract::ArtifactIdArgument>,
        /// Exact term that must be preserved. May be repeated.
        #[arg(long = "protect", value_name = "TERM")]
        protected_terms: Vec<String>,
    },
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
        /// An existing destination is never replaced. The source is modified only
        /// with --in-place.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Replace the source after retaining a sibling .retonr-backup.
        ///
        /// Standard input, --output, symlinks, and directories are refused.
        #[arg(short, long)]
        in_place: bool,
        /// Accepted with --in-place. Implied by --in-place.
        #[arg(long, hide = true)]
        backup: bool,
        /// Permit exact unescaped bytes on a terminal. Requires --yes.
        ///
        /// Without both flags, a terminal receives escaped rendering.
        #[arg(long)]
        raw_terminal: bool,
        /// Confirm the raw terminal output opt-in.
        #[arg(short = 'y', long)]
        yes: bool,
        /// Write an escaped linear diff of source versus accepted output.
        #[arg(long)]
        diff: bool,
        /// Compute the report without writing --output or replacing the source.
        #[arg(long)]
        dry_run: bool,
        /// Write the redacted rewrite record to a new file.
        #[arg(long, value_name = "PATH")]
        trace: Option<PathBuf>,
    },
    /// Inventory one source document or directory before rewrite without mutation.
    ///
    /// Reports encoding, BOM, newline kind, control-class counts, sibling
    /// sidecar presence, and whether an explicit derivative decision is
    /// required. A directory is a discovery manifest. Recursion is bounded,
    /// does not follow links, and skips hidden names plus `target` and
    /// `node_modules`. It does not parse Content Credentials, follow
    /// external references, or strip bytes.
    Inspect {
        /// UTF-8 source file, directory, or - for standard input.
        #[arg(value_name = "SOURCE")]
        source: PathBuf,
        /// Recurse into real child directories without following links.
        #[arg(short, long)]
        recursive: bool,
    },
    /// Administer exact local model artifacts without network access.
    Model {
        #[command(subcommand)]
        command: ModelCommand,
    },
    /// Report product and machine-contract versions without accessing storage.
    Version,
    /// Inspect local identity, optional repository schema, and recovery needs without mutation.
    Doctor,
    /// Write a completion script for one supported shell.
    ///
    /// JSON reports the shell and script. Text writes the raw script so it can
    /// be sourced or saved without a machine envelope.
    Completions {
        /// Shell that will consume the generated script.
        #[arg(value_enum, value_name = "SHELL")]
        shell: Shell,
    },
    /// Write a generated section-1 manual page for the CLI.
    ///
    /// JSON reports the name, section, and page. Text writes the raw manual
    /// page without a machine envelope.
    Man,
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
        Command::Rewrite {
            source,
            output,
            in_place,
            backup,
            artifact_id,
            protected_terms,
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
            format,
        })
        .map_err(|error| (error, format)),
        Command::Check {
            source,
            candidate,
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
        } => check::run(
            check::CheckRequest {
                source,
                candidate,
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
            },
            format,
        )
        .map_err(|error| (error, format)),
        Command::Inspect { source, recursive } => {
            let (command, output, exit_code) =
                inspect_source::run(&source, recursive).map_err(|error| (error, format))?;
            write_model_report(command, &output, format)
                .map_err(|_| (RunFailure::operational(command), format))?;
            Ok(exit_code)
        }
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
