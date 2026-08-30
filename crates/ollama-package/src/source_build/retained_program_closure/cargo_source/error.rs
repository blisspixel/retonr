use thiserror::Error;

/// Failure from retained Cargo source-closure verification.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CargoSourceClosureError {
    /// One configured resource ceiling is zero or exceeds the hard limit.
    #[error("Cargo source-closure limits are invalid")]
    InvalidLimits,
    /// Verification was cancelled.
    #[error("Cargo source-closure verification was cancelled")]
    Cancelled,
    /// A required component role is absent, duplicated, combined, or aliased.
    #[error("Cargo source-closure component roles are invalid")]
    InvalidComponentRoles,
    /// A required component could not be opened.
    #[error("Cargo source-closure component is unavailable")]
    ComponentUnavailable,
    /// Reopened component bytes do not match their verified measurement.
    #[error("Cargo source-closure component measurement changed")]
    ComponentMismatch,
    /// The lockfile exceeded a fixed parse or count ceiling.
    #[error("Cargo.lock exceeds a source-closure limit")]
    LockfileQuotaExceeded,
    /// `Cargo.lock` is malformed, unsupported, or structurally incomplete.
    #[error("Cargo.lock is invalid")]
    InvalidLockfile,
    /// A lock package uses a non-crates.io source or invalid checksum disposition.
    #[error("Cargo.lock contains an unsupported package source")]
    UnsupportedPackageSource,
    /// Two lock packages have the same complete identity.
    #[error("Cargo.lock contains a duplicate package identity")]
    DuplicatePackageIdentity,
    /// A dependency edge does not resolve to exactly one lock package.
    #[error("Cargo.lock contains an ambiguous dependency edge")]
    AmbiguousDependencyEdge,
    /// An outer tar stream is malformed.
    #[error("Cargo source archive is invalid")]
    InvalidArchive,
    /// An outer archive violates the canonical GNU archive contract.
    #[error("Cargo source archive is not canonical")]
    NoncanonicalArchive,
    /// An archive contains a link, special file, or colliding entry.
    #[error("Cargo source archive contains an unsafe entry")]
    UnsafeArchiveEntry,
    /// An archive path is ambiguous or nonportable.
    #[error("Cargo source archive contains a nonportable path")]
    NonportableArchivePath,
    /// An archive does not have the exact expected single root.
    #[error("Cargo source archive root does not match")]
    ArchiveRootMismatch,
    /// An archive exceeded a fixed entry, path, or byte ceiling.
    #[error("Cargo source archive exceeds a fixed limit")]
    ArchiveQuotaExceeded,
    /// Repository and separately retained lockfile bytes differ.
    #[error("repository Cargo.lock differs from the retained lockfile")]
    RepositoryLockMismatch,
    /// Source-less lock packages do not close over repository manifests.
    #[error("repository Cargo manifests do not match source-less lock packages")]
    RepositoryManifestMismatch,
    /// A path package reference escapes the repository archive root.
    #[error("Cargo path package escapes the repository archive root")]
    PathPackageEscape,
    /// A raw `.crate` gzip or tar stream is invalid.
    #[error("raw Cargo crate archive is invalid")]
    InvalidCrateArchive,
    /// A raw `.crate` digest differs from its lock checksum.
    #[error("raw Cargo crate checksum does not match Cargo.lock")]
    CrateChecksumMismatch,
    /// A raw `.crate` manifest differs from its lock package identity.
    #[error("raw Cargo crate manifest does not match Cargo.lock")]
    CrateManifestMismatch,
    /// Raw crate members do not correspond exactly to registry lock packages.
    #[error("raw Cargo crate bundle does not match Cargo.lock")]
    RawCrateSetMismatch,
    /// A vendor package checksum record is malformed or inconsistent.
    #[error("Cargo vendor checksum record is invalid")]
    VendorChecksumMismatch,
    /// The vendor package set or file trees differ from raw crates.
    #[error("Cargo vendor tree differs from raw crate sources")]
    VendorTreeMismatch,
}
