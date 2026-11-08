//! Explicit offline removal of selected query parameters.
//!
//! The command writes a new copy only when asked. It does not fetch, decode,
//! or follow a link, and check and rewrite do not call it.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Args, Subcommand};
use rewrite_app::{
    LINK_RULE_CATALOG_VERSION, LinkCleanGroup, LinkCleanScope, LinkError,
    MAX_CANDIDATE_CHECK_BYTES, clean_links,
};
use rewrite_types::CancellationToken;
use serde::Serialize;

use crate::{
    check::{self, OutputSink, ReportTarget},
    contract::{
        CommandName, EXIT_COMPATIBILITY, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode,
        ReportFormat, STANDARD_STREAM_PATH, SuccessEnvelope, read_input_bounded,
    },
    failure::RunFailure,
    presentation, render,
};

/// Link operations. Cleanup is the only one, and it is never the default of check.
#[derive(Debug, Subcommand)]
pub(crate) enum LinksAction {
    /// Remove selected query parameters from one document and write a new copy.
    ///
    /// The default removes only the exact pair `utm_source=chatgpt.com`. Other
    /// families stay until their flag is set. The source file is not modified.
    Clean(CleanArgs),
}

/// Arguments for one explicit link cleanup.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each cleanup family is an independent opt-in flag"
)]
#[derive(Debug, Args)]
pub(crate) struct CleanArgs {
    /// UTF-8 document to copy, or - for standard input.
    #[arg(value_name = "SOURCE")]
    source: PathBuf,
    /// Write the cleaned bytes to a new file, or to - for standard output.
    ///
    /// An existing destination is never replaced. Without this flag the report
    /// is printed and no document is written.
    #[arg(short, long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Report the removals and write nothing.
    #[arg(long)]
    dry_run: bool,
    /// Also remove Google Analytics campaign keys, whatever their values.
    #[arg(long)]
    campaign: bool,
    /// Also remove the reviewed advertising and commerce click-identifier keys.
    #[arg(long)]
    tracking: bool,
    /// Also remove the exact pair `source=chatgpt.com`.
    ///
    /// Other source values stay. A generic source parameter can select content.
    #[arg(long = "source-chatgpt")]
    source_chatgpt: bool,
    /// Also remove the exact pair `sz=32`.
    ///
    /// Other sz values stay. sz is an image size in some photo URLs, not a tracker class.
    #[arg(long = "sz-32")]
    sz_32: bool,
    /// Limit cleanup to URLs whose host equals HOST byte for byte.
    ///
    /// No DNS, no case folding, and no port. Omit the flag to accept every host.
    #[arg(long, value_name = "HOST")]
    host: Option<String>,
    /// Limit cleanup to URLs whose path is PREFIX or a child of PREFIX.
    ///
    /// The path includes its leading slash. `/guide` matches `/guide` and
    /// `/guide/page`, and does not match `/guidebook`.
    #[arg(long = "path-prefix", value_name = "PREFIX")]
    path_prefix: Option<String>,
    /// Permit exact unescaped bytes on a terminal. Requires --yes.
    #[arg(long)]
    raw_terminal: bool,
    /// Confirm the raw terminal output opt-in.
    #[arg(short = 'y', long)]
    yes: bool,
}

/// Runs one link operation.
pub(crate) fn run(action: &LinksAction, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    match action {
        LinksAction::Clean(args) => clean(args, format),
    }
}

fn clean(args: &CleanArgs, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    validate(args)?;
    let sink = check::resolve_output_sink_for(args.output.as_deref(), CommandName::LinksClean)?;
    check::replace::validate_destination_preflight(
        &check::replace::Destination::Sink(sink.clone()),
        args.dry_run,
        CommandName::LinksClean,
    )?;
    let cancellation = CancellationToken::new();
    let signal_cancellation = cancellation.clone();
    ctrlc::try_set_handler(move || signal_cancellation.cancel())
        .map_err(|_| RunFailure::operational(CommandName::LinksClean))?;
    let text = read_document(&args.source)?;
    let cleaned = clean_links(&text, &selected_groups(args), scope(args))
        .map_err(|LinkError::ResourceLimit| link_limit())?;
    let wrote = !args.dry_run && writes_document(&sink);
    if wrote {
        check::require_not_cancelled(&cancellation, CommandName::LinksClean)?;
        check::emit_document(
            &sink,
            cleaned.text.as_bytes(),
            args.raw_terminal,
            args.yes,
            CommandName::LinksClean,
        )?;
    }
    write_report(
        check::report_target(&sink),
        format,
        &CleanReport::from_engine(&cleaned, wrote),
    )?;
    Ok(ExitCode::SUCCESS)
}

fn validate(args: &CleanArgs) -> Result<(), RunFailure> {
    if is_directory(&args.source) {
        return Err(usage("links clean reads one document"));
    }
    if args.host.as_deref().is_some_and(str::is_empty)
        || args.path_prefix.as_deref().is_some_and(str::is_empty)
    {
        return Err(usage("a host or path prefix must not be empty"));
    }
    Ok(())
}

fn is_directory(source: &Path) -> bool {
    source.as_os_str() != STANDARD_STREAM_PATH
        && fs::symlink_metadata(source)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn read_document(source: &Path) -> Result<String, RunFailure> {
    let bytes = read_input_bounded(source, MAX_CANDIDATE_CHECK_BYTES)
        .map_err(|error| RunFailure::input_read(CommandName::LinksClean, &error))?;
    String::from_utf8(bytes).map_err(|_| invalid_utf8())
}

fn selected_groups(args: &CleanArgs) -> Vec<LinkCleanGroup> {
    let mut groups = Vec::new();
    if args.campaign {
        groups.push(LinkCleanGroup::Campaign);
    }
    if args.tracking {
        groups.push(LinkCleanGroup::Tracking);
    }
    if args.source_chatgpt {
        groups.push(LinkCleanGroup::SourceChatgpt);
    }
    if args.sz_32 {
        groups.push(LinkCleanGroup::Sz32);
    }
    groups
}

fn scope(args: &CleanArgs) -> LinkCleanScope<'_> {
    LinkCleanScope {
        host: args.host.as_deref(),
        path_prefix: args.path_prefix.as_deref(),
    }
}

const fn writes_document(sink: &OutputSink) -> bool {
    matches!(sink, OutputSink::File(_) | OutputSink::Standard)
}

fn write_report(
    target: ReportTarget,
    format: ReportFormat,
    report: &CleanReport,
) -> Result<(), RunFailure> {
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes =
                render::to_safe_pretty_json(&SuccessEnvelope::new(CommandName::LinksClean, report))
                    .map_err(|_| RunFailure::operational(CommandName::LinksClean))?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => {
            let terminal = match target {
                ReportTarget::Data => presentation::stdout_is_terminal(),
                ReportTarget::Diagnostic => presentation::stderr_is_terminal(),
            };
            presentation::wrap(&render_text(report), presentation::terminal_width(terminal))
                .into_bytes()
        }
    };
    write_bytes(target, &bytes)
}

fn render_text(report: &CleanReport) -> String {
    let verb = if report.wrote_output {
        "removed"
    } else {
        "would remove"
    };
    let parameters = if report.removals.len() == 1 {
        "parameter"
    } else {
        "parameters"
    };
    let urls = if report.http_url_count == 1 {
        "url"
    } else {
        "urls"
    };
    let mut lines = vec![
        format!("catalog_version: {}", report.catalog_version),
        format!(
            "link_hygiene: {verb} {} {parameters}; {} http {urls}",
            report.removals.len(),
            report.http_url_count
        ),
    ];
    for removal in &report.removals {
        lines.push(format!(
            "removed line {} col {} rule {} name {}",
            removal.line, removal.column, removal.rule_id, removal.name
        ));
    }
    for skip in &report.skips {
        lines.push(format!(
            "skipped line {} col {} reason {}",
            skip.line, skip.column, skip.reason
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn write_bytes(target: ReportTarget, bytes: &[u8]) -> Result<(), RunFailure> {
    let write_result = match target {
        ReportTarget::Data => {
            let mut stdout = io::stdout().lock();
            stdout.write_all(bytes).and_then(|()| stdout.flush())
        }
        ReportTarget::Diagnostic => {
            let mut stderr = io::stderr().lock();
            stderr.write_all(bytes).and_then(|()| stderr.flush())
        }
    };
    write_result.map_err(|_| RunFailure::operational(CommandName::LinksClean))
}

#[derive(Serialize)]
struct CleanReport {
    schema_version: u32,
    catalog_version: u32,
    wrote_output: bool,
    http_url_count: u32,
    removals: Vec<CleanRemoval>,
    skips: Vec<CleanSkip>,
}

#[derive(Serialize)]
struct CleanRemoval {
    line: u32,
    column: u32,
    rule_id: &'static str,
    name: &'static str,
}

#[derive(Serialize)]
struct CleanSkip {
    line: u32,
    column: u32,
    reason: &'static str,
}

impl CleanReport {
    fn from_engine(report: &rewrite_app::LinkCleanReport, wrote_output: bool) -> Self {
        Self {
            schema_version: 1,
            catalog_version: LINK_RULE_CATALOG_VERSION,
            wrote_output,
            http_url_count: report.http_url_count,
            removals: report
                .removals
                .iter()
                .map(|removal| CleanRemoval {
                    line: removal.line,
                    column: removal.column,
                    rule_id: removal.rule_id,
                    name: removal.name,
                })
                .collect(),
            skips: report
                .skips
                .iter()
                .map(|skip| CleanSkip {
                    line: skip.line,
                    column: skip.column,
                    reason: skip.reason,
                })
                .collect(),
        }
    }
}

fn usage(message: &'static str) -> RunFailure {
    RunFailure {
        command: CommandName::LinksClean,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message,
    }
}

fn invalid_utf8() -> RunFailure {
    RunFailure {
        command: CommandName::LinksClean,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InputUnreadable, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message: "input text is not a supported UTF-8 document",
    }
}

fn link_limit() -> RunFailure {
    RunFailure {
        command: CommandName::LinksClean,
        body: ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false,
        ),
        exit_code: ExitCode::from(EXIT_COMPATIBILITY),
        message: "document exceeds the supported link limit",
    }
}
