//! Bounded directory discovery for pre-model inspect.

use std::{
    fs::{self, DirEntry},
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde::Serialize;

use super::report::{InspectReport, inspect_file};
use crate::contract::{CommandName, EXIT_COMPATIBILITY, ErrorBody, ErrorCategory, ErrorCode};
use crate::failure::RunFailure;
use crate::model::ModelOutput;

const MAXIMUM_DIRECTORY_ENTRIES: usize = 4_096;
const MAXIMUM_DIRECTORY_DEPTH: usize = 8;

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
    let (mut documents, mut skipped) = walk(directory, recursive, command)?;
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
    let mut pending = vec![Frame {
        path: directory.to_path_buf(),
        relative: String::new(),
        depth: 0,
    }];
    let mut documents = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = 0_usize;
    while let Some(frame) = pending.pop() {
        let mut entries = read_sorted_entries(&frame.path, command)?;
        seen = seen.saturating_add(entries.len());
        if seen > max_entries {
            return Err(directory_limit(command));
        }
        for entry in entries.drain(..) {
            match classify_entry(&entry, &frame.relative) {
                Class::Document {
                    relative_path,
                    path,
                } => {
                    let report = inspect_file(&path, command)?;
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
    command: CommandName,
) -> Result<Vec<DirEntry>, RunFailure> {
    let mut entries = Vec::new();
    let reader =
        fs::read_dir(directory).map_err(|error| RunFailure::input_read(command, &error))?;
    for entry in reader {
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
        message: "directory exceeds the supported entry limit",
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
        lines.push(format!("skipped: {}", self.skipped_count));
        lines.push(format!("derivative: {}", self.derivative));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn recursive_walk_lists_nested_files_and_skips_ignored_names() {
        let root = tempdir().expect("temporary directory");
        let path = root.path();
        fs::write(path.join("a.txt"), "alpha\n").expect("write a");
        fs::create_dir(path.join("nested")).expect("create nested");
        fs::write(path.join("nested").join("inner.txt"), "inner\n").expect("write nested");
        fs::create_dir(path.join("nested").join(".cache")).expect("create hidden");
        fs::write(path.join("nested").join(".cache").join("x.txt"), "x\n").expect("write hidden");
        fs::create_dir(path.join("node_modules")).expect("create node_modules");
        fs::write(path.join("node_modules").join("pkg.js"), "pkg\n").expect("write ignored");

        let (documents, skipped) = walk_bounded(
            path,
            true,
            MAXIMUM_DIRECTORY_ENTRIES,
            MAXIMUM_DIRECTORY_DEPTH,
            CommandName::Inspect,
        )
        .expect("walk");
        let document_paths: Vec<&str> = documents
            .iter()
            .map(|document| document.relative_path.as_str())
            .collect();
        assert_eq!(document_paths, vec!["a.txt", "nested/inner.txt"]);
        assert!(skipped.iter().any(
            |entry| entry.relative_path.as_deref() == Some("node_modules")
                && entry.reason == "ignored"
        ));
        assert!(skipped.iter().any(|entry| entry.relative_path.as_deref()
            == Some("nested/.cache")
            && entry.reason == "hidden"));
        assert!(!document_paths.iter().any(|name| name.contains("pkg.js")));
        assert!(!document_paths.iter().any(|name| name.contains('\\')));
    }

    #[test]
    fn recursive_walk_skips_directories_past_the_depth_limit() {
        let root = tempdir().expect("temporary directory");
        let path = root.path();
        let nested = path.join("d1").join("d2");
        fs::create_dir_all(&nested).expect("create nested");
        fs::write(nested.join("leaf.txt"), "leaf\n").expect("write leaf");

        let (documents, skipped) = walk_bounded(
            path,
            true,
            MAXIMUM_DIRECTORY_ENTRIES,
            1,
            CommandName::Inspect,
        )
        .expect("walk");
        assert!(documents.is_empty());
        assert!(
            skipped
                .iter()
                .any(|entry| entry.relative_path.as_deref() == Some("d1/d2")
                    && entry.reason == "depth_limit")
        );
    }

    #[test]
    fn recursive_walk_refuses_when_the_entry_limit_is_exceeded() {
        let root = tempdir().expect("temporary directory");
        let path = root.path();
        fs::write(path.join("a.txt"), "a\n").expect("write a");
        fs::write(path.join("b.txt"), "b\n").expect("write b");
        fs::write(path.join("c.txt"), "c\n").expect("write c");

        let Err(failure) =
            walk_bounded(path, true, 2, MAXIMUM_DIRECTORY_DEPTH, CommandName::Inspect)
        else {
            panic!("entry limit should refuse");
        };
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_COMPATIBILITY));
        assert!(failure.message.contains("entry limit"));
    }

    #[test]
    fn non_recursive_walk_skips_child_directories() {
        let root = tempdir().expect("temporary directory");
        let path = root.path();
        fs::write(path.join("a.txt"), "a\n").expect("write a");
        fs::create_dir(path.join("nested")).expect("create nested");
        fs::write(path.join("nested").join("inner.txt"), "inner\n").expect("write nested");

        let (documents, skipped) = walk_bounded(
            path,
            false,
            MAXIMUM_DIRECTORY_ENTRIES,
            MAXIMUM_DIRECTORY_DEPTH,
            CommandName::Inspect,
        )
        .expect("walk");
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].relative_path, "a.txt");
        assert!(
            skipped
                .iter()
                .any(|entry| entry.relative_path.as_deref() == Some("nested")
                    && entry.reason == "directory")
        );
    }

    #[test]
    fn portable_relative_paths_reject_separators_and_join_with_slash() {
        assert!(!portable_component("a/b"));
        assert!(!portable_component("a\\b"));
        assert!(!portable_component(""));
        assert_eq!(join_relative("", "a.txt"), "a.txt");
        assert_eq!(join_relative("nested", "inner.txt"), "nested/inner.txt");
        assert!(is_ignored("TARGET"));
        assert!(is_ignored("Node_Modules"));
        assert!(!is_ignored("src"));
    }
}
