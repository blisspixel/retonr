//! Durable `SQLite` storage for artifact lifecycle records and active bindings.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod binding;
mod error;
mod lifecycle;
mod record;
mod removal;
mod schema;
mod store;

pub use error::{StoreError, StoreResult};
pub use lifecycle::ExclusiveArtifactLifecycleLock;
pub use removal::{
    ArtifactInstallationEpoch, ArtifactRemovalPhase, StoredArtifactInstallation,
    StoredArtifactRemoval,
};
pub use store::{
    ArtifactStateStore, InstallationWriteDisposition, RemovalCompletionDisposition,
    RemovalPreparationDisposition, StoredArtifactState, WriteDisposition,
};
