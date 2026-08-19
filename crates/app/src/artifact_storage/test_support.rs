use std::path::Path;

use rewrite_model_store::{
    ArtifactStateStore, RemovalCompletionDisposition, RemovalPreparationDisposition, StoreResult,
    StoredArtifactInstallation, StoredArtifactSetInstallation,
};

use super::{ExistingArtifactStorage, LifecycleLockMode};

pub(crate) fn prepare_artifact_removal(
    root: &Path,
    store: &mut ArtifactStateStore,
    selection: &StoredArtifactInstallation,
) -> StoreResult<RemovalPreparationDisposition> {
    let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
        .expect("open exclusive test storage");
    store.prepare_artifact_removal(
        storage
            .exclusive_lifecycle_lock()
            .expect("test storage owns exclusive lifecycle lock"),
        selection,
    )
}

pub(crate) fn complete_artifact_removal(
    root: &Path,
    store: &mut ArtifactStateStore,
    selection: &StoredArtifactInstallation,
) -> StoreResult<RemovalCompletionDisposition> {
    let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
        .expect("open exclusive test storage");
    store.complete_artifact_removal(
        storage
            .exclusive_lifecycle_lock()
            .expect("test storage owns exclusive lifecycle lock"),
        selection,
    )
}

pub(crate) fn prepare_artifact_set_removal(
    root: &Path,
    store: &mut ArtifactStateStore,
    selection: &StoredArtifactSetInstallation,
) -> StoreResult<RemovalPreparationDisposition> {
    let storage = ExistingArtifactStorage::open(root, LifecycleLockMode::Exclusive)
        .expect("open exclusive test storage");
    store.prepare_artifact_set_removal(
        storage
            .exclusive_lifecycle_lock()
            .expect("test storage owns exclusive lifecycle lock"),
        selection,
    )
}
