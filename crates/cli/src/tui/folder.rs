//! Exact, bounded directory selection with retained and digest-checked reads.

use std::{collections::BTreeSet, fs, path::Path};

use rewrite_app::{
    MAX_CANDIDATE_CHECK_BYTES, TextEncoding,
    document_intake::{DerivativeDisposition, DocumentIntakeError, DocumentIntakeService},
    document_selection::{DocumentSelection, RelativeDocumentPath},
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    ReviewRequest, service,
    state::{DirectoryNavigation, Snapshot},
};
use crate::{
    contract::CommandName,
    failure::RunFailure,
    inspect_source::directory::{DiscoveredDocument, Discovery, discover_cancellable},
};

pub(super) fn load(
    request: &ReviewRequest,
    cancellation: &CancellationToken,
) -> Result<Snapshot, RunFailure> {
    if cancellation.is_cancelled() {
        return Err(RunFailure::cancelled(CommandName::Tui));
    }
    if !request.args.recursive {
        if request.args.document.is_some() {
            return Err(RunFailure::usage_for(CommandName::Tui));
        }
        return service::load(&request.args, cancellation)
            .map(service::Snapshot::into_presentation);
    }
    require_root(&request.args.source)?;
    if let Some(root) = &request.args.candidate {
        require_root(root)?;
        require_separate(&request.args.source, root)?;
    }
    let sources = discover_cancellable(&request.args.source, true, CommandName::Tui, cancellation)?;
    let candidates = request
        .args
        .candidate
        .as_ref()
        .map(|root| discover_cancellable(root, true, CommandName::Tui, cancellation))
        .transpose()?;
    let eligible: Vec<_> = sources
        .documents
        .iter()
        .filter(|document| supported(document))
        .collect();
    let selected = selection(request, &eligible)?;
    let source_paths: BTreeSet<_> = sources
        .documents
        .iter()
        .map(|document| document.relative_path.as_str())
        .collect();
    let unmatched_candidates = candidates.as_ref().map_or(0, |catalog| {
        catalog
            .documents
            .iter()
            .filter(|document| !source_paths.contains(document.relative_path.as_str()))
            .count()
    });
    let mut navigation = DirectoryNavigation {
        selected,
        total: eligible.len(),
        source_skipped: skipped_count(&sources),
        candidate_skipped: candidates.as_ref().map_or(0, skipped_count),
        unmatched_candidates,
        selected_relative: eligible
            .get(selected)
            .map(|document| document.relative_path.clone()),
    };
    let mut summaries = summary_rows(&sources, candidates.as_ref());
    summaries.push(format!("{unmatched_candidates} candidate documents have no exact source path. Only the selected document is linted or checked."));
    if let Some(candidates) = &candidates {
        summaries.extend(
            candidates
                .documents
                .iter()
                .filter(|document| !source_paths.contains(document.relative_path.as_str()))
                .take(16)
                .map(|document| format!("Unmatched candidate: {}", document.relative_path)),
        );
    }
    let mut snapshot = if let Some(source) = eligible.get(selected) {
        let mut snapshot = review_selected(
            request,
            source,
            candidates.as_ref(),
            cancellation,
            &mut summaries,
        )?;
        let first = selected.saturating_sub(4);
        for (index, document) in eligible.iter().enumerate().skip(first).take(9) {
            summaries.push(format!(
                "{} {}",
                if index == selected { ">" } else { " " },
                document.relative_path
            ));
        }
        snapshot.status = format!(
            "Document {}/{}. [ / ] select; r rediscovers. {}",
            selected + 1,
            eligible.len(),
            snapshot.status
        );
        snapshot
    } else {
        navigation.selected = 0;
        Snapshot { source_label: request.args.source.to_string_lossy().into_owned(), status: "No supported source documents; no candidate checks performed. r rediscovers; q quits.".into(), ..Snapshot::default() }
    };
    summaries.append(&mut snapshot.findings);
    snapshot.findings = summaries;
    snapshot.directory = Some(navigation);
    if cancellation.is_cancelled() {
        return Err(RunFailure::cancelled(CommandName::Tui));
    }
    Ok(snapshot.bounded())
}

fn review_selected(
    request: &ReviewRequest,
    source: &DiscoveredDocument,
    candidates: Option<&Discovery>,
    cancellation: &CancellationToken,
    summaries: &mut Vec<String>,
) -> Result<Snapshot, RunFailure> {
    let source_bytes = read(&request.args.source, source, cancellation)?;
    let counterpart = candidates.and_then(|catalog| {
        catalog
            .documents
            .iter()
            .find(|document| document.relative_path == source.relative_path)
    });
    let candidate_bytes = match (request.args.candidate.as_ref(), counterpart) {
        (Some(root), Some(document)) if supported(document) => {
            Some(read(root, document, cancellation)?)
        }
        _ => None,
    };
    let mut selected_request = request.args.clone();
    selected_request.source = request.args.source.join(&source.relative_path);
    selected_request.candidate = candidate_bytes
        .as_ref()
        .and(request.args.candidate.as_ref())
        .map(|root| root.join(&source.relative_path));
    if candidate_bytes.is_none() {
        selected_request.protected_terms.clear();
        if request.args.candidate.is_some() {
            summaries.push("Candidate check unavailable: exact relative-path counterpart is missing, skipped, or unsupported; protected terms were not checked.".into());
        }
    }
    let snapshot = service::load_bytes(
        &selected_request,
        source_bytes,
        candidate_bytes,
        cancellation,
    )?
    .into_presentation();
    Ok(snapshot)
}

fn supported(document: &DiscoveredDocument) -> bool {
    document.encoding == TextEncoding::Utf8 && document.derivative != "explicit_decision_required"
}

fn skipped_count(catalog: &Discovery) -> usize {
    catalog.skipped.len()
        + catalog
            .documents
            .iter()
            .filter(|document| !supported(document))
            .count()
}

fn selection(
    request: &ReviewRequest,
    documents: &[&DiscoveredDocument],
) -> Result<usize, RunFailure> {
    if let Some(index) = request.index {
        return Ok(index.min(documents.len().saturating_sub(1)));
    }
    request.args.document.as_ref().map_or(Ok(0), |path| {
        documents
            .iter()
            .position(|document| document.relative_path == *path)
            .ok_or_else(|| RunFailure::usage_for(CommandName::Tui))
    })
}

fn read(
    root: &Path,
    document: &DiscoveredDocument,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, RunFailure> {
    if cancellation.is_cancelled() {
        return Err(RunFailure::cancelled(CommandName::Tui));
    }
    let relative = RelativeDocumentPath::new(document.relative_path.clone())
        .map_err(|_| RunFailure::operational(CommandName::Tui))?;
    let expected = Digest::from_sha256_hex(document.digest.clone())
        .map_err(|_| RunFailure::operational(CommandName::Tui))?;
    let selected = DocumentIntakeService::read(
        &DocumentSelection::catalog(root, relative, expected),
        MAX_CANDIDATE_CHECK_BYTES,
        cancellation,
    )
    .map_err(|error| match error {
        DocumentIntakeError::Input(error) => RunFailure::input_read(CommandName::Tui, &error),
        DocumentIntakeError::Inspection(error) => RunFailure::app(CommandName::Tui, &error),
        DocumentIntakeError::Changed => RunFailure::concurrent_modification(CommandName::Tui),
        DocumentIntakeError::Cancelled => RunFailure::cancelled(CommandName::Tui),
    })?;
    if selected.observation.derivative == DerivativeDisposition::ExplicitDecisionRequired {
        return Err(RunFailure::usage_for(CommandName::Tui));
    }
    Ok(selected.bytes)
}

fn require_root(path: &Path) -> Result<(), RunFailure> {
    if fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        Ok(())
    } else {
        Err(RunFailure::usage_for(CommandName::Tui))
    }
}

fn require_separate(source: &Path, candidate: &Path) -> Result<(), RunFailure> {
    let key = |path: &Path| -> Result<std::path::PathBuf, RunFailure> {
        let canonical =
            fs::canonicalize(path).map_err(|_| RunFailure::operational(CommandName::Tui))?;
        #[cfg(windows)]
        let canonical = std::path::PathBuf::from(canonical.to_string_lossy().to_lowercase());
        Ok(canonical)
    };
    let source = key(source)?;
    let candidate = key(candidate)?;
    if source.starts_with(&candidate) || candidate.starts_with(&source) {
        return Err(RunFailure::usage_for(CommandName::Tui));
    }
    Ok(())
}

fn summary_rows(source: &Discovery, candidate: Option<&Discovery>) -> Vec<String> {
    let mut rows = vec![format!(
        "Directory discovery: {} source documents, {} skipped/unsupported; {} candidate documents, {} skipped/unsupported. Limits: 4096 entries, depth 8, 64 MiB/root; each selected file 16 MiB.",
        source.documents.len(),
        skipped_count(source),
        candidate.map_or(0, |catalog| catalog.documents.len()),
        candidate.map_or(0, skipped_count)
    )];
    for (label, catalog) in [("Source", Some(source)), ("Candidate", candidate)] {
        if let Some(catalog) = catalog {
            rows.extend(catalog.skipped.iter().take(16).map(|entry| {
                format!(
                    "{label} skipped: {} ({})",
                    entry.relative_path.as_deref().unwrap_or("unavailable path"),
                    entry.reason
                )
            }));
            rows.extend(
                catalog
                    .documents
                    .iter()
                    .filter(|document| !supported(document))
                    .take(16)
                    .map(|document| {
                        format!(
                            "{label} unsupported: {} ({})",
                            document.relative_path,
                            if document.encoding == TextEncoding::Utf8 {
                                "derivative decision required"
                            } else {
                                "encoding"
                            }
                        )
                    }),
            );
        }
    }
    rows
}

#[cfg(test)]
mod tests;
