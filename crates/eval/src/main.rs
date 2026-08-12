//! Command-line runner for deterministic evaluation suites.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::Parser;
use rewrite_eval::{MAX_EVALUATION_SUITE_BYTES, parse_suite, run_suite};

/// Runs a versioned fidelity regression suite.
#[derive(Debug, Parser)]
#[command(name = "rewrite-eval", version, about)]
struct Cli {
    /// JSON evaluation suite.
    #[arg(value_name = "SUITE")]
    suite: PathBuf,
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
    let mut bytes = Vec::with_capacity(64 * 1024);
    File::open(cli.suite)?
        .take(
            u64::try_from(MAX_EVALUATION_SUITE_BYTES)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_EVALUATION_SUITE_BYTES {
        return Err("evaluation suite exceeds the supported byte limit".into());
    }
    let input = String::from_utf8(bytes)?;
    let suite = parse_suite(&input)?;
    let report = run_suite(&suite);
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    writeln!(stdout)?;
    Ok(if report.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
