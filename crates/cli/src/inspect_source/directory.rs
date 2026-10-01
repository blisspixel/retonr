//! Bounded directory discovery for pre-model inspect.

use std::{
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;

use super::report::{InspectReport, inspect_direct_file_bounded};
use crate::contract::{CommandName, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode};
use crate::failure::RunFailure;
use crate::model::ModelOutput;
use crate::render::escape_inline_for_display;

const MAXIMUM_DIRECTORY_ENTRIES: usize = 4_096;
const MAXIMUM_DIRECTORY_DEPTH: usize = 8;
const MAXIMUM_DIRECTORY_BYTES: usize = 64 * 1024 * 1024;

pub(crate) struct Discovery {
    pub documents: Vec<DiscoveredDocument>,
    pub skipped: Vec<SkippedEntry>,
}

pub(crate) struct DiscoveredDocument {
    pub relative_path: String,
    pub encoding: rewrite_app::TextEncoding,
    pub digest: String,
    pub derivative: &'static str,
}

pub(super) fn inspect(
    directory: &Path,
    recursive: bool,
) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
    let (mut documents, mut skipped) = walk(directory, recursive, CommandName::Inspect)?;
    documents.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    skipped.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let derivative = if documents
        .iter()
        .any(|document| document.report.derivative() == "explicit_decision_required")
    {
        "explicit_decision_required"
    } else {
        "not_required"
    };
    let result = DirectoryReport {
        scope: "directory",
        recursion: if recursive { "bounded" } else { "none" },
        links: "not_followed",
        max_depth: recursive.then(|| MAXIMUM_DIRECTORY_DEPTH.to_string()),
        maximum_total_bytes: MAXIMUM_DIRECTORY_BYTES.to_string(),
        document_count: documents.len().to_string(),
        skipped_count: skipped.len().to_string(),
        documents,
        skipped,
        derivative,
    };
    Ok((
        CommandName::Inspect,
        ModelOutput {
            value: serde_json::to_value(&result).expect("directory inspect serializes"),
            text: result.text(),
            findings: false,
        },
        ExitCode::SUCCESS,
    ))
}

pub(crate) fn discover(
    directory: &Path,
    recursive: bool,
    command: CommandName,
) -> Result<Discovery, RunFailure> {
    discover_cancellable(
        directory,
        recursive,
        command,
        &rewrite_types::CancellationToken::new(),
    )
}

pub(crate) fn discover_cancellable(
    directory: &Path,
    recursive: bool,
    command: CommandName,
    cancellation: &rewrite_types::CancellationToken,
) -> Result<Discovery, RunFailure> {
    let (mut documents, mut skipped) = walk_cancellable(
        directory,
        recursive,
        MAXIMUM_DIRECTORY_ENTRIES,
        MAXIMUM_DIRECTORY_DEPTH,
        MAXIMUM_DIRECTORY_BYTES,
        command,
        cancellation,
    )?;
    documents.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    skipped.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(Discovery {
        documents: documents
            .into_iter()
            .map(|document| DiscoveredDocument {
                relative_path: document.relative_path,
                encoding: document.report.encoding(),
                digest: document.report.digest().to_owned(),
                derivative: document.report.derivative(),
            })
            .collect(),
        skipped,
    })
}

fn walk(
    directory: &Path,
    recursive: bool,
    command: CommandName,
) -> Result<(Vec<DirectoryDocument>, Vec<SkippedEntry>), RunFailure> {
    walk_bounded(
        directory,
        recursive,
        MAXIMUM_DIRECTORY_ENTRIES,
        MAXIMUM_DIRECTORY_DEPTH,
        command,
    )
}

fn walk_bounded(
    directory: &Path,
    recursive: bool,
    max_entries: usize,
    max_depth: usize,
    command: CommandName,
) -> Result<(Vec<DirectoryDocument>, Vec<SkippedEntry>), RunFailure> {
    walk_cancellable(
        directory,
        recursive,
        max_entries,
        max_depth,
        MAXIMUM_DIRECTORY_BYTES,
        command,
        &rewrite_types::CancellationToken::new(),
    )
}

fn walk_cancellable(
    directory: &Path,
    recursive: bool,
    max_entries: usize,
    max_depth: usize,
    maximum_bytes: usize,
    command: CommandName,
    cancellation: &rewrite_types::CancellationToken,
) -> Result<(Vec<DirectoryDocument>, Vec<SkippedEntry>), RunFailure> {
    let mut pending = vec![Frame {
        path: directory.to_path_buf(),
        relative: String::new(),
        depth: 0,
    }];
    let mut documents = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = 0_usize;
    let mut inspected_bytes = 0_usize;
    while let Some(frame) = pending.pop() {
        if cancellation.is_cancelled() {
            return Err(RunFailure::cancelled(command));
        }
        let mut entries = read_sorted_entries(
            &frame.path,
            max_entries.saturating_sub(seen),
            command,
            cancellation,
        )?;
        seen = seen.saturating_add(entries.len());
        if seen > max_entries {
            return Err(directory_limit(command));
        }
        for entry in entries.drain(..) {
            if cancellation.is_cancelled() {
                return Err(RunFailure::cancelled(command));
            }
            match classify_entry(&entry, &frame.relative) {
                Class::Document {
                    relative_path,
                    path,
                } => {
                    let remaining_bytes = maximum_bytes.saturating_sub(inspected_bytes);
                    let metadata = fs::symlink_metadata(&path)
                        .map_err(|error| RunFailure::input_read(command, &error))?;
                    if metadata.len() > remaining_bytes as u64 {
                        return Err(directory_limit(command));
                    }
                    let (report, byte_count) = inspect_direct_file_bounded(
                        directory,
                        &path,
                        command,
                        remaining_bytes.min(rewrite_app::MAX_CANDIDATE_CHECK_BYTES),
                    )?;
                    inspected_bytes = inspected_bytes
                        .checked_add(byte_count)
                        .filter(|total| *total <= maximum_bytes)
                        .ok_or_else(|| directory_limit(command))?;
                    documents.push(DirectoryDocument {
                        relative_path,
                        report,
                    });
                }
                Class::Descend {
                    relative_path,
                    path,
                } => {
                    let depth = frame.depth.saturating_add(1);
                    if !recursive {
                        skipped.push(SkippedEntry {
                            relative_path: Some(relative_path),
                            reason: "directory",
                        });
                    } else if depth > max_depth {
                        skipped.push(SkippedEntry {
                            relative_path: Some(relative_path),
                            reason: "depth_limit",
                        });
                    } else {
                        pending.push(Frame {
                            path,
                            relative: relative_path,
                            depth,
                        });
                    }
                }
                Class::Skipped(entry) => skipped.push(entry),
            }
        }
    }
    Ok((documents, skipped))
}

fn read_sorted_entries(
    directory: &Path,
    maximum_entries: usize,
    command: CommandName,
    cancellation: &rewrite_types::CancellationToken,
) -> Result<Vec<DirEntry>, RunFailure> {
    let mut entries = Vec::new();
    let reader =
        fs::read_dir(directory).map_err(|error| RunFailure::input_read(command, &error))?;
    for entry in reader {
        if cancellation.is_cancelled() {
            return Err(RunFailure::cancelled(command));
        }
        if entries.len() == maximum_entries {
            return Err(directory_limit(command));
        }
        entries.push(entry.map_err(|error| RunFailure::input_read(command, &error))?);
    }
    entries.sort_by_key(DirEntry::file_name);
    Ok(entries)
}

fn classify_entry(entry: &DirEntry, prefix: &str) -> Class {
    let os_name = entry.file_name();
    let Some(name) = os_name.to_str() else {
        return Class::Skipped(SkippedEntry {
            relative_path: None,
            reason: "malformed_name",
        });
    };
    if !portable_component(name) {
        return Class::Skipped(SkippedEntry {
            relative_path: None,
            reason: "malformed_name",
        });
    }
    let relative_path = join_relative(prefix, name);
    if name.starts_with('.') {
        return Class::Skipped(SkippedEntry {
            relative_path: Some(relative_path),
            reason: "hidden",
        });
    }
    if is_ignored(name) {
        return Class::Skipped(SkippedEntry {
            relative_path: Some(relative_path),
            reason: "ignored",
        });
    }
    let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
        return Class::Skipped(SkippedEntry {
            relative_path: Some(relative_path),
            reason: "unreadable",
        });
    };
    if metadata.file_type().is_symlink() {
        return Class::Skipped(SkippedEntry {
            relative_path: Some(relative_path),
            reason: "symlink",
        });
    }
    if metadata.is_dir() {
        return Class::Descend {
            relative_path,
            path: entry.path(),
        };
    }
    if !metadata.is_file() {
        return Class::Skipped(SkippedEntry {
            relative_path: Some(relative_path),
            reason: "non_regular",
        });
    }
    Class::Document {
        relative_path,
        path: entry.path(),
    }
}

fn portable_component(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains('\0')
}

fn is_ignored(name: &str) -> bool {
    name.eq_ignore_ascii_case("target") || name.eq_ignore_ascii_case("node_modules")
}

fn join_relative(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

fn directory_limit(command: CommandName) -> RunFailure {
    RunFailure {
        command,
        body: ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false,
        ),
        exit_code: ExitCode::from(EXIT_COMPATIBILITY),
        message: "directory exceeds the supported resource limits",
    }
}

struct Frame {
    path: PathBuf,
    relative: String,
    depth: usize,
}

enum Class {
    Document {
        relative_path: String,
        path: PathBuf,
    },
    Descend {
        relative_path: String,
        path: PathBuf,
    },
    Skipped(SkippedEntry),
}

#[derive(Serialize)]
struct DirectoryDocument {
    relative_path: String,
    #[serde(flatten)]
    report: InspectReport,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct SkippedEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    pub reason: &'static str,
}

#[derive(Serialize)]
struct DirectoryReport {
    scope: &'static str,
    recursion: &'static str,
    links: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_depth: Option<String>,
    maximum_total_bytes: String,
    document_count: String,
    skipped_count: String,
    documents: Vec<DirectoryDocument>,
    skipped: Vec<SkippedEntry>,
    derivative: &'static str,
}

impl DirectoryReport {
    fn text(&self) -> String {
        let mut lines = vec![
            format!("scope: {}", self.scope),
            format!("recursion: {}", self.recursion),
            format!("links: {}", self.links),
        ];
        if let Some(max_depth) = &self.max_depth {
            lines.push(format!("max_depth: {max_depth}"));
        }
        lines.push(format!("documents: {}", self.document_count));
        lines.push(format!("maximum_total_bytes: {}", self.maximum_total_bytes));
        lines.push(format!("skipped: {}", self.skipped_count));
        lines.push(format!("derivative: {}", self.derivative));
        for document in &self.documents {
            lines.push(format!(
                "document {} derivative={}",
                escape_inline_for_display(&document.relative_path),
                document.report.derivative()
            ));
        }
        for skipped in &self.skipped {
            match &skipped.relative_path {
                Some(name) => lines.push(format!(
                    "skipped {} reason={}",
                    escape_inline_for_display(name),
                    skipped.reason
                )),
                None => lines.push(format!("skipped reason={}", skipped.reason)),
            }
        }
        lines.push(String::new());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests;
