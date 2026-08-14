//! Durable `SQLite` storage for artifact lifecycle authority and inert model evidence.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod artifact_set_installation;
mod binding;
mod error;
mod integrity;
mod lifecycle;
mod migration;
mod record;
mod removal;
mod schema;
mod store;

pub use artifact_set_installation::{
    ArtifactSetInstallationEpoch, ArtifactSetInstallationWriteDisposition,
    StoredArtifactSetInstallation,
};
pub use error::{StoreError, StoreResult};
pub use lifecycle::ExclusiveArtifactLifecycleLock;
pub use migration::{
    ExistingStoreMigration, StoreMigrationDisposition, StoreMigrationResult, StoreSchemaStatus,
};
pub use removal::{
    ArtifactInstallationEpoch, ArtifactRemovalPhase, StoredArtifactInstallation,
    StoredArtifactRemoval,
};
pub use store::{
    ArtifactStateStore, InstallationWriteDisposition, RemovalCompletionDisposition,
    RemovalPreparationDisposition, StoredArtifactState, WriteDisposition,
};
