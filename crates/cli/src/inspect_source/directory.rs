//! Non-recursive, non-mutating directory discovery for pre-model inspect.

use std::{fs, path::Path, process::ExitCode};

use serde::Serialize;

use super::report::{InspectReport, inspect_file};
use crate::contract::{CommandName, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode};
use crate::failure::RunFailure;
use crate::model::ModelOutput;

const MAXIMUM_DIRECTORY_ENTRIES: usize = 4_096;

pub(super) fn inspect(
    directory: &Path,
) -> Result<(CommandName, ModelOutput, ExitCode), RunFailure> {
    let mut entries = Vec::new();
    let reader = fs::read_dir(directory)
        .map_err(|error| RunFailure::input_read(CommandName::Inspect, &error))?;
    for entry in reader {
        let entry = entry.map_err(|error| RunFailure::input_read(CommandName::Inspect, &error))?;
        if entries.len() >= MAXIMUM_DIRECTORY_ENTRIES {
            return Err(directory_limit());
        }
        entries.push(entry);
    }
    entries.sort_by_key(fs::DirEntry::file_name);

    let mut documents = Vec::new();
    let mut skipped = Vec::new();
    for entry in entries {
        match classify_entry(&entry) {
            Entry::Document(relative_path, path) => {
                let report = inspect_file(&path)?;
                documents.push(DirectoryDocument {
                    relative_path,
                    report,
                });
            }
            Entry::Skipped(skipped_entry) => skipped.push(skipped_entry),
        }
    }

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
        recursion: "none",
        links: "not_followed",
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

fn classify_entry(entry: &fs::DirEntry) -> Entry {
    let os_name = entry.file_name();
    let Some(name) = os_name.to_str() else {
        return Entry::Skipped(SkippedEntry {
            relative_path: None,
            reason: "malformed_name",
        });
    };
    if name.starts_with('.') {
        return Entry::Skipped(SkippedEntry {
            relative_path: Some(name.to_owned()),
            reason: "hidden",
        });
    }
    let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
        return Entry::Skipped(SkippedEntry {
            relative_path: Some(name.to_owned()),
            reason: "unreadable",
        });
    };
    if metadata.file_type().is_symlink() {
        return Entry::Skipped(SkippedEntry {
            relative_path: Some(name.to_owned()),
            reason: "symlink",
        });
    }
    if metadata.is_dir() {
        return Entry::Skipped(SkippedEntry {
            relative_path: Some(name.to_owned()),
            reason: "directory",
        });
    }
    if !metadata.is_file() {
        return Entry::Skipped(SkippedEntry {
            relative_path: Some(name.to_owned()),
            reason: "non_regular",
        });
    }
    Entry::Document(name.to_owned(), entry.path())
}

fn directory_limit() -> RunFailure {
    RunFailure {
        command: CommandName::Inspect,
        body: ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false,
        ),
        exit_code: ExitCode::from(EXIT_COMPATIBILITY),
        message: "directory exceeds the supported entry limit",
    }
}

enum Entry {
    Document(String, std::path::PathBuf),
    Skipped(SkippedEntry),
}

#[derive(Serialize)]
struct DirectoryDocument {
    relative_path: String,
    #[serde(flatten)]
    report: InspectReport,
}

#[derive(Serialize)]
struct SkippedEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    relative_path: Option<String>,
    reason: &'static str,
}

#[derive(Serialize)]
struct DirectoryReport {
    scope: &'static str,
    recursion: &'static str,
    links: &'static str,
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
            format!("documents: {}", self.document_count),
            format!("skipped: {}", self.skipped_count),
            format!("derivative: {}", self.derivative),
        ];
        for document in &self.documents {
            lines.push(format!(
                "document {} derivative={}",
                document.relative_path,
                document.report.derivative()
            ));
        }
        for skipped in &self.skipped {
            match &skipped.relative_path {
                Some(name) => lines.push(format!("skipped {name} reason={}", skipped.reason)),
                None => lines.push(format!("skipped reason={}", skipped.reason)),
            }
        }
        lines.push(String::new());
        lines.join("\n")
    }
}
