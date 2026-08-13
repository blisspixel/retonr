//! Durable `SQLite` storage for artifact lifecycle records and active bindings.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod binding;
mod error;
mod record;
mod schema;
mod store;

pub use error::{StoreError, StoreResult};
pub use store::{ArtifactStateStore, InstallationWriteDisposition, WriteDisposition};
