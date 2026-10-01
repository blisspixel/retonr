//! Bounded directory discovery for pre-model inspect.

use std::{path::Path, process::ExitCode};

use serde::Serialize;

use super::report::InspectReport;
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
    use rewrite_app::document_catalog::{DocumentCatalogLimits, DocumentCatalogService};
    let catalog = DocumentCatalogService::discover(
        directory,
        recursive,
        DocumentCatalogLimits {
            entries: max_entries,
            depth: max_depth,
            bytes: maximum_bytes,
        },
        cancellation,
    )
    .map_err(|error| catalog_failure(command, error))?;
    let documents = catalog
        .documents
        .into_iter()
        .map(|entry| {
            let relative_path = entry.relative_path.as_str().to_owned();
            DirectoryDocument {
                report: InspectReport::from_observation(
                    &directory.join(&relative_path),
                    &entry.observation,
                ),
                relative_path,
            }
        })
        .collect();
    let skipped = catalog
        .skipped
        .into_iter()
        .map(|entry| SkippedEntry {
            relative_path: entry.relative_path,
            reason: entry.reason.as_str(),
        })
        .collect();
    Ok((documents, skipped))
}

fn catalog_failure(
    command: CommandName,
    error: rewrite_app::document_catalog::DocumentCatalogError,
) -> RunFailure {
    use rewrite_app::{
        document_catalog::DocumentCatalogError as Error, document_intake::DocumentIntakeError,
    };
    match error {
        Error::Cancelled | Error::Intake(DocumentIntakeError::Cancelled) => {
            RunFailure::cancelled(command)
        }
        Error::ResourceLimitExceeded => directory_limit(command),
        Error::Input(error) | Error::Intake(DocumentIntakeError::Input(error)) => {
            RunFailure::input_read(command, &error)
        }
        Error::Intake(DocumentIntakeError::Inspection(error)) => RunFailure::app(command, &error),
        Error::InvalidPath | Error::Intake(DocumentIntakeError::Changed) => RunFailure::input_read(
            command,
            &std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid catalog input"),
        ),
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
