//! Declarative command-line arguments shared by help, completion, and dispatch.

use crate::{EditLevelArg, contract::ReportFormat, model::ModelCommand};
use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

const EXAMPLES: &str = "\
Examples:
  retonr inspect draft.txt
  retonr rewrite draft.txt -o rewritten.txt
  retonr rewrite draft.txt -i
  retonr rewrite docs/ -r --output-dir rewritten --dry-run
  retonr check original.txt candidate.txt --diff
  retonr check originals/ candidates/ -r --dry-run
  retonr tui draft.txt --candidate edited.txt
  retonr -D .retonr model list
";

/// Fidelity-gated rewriting prototype.
#[derive(Debug, Parser)]
#[command(name = "retonr", version, about, after_help = EXAMPLES)]
pub(crate) struct Cli {
    /// json for machines, text for humans. A terminal defaults to text.
    #[arg(
        short,
        long,
        value_enum,
        value_name = "json|text",
        global = true,
        hide_possible_values = true
    )]
    pub(crate) format: Option<ReportFormat>,
    /// Explicit repository root. Also read from `RETONR_DATA_DIR`.
    #[arg(
        short = 'D',
        long,
        value_name = "DIRECTORY",
        global = true,
        env = "RETONR_DATA_DIR",
        hide_env_values = true
    )]
    pub(crate) data_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Supported prototype operations.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Open the experimental read-only document review workbench.
    ///
    /// Reviews existing documents using the same lint and candidate checks as the CLI.
    Tui(crate::tui::TuiArgs),
    /// Rewrite one UTF-8 source after grounded generation and engine gates.
    ///
    /// A recovered generation binding can attach in-process fake-backend
    /// conformance. The command does not start a runtime or access the network.
    Rewrite {
        /// UTF-8 source file, directory, or - for standard input.
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
        /// Return exit code 3 when validation safely abstains.
        #[arg(long)]
        fail_on_abstain: bool,
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
        /// Recurse into real child directories without following links.
        #[arg(short, long)]
        recursive: bool,
        /// Map directory sources onto a separate output root.
        ///
        /// Required for directory rewrite. Existing files are not replaced.
        /// Directory rewrite is dry-run only.
        #[arg(long, value_name = "DIRECTORY")]
        output_dir: Option<PathBuf>,
    },
    /// Validate supplied plain-text candidates without using a model.
    ///
    /// Directory pairs require --dry-run and separate roots. Relative paths
    /// must match exactly. Directory checks report results without writing bytes.
    Check {
        /// UTF-8 source file or directory to protect and validate against, or - for standard input.
        #[arg(value_name = "SOURCE")]
        source: PathBuf,
        /// UTF-8 file or directory containing supplied replacements, or - for standard input.
        #[arg(value_name = "CANDIDATE")]
        candidate: PathBuf,
        /// Recurse into real child directories for paired-directory dry-run checks.
        #[arg(short, long)]
        recursive: bool,
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
        /// Degree of editorial freedom (touch-up, voice-pass, rewrite, reconstruct).
        #[arg(long = "edit-level", value_enum, value_name = "LEVEL")]
        edit_level: Option<EditLevelArg>,
        /// Maximum allowed character count.
        #[arg(long = "max-chars", value_name = "COUNT")]
        max_chars: Option<usize>,
        /// Minimum allowed character count.
        #[arg(long = "min-chars", value_name = "COUNT")]
        min_chars: Option<usize>,
        /// Maximum allowable percentage expansion over source character count.
        #[arg(long = "max-expansion-pct", value_name = "PERCENT")]
        max_expansion_pct: Option<u8>,
        /// Maximum allowed line count.
        #[arg(long = "max-lines", value_name = "COUNT")]
        max_lines: Option<usize>,
        /// Enforce exact line count preservation.
        #[arg(long = "preserve-line-count")]
        preserve_line_count: bool,
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
    /// Inspect plain text for editorial style patterns, conversational residue, and AI slop.
    Lint {
        /// UTF-8 file or directory to inspect, or - for standard input.
        #[arg(value_name = "SOURCE")]
        source: PathBuf,
        /// Recurse into real child directories without following links.
        #[arg(short, long)]
        recursive: bool,
        /// Optional candidate file to compare against the source.
        #[arg(long, value_name = "CANDIDATE")]
        candidate: Option<PathBuf>,
        /// Return exit code 3 when editorial defects are detected.
        #[arg(long)]
        fail_on_findings: bool,
    },
}
