use super::ArtifactInventoryError;

/// Exact failure outcome for callers that must preserve cleanup evidence.
pub(crate) struct NoReplacePublicationFailure {
    primary: Box<ArtifactInventoryError>,
    cleanup: Option<Box<ArtifactInventoryError>>,
    committed: bool,
}

impl NoReplacePublicationFailure {
    pub(crate) fn into_parts(
        self,
    ) -> (ArtifactInventoryError, Option<ArtifactInventoryError>, bool) {
        (
            *self.primary,
            self.cleanup.map(|cleanup| *cleanup),
            self.committed,
        )
    }

    pub(super) fn before_commit(
        primary: ArtifactInventoryError,
        cleanup: Option<ArtifactInventoryError>,
    ) -> Self {
        Self {
            primary: Box::new(primary),
            cleanup: cleanup.map(Box::new),
            committed: false,
        }
    }

    pub(super) fn after_commit(primary: ArtifactInventoryError) -> Self {
        Self {
            primary: Box::new(primary),
            cleanup: None,
            committed: true,
        }
    }

    pub(super) fn into_legacy_error(self) -> ArtifactInventoryError {
        self.cleanup.map_or(*self.primary, |cleanup| *cleanup)
    }
}
