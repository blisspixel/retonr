//! Command-line runner for deterministic suites and editorial corpus validation.

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
use rewrite_eval::{
    MAX_EDITORIAL_CORPUS_BYTES, MAX_EVALUATION_SUITE_BYTES, parse_editorial_corpus, parse_suite,
    run_suite,
};

/// Runs a versioned fidelity suite or validates an editorial-quality corpus.
#[derive(Debug, Parser)]
#[command(name = "rewrite-eval", version, about)]
struct Cli {
    /// JSON evaluation suite.
    #[arg(
        value_name = "SUITE",
        required_unless_present = "editorial_corpus",
        conflicts_with = "editorial_corpus"
    )]
    suite: Option<PathBuf>,
    /// Validate a synthetic editorial-quality corpus instead of running a suite.
    #[arg(long, value_name = "CORPUS", conflicts_with = "suite")]
    editorial_corpus: Option<PathBuf>,
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
    if let Some(path) = cli.editorial_corpus {
        let input = read_bounded_utf8(path, MAX_EDITORIAL_CORPUS_BYTES)?;
        let corpus = parse_editorial_corpus(&input)?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &corpus.summary())?;
        writeln!(stdout)?;
        return Ok(ExitCode::SUCCESS);
    }
    let path = cli.suite.ok_or("evaluation suite path is required")?;
    let input = read_bounded_utf8(path, MAX_EVALUATION_SUITE_BYTES)?;
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

fn read_bounded_utf8(path: PathBuf, maximum: usize) -> Result<String, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(64 * 1024);
    File::open(path)?
        .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err("input exceeds the supported byte limit".into());
    }
    Ok(String::from_utf8(bytes)?)
}
