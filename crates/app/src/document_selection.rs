//! Inert explicit-file and digest-bound catalog selections.

use rewrite_types::Digest;
use std::{fmt, path::PathBuf};

/// Validated forward-slash relative document path with no escape components.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativeDocumentPath(String);

impl RelativeDocumentPath {
    /// Validates the existing portable catalog separator and component policy.
    ///
    /// # Errors
    /// Refuses empty paths, empty or dot components, backslashes and NUL bytes.
    pub fn new(value: impl Into<String>) -> Result<Self, DocumentSelectionError> {
        let value = value.into();
        if value.contains('\\')
            || value.contains('\0')
            || value
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return Err(DocumentSelectionError);
        }
        Ok(Self(value))
    }

    /// Returns the exact relative path for filesystem selection or presentation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RelativeDocumentPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RelativeDocumentPath")
    }
}

/// A relative path failed its structural selection contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("document relative path is invalid")]
pub struct DocumentSelectionError;

/// A requested read selection, never rewrite or runtime authority.
pub struct DocumentSelection {
    pub(crate) kind: SelectionKind,
}

pub(crate) enum SelectionKind {
    Explicit(PathBuf),
    Catalog {
        root: PathBuf,
        relative: RelativeDocumentPath,
        expected: Digest,
    },
}

impl DocumentSelection {
    /// Selects an ordinary regular file, preserving explicitly followed links.
    #[must_use]
    pub fn explicit(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: SelectionKind::Explicit(path.into()),
        }
    }

    /// Selects one catalog document whose bytes must still match its digest.
    ///
    /// Read validation refuses indirect descendants and hard-link aliases.
    #[must_use]
    pub fn catalog(
        root: impl Into<PathBuf>,
        relative: RelativeDocumentPath,
        expected: Digest,
    ) -> Self {
        Self {
            kind: SelectionKind::Catalog {
                root: root.into(),
                relative,
                expected,
            },
        }
    }

    pub(crate) fn path(&self) -> PathBuf {
        match &self.kind {
            SelectionKind::Explicit(path) => path.clone(),
            SelectionKind::Catalog { root, relative, .. } => root.join(relative.as_str()),
        }
    }
}

impl fmt::Debug for DocumentSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            SelectionKind::Explicit(_) => "DocumentSelection::Explicit",
            SelectionKind::Catalog { .. } => "DocumentSelection::Catalog",
        })
    }
}

#[cfg(test)]
mod tests;
