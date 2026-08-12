//! Private command-line interface for deterministic candidate validation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};
use rewrite_app::{CandidateCheckRequest, CandidateCheckService, MAX_CANDIDATE_CHECK_BYTES};
use rewrite_types::{ReasonCode, RewriteRecord, RewriteStatus};

const ABSTAINED_EXIT_CODE: u8 = 2;

/// Fidelity-gated rewriting prototype.
#[derive(Debug, Parser)]
#[command(name = "retonr", version, about)]
struct Cli {
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
        /// Machine-readable JSON or concise text report.
        #[arg(long, value_enum, default_value_t = ReportFormat::Json)]
        format: ReportFormat,
        /// Return exit code 2 when validation safely abstains.
        #[arg(long)]
        fail_on_abstain: bool,
    },
}

/// CLI report representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ReportFormat {
    /// Pretty-printed JSON transaction record.
    Json,
    /// Concise human-readable transaction summary.
    Text,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(error) => {
            let mut stderr = io::stderr().lock();
            if writeln!(stderr, "error: {error}").is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn Error>> {
    match cli.command {
        Command::Check {
            source,
            candidate,
            protected_terms,
            format,
            fail_on_abstain,
        } => {
            let source_bytes = read_file_bounded(&source, MAX_CANDIDATE_CHECK_BYTES)?;
            let candidate_bytes = read_file_bounded(&candidate, MAX_CANDIDATE_CHECK_BYTES)?;
            let candidate_text = String::from_utf8(candidate_bytes)?;
            let result = CandidateCheckService::check(CandidateCheckRequest {
                source: source_bytes,
                candidate: candidate_text,
                protected_terms,
            })?;
            write_report(&result.record, format)?;
            Ok(exit_code(result.record.status, fail_on_abstain))
        }
    }
}

fn read_file_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    read_bounded(File::open(path)?, limit)
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

fn write_report(record: &RewriteRecord, format: ReportFormat) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    match format {
        ReportFormat::Json => {
            serde_json::to_writer_pretty(&mut stdout, record)?;
            writeln!(stdout)?;
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

fn exit_code(status: RewriteStatus, fail_on_abstain: bool) -> ExitCode {
    match status {
        RewriteStatus::Failed => ExitCode::FAILURE,
        RewriteStatus::Abstained if fail_on_abstain => ExitCode::from(ABSTAINED_EXIT_CODE),
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

    use super::{ABSTAINED_EXIT_CODE, exit_code, read_bounded, reason_name, status_name};

    #[test]
    fn stable_exit_code_policy() {
        assert_eq!(exit_code(RewriteStatus::Rewritten, true), ExitCode::SUCCESS);
        assert_eq!(
            exit_code(RewriteStatus::Abstained, true),
            ExitCode::from(ABSTAINED_EXIT_CODE)
        );
        assert_eq!(
            exit_code(RewriteStatus::Abstained, false),
            ExitCode::SUCCESS
        );
        assert_eq!(exit_code(RewriteStatus::Failed, false), ExitCode::FAILURE);
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
    fn bounded_reader_stops_oversized_input() {
        let exact = read_bounded(Cursor::new(b"abc"), 3).expect("exact limit is valid");
        assert_eq!(exact, b"abc");
        let oversized =
            read_bounded(Cursor::new(b"abcd"), 3).expect_err("input beyond the limit must fail");
        assert_eq!(oversized.kind(), std::io::ErrorKind::InvalidData);
    }
}
