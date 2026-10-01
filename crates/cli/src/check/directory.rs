//! Bounded, model-free verification of exact relative-path candidate pairs.

use std::{collections::BTreeMap, fs, path::Path, process::ExitCode};

use rewrite_app::{
    CandidateCheckRequest, CandidateCheckService, MAX_CANDIDATE_CHECK_BYTES, TextEncoding,
};
use rewrite_types::{CancellationToken, Digest, ReasonCode, RewriteStatus};

use super::{CheckRequest, replace, require_not_cancelled};
use crate::{
    contract::{
        CommandName, EXIT_COMPATIBILITY, EXIT_POLICY, ErrorBody, ErrorCategory, ErrorCode,
        ReportFormat,
    },
    failure::RunFailure,
    inspect_source::directory::{DiscoveredDocument, discover_cancellable},
};

mod report;
use report::{DirectoryCheckReport, PairReport, UnmatchedPath};

const MAXIMUM_TOTAL_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

pub(super) fn run(request: &CheckRequest, format: ReportFormat) -> Result<ExitCode, RunFailure> {
    validate_request(request)?;
    let cancellation = CancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::try_set_handler(move || signal.cancel())
        .map_err(|_| RunFailure::operational(CommandName::Check))?;
    let report = build_report(request, &cancellation)?;
    require_not_cancelled(&cancellation, CommandName::Check)?;
    report.write(format)?;
    Ok(
        if request.fail_on_abstain
            && (report.abstained_count > 0
                || !report.unmatched.is_empty()
                || report.skipped_count > 0)
        {
            ExitCode::from(EXIT_POLICY)
        } else {
            ExitCode::SUCCESS
        },
    )
}

fn validate_request(request: &CheckRequest) -> Result<(), RunFailure> {
    if !request.inspection.dry_run {
        return Err(RunFailure {
            command: CommandName::Check,
            body: ErrorBody::new(ErrorCategory::Compatibility, ErrorCode::Unsupported, false),
            exit_code: ExitCode::from(EXIT_COMPATIBILITY),
            message: "directory check requires --dry-run",
        });
    }
    if !is_real_directory(&request.candidate)
        || request.output.is_some()
        || request.in_place.requested
        || request.in_place.backup
        || request.inspection.diff
        || request.inspection.trace.is_some()
        || request.raw_terminal
        || request.confirmed
    {
        return Err(RunFailure::usage_for(CommandName::Check));
    }
    let source = root_key(&request.source)?;
    let candidate = root_key(&request.candidate)?;
    if source.starts_with(&candidate) || candidate.starts_with(&source) {
        return Err(RunFailure {
            command: CommandName::Check,
            body: ErrorBody::new(ErrorCategory::Policy, ErrorCode::PolicyRefusal, false),
            exit_code: ExitCode::from(EXIT_POLICY),
            message: "directory check requires separate nonoverlapping roots",
        });
    }
    Ok(())
}

fn root_key(path: &Path) -> Result<std::path::PathBuf, RunFailure> {
    let path = fs::canonicalize(path).map_err(|_| RunFailure::operational(CommandName::Check))?;
    #[cfg(windows)]
    let path = std::path::PathBuf::from(path.as_os_str().to_string_lossy().to_lowercase());
    Ok(path)
}

fn build_report(
    request: &CheckRequest,
    cancellation: &CancellationToken,
) -> Result<DirectoryCheckReport, RunFailure> {
    let source = discover_cancellable(
        &request.source,
        request.traversal.recursive(),
        CommandName::Check,
        cancellation,
    )?;
    let candidate = discover_cancellable(
        &request.candidate,
        request.traversal.recursive(),
        CommandName::Check,
        cancellation,
    )?;
    let mut candidates: BTreeMap<_, _> = candidate
        .documents
        .into_iter()
        .map(|document| (document.relative_path.clone(), document))
        .collect();
    let mut report = DirectoryCheckReport::new(
        request.traversal.recursive(),
        MAXIMUM_TOTAL_BYTES,
        source.skipped,
        candidate.skipped,
    );
    let mut total_bytes = 0;
    for source in source.documents {
        require_not_cancelled(cancellation, CommandName::Check)?;
        let Some(candidate) = candidates.remove(&source.relative_path) else {
            report.unmatched.push(UnmatchedPath {
                relative_path: source.relative_path,
                side: "source",
                reason: "missing_candidate",
            });
            continue;
        };
        if source.encoding != TextEncoding::Utf8 || candidate.encoding != TextEncoding::Utf8 {
            report.unmatched.push(UnmatchedPath {
                relative_path: source.relative_path,
                side: "pair",
                reason: "unsupported_encoding",
            });
            continue;
        }
        if source.derivative == "explicit_decision_required"
            || candidate.derivative == "explicit_decision_required"
        {
            report.unmatched.push(UnmatchedPath {
                relative_path: source.relative_path,
                side: "pair",
                reason: "explicit_derivative_decision_required",
            });
            continue;
        }
        let source_bytes = read_coherent(&request.source, &source, &mut total_bytes)?;
        let candidate_bytes = read_coherent(&request.candidate, &candidate, &mut total_bytes)?;
        let candidate_text = String::from_utf8(candidate_bytes)
            .map_err(|_| RunFailure::concurrent_modification(CommandName::Check))?;
        let result = CandidateCheckService::check_with_cancellation(
            CandidateCheckRequest {
                source: source_bytes,
                candidate: candidate_text,
                protected_terms: request.protected_terms.clone(),
                layout: request.layout,
                edit_level: request.edit_level,
            },
            cancellation,
        )
        .map_err(|error| RunFailure::check_app(&error))?;
        if result.record.reason == Some(ReasonCode::Cancelled) {
            return Err(RunFailure::cancelled(CommandName::Check));
        }
        if result.record.status == RewriteStatus::Failed {
            return Err(RunFailure::operational(CommandName::Check));
        }
        report.abstained_count += usize::from(result.record.status == RewriteStatus::Abstained);
        report.pairs.push(PairReport {
            relative_path: source.relative_path,
            record: result.record,
        });
    }
    for (_, candidate) in candidates {
        report.unmatched.push(UnmatchedPath {
            relative_path: candidate.relative_path,
            side: "candidate",
            reason: "missing_source",
        });
    }
    report
        .unmatched
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    report.checked_count = report.pairs.len();
    report.skipped_count = report.source_skipped.len() + report.candidate_skipped.len();
    Ok(report)
}

fn read_coherent(
    root: &Path,
    document: &DiscoveredDocument,
    total_bytes: &mut usize,
) -> Result<Vec<u8>, RunFailure> {
    let path = root.join(&document.relative_path);
    let mut parent = path.parent();
    while let Some(current) = parent {
        let metadata = fs::symlink_metadata(current)
            .map_err(|_| RunFailure::concurrent_modification(CommandName::Check))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(RunFailure::concurrent_modification(CommandName::Check));
        }
        if current == root {
            break;
        }
        parent = current.parent();
    }
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| RunFailure::concurrent_modification(CommandName::Check))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RunFailure::concurrent_modification(CommandName::Check));
    }
    replace::require_single_link(&path, &metadata, CommandName::Check)
        .map_err(|_| RunFailure::usage_for(CommandName::Check))?;
    let remaining = MAXIMUM_TOTAL_BYTES
        .saturating_sub(*total_bytes)
        .min(MAX_CANDIDATE_CHECK_BYTES);
    if metadata.len() > u64::try_from(remaining).unwrap_or(u64::MAX) {
        return Err(aggregate_limit());
    }
    let bytes = crate::file_input::read_directory_unaliased_bounded(
        root,
        Path::new(&document.relative_path),
        remaining,
    )
    .map_err(|error| RunFailure::check_read(&error))?;
    *total_bytes = total_bytes
        .checked_add(bytes.len())
        .filter(|total| *total <= MAXIMUM_TOTAL_BYTES)
        .ok_or_else(aggregate_limit)?;
    if Digest::sha256(&bytes).as_str() != document.digest {
        return Err(RunFailure::concurrent_modification(CommandName::Check));
    }
    Ok(bytes)
}

fn aggregate_limit() -> RunFailure {
    RunFailure {
        command: CommandName::Check,
        body: ErrorBody::new(
            ErrorCategory::Compatibility,
            ErrorCode::ResourceLimitExceeded,
            false,
        ),
        exit_code: ExitCode::from(EXIT_COMPATIBILITY),
        message: "directory check exceeds the supported aggregate byte limit",
    }
}

#[cfg(test)]
mod tests;
