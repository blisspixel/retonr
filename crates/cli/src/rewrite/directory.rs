//! Model-free directory rewrite dry-run: discovery and destination mapping.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use rewrite_app::TextEncoding;
use serde::Serialize;

use super::RewriteRequest;
use crate::contract::{
    CommandName, EXIT_COMPATIBILITY, EXIT_POLICY, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode,
    ReportFormat, SuccessEnvelope,
};
use crate::failure::RunFailure;
use crate::inspect_source::directory::{self, Discovery};

pub(super) fn run(request: &RewriteRequest) -> Result<ExitCode, RunFailure> {
    validate_directory_invocation(request)?;
    let output_dir = request
        .directory
        .output_dir
        .as_ref()
        .ok_or_else(|| usage("directory rewrite requires --output-dir"))?;
    refuse_unsafe_output_root(&request.source, output_dir)?;
    let discovery = directory::discover(
        &request.source,
        request.directory.recursive,
        CommandName::Rewrite,
    )?;
    let plan = plan_transaction(&discovery, output_dir, request.directory.recursive);
    write_plan(&plan, request.format)?;
    Ok(ExitCode::SUCCESS)
}

pub(super) fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn validate_directory_invocation(request: &RewriteRequest) -> Result<(), RunFailure> {
    if request.directory.output_dir.is_none() {
        return Err(usage("directory rewrite requires --output-dir"));
    }
    if !request.inspection.dry_run {
        return Err(unsupported("directory rewrite is dry-run only"));
    }
    if request.output.is_some() {
        return Err(usage("directory rewrite is incompatible with --output"));
    }
    if request.in_place.requested || request.in_place.backup {
        return Err(usage("directory rewrite is incompatible with --in-place"));
    }
    if request.artifact_id.is_some() {
        return Err(usage("directory dry-run does not select an artifact"));
    }
    if request.inspection.diff {
        return Err(usage("directory dry-run does not write a document diff"));
    }
    if request.inspection.trace.is_some() {
        return Err(usage("directory dry-run does not write a rewrite trace"));
    }
    Ok(())
}

fn refuse_unsafe_output_root(source: &Path, output: &Path) -> Result<(), RunFailure> {
    if output.as_os_str().is_empty() {
        return Err(usage("directory rewrite requires --output-dir"));
    }
    if let Ok(metadata) = fs::symlink_metadata(output)
        && (metadata.file_type().is_symlink() || !metadata.is_dir())
    {
        return Err(usage("output-dir must be a real directory when it exists"));
    }
    let source_key = normalized(source)?;
    let output_key = normalized(output)?;
    if source_key == output_key {
        return Err(policy("output-dir cannot be the source directory"));
    }
    if is_nested(&source_key, &output_key) {
        return Err(policy("output-dir cannot be inside the source directory"));
    }
    if is_nested(&output_key, &source_key) {
        return Err(policy("output-dir cannot contain the source directory"));
    }
    Ok(())
}

fn normalized(path: &Path) -> Result<PathBuf, RunFailure> {
    let absolute = std::path::absolute(path).map_err(|_| operational())?;
    Ok(lexically_normal(&absolute))
}

fn lexically_normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_nested(parent: &Path, child: &Path) -> bool {
    child.starts_with(parent) && child != parent
}

fn plan_transaction(discovery: &Discovery, output_dir: &Path, recursive: bool) -> DirectoryPlan {
    let mut planned = Vec::new();
    let mut blocked = Vec::new();
    let mut skipped = discovery.skipped.clone();
    for document in &discovery.documents {
        if document.encoding != TextEncoding::Utf8 {
            skipped.push(directory::SkippedEntry {
                relative_path: Some(document.relative_path.clone()),
                reason: "unsupported_encoding",
            });
            continue;
        }
        if document.derivative == "explicit_decision_required" {
            blocked.push(MappedPath {
                source: document.relative_path.clone(),
                destination: document.relative_path.clone(),
                digest: document.digest.clone(),
                reason: Some("derivative"),
            });
            continue;
        }
        let destination_path = output_dir.join(&document.relative_path);
        if destination_path.exists() {
            blocked.push(MappedPath {
                source: document.relative_path.clone(),
                destination: document.relative_path.clone(),
                digest: document.digest.clone(),
                reason: Some("collision"),
            });
            continue;
        }
        planned.push(MappedPath {
            source: document.relative_path.clone(),
            destination: document.relative_path.clone(),
            digest: document.digest.clone(),
            reason: None,
        });
    }
    skipped.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    planned.sort_by(|left, right| left.source.cmp(&right.source));
    blocked.sort_by(|left, right| left.source.cmp(&right.source));
    DirectoryPlan {
        scope: "directory",
        mode: "dry_run",
        recursion: if recursive { "bounded" } else { "none" },
        links: "not_followed",
        collision_policy: "refuse",
        planned_count: planned.len().to_string(),
        blocked_count: blocked.len().to_string(),
        skipped_count: skipped.len().to_string(),
        planned,
        blocked,
        skipped,
    }
}

fn write_plan(plan: &DirectoryPlan, format: ReportFormat) -> Result<(), RunFailure> {
    let bytes = match format {
        ReportFormat::Json => {
            let mut bytes =
                serde_json::to_vec_pretty(&SuccessEnvelope::new(CommandName::Rewrite, plan))
                    .map_err(|_| operational())?;
            bytes.push(b'\n');
            bytes
        }
        ReportFormat::Text => plan.text().into_bytes(),
    };
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&bytes)
        .and_then(|()| stdout.flush())
        .map_err(|_| operational())
}

fn usage(message: &'static str) -> RunFailure {
    RunFailure {
        command: CommandName::Rewrite,
        body: ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidInvocation, false),
        exit_code: ExitCode::from(EXIT_USAGE),
        message,
    }
}

fn policy(message: &'static str) -> RunFailure {
    RunFailure {
        command: CommandName::Rewrite,
        body: ErrorBody::new(ErrorCategory::Policy, ErrorCode::PolicyRefusal, false),
        exit_code: ExitCode::from(EXIT_POLICY),
        message,
    }
}

fn unsupported(message: &'static str) -> RunFailure {
    RunFailure {
        command: CommandName::Rewrite,
        body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
        exit_code: ExitCode::from(EXIT_COMPATIBILITY),
        message,
    }
}

fn operational() -> RunFailure {
    RunFailure::operational(CommandName::Rewrite)
}

#[derive(Serialize)]
struct MappedPath {
    source: String,
    destination: String,
    digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
}

#[derive(Serialize)]
struct DirectoryPlan {
    scope: &'static str,
    mode: &'static str,
    recursion: &'static str,
    links: &'static str,
    collision_policy: &'static str,
    planned_count: String,
    blocked_count: String,
    skipped_count: String,
    planned: Vec<MappedPath>,
    blocked: Vec<MappedPath>,
    skipped: Vec<directory::SkippedEntry>,
}

impl DirectoryPlan {
    fn text(&self) -> String {
        let mut lines = vec![
            format!("scope: {}", self.scope),
            format!("mode: {}", self.mode),
            format!("recursion: {}", self.recursion),
            format!("links: {}", self.links),
            format!("collision_policy: {}", self.collision_policy),
            format!("planned: {}", self.planned_count),
            format!("blocked: {}", self.blocked_count),
            format!("skipped: {}", self.skipped_count),
        ];
        for item in &self.planned {
            lines.push(format!(
                "planned {} destination={}",
                item.source, item.destination
            ));
        }
        for item in &self.blocked {
            let reason = item.reason.unwrap_or("blocked");
            lines.push(format!("blocked {} reason={reason}", item.source));
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
    use tempfile::tempdir;

    #[test]
    fn nested_output_root_is_refused() {
        let root = tempdir().expect("temporary directory");
        let source = root.path().join("docs");
        fs::create_dir(&source).expect("create source");
        let nested = source.join("out");
        let failure = refuse_unsafe_output_root(&source, &nested).expect_err("nested");
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_POLICY));
    }

    #[test]
    fn identical_roots_are_refused() {
        let root = tempdir().expect("temporary directory");
        let source = root.path().join("docs");
        fs::create_dir(&source).expect("create source");
        let failure = refuse_unsafe_output_root(&source, &source).expect_err("same");
        assert_eq!(failure.exit_code, ExitCode::from(EXIT_POLICY));
    }

    #[test]
    fn sibling_output_root_is_accepted() {
        let root = tempdir().expect("temporary directory");
        let source = root.path().join("docs");
        fs::create_dir(&source).expect("create source");
        let output = root.path().join("rewritten");
        refuse_unsafe_output_root(&source, &output).expect("sibling");
    }
}
