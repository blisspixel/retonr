//! Experimental read-only document review through the shared application core.

use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use clap::Args;
use rewrite_types::CancellationToken;
use serde::Serialize;

use crate::{
    contract::{CommandName, ReportFormat, SuccessEnvelope},
    failure::RunFailure,
};

mod event;
mod service;
mod state;
mod terminal;
mod ui;
mod worker;

/// Read-only workbench options. Interactive review requires real terminal streams.
#[derive(Args, Clone, Debug)]
pub(crate) struct TuiArgs {
    /// UTF-8 source file to inspect without mutation.
    #[arg(value_name = "SOURCE")]
    pub(crate) source: PathBuf,
    /// Optional supplied candidate to compare and validate.
    #[arg(long, value_name = "FILE")]
    pub(crate) candidate: Option<PathBuf>,
    /// Exact term that must be preserved. May be repeated.
    #[arg(long = "protect", value_name = "TERM", requires = "candidate")]
    pub(crate) protected_terms: Vec<String>,
    /// Print a bounded linear review for accessible terminals and redirected output.
    #[arg(long)]
    pub(crate) plain: bool,
}

pub(crate) fn run(
    request: &TuiArgs,
    explicit_format: Option<ReportFormat>,
) -> Result<ExitCode, RunFailure> {
    validate_mode(
        request,
        explicit_format,
        io::stdin().is_terminal(),
        io::stdout().is_terminal(),
    )?;
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())
        .map_err(|_| RunFailure::operational(CommandName::Tui))?;
    if request.plain {
        let snapshot = service::load(request, &cancellation)?.into_presentation();
        require_not_cancelled(&cancellation)?;
        let format = ReportFormat::from_invocation(explicit_format, io::stdout().is_terminal());
        let output = write_plain(&snapshot, format, &mut io::stdout().lock());
        finish_output(output, &cancellation)?;
        return Ok(ExitCode::SUCCESS);
    }
    interactive(request, &cancellation)
}

fn validate_mode(
    request: &TuiArgs,
    explicit_format: Option<ReportFormat>,
    stdin_terminal: bool,
    stdout_terminal: bool,
) -> Result<(), RunFailure> {
    if !request.plain
        && (!stdin_terminal || !stdout_terminal || explicit_format == Some(ReportFormat::Json))
    {
        let mut failure = RunFailure::usage_for(CommandName::Tui);
        failure.message = "interactive review requires terminal input and output; use --plain for a linear review";
        return Err(failure);
    }
    Ok(())
}

fn interactive(
    request: &TuiArgs,
    cancellation: &CancellationToken,
) -> Result<ExitCode, RunFailure> {
    let mut worker = worker::Worker::new(|request, cancellation| {
        service::load(&request, cancellation)
            .map(service::Snapshot::into_presentation)
            .map_err(|error| error.message)
    })
    .map_err(|_| RunFailure::operational(CommandName::Tui))?;
    let result = terminal::with_terminal(|terminal| {
        let mut state = state::State::default();
        submit(&mut state, &worker, request);
        while !state.quit && !cancellation.is_cancelled() {
            if let Some(response) = worker.response() {
                if response.stopped {
                    return Err(io::Error::other("document review worker stopped"));
                }
                state.complete(response.operation, response.result);
            }
            terminal.draw(|frame| ui::draw(frame, &state))?;
            if crossterm::event::poll(Duration::from_millis(50))? {
                let action = event::action(&crossterm::event::read()?);
                if action == event::Action::Interrupt {
                    cancellation.cancel();
                }
                if state.apply(action) {
                    submit(&mut state, &worker, request);
                }
            }
        }
        Ok(())
    });
    // Restore the terminal before joining any noncooperative lint computation.
    worker.shutdown();
    finish_output(result, cancellation)?;
    Ok(ExitCode::SUCCESS)
}

fn finish_output(
    result: io::Result<()>,
    cancellation: &CancellationToken,
) -> Result<(), RunFailure> {
    require_not_cancelled(cancellation)?;
    result.map_err(|_| RunFailure::operational(CommandName::Tui))
}

fn submit(state: &mut state::State, worker: &worker::Worker<TuiArgs>, request: &TuiArgs) {
    if let Some(operation) = state.begin()
        && !worker.submit(operation, request.clone())
    {
        state.complete(operation, Err("document review worker is unavailable"));
    }
}

fn require_not_cancelled(cancellation: &CancellationToken) -> Result<(), RunFailure> {
    if cancellation.is_cancelled() {
        Err(RunFailure::cancelled(CommandName::Tui))
    } else {
        Ok(())
    }
}

#[derive(Serialize)]
struct PlainReview<'a> {
    source_label: &'a str,
    candidate_label: Option<&'a str>,
    source_preview: &'a str,
    candidate_preview: Option<&'a str>,
    findings: &'a [String],
    summary: &'a str,
}

fn write_plain(
    snapshot: &state::Snapshot,
    format: ReportFormat,
    output: &mut impl Write,
) -> io::Result<()> {
    if format == ReportFormat::Json {
        let review = PlainReview {
            source_label: &snapshot.source_label,
            candidate_label: snapshot.candidate_label.as_deref(),
            source_preview: &snapshot.source,
            candidate_preview: snapshot.candidate.as_deref(),
            findings: &snapshot.findings,
            summary: &snapshot.status,
        };
        let bytes =
            crate::render::to_safe_pretty_json(&SuccessEnvelope::new(CommandName::Tui, review))
                .map_err(io::Error::other)?;
        output.write_all(&bytes)?;
        return output.write_all(b"\n");
    }
    writeln!(output, "Retonr document review")?;
    writeln!(output, "{}", snapshot.status)?;
    writeln!(
        output,
        "\nSource: {}\n{}",
        snapshot.source_label, snapshot.source
    )?;
    if let Some(candidate) = &snapshot.candidate {
        writeln!(
            output,
            "\nCandidate: {}\n{candidate}",
            snapshot
                .candidate_label
                .as_deref()
                .unwrap_or("supplied candidate")
        )?;
    }
    writeln!(output, "\nFindings")?;
    for finding in &snapshot.findings {
        writeln!(output, "{finding}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
