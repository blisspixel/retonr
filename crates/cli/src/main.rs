//! Private command-line interface for deterministic candidate validation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    ffi::OsString,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand, error::ErrorKind};
use rewrite_app::{
    AppError, CandidateCheckRequest, CandidateCheckService, MAX_CANDIDATE_CHECK_BYTES,
};
use rewrite_types::{CancellationToken, ReasonCode, RewriteRecord, RewriteStatus};

use crate::contract::{
    CommandName, EXIT_CANCELLED, EXIT_COMPATIBILITY, EXIT_OPERATIONAL, EXIT_POLICY, EXIT_USAGE,
    ErrorBody, ErrorCategory, ErrorCode, ErrorEnvelope, ReportFormat, SuccessEnvelope,
    open_regular_file,
};
use crate::model::{ModelCommand, ModelFailure};

pub mod contract;
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
        /// UTF-8 source file to protect and validate against.
        #[arg(value_name = "SOURCE")]
        source: PathBuf,
        /// UTF-8 file containing the complete proposed replacement.
        #[arg(value_name = "CANDIDATE")]
        candidate: PathBuf,
        /// Exact term that must be preserved. May be repeated.
        #[arg(long = "protect", value_name = "TERM")]
        protected_terms: Vec<String>,
        /// Return exit code 3 when validation safely abstains.
        #[arg(long)]
        fail_on_abstain: bool,
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
        } => {
            let cancellation = CancellationToken::new();
            let signal_cancellation = cancellation.clone();
            ctrlc::try_set_handler(move || signal_cancellation.cancel())
                .map_err(|_| (RunFailure::operational(CommandName::Check), format))?;
            let source_bytes = read_file_bounded(&source, MAX_CANDIDATE_CHECK_BYTES)
                .map_err(|error| (RunFailure::check_read(&error), format))?;
            let candidate_bytes = read_file_bounded(&candidate, MAX_CANDIDATE_CHECK_BYTES)
                .map_err(|error| (RunFailure::check_read(&error), format))?;
            let candidate_text = String::from_utf8(candidate_bytes)
                .map_err(|_| (RunFailure::usage_for(CommandName::Check), format))?;
            let result = CandidateCheckService::check_with_cancellation(
                CandidateCheckRequest {
                    source: source_bytes,
                    candidate: candidate_text,
                    protected_terms,
                },
                &cancellation,
            )
            .map_err(|error| (RunFailure::check_app(&error), format))?;
            write_report(&result.record, format)
                .map_err(|_| (RunFailure::operational(CommandName::Check), format))?;
            Ok(exit_code(&result.record, fail_on_abstain))
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
    }
}

fn read_file_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    read_bounded(open_regular_file(path)?, limit)
}

fn read_bounded(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    reader.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input exceeds the supported byte limit",
        ));
    }
    Ok(bytes)
}

fn write_report(record: &RewriteRecord, format: ReportFormat) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    match format {
        ReportFormat::Json => {
            let mut bytes =
                serde_json::to_vec_pretty(&SuccessEnvelope::new(CommandName::Check, record))
                    .map_err(io::Error::other)?;
            bytes.push(b'\n');
            stdout.write_all(&bytes)?;
        }
        ReportFormat::Text => {
            writeln!(stdout, "status: {}", status_name(record.status))?;
            if let Some(reason) = record.reason {
                writeln!(stdout, "reason: {}", reason_name(reason))?;
            }
            writeln!(stdout, "source_digest: {}", record.source_digest.as_str())?;
            writeln!(stdout, "output_digest: {}", record.output_digest.as_str())?;
            writeln!(stdout, "candidates: {}", record.assessments.len())?;
            writeln!(
                stdout,
                "eligible_candidates: {}",
                record
                    .assessments
                    .iter()
                    .filter(|assessment| assessment.eligible)
                    .count()
            )?;
        }
    }
    Ok(())
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

struct RunFailure {
    command: CommandName,
    body: ErrorBody,
    exit_code: ExitCode,
    message: &'static str,
}

impl RunFailure {
    fn usage() -> Self {
        Self::usage_for(CommandName::Cli)
    }

    fn usage_for(command: CommandName) -> Self {
        Self {
            command,
            body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
            exit_code: ExitCode::from(crate::contract::EXIT_USAGE),
            message: "command input is invalid",
        }
    }

    fn operational(command: CommandName) -> Self {
        Self {
            command,
            body: ErrorBody::new(
                ErrorCategory::Operational,
                ErrorCode::OperationalFailure,
                true,
            ),
            exit_code: ExitCode::from(EXIT_OPERATIONAL),
            message: "operation failed",
        }
    }

    fn check_read(error: &io::Error) -> Self {
        if error.kind() == io::ErrorKind::InvalidInput {
            return Self {
                command: CommandName::Check,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "input must be a regular file",
            };
        }
        if error.kind() == io::ErrorKind::InvalidData {
            Self {
                command: CommandName::Check,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported byte limit",
            }
        } else {
            Self {
                command: CommandName::Check,
                body: ErrorBody::new(
                    ErrorCategory::Operational,
                    ErrorCode::InputUnreadable,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_OPERATIONAL),
                message: "input could not be read",
            }
        }
    }

    fn check_app(error: &AppError) -> Self {
        match error {
            AppError::CandidateTooLarge { .. } => Self {
                command: CommandName::Check,
                body: ErrorBody::new(
                    ErrorCategory::Compatibility,
                    ErrorCode::ResourceLimitExceeded,
                    false,
                ),
                exit_code: ExitCode::from(EXIT_COMPATIBILITY),
                message: "input exceeds the supported byte limit",
            },
            AppError::TextAdapter(_) => Self {
                command: CommandName::Check,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "source text is not a supported UTF-8 document",
            },
            AppError::Engine(_) | AppError::Protection(_) => Self {
                command: CommandName::Check,
                body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
                exit_code: ExitCode::from(EXIT_USAGE),
                message: "command input is invalid",
            },
            AppError::Grounded(_) => Self::operational(CommandName::Check),
        }
    }

    fn from_model(error: ModelFailure) -> Self {
        Self {
            command: error.command,
            body: error.body,
            exit_code: error.exit_code,
            message: error.message,
        }
    }
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

fn exit_code(record: &RewriteRecord, fail_on_abstain: bool) -> ExitCode {
    exit_status(record.status, record.reason, fail_on_abstain)
}

fn exit_status(
    status: RewriteStatus,
    reason: Option<ReasonCode>,
    fail_on_abstain: bool,
) -> ExitCode {
    if reason == Some(ReasonCode::Cancelled) {
        return ExitCode::from(EXIT_CANCELLED);
    }
    match status {
        RewriteStatus::Failed => ExitCode::FAILURE,
        RewriteStatus::Abstained if fail_on_abstain => ExitCode::from(EXIT_POLICY),
        RewriteStatus::Rewritten
        | RewriteStatus::UnchangedNoEligibleContent
        | RewriteStatus::Abstained => ExitCode::SUCCESS,
    }
}

const fn status_name(status: RewriteStatus) -> &'static str {
    match status {
        RewriteStatus::Rewritten => "rewritten",
        RewriteStatus::UnchangedNoEligibleContent => "unchanged_no_eligible_content",
        RewriteStatus::Abstained => "abstained",
        RewriteStatus::Failed => "failed",
    }
}

const fn reason_name(reason: ReasonCode) -> &'static str {
    match reason {
        ReasonCode::NoCandidate => "no_candidate",
        ReasonCode::InvalidCandidate => "invalid_candidate",
        ReasonCode::SentinelIntegrity => "sentinel_integrity",
        ReasonCode::ProtectedValueChanged => "protected_value_changed",
        ReasonCode::StructureChanged => "structure_changed",
        ReasonCode::UnsafeText => "unsafe_text",
        ReasonCode::SemanticMismatch => "semantic_mismatch",
        ReasonCode::SemanticUncertain => "semantic_uncertain",
        ReasonCode::ReassemblyVerification => "reassembly_verification",
        ReasonCode::Cancelled => "cancelled",
        ReasonCode::UnsupportedAtomicity => "unsupported_atomicity",
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, process::ExitCode};

    use rewrite_types::{ReasonCode, RewriteStatus};

    use super::{exit_status, read_bounded, reason_name, status_name};
    use crate::contract::{EXIT_CANCELLED, EXIT_POLICY};

    #[test]
    fn stable_exit_code_policy() {
        assert_eq!(
            exit_status(RewriteStatus::Rewritten, None, true),
            ExitCode::SUCCESS
        );
        assert_eq!(
            exit_status(RewriteStatus::Abstained, None, true),
            ExitCode::from(EXIT_POLICY)
        );
        assert_eq!(
            exit_status(RewriteStatus::Abstained, None, false),
            ExitCode::SUCCESS
        );
        assert_eq!(
            exit_status(RewriteStatus::Failed, None, false),
            ExitCode::FAILURE
        );
        assert_eq!(
            exit_status(RewriteStatus::Abstained, Some(ReasonCode::Cancelled), false),
            ExitCode::from(EXIT_CANCELLED)
        );
    }

    #[test]
    fn text_names_match_serialized_contract() {
        assert_eq!(status_name(RewriteStatus::Rewritten), "rewritten");
        assert_eq!(
            reason_name(ReasonCode::ProtectedValueChanged),
            "protected_value_changed"
        );
    }

    #[test]
    fn check_read_classifies_limit_and_missing_inputs() {
        use super::RunFailure;
        use crate::contract::{CommandName, ErrorBody, ErrorCategory, ErrorCode};

        let limit = RunFailure::check_read(&std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "input exceeds the supported byte limit",
        ));
        assert_eq!(limit.command, CommandName::Check);
        assert_eq!(
            limit.body,
            ErrorBody::new(
                ErrorCategory::Compatibility,
                ErrorCode::ResourceLimitExceeded,
                false
            )
        );

        let missing = RunFailure::check_read(&std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
        assert_eq!(
            missing.body,
            ErrorBody::new(
                ErrorCategory::Operational,
                ErrorCode::InputUnreadable,
                false
            )
        );
    }

    #[test]
    fn bounded_reader_stops_oversized_input() {
        let exact = read_bounded(Cursor::new(b"abc"), 3).expect("exact limit is valid");
        assert_eq!(exact, b"abc");
        let oversized =
            read_bounded(Cursor::new(b"abcd"), 3).expect_err("input beyond the limit must fail");
        assert_eq!(oversized.kind(), std::io::ErrorKind::InvalidData);
    }
}
