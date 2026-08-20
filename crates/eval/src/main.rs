//! Command-line runner for deterministic suites, offline baselines, and corpus validation.

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
    MAX_BASELINE_DEFINITION_BYTES, MAX_CLAIM_SHADOW_CALIBRATION_BYTES, MAX_EDITORIAL_CORPUS_BYTES,
    MAX_EVALUATION_SUITE_BYTES, MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES,
    MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES, MAX_WATERMARK_RESEARCH_BYTES,
    MAX_WRITING_SAMPLE_LIBRARY_BYTES, parse_baseline_definition, parse_claim_shadow_calibration,
    parse_editorial_corpus, parse_local_ollama_attested_preflight_plan,
    parse_local_ollama_preflight_plan, parse_suite, parse_watermark_research_corpus,
    parse_writing_sample_library, run_attached_baseline, run_claim_shadow_calibration,
    run_local_ollama_attested_preflight, run_local_ollama_preflight, run_suite,
};
use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};

/// Runs a versioned fidelity suite, baseline, claim-shadow calibration, or corpus validation.
#[derive(Debug, Parser)]
#[command(name = "rewrite-eval", version, about)]
struct Cli {
    /// JSON evaluation suite.
    #[arg(
        value_name = "SUITE",
        required_unless_present_any = [
            "editorial_corpus",
            "writing_samples",
            "watermark_research",
            "claim_shadow_calibration",
            "ollama_preflight",
            "ollama_attested_preflight"
        ],
        conflicts_with_all = [
            "editorial_corpus",
            "writing_samples",
            "watermark_research",
            "claim_shadow_calibration",
            "ollama_preflight",
            "ollama_attested_preflight"
        ]
    )]
    suite: Option<PathBuf>,
    /// Run a versioned offline baseline definition against SUITE.
    #[arg(
        long,
        value_name = "DEFINITION",
        conflicts_with_all = [
            "editorial_corpus",
            "writing_samples",
            "watermark_research",
            "claim_shadow_calibration",
            "ollama_attested_preflight"
        ]
    )]
    baseline: Option<PathBuf>,
    /// Validate a synthetic editorial-quality corpus instead of running a suite.
    #[arg(
        long,
        value_name = "CORPUS",
        conflicts_with_all = [
            "suite",
            "writing_samples",
            "watermark_research",
            "claim_shadow_calibration",
            "baseline",
            "ollama_attested_preflight"
        ]
    )]
    editorial_corpus: Option<PathBuf>,
    /// Validate a writing-sample library.
    #[arg(
        long,
        value_name = "LIBRARY",
        conflicts_with_all = [
            "suite",
            "editorial_corpus",
            "watermark_research",
            "claim_shadow_calibration",
            "baseline",
            "ollama_attested_preflight"
        ]
    )]
    writing_samples: Option<PathBuf>,
    /// Validate a research-only watermark refusal corpus.
    #[arg(
        long,
        value_name = "CORPUS",
        conflicts_with_all = [
            "suite",
            "editorial_corpus",
            "writing_samples",
            "claim_shadow_calibration",
            "baseline",
            "ollama_attested_preflight"
        ]
    )]
    watermark_research: Option<PathBuf>,
    /// Run an independent claim-shadow calibration corpus.
    #[arg(
        long,
        value_name = "CORPUS",
        conflicts_with_all = [
            "suite",
            "editorial_corpus",
            "writing_samples",
            "baseline",
            "ollama_attested_preflight"
        ]
    )]
    claim_shadow_calibration: Option<PathBuf>,
    /// Run one versioned read-only Ollama preflight without generation.
    #[arg(
        long,
        value_name = "PLAN",
        conflicts_with_all = [
            "suite",
            "editorial_corpus",
            "writing_samples",
            "watermark_research",
            "claim_shadow_calibration",
            "baseline",
            "ollama_attested_preflight",
            "data_dir",
            "artifact_id"
        ]
    )]
    ollama_preflight: Option<PathBuf>,
    /// Run one native listener-owner witness around read-only Ollama preflight.
    #[arg(
        long,
        value_name = "PLAN",
        conflicts_with_all = [
            "suite",
            "editorial_corpus",
            "writing_samples",
            "watermark_research",
            "claim_shadow_calibration",
            "baseline",
            "ollama_preflight",
            "data_dir",
            "artifact_id"
        ]
    )]
    ollama_attested_preflight: Option<PathBuf>,
    /// Explicit repository root used to attach recovered fake-backend conformance.
    #[arg(long, value_name = "DIRECTORY")]
    data_dir: Option<PathBuf>,
    /// Exact installed artifact that must match the active generation binding.
    #[arg(long, value_name = "ARTIFACT_ID", requires = "data_dir")]
    artifact_id: Option<String>,
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
    if let Some(path) = cli.ollama_attested_preflight {
        return run_ollama_attested_preflight_command(path);
    }
    if let Some(path) = cli.ollama_preflight {
        let input = read_bounded_bytes(path, MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES)?;
        let plan = parse_local_ollama_preflight_plan(&input)?;
        let cancellation = CancellationToken::new();
        let signal_cancellation = cancellation.clone();
        ctrlc::try_set_handler(move || signal_cancellation.cancel())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let report = runtime.block_on(run_local_ollama_preflight(&plan, &cancellation))?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &report)?;
        writeln!(stdout)?;
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(path) = cli.editorial_corpus {
        let input = read_bounded_utf8(path, MAX_EDITORIAL_CORPUS_BYTES)?;
        let corpus = parse_editorial_corpus(&input)?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &corpus.summary())?;
        writeln!(stdout)?;
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(path) = cli.writing_samples {
        let input = read_bounded_utf8(path, MAX_WRITING_SAMPLE_LIBRARY_BYTES)?;
        let library = parse_writing_sample_library(&input)?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &library.summary())?;
        writeln!(stdout)?;
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(path) = cli.watermark_research {
        let input = read_bounded_utf8(path, MAX_WATERMARK_RESEARCH_BYTES)?;
        let corpus = parse_watermark_research_corpus(&input)?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &corpus.summary())?;
        writeln!(stdout)?;
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(path) = cli.claim_shadow_calibration {
        let input = read_bounded_utf8(path, MAX_CLAIM_SHADOW_CALIBRATION_BYTES)?;
        let corpus = parse_claim_shadow_calibration(&input)?;
        let report = run_claim_shadow_calibration(&corpus);
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &report)?;
        writeln!(stdout)?;
        return Ok(if report.is_success() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }
    let path = cli.suite.ok_or("evaluation suite path is required")?;
    let input = read_bounded_utf8(path, MAX_EVALUATION_SUITE_BYTES)?;
    let suite = parse_suite(&input)?;
    if let Some(definition_path) = cli.baseline {
        let definition_input = read_bounded_utf8(definition_path, MAX_BASELINE_DEFINITION_BYTES)?;
        let definition = parse_baseline_definition(&definition_input)?;
        let requested = cli
            .artifact_id
            .as_deref()
            .map(parse_artifact_id)
            .transpose()?;
        let report = run_attached_baseline(
            &definition,
            &suite,
            cli.data_dir.as_deref(),
            requested.as_ref(),
            &CancellationToken::new(),
        )?;
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &report)?;
        writeln!(stdout)?;
        return Ok(if report.is_success() {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        });
    }
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

fn run_ollama_attested_preflight_command(path: PathBuf) -> Result<ExitCode, Box<dyn Error>> {
    let input = read_bounded_bytes(path, MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES)?;
    let plan = parse_local_ollama_attested_preflight_plan(&input)?;
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let report = runtime.block_on(run_local_ollama_attested_preflight(&plan, &cancellation))?;
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &report)?;
    writeln!(stdout)?;
    Ok(ExitCode::SUCCESS)
}

fn parse_artifact_id(value: &str) -> Result<ArtifactId, Box<dyn Error>> {
    let digest = Digest::from_sha256_hex(value).map_err(|_| "invalid artifact id")?;
    Ok(ArtifactId::from_digest(digest))
}

fn read_bounded_utf8(path: PathBuf, maximum: usize) -> Result<String, Box<dyn Error>> {
    Ok(String::from_utf8(read_bounded_bytes(path, maximum)?)?)
}

fn read_bounded_bytes(path: PathBuf, maximum: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(64 * 1024);
    File::open(path)?
        .take(u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err("input exceeds the supported byte limit".into());
    }
    Ok(bytes)
}
