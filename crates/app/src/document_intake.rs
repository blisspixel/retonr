//! Typed bounded filesystem intake and supported metadata decisions.

use crate::document_selection::{DocumentSelection, SelectionKind};
use crate::{
    AppError, CarrierPresence, MAX_CANDIDATE_CHECK_BYTES, TextEncoding, document_input,
    inspect_plain_text,
};
use rewrite_types::{CancellationToken, Digest};
use std::{io, path::Path};

mod contract;
mod sidecars;
pub use contract::{
    DerivativeDisposition, DocumentIntakeObservation, SelectedDocument, SidecarKind,
    SidecarObservation, SidecarScanCompleteness,
};

/// Content-redacted failure of a selected bounded read or inventory.
pub enum DocumentIntakeError {
    /// The underlying reader refused or could not read the selected file.
    Input(io::Error),
    /// Complete-byte inventory exceeded its supported bounds.
    Inspection(AppError),
    /// Catalog bytes no longer match the selected digest.
    Changed,
    /// Original cancellation discards the operation outcome.
    Cancelled,
}

impl std::fmt::Debug for DocumentIntakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Input(_) => "DocumentIntakeError::Input",
            Self::Inspection(_) => "DocumentIntakeError::Inspection",
            Self::Changed => "DocumentIntakeError::Changed",
            Self::Cancelled => "DocumentIntakeError::Cancelled",
        })
    }
}
impl std::fmt::Display for DocumentIntakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Input(_) => "selected document could not be read",
            Self::Inspection(_) => "document inventory failed",
            Self::Changed => "selected document changed",
            Self::Cancelled => "document intake cancelled",
        })
    }
}
impl std::error::Error for DocumentIntakeError {}

/// Read-only intake shared by command-line, terminal and native presentations.
#[derive(Clone, Copy, Debug, Default)]
pub struct DocumentIntakeService;

impl DocumentIntakeService {
    /// Reads and inventories an explicit file or exact digest-bound catalog entry.
    ///
    /// The byte ceiling is capped at the supported plain-text maximum. Reads
    /// retain file and parent identity checks. Metadata is a bounded observation;
    /// applications still own an explicit derivative decision and commit policy.
    ///
    /// # Errors
    /// Refuses reader failures, changed catalog bytes, excessive input or cancellation.
    pub fn read(
        selection: &DocumentSelection,
        maximum_bytes: usize,
        cancellation: &CancellationToken,
    ) -> Result<SelectedDocument, DocumentIntakeError> {
        read_with_post_read(selection, maximum_bytes, cancellation, || {})
    }

    /// Inventories already retained bytes and observes supported adjacent metadata.
    ///
    /// A source path selects metadata lookup only; it does not attest to the
    /// supplied bytes. None identifies a byte stream without filesystem sidecars.
    /// Sidecar contents and external references are never opened or followed.
    ///
    /// # Errors
    /// Refuses excessive input and cancellation. Incomplete metadata scans remain
    /// explicit typed observations requiring a derivative decision.
    pub fn inspect_bytes(
        source: Option<&Path>,
        bytes: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<DocumentIntakeObservation, DocumentIntakeError> {
        ensure_active(cancellation)?;
        let inventory = inspect_plain_text(bytes).map_err(DocumentIntakeError::Inspection);
        ensure_active(cancellation)?;
        let inventory = inventory?;
        let sidecars = sidecars::scan(source, cancellation)?;
        let derivative = if inventory.c2pa_unstructured_text == CarrierPresence::Possible
            || !sidecars.present.is_empty()
            || sidecars.completeness == SidecarScanCompleteness::Incomplete
        {
            DerivativeDisposition::ExplicitDecisionRequired
        } else if inventory.encoding != TextEncoding::Utf8 {
            DerivativeDisposition::NotChecked
        } else {
            DerivativeDisposition::NotRequired
        };
        ensure_active(cancellation)?;
        Ok(DocumentIntakeObservation {
            inventory,
            sidecars,
            derivative,
        })
    }
}

fn read_with_post_read(
    selection: &DocumentSelection,
    maximum_bytes: usize,
    cancellation: &CancellationToken,
    after_read: impl FnOnce(),
) -> Result<SelectedDocument, DocumentIntakeError> {
    ensure_active(cancellation)?;
    let limit = maximum_bytes.min(MAX_CANDIDATE_CHECK_BYTES);
    let bytes = match &selection.kind {
        SelectionKind::Explicit(path) => document_input::read_regular_bounded(path, limit),
        SelectionKind::Catalog { root, relative, .. } => {
            document_input::read_directory_unaliased_bounded(
                root,
                Path::new(relative.as_str()),
                limit,
            )
        }
    };
    after_read();
    ensure_active(cancellation)?;
    let bytes = bytes.map_err(DocumentIntakeError::Input)?;
    if let SelectionKind::Catalog { expected, .. } = &selection.kind {
        let actual = Digest::sha256(&bytes);
        ensure_active(cancellation)?;
        if &actual != expected {
            return Err(DocumentIntakeError::Changed);
        }
    }
    let observation =
        DocumentIntakeService::inspect_bytes(Some(&selection.path()), &bytes, cancellation)?;
    Ok(SelectedDocument { bytes, observation })
}

pub(super) fn ensure_active(cancellation: &CancellationToken) -> Result<(), DocumentIntakeError> {
    if cancellation.is_cancelled() {
        Err(DocumentIntakeError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
